use crate::entities::{
    YoutubeChannel, YoutubeChannelThumbnail, YoutubeChannelThumbnailDraft, YoutubeGroupChannel,
    YoutubeGroupRecord, YoutubeSubscription, YoutubeVideo, YoutubeVideoDraft,
};
use sqlx::SqlitePool;

/// Ensures the global cache contains one channel row.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the row cannot be stored or loaded.
pub async fn ensure_youtube_channel(
    pool: &SqlitePool,
    channel_id: &str,
) -> Result<YoutubeChannel, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO youtube_channels \
         (channel_id, title, channel_url, created_at, updated_at) VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(channel_id) DO NOTHING",
    )
    .bind(channel_id)
    .bind(channel_id)
    .bind(format!("https://www.youtube.com/channel/{channel_id}"))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_youtube_channel(pool, channel_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Loads one globally cached channel.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_youtube_channel(
    pool: &SqlitePool,
    channel_id: &str,
) -> Result<Option<YoutubeChannel>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeChannel>(
        "SELECT channel_id, title, channel_url, thumbnail_url, thumbnail_fetched_at, \
         last_fetched_at, refresh_started_at, last_error, created_at, updated_at \
         FROM youtube_channels WHERE channel_id = ?",
    )
    .bind(channel_id)
    .fetch_optional(pool)
    .await
}

/// Claims a due refresh with a short cross-process lease.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the atomic update cannot be completed.
pub async fn claim_youtube_channel_refresh(
    pool: &SqlitePool,
    channel_id: &str,
    due_before: &str,
    abandoned_before: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE youtube_channels SET refresh_started_at = ?, updated_at = ? \
         WHERE channel_id = ? \
         AND ((last_fetched_at IS NULL OR datetime(last_fetched_at) <= datetime(?)) \
              OR COALESCE(length(thumbnail_data), 0) = 0) \
         AND (refresh_started_at IS NULL OR datetime(refresh_started_at) <= datetime(?))",
    )
    .bind(&now)
    .bind(&now)
    .bind(channel_id)
    .bind(due_before)
    .bind(abandoned_before)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

/// Lists subscribed channels whose shared fetch window and lease have elapsed.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when due channels cannot be loaded.
pub async fn list_due_youtube_channel_ids(
    pool: &SqlitePool,
    due_before: &str,
    limit: usize,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT c.channel_id FROM youtube_channels c \
         WHERE EXISTS (SELECT 1 FROM youtube_subscriptions s WHERE s.channel_id = c.channel_id) \
         AND ((c.last_fetched_at IS NULL OR datetime(c.last_fetched_at) <= datetime(?)) \
              OR COALESCE(length(c.thumbnail_data), 0) = 0) \
         AND (c.refresh_started_at IS NULL \
              OR datetime(c.refresh_started_at) <= datetime('now', '-10 minutes')) \
         ORDER BY c.last_fetched_at IS NOT NULL, datetime(c.last_fetched_at) ASC LIMIT ?",
    )
    .bind(due_before)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await
}

/// Stores channel metadata and globally deduplicated videos in one transaction.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be committed.
pub async fn store_youtube_channel_refresh(
    pool: &SqlitePool,
    channel_id: &str,
    title: &str,
    channel_url: &str,
    thumbnail: Option<&YoutubeChannelThumbnailDraft>,
    videos: &[YoutubeVideoDraft],
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "UPDATE youtube_channels SET title = ?, channel_url = ?, \
         thumbnail_url = COALESCE(?, thumbnail_url), \
         thumbnail_content_type = COALESCE(?, thumbnail_content_type), \
         thumbnail_data = COALESCE(?, thumbnail_data), \
         thumbnail_fetched_at = CASE WHEN ? IS NULL THEN thumbnail_fetched_at ELSE ? END, \
         last_fetched_at = ?, refresh_started_at = NULL, last_error = NULL, updated_at = ? \
         WHERE channel_id = ?",
    )
    .bind(title)
    .bind(channel_url)
    .bind(thumbnail.map(|value| value.source_url.as_str()))
    .bind(thumbnail.map(|value| value.content_type.as_str()))
    .bind(thumbnail.map(|value| value.data.as_slice()))
    .bind(thumbnail.map(|value| value.source_url.as_str()))
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(channel_id)
    .execute(&mut *transaction)
    .await?;
    for video in videos {
        sqlx::query(
            "INSERT INTO youtube_videos \
             (id, external_id, channel_id, url, thumbnail_url, title, published_at, fetched_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(external_id) DO UPDATE SET url = excluded.url, \
             thumbnail_url = excluded.thumbnail_url, title = excluded.title, \
             published_at = excluded.published_at, fetched_at = excluded.fetched_at",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&video.external_id)
        .bind(channel_id)
        .bind(&video.url)
        .bind(&video.thumbnail_url)
        .bind(&video.title)
        .bind(&video.published_at)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await
}

/// Records a failed attempt and clears its refresh lease.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the channel cannot be updated.
pub async fn set_youtube_refresh_error(
    pool: &SqlitePool,
    channel_id: &str,
    message: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE youtube_channels SET last_fetched_at = ?, refresh_started_at = NULL, \
         last_error = ?, updated_at = ? WHERE channel_id = ?",
    )
    .bind(&now)
    .bind(message)
    .bind(&now)
    .bind(channel_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Creates one user-to-channel subscription.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the row cannot be inserted.
pub async fn create_youtube_subscription(
    pool: &SqlitePool,
    user_id: &str,
    channel_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "INSERT INTO youtube_subscriptions (user_id, channel_id, created_at) VALUES (?, ?, ?) \
         ON CONFLICT(user_id, channel_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(channel_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

/// Removes a subscription and its user-owned group memberships.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be committed.
pub async fn delete_youtube_subscription(
    pool: &SqlitePool,
    user_id: &str,
    channel_id: &str,
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "DELETE FROM youtube_group_channels WHERE channel_id = ? AND group_id IN \
         (SELECT id FROM youtube_groups WHERE user_id = ?)",
    )
    .bind(channel_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    let deleted =
        sqlx::query("DELETE FROM youtube_subscriptions WHERE user_id = ? AND channel_id = ?")
            .bind(user_id)
            .bind(channel_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected()
            == 1;
    transaction.commit().await?;
    Ok(deleted)
}

/// Lists one user's subscriptions with shared channel metadata.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when subscriptions cannot be loaded.
pub async fn list_youtube_subscriptions(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<YoutubeSubscription>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeSubscription>(
        "SELECT c.channel_id, c.title, c.channel_url, \
         CASE WHEN length(c.thumbnail_data) > 0 \
              THEN '/api/youtube/channels/' || c.channel_id || '/thumbnail?v=' || \
                   COALESCE(c.thumbnail_fetched_at, '0') ELSE '' END AS thumbnail_url, \
         c.last_fetched_at, \
         c.last_error, s.created_at FROM youtube_subscriptions s JOIN youtube_channels c \
         ON c.channel_id = s.channel_id WHERE s.user_id = ? \
         ORDER BY c.title COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Loads one cached channel portrait only when the account subscribes to the channel.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the portrait cannot be loaded.
pub async fn get_youtube_channel_thumbnail(
    pool: &SqlitePool,
    user_id: &str,
    channel_id: &str,
) -> Result<Option<YoutubeChannelThumbnail>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeChannelThumbnail>(
        "SELECT c.thumbnail_content_type AS content_type, c.thumbnail_data AS data \
         FROM youtube_channels c JOIN youtube_subscriptions s ON s.channel_id = c.channel_id \
         WHERE s.user_id = ? AND c.channel_id = ? AND length(c.thumbnail_data) > 0",
    )
    .bind(user_id)
    .bind(channel_id)
    .fetch_optional(pool)
    .await
}

/// Lists globally stored videos visible through one user's subscriptions.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when videos cannot be loaded.
pub async fn list_youtube_videos(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<YoutubeVideo>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeVideo>(
        "SELECT v.id, v.channel_id, c.title AS channel_title, v.url, v.thumbnail_url, \
         v.title, v.published_at, v.fetched_at, wl.saved_at AS watch_later_at \
         FROM youtube_videos v \
         JOIN youtube_channels c ON c.channel_id = v.channel_id \
         JOIN youtube_subscriptions s ON s.channel_id = v.channel_id \
         LEFT JOIN youtube_watch_later wl ON wl.video_id = v.id AND wl.user_id = ? \
         WHERE s.user_id = ? ORDER BY datetime(v.published_at) DESC, v.fetched_at DESC LIMIT 500",
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Lists one user's saved YouTube videos in most-recently-saved order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when saved videos cannot be loaded.
pub async fn list_youtube_watch_later(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<YoutubeVideo>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeVideo>(
        "SELECT v.id, v.channel_id, c.title AS channel_title, v.url, v.thumbnail_url, \
         v.title, v.published_at, v.fetched_at, wl.saved_at AS watch_later_at \
         FROM youtube_watch_later wl \
         JOIN youtube_videos v ON v.id = wl.video_id \
         JOIN youtube_channels c ON c.channel_id = v.channel_id \
         WHERE wl.user_id = ? ORDER BY datetime(wl.saved_at) DESC, wl.saved_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Adds or removes a cached video from one user's Watch Later collection.
///
/// Saving requires a current subscription to the video's channel. Removing requires only the
/// account-owned saved row, so a video can still be removed after unsubscribing.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the write cannot be completed.
pub async fn set_youtube_watch_later(
    pool: &SqlitePool,
    user_id: &str,
    video_id: &str,
    saved: bool,
) -> Result<bool, sqlx::Error> {
    let result = if saved {
        sqlx::query(
            "INSERT INTO youtube_watch_later (user_id, video_id, saved_at) \
             SELECT ?, v.id, ? FROM youtube_videos v \
             JOIN youtube_subscriptions s ON s.channel_id = v.channel_id \
             WHERE v.id = ? AND s.user_id = ? \
             ON CONFLICT(user_id, video_id) DO UPDATE SET saved_at = excluded.saved_at",
        )
        .bind(user_id)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(video_id)
        .bind(user_id)
        .execute(pool)
        .await?
    } else {
        sqlx::query("DELETE FROM youtube_watch_later WHERE user_id = ? AND video_id = ?")
            .bind(user_id)
            .bind(video_id)
            .execute(pool)
            .await?
    };
    Ok(result.rows_affected() > 0)
}

/// Lists one user's groups in display order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when groups cannot be loaded.
pub async fn list_youtube_groups(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<YoutubeGroupRecord>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeGroupRecord>(
        "SELECT id, name, position, created_at, updated_at FROM youtube_groups \
         WHERE user_id = ? ORDER BY position ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Lists one user's ordered group memberships.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when memberships cannot be loaded.
pub async fn list_youtube_group_channels(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<YoutubeGroupChannel>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeGroupChannel>(
        "SELECT gc.group_id, gc.channel_id FROM youtube_group_channels gc \
         JOIN youtube_groups g ON g.id = gc.group_id WHERE g.user_id = ? \
         ORDER BY g.position ASC, gc.position ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Creates an empty user-owned group.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the group cannot be inserted.
pub async fn create_youtube_group(
    pool: &SqlitePool,
    user_id: &str,
    name: &str,
) -> Result<YoutubeGroupRecord, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO youtube_groups (id, user_id, name, position, created_at, updated_at) \
         VALUES (?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM youtube_groups WHERE user_id = ?), ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(name)
    .bind(user_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, YoutubeGroupRecord>(
        "SELECT id, name, position, created_at, updated_at FROM youtube_groups WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

/// Replaces one group's name and ordered channel memberships atomically.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when validation or persistence fails.
pub async fn update_youtube_group(
    pool: &SqlitePool,
    user_id: &str,
    group_id: &str,
    name: &str,
    channel_ids: &[String],
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE youtube_groups SET name = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(name)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(group_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if updated == 0 {
        transaction.rollback().await?;
        return Ok(false);
    }
    for channel_id in channel_ids {
        let subscribed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM youtube_subscriptions WHERE user_id = ? AND channel_id = ?",
        )
        .bind(user_id)
        .bind(channel_id)
        .fetch_one(&mut *transaction)
        .await?;
        if subscribed == 0 {
            transaction.rollback().await?;
            return Err(sqlx::Error::RowNotFound);
        }
    }
    sqlx::query("DELETE FROM youtube_group_channels WHERE group_id = ?")
        .bind(group_id)
        .execute(&mut *transaction)
        .await?;
    for (position, channel_id) in channel_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO youtube_group_channels (group_id, channel_id, position) VALUES (?, ?, ?)",
        )
        .bind(group_id)
        .bind(channel_id)
        .bind(i64::try_from(position).unwrap_or(127))
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}

/// Deletes one user-owned group.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_youtube_group(
    pool: &SqlitePool,
    user_id: &str,
    group_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM youtube_groups WHERE id = ? AND user_id = ?")
            .bind(group_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}

/// Loads one user's persisted video display mode.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the preference cannot be loaded.
pub async fn get_youtube_display_mode(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<String, sqlx::Error> {
    Ok(
        sqlx::query_scalar("SELECT display_mode FROM youtube_settings WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| "thumbnails".to_owned()),
    )
}

/// Persists one user's video display mode.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the preference cannot be stored.
pub async fn set_youtube_display_mode(
    pool: &SqlitePool,
    user_id: &str,
    display_mode: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO youtube_settings (user_id, display_mode, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(user_id) DO UPDATE SET display_mode = excluded.display_mode, \
         updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(display_mode)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn channels_videos_and_fetch_claims_are_shared_across_users() {
        let pool = crate::connect("sqlite::memory:").await.unwrap();
        let (first, _) = crate::queries::create_account(
            &pool,
            "video-one@example.com",
            "$argon2id$one",
            "Video One",
        )
        .await
        .unwrap();
        let (second, _) = crate::queries::create_account(
            &pool,
            "video-two@example.com",
            "$argon2id$two",
            "Video Two",
        )
        .await
        .unwrap();
        let channel_id = "UCabcdefghijklmnopqrstuv";
        ensure_youtube_channel(&pool, channel_id).await.unwrap();
        assert!(
            create_youtube_subscription(&pool, &first.id, channel_id)
                .await
                .unwrap()
        );
        assert!(
            create_youtube_subscription(&pool, &second.id, channel_id)
                .await
                .unwrap()
        );

        let due_before = chrono::Utc::now().to_rfc3339();
        let abandoned_before = (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        assert!(
            claim_youtube_channel_refresh(&pool, channel_id, &due_before, &abandoned_before)
                .await
                .unwrap()
        );
        assert!(
            !claim_youtube_channel_refresh(&pool, channel_id, &due_before, &abandoned_before)
                .await
                .unwrap()
        );

        let video = YoutubeVideoDraft {
            external_id: "video-123".to_owned(),
            url: "https://www.youtube.com/watch?v=video-123".to_owned(),
            thumbnail_url: "https://i.ytimg.com/vi/video-123/hqdefault.jpg".to_owned(),
            title: "Shared upload".to_owned(),
            published_at: "2026-08-14T10:00:00Z".to_owned(),
        };
        store_youtube_channel_refresh(
            &pool,
            channel_id,
            "Shared Channel",
            "https://www.youtube.com/channel/UCabcdefghijklmnopqrstuv",
            None,
            std::slice::from_ref(&video),
        )
        .await
        .unwrap();
        let not_due_before = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert_eq!(
            list_due_youtube_channel_ids(&pool, &not_due_before, 10,)
                .await
                .unwrap(),
            vec![channel_id.to_owned()]
        );
        assert!(
            claim_youtube_channel_refresh(&pool, channel_id, &not_due_before, &abandoned_before,)
                .await
                .unwrap()
        );

        let thumbnail = YoutubeChannelThumbnailDraft {
            source_url: "https://yt3.ggpht.com/channel-avatar=s176".to_owned(),
            content_type: "image/jpeg".to_owned(),
            data: vec![0xff, 0xd8, 0xff],
        };
        store_youtube_channel_refresh(
            &pool,
            channel_id,
            "Shared Channel",
            "https://www.youtube.com/channel/UCabcdefghijklmnopqrstuv",
            Some(&thumbnail),
            std::slice::from_ref(&video),
        )
        .await
        .unwrap();
        store_youtube_channel_refresh(
            &pool,
            channel_id,
            "Shared Channel",
            "https://www.youtube.com/channel/UCabcdefghijklmnopqrstuv",
            None,
            &[video],
        )
        .await
        .unwrap();
        let channel = get_youtube_channel(&pool, channel_id)
            .await
            .unwrap()
            .expect("channel is cached");
        assert_eq!(
            channel.thumbnail_url,
            "https://yt3.ggpht.com/channel-avatar=s176"
        );
        assert!(channel.thumbnail_fetched_at.is_some());
        let subscriptions = list_youtube_subscriptions(&pool, &first.id).await.unwrap();
        assert!(
            subscriptions[0]
                .thumbnail_url
                .starts_with(&format!("/api/youtube/channels/{channel_id}/thumbnail?v="))
        );
        let cached_thumbnail = get_youtube_channel_thumbnail(&pool, &first.id, channel_id)
            .await
            .unwrap()
            .expect("subscribed user can load cached portrait");
        assert_eq!(cached_thumbnail.content_type, "image/jpeg");
        assert_eq!(cached_thumbnail.data, thumbnail.data);
        assert_eq!(
            list_youtube_videos(&pool, &first.id).await.unwrap().len(),
            1
        );
        assert_eq!(
            list_youtube_videos(&pool, &second.id).await.unwrap().len(),
            1
        );
        let video_id = list_youtube_videos(&pool, &first.id).await.unwrap()[0]
            .id
            .clone();
        assert!(
            set_youtube_watch_later(&pool, &first.id, &video_id, true)
                .await
                .unwrap()
        );
        assert_eq!(
            list_youtube_watch_later(&pool, &first.id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            list_youtube_watch_later(&pool, &second.id)
                .await
                .unwrap()
                .is_empty()
        );

        let gaming = create_youtube_group(&pool, &first.id, "Gaming")
            .await
            .unwrap();
        let japan = create_youtube_group(&pool, &first.id, "Japan")
            .await
            .unwrap();
        let channels = vec![channel_id.to_owned()];
        assert!(
            update_youtube_group(&pool, &first.id, &gaming.id, "Gaming", &channels)
                .await
                .unwrap()
        );
        assert!(
            update_youtube_group(&pool, &first.id, &japan.id, "Japan", &channels)
                .await
                .unwrap()
        );
        assert_eq!(
            list_youtube_group_channels(&pool, &first.id)
                .await
                .unwrap()
                .len(),
            2
        );

        delete_youtube_subscription(&pool, &first.id, channel_id)
            .await
            .unwrap();
        assert!(
            list_youtube_videos(&pool, &first.id)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            list_youtube_watch_later(&pool, &first.id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            set_youtube_watch_later(&pool, &first.id, &video_id, false)
                .await
                .unwrap()
        );
        assert!(
            list_youtube_watch_later(&pool, &first.id)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            list_youtube_videos(&pool, &second.id).await.unwrap().len(),
            1
        );
    }
}
