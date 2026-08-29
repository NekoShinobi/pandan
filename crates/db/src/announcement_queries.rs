use crate::entities::{
    Announcement, AnnouncementDraft, AnnouncementImage, AnnouncementReaction, UserAvatar,
};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, FromRow)]
struct AnnouncementRecord {
    id: String,
    author_id: Option<String>,
    author_name: String,
    title: String,
    content: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct AnnouncementReactionRecord {
    emoji: String,
    count: i64,
    reacted_by_viewer: bool,
}

async fn hydrate_announcement(
    pool: &SqlitePool,
    viewer_id: &str,
    record: AnnouncementRecord,
) -> Result<Announcement, sqlx::Error> {
    let images = sqlx::query_as::<_, AnnouncementImage>(
        "SELECT id, file_name, mime_type, byte_size, created_at \
         FROM announcement_images WHERE announcement_id = ? \
         ORDER BY created_at ASC, id ASC",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let reactions = sqlx::query_as::<_, AnnouncementReactionRecord>(
        "SELECT emoji, COUNT(*) AS count, \
                CAST(MAX(CASE WHEN user_id = ? THEN 1 ELSE 0 END) AS BOOLEAN) \
                    AS reacted_by_viewer \
         FROM announcement_reactions WHERE announcement_id = ? \
         GROUP BY emoji ORDER BY MIN(created_at) ASC, emoji ASC",
    )
    .bind(viewer_id)
    .bind(&record.id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|reaction| AnnouncementReaction {
        emoji: reaction.emoji,
        count: reaction.count,
        reacted_by_viewer: reaction.reacted_by_viewer,
    })
    .collect();

    Ok(Announcement {
        id: record.id,
        author_id: record.author_id,
        author_name: record.author_name,
        title: record.title,
        content: record.content,
        images,
        reactions,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

/// Lists the newest instance announcements for an authenticated viewer.
///
/// # Errors
///
/// Returns the underlying SQLx error when announcements or related records cannot load.
pub async fn list_announcements(
    pool: &SqlitePool,
    viewer_id: &str,
) -> Result<Vec<Announcement>, sqlx::Error> {
    let records = sqlx::query_as::<_, AnnouncementRecord>(
        "SELECT a.id, a.author_id, \
                COALESCE(author.display_name, 'Former administrator') AS author_name, \
                a.title, a.content, a.created_at, a.updated_at \
         FROM announcements a \
         LEFT JOIN user_settings author ON author.user_id = a.author_id \
         ORDER BY a.created_at DESC, a.id DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await?;
    let mut announcements = Vec::with_capacity(records.len());
    for record in records {
        announcements.push(hydrate_announcement(pool, viewer_id, record).await?);
    }
    Ok(announcements)
}

/// Loads one instance announcement for an authenticated viewer.
///
/// # Errors
///
/// Returns the underlying SQLx error when the announcement cannot be queried.
pub async fn get_announcement(
    pool: &SqlitePool,
    viewer_id: &str,
    announcement_id: &str,
) -> Result<Option<Announcement>, sqlx::Error> {
    let record = sqlx::query_as::<_, AnnouncementRecord>(
        "SELECT a.id, a.author_id, \
                COALESCE(author.display_name, 'Former administrator') AS author_name, \
                a.title, a.content, a.created_at, a.updated_at \
         FROM announcements a \
         LEFT JOIN user_settings author ON author.user_id = a.author_id \
         WHERE a.id = ?",
    )
    .bind(announcement_id)
    .fetch_optional(pool)
    .await?;
    match record {
        Some(record) => Ok(Some(hydrate_announcement(pool, viewer_id, record).await?)),
        None => Ok(None),
    }
}

/// Loads the current author's avatar for an existing announcement.
///
/// # Errors
///
/// Returns the underlying SQLx error when the avatar cannot be queried.
pub async fn get_announcement_author_avatar(
    pool: &SqlitePool,
    announcement_id: &str,
) -> Result<Option<UserAvatar>, sqlx::Error> {
    sqlx::query_as::<_, UserAvatar>(
        "SELECT avatar.mime_type, avatar.image_data, avatar.updated_at \
         FROM announcements announcement \
         INNER JOIN user_avatars avatar ON avatar.user_id = announcement.author_id \
         WHERE announcement.id = ?",
    )
    .bind(announcement_id)
    .fetch_optional(pool)
    .await
}

/// Creates an announcement authored by an administrator.
///
/// # Errors
///
/// Returns the underlying SQLx error when the announcement cannot be stored or reloaded.
pub async fn create_announcement(
    pool: &SqlitePool,
    author_id: &str,
    draft: &AnnouncementDraft,
) -> Result<Announcement, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO announcements (id, author_id, title, content, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(author_id)
    .bind(&draft.title)
    .bind(&draft.content)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_announcement(pool, author_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Updates the title and Markdown body of one announcement.
///
/// # Errors
///
/// Returns the underlying SQLx error when the announcement cannot be updated or reloaded.
pub async fn update_announcement(
    pool: &SqlitePool,
    viewer_id: &str,
    announcement_id: &str,
    draft: &AnnouncementDraft,
) -> Result<Option<Announcement>, sqlx::Error> {
    let changed =
        sqlx::query("UPDATE announcements SET title = ?, content = ?, updated_at = ? WHERE id = ?")
            .bind(&draft.title)
            .bind(&draft.content)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(announcement_id)
            .execute(pool)
            .await?
            .rows_affected();
    if changed == 0 {
        return Ok(None);
    }
    get_announcement(pool, viewer_id, announcement_id).await
}

/// Deletes one announcement and its cascading images and reactions.
///
/// # Errors
///
/// Returns the underlying SQLx error when the announcement cannot be deleted.
pub async fn delete_announcement(
    pool: &SqlitePool,
    announcement_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM announcements WHERE id = ?")
        .bind(announcement_id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

/// Stores one validated image on an announcement.
///
/// # Errors
///
/// Returns the underlying SQLx error when the parent lookup or insert fails.
pub async fn create_announcement_image(
    pool: &SqlitePool,
    announcement_id: &str,
    file_name: &str,
    mime_type: &str,
    data: &[u8],
) -> Result<Option<AnnouncementImage>, sqlx::Error> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM announcements WHERE id = ?)")
            .bind(announcement_id)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Ok(None);
    }
    let image = AnnouncementImage {
        id: uuid::Uuid::new_v4().to_string(),
        file_name: file_name.to_owned(),
        mime_type: mime_type.to_owned(),
        byte_size: i64::try_from(data.len()).unwrap_or(i64::MAX),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    sqlx::query(
        "INSERT INTO announcement_images \
         (id, announcement_id, file_name, mime_type, byte_size, image_data, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&image.id)
    .bind(announcement_id)
    .bind(&image.file_name)
    .bind(&image.mime_type)
    .bind(image.byte_size)
    .bind(data)
    .bind(&image.created_at)
    .execute(pool)
    .await?;
    Ok(Some(image))
}

/// Loads announcement image bytes after the request has been authenticated.
///
/// # Errors
///
/// Returns the underlying SQLx error when the image cannot be queried.
pub async fn get_announcement_image(
    pool: &SqlitePool,
    announcement_id: &str,
    image_id: &str,
) -> Result<Option<(String, String, Vec<u8>)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT file_name, mime_type, image_data FROM announcement_images \
         WHERE id = ? AND announcement_id = ?",
    )
    .bind(image_id)
    .bind(announcement_id)
    .fetch_optional(pool)
    .await
}

/// Deletes one image from an announcement.
///
/// # Errors
///
/// Returns the underlying SQLx error when the image cannot be deleted.
pub async fn delete_announcement_image(
    pool: &SqlitePool,
    announcement_id: &str,
    image_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM announcement_images WHERE id = ? AND announcement_id = ?")
            .bind(image_id)
            .bind(announcement_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Adds one emoji reaction from an authenticated account.
///
/// # Errors
///
/// Returns the underlying SQLx error when the reaction cannot be stored.
pub async fn add_announcement_reaction(
    pool: &SqlitePool,
    user_id: &str,
    announcement_id: &str,
    emoji: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "INSERT OR IGNORE INTO announcement_reactions \
         (announcement_id, user_id, emoji, created_at) \
         SELECT id, ?, ?, ? FROM announcements WHERE id = ?",
    )
    .bind(user_id)
    .bind(emoji)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(announcement_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Removes one emoji reaction owned by an authenticated account.
///
/// # Errors
///
/// Returns the underlying SQLx error when the reaction cannot be deleted.
pub async fn remove_announcement_reaction(
    pool: &SqlitePool,
    user_id: &str,
    announcement_id: &str,
    emoji: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM announcement_reactions \
         WHERE announcement_id = ? AND user_id = ? AND emoji = ?",
    )
    .bind(announcement_id)
    .bind(user_id)
    .bind(emoji)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn announcements_are_instance_visible_and_reactions_are_viewer_scoped() {
        let pool = crate::connect("sqlite::memory:")
            .await
            .expect("database connects");
        let (author, _) = crate::queries::create_account(
            &pool,
            "announcement-author@example.com",
            "$argon2id$announcement-author",
            "Server Admin",
        )
        .await
        .expect("author creates");
        let (reader, _) = crate::queries::create_account(
            &pool,
            "announcement-reader@example.com",
            "$argon2id$announcement-reader",
            "Server Reader",
        )
        .await
        .expect("reader creates");

        let announcement = create_announcement(
            &pool,
            &author.id,
            &AnnouncementDraft {
                title: "Maintenance window".to_owned(),
                content: "Services restart at **03:00 UTC**.".to_owned(),
            },
        )
        .await
        .expect("announcement creates");
        crate::queries::upsert_user_avatar(&pool, &author.id, "image/png", b"avatar")
            .await
            .expect("author avatar stores");
        let avatar = get_announcement_author_avatar(&pool, &announcement.id)
            .await
            .expect("author avatar loads")
            .expect("announcement author has an avatar");
        assert_eq!(avatar.mime_type, "image/png");
        assert_eq!(avatar.image_data, b"avatar");
        assert_eq!(
            list_announcements(&pool, &reader.id).await.unwrap().len(),
            1
        );

        add_announcement_reaction(&pool, &reader.id, &announcement.id, "👍")
            .await
            .expect("reaction stores");
        let author_view = get_announcement(&pool, &author.id, &announcement.id)
            .await
            .unwrap()
            .unwrap();
        let reader_view = get_announcement(&pool, &reader.id, &announcement.id)
            .await
            .unwrap()
            .unwrap();
        assert!(!author_view.reactions[0].reacted_by_viewer);
        assert!(reader_view.reactions[0].reacted_by_viewer);
        assert_eq!(reader_view.reactions[0].count, 1);
    }
}
