use crate::entities::{
    Podcast, PodcastArtwork, PodcastArtworkDraft, PodcastCachedFile, PodcastDownloadJob,
    PodcastDraft, PodcastEpisode, PodcastEpisodeDraft, PodcastFeedPreview, PodcastRefreshTarget,
    PodcastRequest, PodcastRequestDraft, PodcastSettings, PodcastSummary,
};
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

/// The most episodes one listener may queue.
///
/// Bounded by the `podcast_queue.position` CHECK: final positions occupy 0..255 and the
/// reorder parking band occupies 256..511.
pub const PODCAST_QUEUE_LIMIT: i64 = 256;

/// First position in the reorder parking band.
const QUEUE_PARK_OFFSET: i64 = 256;

/// Builds a catalogue query from the shared `Podcast` projection.
///
/// The projection is stitched together at compile time so no query string is ever
/// assembled from runtime values.
macro_rules! podcast_select {
    ($tail:literal) => {
        concat!(
            "SELECT id, feed_url, normalized_url, title, description, author, \
             site_url, language, artwork_url, \
             COALESCE(length(artwork_data), 0) > 0 AS has_artwork, \
             auto_download_count, max_retained_episodes, added_by, last_fetched_at, \
             last_error, created_at, updated_at FROM podcasts ",
            $tail
        )
    };
}

/// Builds a request query, resolving both user references to display names.
macro_rules! request_select {
    ($tail:literal) => {
        concat!(
            "SELECT podcast_requests.id, podcast_requests.user_id, \
             COALESCE(requester.display_name, 'Removed user') AS requester_name, \
             podcast_requests.feed_url, podcast_requests.resolved_title, \
             podcast_requests.resolved_author, podcast_requests.resolved_artwork_url, \
             podcast_requests.note, podcast_requests.status, podcast_requests.decision_note, \
             decider.display_name AS decided_by_name, podcast_requests.decided_at, \
             podcast_requests.podcast_id, podcast_requests.created_at, \
             podcast_requests.updated_at \
             FROM podcast_requests \
             LEFT JOIN user_settings AS requester \
                  ON requester.user_id = podcast_requests.user_id \
             LEFT JOIN user_settings AS decider \
                  ON decider.user_id = podcast_requests.decided_by ",
            $tail
        )
    };
}

/// Builds an episode query joined with one viewer's listening state.
///
/// The joins bind `user_id` three times, before any caller-supplied parameter.
macro_rules! episode_select {
    ($tail:literal) => {
        concat!(
            "SELECT podcast_episodes.id, podcast_episodes.podcast_id, \
             podcasts.title AS podcast_title, podcast_episodes.title, \
             podcast_episodes.description, podcast_episodes.episode_url, \
             podcast_episodes.enclosure_type, podcast_episodes.enclosure_bytes, \
             podcast_episodes.duration_seconds, podcast_episodes.published_at, \
             podcast_downloads.status AS download_status, \
             CASE WHEN COALESCE(podcast_downloads.byte_size, 0) > 0 \
                  THEN MIN(1.0, CAST(podcast_downloads.downloaded_bytes AS REAL) \
                                / podcast_downloads.byte_size) \
                  ELSE 0.0 END AS download_progress, \
             COALESCE(podcast_episode_progress.position_seconds, 0) AS position_seconds, \
             podcast_episode_progress.completed_at, podcast_saved_episodes.saved_at, \
             podcast_queue.position AS queue_position \
             FROM podcast_episodes \
             JOIN podcasts ON podcasts.id = podcast_episodes.podcast_id \
             LEFT JOIN podcast_downloads \
                  ON podcast_downloads.episode_id = podcast_episodes.id \
             LEFT JOIN podcast_episode_progress \
                  ON podcast_episode_progress.episode_id = podcast_episodes.id \
                  AND podcast_episode_progress.user_id = ? \
             LEFT JOIN podcast_saved_episodes \
                  ON podcast_saved_episodes.episode_id = podcast_episodes.id \
                  AND podcast_saved_episodes.user_id = ? \
             LEFT JOIN podcast_queue ON podcast_queue.episode_id = podcast_episodes.id \
                  AND podcast_queue.user_id = ? ",
            $tail
        )
    };
}

// ---------------------------------------------------------------------------
// Administrator policy
// ---------------------------------------------------------------------------

/// Loads the singleton podcast policy.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_podcast_settings(pool: &SqlitePool) -> Result<PodcastSettings, sqlx::Error> {
    sqlx::query_as::<_, PodcastSettings>(
        "SELECT requests_enabled, member_downloads_enabled, max_pending_requests_per_user, \
                storage_budget_bytes, max_episode_bytes, default_auto_download_count, updated_at \
         FROM podcast_settings WHERE id = 1",
    )
    .fetch_one(pool)
    .await
}

/// Replaces the singleton podcast policy.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update or reload cannot be completed.
pub async fn update_podcast_settings(
    pool: &SqlitePool,
    requests_enabled: bool,
    member_downloads_enabled: bool,
    max_pending_requests_per_user: i64,
    storage_budget_bytes: i64,
    max_episode_bytes: i64,
    default_auto_download_count: i64,
) -> Result<PodcastSettings, sqlx::Error> {
    sqlx::query(
        "UPDATE podcast_settings SET requests_enabled = ?, member_downloads_enabled = ?, \
         max_pending_requests_per_user = ?, storage_budget_bytes = ?, max_episode_bytes = ?, \
         default_auto_download_count = ?, updated_at = ? WHERE id = 1",
    )
    .bind(requests_enabled)
    .bind(member_downloads_enabled)
    .bind(max_pending_requests_per_user)
    .bind(storage_budget_bytes)
    .bind(max_episode_bytes)
    .bind(default_auto_download_count)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    get_podcast_settings(pool).await
}

// ---------------------------------------------------------------------------
// Catalogue
// ---------------------------------------------------------------------------

/// Loads one catalogue entry by identifier.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_podcast(pool: &SqlitePool, id: &str) -> Result<Option<Podcast>, sqlx::Error> {
    sqlx::query_as::<_, Podcast>(podcast_select!("WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Finds an existing catalogue entry for a normalized feed URL.
///
/// This is what makes a duplicate request impossible: a feed already in the catalogue is
/// answered with a subscription instead of a new review item.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_podcast_by_normalized_url(
    pool: &SqlitePool,
    normalized_url: &str,
) -> Result<Option<Podcast>, sqlx::Error> {
    sqlx::query_as::<_, Podcast>(podcast_select!("WHERE normalized_url = ?"))
        .bind(normalized_url)
        .fetch_optional(pool)
        .await
}

/// Creates one catalogue entry.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert or reload cannot be completed.
pub async fn insert_podcast(
    pool: &SqlitePool,
    draft: &PodcastDraft,
) -> Result<Podcast, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO podcasts (id, feed_url, normalized_url, title, description, author, \
         site_url, language, artwork_url, auto_download_count, added_by, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&draft.feed_url)
    .bind(&draft.normalized_url)
    .bind(&draft.preview.title)
    .bind(&draft.preview.description)
    .bind(&draft.preview.author)
    .bind(&draft.preview.site_url)
    .bind(&draft.preview.language)
    .bind(&draft.preview.artwork_url)
    .bind(draft.auto_download_count)
    .bind(&draft.added_by)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_podcast(pool, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Applies administrator retention settings to one catalogue entry.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_podcast_retention(
    pool: &SqlitePool,
    id: &str,
    auto_download_count: i64,
    max_retained_episodes: i64,
) -> Result<Option<Podcast>, sqlx::Error> {
    let affected = sqlx::query(
        "UPDATE podcasts SET auto_download_count = ?, max_retained_episodes = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(auto_download_count)
    .bind(max_retained_episodes)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Ok(None);
    }
    get_podcast(pool, id).await
}

/// Refreshes channel-level metadata discovered during a feed refresh.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_podcast_metadata(
    pool: &SqlitePool,
    id: &str,
    preview: &PodcastFeedPreview,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE podcasts SET title = ?, description = ?, author = ?, site_url = ?, \
         language = ?, artwork_url = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&preview.title)
    .bind(&preview.description)
    .bind(&preview.author)
    .bind(&preview.site_url)
    .bind(&preview.language)
    .bind(&preview.artwork_url)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Removes one catalogue entry and reports the cached files its removal orphans.
///
/// The caller is responsible for unlinking the returned files; the row cascade cannot
/// reach the filesystem.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query or delete cannot be completed.
pub async fn delete_podcast(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<Vec<String>>, sqlx::Error> {
    let files = sqlx::query_scalar::<_, String>(
        "SELECT podcast_downloads.file_name FROM podcast_downloads \
         JOIN podcast_episodes ON podcast_episodes.id = podcast_downloads.episode_id \
         WHERE podcast_episodes.podcast_id = ? AND podcast_downloads.file_name <> ''",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let affected = sqlx::query("DELETE FROM podcasts WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Ok(None);
    }
    Ok(Some(files))
}

/// Lists the whole catalogue with one viewer's subscription state attached.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_podcast_summaries(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<PodcastSummary>, sqlx::Error> {
    sqlx::query_as::<_, PodcastSummary>(
        "SELECT podcasts.id, podcasts.title, podcasts.description, podcasts.author, \
                podcasts.site_url, podcasts.feed_url, podcasts.artwork_url, \
                COALESCE(length(podcasts.artwork_data), 0) > 0 AS has_artwork, \
                podcasts.auto_download_count, podcasts.max_retained_episodes, \
                podcast_subscriptions.user_id IS NOT NULL AS subscribed, \
                (SELECT COUNT(*) FROM podcast_episodes \
                   WHERE podcast_episodes.podcast_id = podcasts.id) AS episode_count, \
                (SELECT COUNT(*) FROM podcast_downloads \
                   JOIN podcast_episodes ON podcast_episodes.id = podcast_downloads.episode_id \
                   WHERE podcast_episodes.podcast_id = podcasts.id \
                   AND podcast_downloads.status = 'ready') AS downloaded_count, \
                (SELECT MAX(published_at) FROM podcast_episodes \
                   WHERE podcast_episodes.podcast_id = podcasts.id) AS latest_published_at, \
                podcasts.last_fetched_at, podcasts.last_error, podcasts.created_at \
         FROM podcasts \
         LEFT JOIN podcast_subscriptions ON podcast_subscriptions.podcast_id = podcasts.id \
              AND podcast_subscriptions.user_id = ? \
         ORDER BY podcasts.title COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Loads one catalogue entry's cached artwork.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_podcast_artwork(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<PodcastArtwork>, sqlx::Error> {
    sqlx::query_as::<_, PodcastArtwork>(
        "SELECT artwork_content_type AS content_type, artwork_data AS data FROM podcasts \
         WHERE id = ? AND COALESCE(length(artwork_data), 0) > 0",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Stores one fetched artwork image.
///
/// Only ever called with a successful response, so a failed fetch can never populate or
/// invalidate the cache.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn store_podcast_artwork(
    pool: &SqlitePool,
    id: &str,
    draft: &PodcastArtworkDraft,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE podcasts SET artwork_url = ?, artwork_content_type = ?, artwork_data = ?, \
         artwork_fetched_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&draft.source_url)
    .bind(&draft.content_type)
    .bind(&draft.data)
    .bind(&now)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Reports whether one catalogue entry needs its artwork refetched.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn podcast_artwork_is_stale(
    pool: &SqlitePool,
    id: &str,
    stale_before: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM podcasts WHERE id = ? AND artwork_url <> '' \
         AND (COALESCE(length(artwork_data), 0) = 0 OR artwork_fetched_at IS NULL \
              OR datetime(artwork_fetched_at) <= datetime(?))",
    )
    .bind(id)
    .bind(stale_before)
    .fetch_one(pool)
    .await?
        > 0)
}

// ---------------------------------------------------------------------------
// Refresh scheduling
// ---------------------------------------------------------------------------

/// Lists catalogue entries whose shared fetch window and refresh lease have both elapsed.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_due_podcasts(
    pool: &SqlitePool,
    due_before: &str,
    abandoned_before: &str,
    limit: i64,
) -> Result<Vec<PodcastRefreshTarget>, sqlx::Error> {
    sqlx::query_as::<_, PodcastRefreshTarget>(
        "SELECT id, feed_url FROM podcasts \
         WHERE (last_fetched_at IS NULL OR datetime(last_fetched_at) <= datetime(?)) \
         AND (refresh_started_at IS NULL OR datetime(refresh_started_at) <= datetime(?)) \
         ORDER BY COALESCE(last_fetched_at, created_at) ASC LIMIT ?",
    )
    .bind(due_before)
    .bind(abandoned_before)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Claims one due refresh with a short cross-process lease.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the atomic update cannot be completed.
pub async fn claim_podcast_refresh(
    pool: &SqlitePool,
    id: &str,
    due_before: &str,
    abandoned_before: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE podcasts SET refresh_started_at = ?, updated_at = ? WHERE id = ? \
         AND (last_fetched_at IS NULL OR datetime(last_fetched_at) <= datetime(?)) \
         AND (refresh_started_at IS NULL OR datetime(refresh_started_at) <= datetime(?))",
    )
    .bind(&now)
    .bind(&now)
    .bind(id)
    .bind(due_before)
    .bind(abandoned_before)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

/// Releases a refresh lease, recording either success or an isolated failure.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn finish_podcast_refresh(
    pool: &SqlitePool,
    id: &str,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE podcasts SET last_fetched_at = ?, refresh_started_at = NULL, last_error = ?, \
         updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(error)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

/// Counts one user's open requests, for the administrator-configured cap.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn count_pending_podcast_requests(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM podcast_requests WHERE user_id = ? AND status = 'pending'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Reports whether one user already has an open request for a normalized feed URL.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn has_open_podcast_request(
    pool: &SqlitePool,
    user_id: &str,
    normalized_url: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM podcast_requests \
         WHERE user_id = ? AND normalized_url = ? AND status = 'pending'",
    )
    .bind(user_id)
    .bind(normalized_url)
    .fetch_one(pool)
    .await?
        > 0)
}

/// Records one member request awaiting an administrator decision.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert or reload cannot be completed.
pub async fn insert_podcast_request(
    pool: &SqlitePool,
    draft: &PodcastRequestDraft,
) -> Result<PodcastRequest, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO podcast_requests (id, user_id, feed_url, normalized_url, resolved_title, \
         resolved_author, resolved_artwork_url, note, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&draft.user_id)
    .bind(&draft.feed_url)
    .bind(&draft.normalized_url)
    .bind(&draft.resolved_title)
    .bind(&draft.resolved_author)
    .bind(&draft.resolved_artwork_url)
    .bind(&draft.note)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_podcast_request(pool, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Loads one request by identifier.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_podcast_request(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<PodcastRequest>, sqlx::Error> {
    sqlx::query_as::<_, PodcastRequest>(request_select!("WHERE podcast_requests.id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Lists one user's own requests, newest first.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_podcast_requests_for_user(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<PodcastRequest>, sqlx::Error> {
    sqlx::query_as::<_, PodcastRequest>(request_select!(
        "WHERE podcast_requests.user_id = ? ORDER BY podcast_requests.created_at DESC"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Lists the administrator review queue, optionally filtered by status.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_podcast_requests(
    pool: &SqlitePool,
    status: Option<&str>,
) -> Result<Vec<PodcastRequest>, sqlx::Error> {
    match status {
        Some(status) => {
            sqlx::query_as::<_, PodcastRequest>(request_select!(
                "WHERE podcast_requests.status = ? \
                 ORDER BY CASE podcast_requests.status WHEN 'pending' THEN 0 ELSE 1 END ASC, \
                 podcast_requests.created_at DESC"
            ))
            .bind(status)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, PodcastRequest>(request_select!(
                "ORDER BY CASE podcast_requests.status WHEN 'pending' THEN 0 ELSE 1 END ASC, \
                 podcast_requests.created_at DESC"
            ))
            .fetch_all(pool)
            .await
        }
    }
}

/// Withdraws one of the caller's own open requests.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn withdraw_podcast_request(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE podcast_requests SET status = 'withdrawn', updated_at = ? \
         WHERE id = ? AND user_id = ? AND status = 'pending'",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

/// Marks one request approved and subscribes its requester to the resulting podcast.
///
/// Both writes share a transaction so a request can never read as approved without the
/// subscription that approval promised.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn approve_podcast_request(
    pool: &SqlitePool,
    id: &str,
    decided_by: &str,
    podcast_id: &str,
    decision_note: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let affected = sqlx::query(
        "UPDATE podcast_requests SET status = 'approved', decision_note = ?, decided_by = ?, \
         decided_at = ?, podcast_id = ?, updated_at = ? WHERE id = ? AND status = 'pending'",
    )
    .bind(decision_note)
    .bind(decided_by)
    .bind(&now)
    .bind(podcast_id)
    .bind(&now)
    .bind(id)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if affected == 0 {
        transaction.rollback().await?;
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO podcast_subscriptions (user_id, podcast_id, created_at) \
         SELECT user_id, ?, ? FROM podcast_requests WHERE id = ? \
         ON CONFLICT(user_id, podcast_id) DO NOTHING",
    )
    .bind(podcast_id)
    .bind(&now)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(true)
}

/// Marks one request rejected, retaining the reason for the requester.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn reject_podcast_request(
    pool: &SqlitePool,
    id: &str,
    decided_by: &str,
    decision_note: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE podcast_requests SET status = 'rejected', decision_note = ?, decided_by = ?, \
         decided_at = ?, updated_at = ? WHERE id = ? AND status = 'pending'",
    )
    .bind(decision_note)
    .bind(decided_by)
    .bind(&now)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

// ---------------------------------------------------------------------------
// Subscriptions
// ---------------------------------------------------------------------------

/// Subscribes one user to a catalogue entry.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn subscribe_to_podcast(
    pool: &SqlitePool,
    user_id: &str,
    podcast_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO podcast_subscriptions (user_id, podcast_id, created_at) VALUES (?, ?, ?) \
         ON CONFLICT(user_id, podcast_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(podcast_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

/// Removes one subscription.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn unsubscribe_from_podcast(
    pool: &SqlitePool,
    user_id: &str,
    podcast_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM podcast_subscriptions WHERE user_id = ? AND podcast_id = ?")
            .bind(user_id)
            .bind(podcast_id)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}

/// Reports whether one user may reach one episode through an active subscription.
///
/// Every episode read — metadata, audio bytes, progress — resolves through this.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn user_can_access_episode(
    pool: &SqlitePool,
    user_id: &str,
    episode_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM podcast_episodes \
         JOIN podcast_subscriptions ON podcast_subscriptions.podcast_id \
              = podcast_episodes.podcast_id \
         WHERE podcast_episodes.id = ? AND podcast_subscriptions.user_id = ?",
    )
    .bind(episode_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?
        > 0)
}

// ---------------------------------------------------------------------------
// Episodes
// ---------------------------------------------------------------------------

/// Indexes parsed feed items, reporting the identifiers of newly discovered episodes.
///
/// Existing rows are refreshed in place so retitled or re-encoded episodes stay accurate
/// without losing anyone's listening position.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn upsert_podcast_episodes(
    pool: &SqlitePool,
    podcast_id: &str,
    drafts: &[PodcastEpisodeDraft],
) -> Result<Vec<String>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let known =
        sqlx::query_scalar::<_, String>("SELECT guid FROM podcast_episodes WHERE podcast_id = ?")
            .bind(podcast_id)
            .fetch_all(&mut *transaction)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();

    let mut inserted = Vec::new();
    for draft in drafts {
        if known.contains(&draft.guid) {
            sqlx::query(
                "UPDATE podcast_episodes SET title = ?, description = ?, episode_url = ?, \
                 enclosure_url = ?, enclosure_type = ?, enclosure_bytes = ?, \
                 duration_seconds = ?, published_at = ?, fetched_at = ? \
                 WHERE podcast_id = ? AND guid = ?",
            )
            .bind(&draft.title)
            .bind(&draft.description)
            .bind(&draft.episode_url)
            .bind(&draft.enclosure_url)
            .bind(&draft.enclosure_type)
            .bind(draft.enclosure_bytes)
            .bind(draft.duration_seconds)
            .bind(&draft.published_at)
            .bind(&now)
            .bind(podcast_id)
            .bind(&draft.guid)
            .execute(&mut *transaction)
            .await?;
            continue;
        }
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO podcast_episodes (id, podcast_id, guid, title, description, \
             episode_url, enclosure_url, enclosure_type, enclosure_bytes, duration_seconds, \
             published_at, fetched_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(podcast_id)
        .bind(&draft.guid)
        .bind(&draft.title)
        .bind(&draft.description)
        .bind(&draft.episode_url)
        .bind(&draft.enclosure_url)
        .bind(&draft.enclosure_type)
        .bind(draft.enclosure_bytes)
        .bind(draft.duration_seconds)
        .bind(&draft.published_at)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        inserted.push(id);
    }
    transaction.commit().await?;
    Ok(inserted)
}

/// Lists one podcast's episodes with the caller's listening state.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_podcast_episodes(
    pool: &SqlitePool,
    user_id: &str,
    podcast_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<PodcastEpisode>, sqlx::Error> {
    sqlx::query_as::<_, PodcastEpisode>(episode_select!(
        "WHERE podcast_episodes.podcast_id = ? \
         ORDER BY podcast_episodes.published_at DESC LIMIT ? OFFSET ?"
    ))
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(podcast_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Loads one episode with the caller's listening state.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_podcast_episode(
    pool: &SqlitePool,
    user_id: &str,
    episode_id: &str,
) -> Result<Option<PodcastEpisode>, sqlx::Error> {
    sqlx::query_as::<_, PodcastEpisode>(episode_select!("WHERE podcast_episodes.id = ?"))
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(episode_id)
        .fetch_optional(pool)
        .await
}

/// Lists the newest episodes across everything the caller subscribes to.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_recent_podcast_episodes(
    pool: &SqlitePool,
    user_id: &str,
    limit: i64,
) -> Result<Vec<PodcastEpisode>, sqlx::Error> {
    sqlx::query_as::<_, PodcastEpisode>(episode_select!(
        "JOIN podcast_subscriptions ON podcast_subscriptions.podcast_id \
              = podcast_episodes.podcast_id AND podcast_subscriptions.user_id = ? \
         ORDER BY podcast_episodes.published_at DESC LIMIT ?"
    ))
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Lists episodes the caller has started but not finished.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_in_progress_podcast_episodes(
    pool: &SqlitePool,
    user_id: &str,
    limit: i64,
) -> Result<Vec<PodcastEpisode>, sqlx::Error> {
    sqlx::query_as::<_, PodcastEpisode>(episode_select!(
        "WHERE podcast_episode_progress.position_seconds > 0 \
         AND podcast_episode_progress.completed_at IS NULL \
         ORDER BY podcast_episode_progress.updated_at DESC LIMIT ?"
    ))
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Lists the caller's saved episodes.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_saved_podcast_episodes(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<PodcastEpisode>, sqlx::Error> {
    sqlx::query_as::<_, PodcastEpisode>(episode_select!(
        "WHERE podcast_saved_episodes.saved_at IS NOT NULL \
         ORDER BY podcast_saved_episodes.saved_at DESC"
    ))
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Lists the caller's play queue in order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_podcast_queue(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<PodcastEpisode>, sqlx::Error> {
    sqlx::query_as::<_, PodcastEpisode>(episode_select!(
        "WHERE podcast_queue.position IS NOT NULL ORDER BY podcast_queue.position ASC"
    ))
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Lists the newest episode identifiers for a podcast, for automatic caching.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_newest_episode_ids(
    pool: &SqlitePool,
    podcast_id: &str,
    limit: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT id FROM podcast_episodes WHERE podcast_id = ? \
         ORDER BY published_at DESC LIMIT ?",
    )
    .bind(podcast_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Lists the episodes of one podcast that are not cached and not already being fetched.
///
/// A failed download is included so a bulk request also retries what previously broke,
/// which matches the per-episode control offering `Retry` in the same situation.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_downloadable_episode_ids(
    pool: &SqlitePool,
    podcast_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT episodes.id FROM podcast_episodes AS episodes \
         LEFT JOIN podcast_downloads AS downloads ON downloads.episode_id = episodes.id \
         WHERE episodes.podcast_id = ? \
             AND (downloads.episode_id IS NULL OR downloads.status = 'failed') \
         ORDER BY episodes.published_at DESC",
    )
    .bind(podcast_id)
    .fetch_all(pool)
    .await
}

/// Drops episodes beyond a podcast's retention window, reporting orphaned cached files.
///
/// Episodes anyone has saved, queued, or started are exempt so retention never deletes
/// something a listener is midway through.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query or delete cannot be completed.
pub async fn trim_podcast_episodes(
    pool: &SqlitePool,
    podcast_id: &str,
    max_retained: i64,
) -> Result<Vec<String>, sqlx::Error> {
    let doomed = sqlx::query_scalar::<_, String>(
        "SELECT id FROM podcast_episodes WHERE podcast_id = ? AND id NOT IN ( \
             SELECT id FROM podcast_episodes WHERE podcast_id = ? \
             ORDER BY published_at DESC LIMIT ? \
         ) \
         AND NOT EXISTS (SELECT 1 FROM podcast_saved_episodes \
             WHERE podcast_saved_episodes.episode_id = podcast_episodes.id) \
         AND NOT EXISTS (SELECT 1 FROM podcast_queue \
             WHERE podcast_queue.episode_id = podcast_episodes.id) \
         AND NOT EXISTS (SELECT 1 FROM podcast_episode_progress \
             WHERE podcast_episode_progress.episode_id = podcast_episodes.id \
             AND podcast_episode_progress.position_seconds > 0)",
    )
    .bind(podcast_id)
    .bind(podcast_id)
    .bind(max_retained)
    .fetch_all(pool)
    .await?;
    if doomed.is_empty() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut transaction = pool.begin().await?;
    for episode_id in &doomed {
        if let Some(file_name) = sqlx::query_scalar::<_, String>(
            "SELECT file_name FROM podcast_downloads WHERE episode_id = ? AND file_name <> ''",
        )
        .bind(episode_id)
        .fetch_optional(&mut *transaction)
        .await?
        {
            files.push(file_name);
        }
        sqlx::query("DELETE FROM podcast_episodes WHERE id = ?")
            .bind(episode_id)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(files)
}

// ---------------------------------------------------------------------------
// Downloads
// ---------------------------------------------------------------------------

/// Places one episode on the download queue.
///
/// A row that already exists is only revived when it previously failed, so pressing
/// download twice never restarts a healthy transfer. `requested_by` is `None` for the
/// automatic downloads the refresh worker schedules, which have no requester.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the upsert cannot be completed.
pub async fn enqueue_podcast_download(
    pool: &SqlitePool,
    episode_id: &str,
    requested_by: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO podcast_downloads (episode_id, status, requested_by, created_at, updated_at) \
         VALUES (?, 'queued', ?, ?, ?) \
         ON CONFLICT(episode_id) DO UPDATE SET \
             status = CASE WHEN podcast_downloads.status = 'failed' THEN 'queued' \
                           ELSE podcast_downloads.status END, \
             attempts = CASE WHEN podcast_downloads.status = 'failed' THEN 0 \
                             ELSE podcast_downloads.attempts END, \
             last_error = CASE WHEN podcast_downloads.status = 'failed' THEN '' \
                               ELSE podcast_downloads.last_error END, \
             updated_at = excluded.updated_at",
    )
    .bind(episode_id)
    .bind(requested_by)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Claims one queued download, or one whose lease has expired after a crash.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the claim cannot be completed.
pub async fn claim_podcast_download(
    pool: &SqlitePool,
    abandoned_before: &str,
    max_attempts: i64,
) -> Result<Option<PodcastDownloadJob>, sqlx::Error> {
    let Some(episode_id) = sqlx::query_scalar::<_, String>(
        "SELECT episode_id FROM podcast_downloads \
         WHERE attempts < ? AND (status = 'queued' OR (status = 'downloading' \
             AND (lease_started_at IS NULL OR datetime(lease_started_at) <= datetime(?)))) \
         ORDER BY created_at ASC LIMIT 1",
    )
    .bind(max_attempts)
    .bind(abandoned_before)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let now = chrono::Utc::now().to_rfc3339();
    let claimed = sqlx::query(
        "UPDATE podcast_downloads SET status = 'downloading', lease_started_at = ?, \
         downloaded_bytes = 0, attempts = attempts + 1, updated_at = ? WHERE episode_id = ? \
         AND (status = 'queued' OR (status = 'downloading' \
             AND (lease_started_at IS NULL OR datetime(lease_started_at) <= datetime(?))))",
    )
    .bind(&now)
    .bind(&now)
    .bind(&episode_id)
    .bind(abandoned_before)
    .execute(pool)
    .await?
    .rows_affected();
    if claimed == 0 {
        return Ok(None);
    }

    sqlx::query_as::<_, PodcastDownloadJob>(
        "SELECT podcast_downloads.episode_id, podcast_episodes.podcast_id, \
                podcast_episodes.enclosure_url, podcast_episodes.enclosure_type, \
                podcast_downloads.attempts \
         FROM podcast_downloads \
         JOIN podcast_episodes ON podcast_episodes.id = podcast_downloads.episode_id \
         WHERE podcast_downloads.episode_id = ?",
    )
    .bind(&episode_id)
    .fetch_optional(pool)
    .await
}

/// Records streaming progress so the interface can show a live transfer.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_podcast_download_progress(
    pool: &SqlitePool,
    episode_id: &str,
    downloaded_bytes: i64,
    byte_size: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE podcast_downloads SET downloaded_bytes = ?, byte_size = ?, updated_at = ? \
         WHERE episode_id = ?",
    )
    .bind(downloaded_bytes)
    .bind(byte_size)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(episode_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Publishes a completed download.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn mark_podcast_download_ready(
    pool: &SqlitePool,
    episode_id: &str,
    file_name: &str,
    content_type: &str,
    byte_size: i64,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE podcast_downloads SET status = 'ready', file_name = ?, content_type = ?, \
         byte_size = ?, downloaded_bytes = ?, last_error = '', lease_started_at = NULL, \
         last_accessed_at = ?, updated_at = ? WHERE episode_id = ?",
    )
    .bind(file_name)
    .bind(content_type)
    .bind(byte_size)
    .bind(byte_size)
    .bind(&now)
    .bind(&now)
    .bind(episode_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Parks a download with a reason the interface can show.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn mark_podcast_download_failed(
    pool: &SqlitePool,
    episode_id: &str,
    error: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE podcast_downloads SET status = 'failed', last_error = ?, \
         lease_started_at = NULL, downloaded_bytes = 0, updated_at = ? WHERE episode_id = ?",
    )
    .bind(error)
    .bind(&now)
    .bind(episode_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns one episode to the queue without consuming an attempt.
///
/// Used when the worker cannot proceed for a reason unrelated to the episode, such as a
/// storage budget that eviction could not satisfy.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn requeue_podcast_download(
    pool: &SqlitePool,
    episode_id: &str,
    error: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE podcast_downloads SET status = 'queued', last_error = ?, \
         lease_started_at = NULL, downloaded_bytes = 0, \
         attempts = MAX(0, attempts - 1), updated_at = ? WHERE episode_id = ?",
    )
    .bind(error)
    .bind(&now)
    .bind(episode_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Loads one cached file for playback.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_podcast_cached_file(
    pool: &SqlitePool,
    episode_id: &str,
) -> Result<Option<PodcastCachedFile>, sqlx::Error> {
    sqlx::query_as::<_, PodcastCachedFile>(
        "SELECT episode_id, file_name, content_type, byte_size FROM podcast_downloads \
         WHERE episode_id = ? AND status = 'ready' AND file_name <> ''",
    )
    .bind(episode_id)
    .fetch_optional(pool)
    .await
}

/// Marks one cached file as recently used, which is what eviction ranks on.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn touch_podcast_download(
    pool: &SqlitePool,
    episode_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE podcast_downloads SET last_accessed_at = ? WHERE episode_id = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(episode_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Sums the bytes currently committed to cached audio.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn podcast_storage_used_bytes(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(byte_size), 0) FROM podcast_downloads WHERE status = 'ready'",
    )
    .fetch_one(pool)
    .await
}

/// Lists cached files eligible for eviction, least recently used first.
///
/// Pinned files, files still transferring, and anything sitting in someone's play queue
/// are excluded.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_podcast_eviction_candidates(
    pool: &SqlitePool,
) -> Result<Vec<PodcastCachedFile>, sqlx::Error> {
    sqlx::query_as::<_, PodcastCachedFile>(
        "SELECT episode_id, file_name, content_type, byte_size FROM podcast_downloads \
         WHERE status = 'ready' AND pinned = 0 AND file_name <> '' \
         AND NOT EXISTS (SELECT 1 FROM podcast_queue \
             WHERE podcast_queue.episode_id = podcast_downloads.episode_id) \
         ORDER BY COALESCE(last_accessed_at, created_at) ASC",
    )
    .fetch_all(pool)
    .await
}

/// Forgets one cached file, reporting what the caller must unlink.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query or delete cannot be completed.
pub async fn delete_podcast_download(
    pool: &SqlitePool,
    episode_id: &str,
) -> Result<Option<PodcastCachedFile>, sqlx::Error> {
    let cached = sqlx::query_as::<_, PodcastCachedFile>(
        "SELECT episode_id, file_name, content_type, byte_size FROM podcast_downloads \
         WHERE episode_id = ?",
    )
    .bind(episode_id)
    .fetch_optional(pool)
    .await?;
    if cached.is_none() {
        return Ok(None);
    }
    sqlx::query("DELETE FROM podcast_downloads WHERE episode_id = ?")
        .bind(episode_id)
        .execute(pool)
        .await?;
    Ok(cached)
}

/// Lists every file name the database believes exists on disk.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_podcast_cached_file_names(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT file_name FROM podcast_downloads WHERE status = 'ready' AND file_name <> ''",
    )
    .fetch_all(pool)
    .await
}

/// Returns downloads abandoned mid-transfer to the queue on startup.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn reset_interrupted_podcast_downloads(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE podcast_downloads SET status = 'queued', lease_started_at = NULL, \
         downloaded_bytes = 0, updated_at = ? WHERE status = 'downloading'",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?
    .rows_affected())
}

/// Marks a cached row failed when its file has vanished from disk.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn invalidate_missing_podcast_download(
    pool: &SqlitePool,
    file_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE podcast_downloads SET status = 'failed', \
         last_error = 'cached file was missing from disk', byte_size = 0, \
         downloaded_bytes = 0, file_name = '', updated_at = ? WHERE file_name = ?",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(file_name)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Listening state
// ---------------------------------------------------------------------------

/// Stores one listener's position in one episode.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the upsert cannot be completed.
pub async fn upsert_podcast_progress(
    pool: &SqlitePool,
    user_id: &str,
    episode_id: &str,
    position_seconds: i64,
    completed: bool,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let completed_at = completed.then(|| now.clone());
    sqlx::query(
        "INSERT INTO podcast_episode_progress \
         (user_id, episode_id, position_seconds, completed_at, updated_at) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(user_id, episode_id) DO UPDATE SET position_seconds = excluded.position_seconds, \
         completed_at = excluded.completed_at, updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(episode_id)
    .bind(position_seconds)
    .bind(completed_at)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Adds or removes one saved episode.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the write cannot be completed.
pub async fn set_podcast_episode_saved(
    pool: &SqlitePool,
    user_id: &str,
    episode_id: &str,
    saved: bool,
) -> Result<(), sqlx::Error> {
    if saved {
        sqlx::query(
            "INSERT INTO podcast_saved_episodes (user_id, episode_id, saved_at) \
             VALUES (?, ?, ?) ON CONFLICT(user_id, episode_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(episode_id)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(pool)
        .await?;
    } else {
        sqlx::query("DELETE FROM podcast_saved_episodes WHERE user_id = ? AND episode_id = ?")
            .bind(user_id)
            .bind(episode_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Appends one episode to the end of a listener's play queue.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn append_to_podcast_queue(
    pool: &SqlitePool,
    user_id: &str,
    episode_id: &str,
    max_entries: i64,
) -> Result<bool, sqlx::Error> {
    let max_entries = max_entries.min(PODCAST_QUEUE_LIMIT);
    let mut transaction = pool.begin().await?;
    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM podcast_queue WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&mut *transaction)
            .await?;
    if count >= max_entries {
        transaction.rollback().await?;
        return Ok(false);
    }
    let next_position = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT MAX(position) FROM podcast_queue WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&mut *transaction)
    .await?
    .map_or(0, |position| position + 1);
    sqlx::query(
        "INSERT INTO podcast_queue (user_id, episode_id, position, added_at) \
         VALUES (?, ?, ?, ?) ON CONFLICT(user_id, episode_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(episode_id)
    .bind(next_position)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(true)
}

/// Removes one episode from a listener's play queue and closes the gap.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn remove_from_podcast_queue(
    pool: &SqlitePool,
    user_id: &str,
    episode_id: &str,
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let removed = sqlx::query("DELETE FROM podcast_queue WHERE user_id = ? AND episode_id = ?")
        .bind(user_id)
        .bind(episode_id)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    if removed == 0 {
        transaction.rollback().await?;
        return Ok(false);
    }
    let ordered = sqlx::query_scalar::<_, String>(
        "SELECT episode_id FROM podcast_queue WHERE user_id = ? ORDER BY position ASC",
    )
    .bind(user_id)
    .fetch_all(&mut *transaction)
    .await?;
    rewrite_queue_positions(&mut transaction, user_id, &ordered).await?;
    transaction.commit().await?;
    Ok(true)
}

/// Rewrites a listener's play queue into the supplied order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn reorder_podcast_queue(
    pool: &SqlitePool,
    user_id: &str,
    episode_ids: &[String],
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let existing =
        sqlx::query_scalar::<_, String>("SELECT episode_id FROM podcast_queue WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&mut *transaction)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
    let requested = episode_ids.iter().cloned().collect::<HashSet<_>>();
    if existing != requested {
        transaction.rollback().await?;
        return Ok(false);
    }
    rewrite_queue_positions(&mut transaction, user_id, episode_ids).await?;
    transaction.commit().await?;
    Ok(true)
}

/// Writes final queue positions in two passes.
///
/// `UNIQUE(user_id, position)` is checked per statement, so a single pass deadlocks on any
/// order that swaps two rows. Every row is first parked in the 256..511 band, which no
/// final position can occupy, and only then given its 0..255 value.
async fn rewrite_queue_positions(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: &str,
    episode_ids: &[String],
) -> Result<(), sqlx::Error> {
    for (index, episode_id) in episode_ids.iter().enumerate() {
        let position = i64::try_from(index).unwrap_or(PODCAST_QUEUE_LIMIT - 1);
        sqlx::query("UPDATE podcast_queue SET position = ? WHERE user_id = ? AND episode_id = ?")
            .bind(QUEUE_PARK_OFFSET + position)
            .bind(user_id)
            .bind(episode_id)
            .execute(&mut **transaction)
            .await?;
    }
    for (index, episode_id) in episode_ids.iter().enumerate() {
        let position = i64::try_from(index).unwrap_or(PODCAST_QUEUE_LIMIT - 1);
        sqlx::query("UPDATE podcast_queue SET position = ? WHERE user_id = ? AND episode_id = ?")
            .bind(position)
            .bind(user_id)
            .bind(episode_id)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

/// Deletes everything one account owns in the Podcasts feature.
///
/// Catalogue entries and cached audio are shared instance resources and deliberately
/// survive: removing one listener must never delete an episode another is midway through.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn delete_user_podcast_content(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<u64, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let mut deleted = 0;
    for statement in [
        "DELETE FROM podcast_queue WHERE user_id = ?",
        "DELETE FROM podcast_saved_episodes WHERE user_id = ?",
        "DELETE FROM podcast_episode_progress WHERE user_id = ?",
        "DELETE FROM podcast_subscriptions WHERE user_id = ?",
        "DELETE FROM podcast_requests WHERE user_id = ?",
    ] {
        deleted += sqlx::query(statement)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
    }
    transaction.commit().await?;
    Ok(deleted)
}

/// Reports how many listeners subscribe to each of a set of podcasts.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn count_podcast_subscribers(
    pool: &SqlitePool,
    podcast_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM podcast_subscriptions WHERE podcast_id = ?")
        .bind(podcast_id)
        .fetch_one(pool)
        .await
}

/// Lists identifiers for every catalogue entry, for maintenance passes.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_podcast_ids(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query("SELECT id FROM podcasts")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| row.try_get::<String, _>("id"))
        .collect()
}
