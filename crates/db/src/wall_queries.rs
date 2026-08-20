use crate::entities::{Wall, WallDraft, WallImage};
use sqlx::{FromRow, Row, SqlitePool};

/// One wall row exactly as the projection returns it, before tags are attached.
#[derive(Debug, Clone, FromRow)]
struct WallRecord {
    id: String,
    user_id: Option<String>,
    submitted_by_name: String,
    title: String,
    description: String,
    status: String,
    decision_note: String,
    decided_by_name: Option<String>,
    decided_at: Option<String>,
    mime_type: String,
    byte_size: i64,
    width: i64,
    height: i64,
    created_at: String,
    updated_at: String,
}

impl WallRecord {
    fn into_wall(self, tags: Vec<String>) -> Wall {
        Wall {
            id: self.id,
            user_id: self.user_id,
            submitted_by_name: self.submitted_by_name,
            title: self.title,
            description: self.description,
            status: self.status,
            decision_note: self.decision_note,
            decided_by_name: self.decided_by_name,
            decided_at: self.decided_at,
            mime_type: self.mime_type,
            byte_size: self.byte_size,
            width: self.width,
            height: self.height,
            tags,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Builds a wall query, resolving both user references to display names.
///
/// The projection is stitched together at compile time so no query string is ever
/// assembled from runtime values. Image and thumbnail blobs are never selected here.
macro_rules! wall_select {
    ($tail:literal) => {
        concat!(
            "SELECT walls.id, walls.user_id, \
             COALESCE(submitter.display_name, 'Removed user') AS submitted_by_name, \
             walls.title, walls.description, walls.status, walls.decision_note, \
             decider.display_name AS decided_by_name, walls.decided_at, \
             walls.mime_type, walls.byte_size, walls.width, walls.height, \
             walls.created_at, walls.updated_at \
             FROM walls \
             LEFT JOIN user_settings AS submitter \
                  ON submitter.user_id = walls.user_id \
             LEFT JOIN user_settings AS decider \
                  ON decider.user_id = walls.decided_by ",
            $tail
        )
    };
}

/// Lists walls the viewer may see, newest first.
///
/// Approved walls are visible to every authenticated account. Pending and rejected walls
/// are visible only to their submitter, and to administrators through `include_all`.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_walls(
    pool: &SqlitePool,
    viewer_id: &str,
    include_all: bool,
    status: &str,
    query: &str,
    tag: &str,
) -> Result<Vec<Wall>, sqlx::Error> {
    let search_pattern = format!("%{}%", query.to_lowercase());
    let records = sqlx::query_as::<_, WallRecord>(wall_select!(
        "WHERE (walls.status = 'approved' OR walls.user_id = ? OR ?) \
         AND (? = '' OR walls.status = ?) \
         AND (? = '' OR LOWER(walls.title) LIKE ? OR LOWER(walls.description) LIKE ?) \
         AND (? = '' OR EXISTS( \
             SELECT 1 FROM wall_tags filter_tags \
             WHERE filter_tags.wall_id = walls.id AND filter_tags.tag = ? COLLATE NOCASE \
         )) \
         ORDER BY walls.created_at DESC, walls.id DESC LIMIT 200"
    ))
    .bind(viewer_id)
    .bind(include_all)
    .bind(status)
    .bind(status)
    .bind(query)
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(tag)
    .bind(tag)
    .fetch_all(pool)
    .await?;

    hydrate_walls(pool, records).await
}

/// Lists only the walls the viewer submitted, newest first, at any status.
///
/// Takes the same search and tag filters as [`list_walls`] so the one page-level filter
/// bar behaves identically on every view.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_walls_by_submitter(
    pool: &SqlitePool,
    user_id: &str,
    query: &str,
    tag: &str,
) -> Result<Vec<Wall>, sqlx::Error> {
    let search_pattern = format!("%{}%", query.to_lowercase());
    let records = sqlx::query_as::<_, WallRecord>(wall_select!(
        "WHERE walls.user_id = ? \
         AND (? = '' OR LOWER(walls.title) LIKE ? OR LOWER(walls.description) LIKE ?) \
         AND (? = '' OR EXISTS( \
             SELECT 1 FROM wall_tags filter_tags \
             WHERE filter_tags.wall_id = walls.id AND filter_tags.tag = ? COLLATE NOCASE \
         )) \
         ORDER BY walls.created_at DESC, walls.id DESC LIMIT 200"
    ))
    .bind(user_id)
    .bind(query)
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(tag)
    .bind(tag)
    .fetch_all(pool)
    .await?;

    hydrate_walls(pool, records).await
}

/// Loads one wall's metadata regardless of status.
///
/// Callers are responsible for the visibility check; the handler layer owns that policy.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_wall(pool: &SqlitePool, wall_id: &str) -> Result<Option<Wall>, sqlx::Error> {
    let Some(record) = sqlx::query_as::<_, WallRecord>(wall_select!("WHERE walls.id = ?"))
        .bind(wall_id)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    let tags = load_wall_tags(pool, &record.id).await?;
    Ok(Some(record.into_wall(tags)))
}

async fn hydrate_walls(
    pool: &SqlitePool,
    records: Vec<WallRecord>,
) -> Result<Vec<Wall>, sqlx::Error> {
    let mut walls = Vec::with_capacity(records.len());
    for record in records {
        let tags = load_wall_tags(pool, &record.id).await?;
        walls.push(record.into_wall(tags));
    }
    Ok(walls)
}

async fn load_wall_tags(pool: &SqlitePool, wall_id: &str) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT tag FROM wall_tags WHERE wall_id = ? ORDER BY tag COLLATE NOCASE",
    )
    .bind(wall_id)
    .fetch_all(pool)
    .await
}

/// Stores one submission and its tags, leaving it pending review.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn create_wall(pool: &SqlitePool, draft: &WallDraft) -> Result<Wall, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let byte_size = i64::try_from(draft.image_data.len()).unwrap_or(i64::MAX);
    let mut transaction = pool.begin().await?;

    sqlx::query(
        "INSERT INTO walls (id, user_id, title, description, status, mime_type, byte_size, \
         width, height, image_data, thumbnail_mime, thumbnail_data, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, 'image/jpeg', ?, ?, ?)",
    )
    .bind(&id)
    .bind(&draft.user_id)
    .bind(&draft.title)
    .bind(&draft.description)
    .bind(&draft.mime_type)
    .bind(byte_size)
    .bind(draft.width)
    .bind(draft.height)
    .bind(&draft.image_data)
    .bind(&draft.thumbnail_data)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;

    for tag in &draft.tags {
        sqlx::query("INSERT OR IGNORE INTO wall_tags (wall_id, tag) VALUES (?, ?)")
            .bind(&id)
            .bind(tag)
            .execute(&mut *transaction)
            .await?;
    }

    transaction.commit().await?;
    get_wall(pool, &id).await?.ok_or(sqlx::Error::RowNotFound)
}

/// Replaces one wall's descriptive fields and tag set.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_wall_details(
    pool: &SqlitePool,
    wall_id: &str,
    title: &str,
    description: &str,
    tags: &[String],
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let affected =
        sqlx::query("UPDATE walls SET title = ?, description = ?, updated_at = ? WHERE id = ?")
            .bind(title)
            .bind(description)
            .bind(&now)
            .bind(wall_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();

    if affected == 0 {
        transaction.rollback().await?;
        return Ok(false);
    }

    sqlx::query("DELETE FROM wall_tags WHERE wall_id = ?")
        .bind(wall_id)
        .execute(&mut *transaction)
        .await?;
    for tag in tags {
        sqlx::query("INSERT OR IGNORE INTO wall_tags (wall_id, tag) VALUES (?, ?)")
            .bind(wall_id)
            .bind(tag)
            .execute(&mut *transaction)
            .await?;
    }

    transaction.commit().await?;
    Ok(true)
}

/// Records an administrator decision on a pending wall.
///
/// Returns `false` when the wall no longer exists or has already been decided.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn decide_wall(
    pool: &SqlitePool,
    wall_id: &str,
    status: &str,
    decided_by: &str,
    decision_note: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE walls SET status = ?, decision_note = ?, decided_by = ?, decided_at = ?, \
         updated_at = ? WHERE id = ? AND status = 'pending'",
    )
    .bind(status)
    .bind(decision_note)
    .bind(decided_by)
    .bind(&now)
    .bind(&now)
    .bind(wall_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Removes one wall. Selections pointing at it cascade away.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_wall(pool: &SqlitePool, wall_id: &str) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM walls WHERE id = ?")
        .bind(wall_id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

/// Loads one wall's full-size image bytes.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_wall_image(
    pool: &SqlitePool,
    wall_id: &str,
) -> Result<Option<WallImage>, sqlx::Error> {
    sqlx::query_as::<_, WallImage>(
        "SELECT mime_type, image_data, updated_at FROM walls WHERE id = ?",
    )
    .bind(wall_id)
    .fetch_optional(pool)
    .await
}

/// Loads one wall's thumbnail bytes.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_wall_thumbnail(
    pool: &SqlitePool,
    wall_id: &str,
) -> Result<Option<WallImage>, sqlx::Error> {
    sqlx::query_as::<_, WallImage>(
        "SELECT thumbnail_mime AS mime_type, thumbnail_data AS image_data, updated_at \
         FROM walls WHERE id = ?",
    )
    .bind(wall_id)
    .fetch_optional(pool)
    .await
}

/// Points one of the caller's wallpaper slots at an approved wall.
///
/// Clears any uploaded image in the same slot so the two sources never disagree.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the write cannot be completed.
pub async fn apply_wall_to_slot(
    pool: &SqlitePool,
    user_id: &str,
    slot: &str,
    wall_id: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;

    sqlx::query("DELETE FROM user_wallpapers WHERE user_id = ? AND slot = ?")
        .bind(user_id)
        .bind(slot)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO user_wallpaper_selections (user_id, slot, wall_id, updated_at) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT(user_id, slot) DO UPDATE SET wall_id = excluded.wall_id, \
         updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(slot)
    .bind(wall_id)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(())
}

/// Points the global login screen at an approved wall.
///
/// The login slot is a singleton: every other login image and login selection is removed
/// first, across all administrators, so the resolved image can never depend on a tiebreak.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the write cannot be completed.
pub async fn apply_wall_to_login(
    pool: &SqlitePool,
    user_id: &str,
    wall_id: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;

    sqlx::query("DELETE FROM user_wallpapers WHERE slot = 'login'")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM user_wallpaper_selections WHERE slot = 'login'")
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO user_wallpaper_selections (user_id, slot, wall_id, updated_at) \
         VALUES (?, 'login', ?, ?)",
    )
    .bind(user_id)
    .bind(wall_id)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(())
}

/// Reports which wall each of the caller's slots currently resolves to.
///
/// Only selections pointing at a still-approved wall are reported, matching what the
/// wallpaper endpoints actually serve.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_wall_selections(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<(String, String)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT s.slot, s.wall_id FROM user_wallpaper_selections s \
         JOIN walls ON walls.id = s.wall_id \
         WHERE s.user_id = ? AND walls.status = 'approved'",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| Ok((row.try_get("slot")?, row.try_get("wall_id")?)))
        .collect()
}

/// Reports the wall the global login screen currently resolves to, when there is one.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_login_wall_selection(pool: &SqlitePool) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT s.wall_id FROM user_wallpaper_selections s \
         JOIN walls ON walls.id = s.wall_id \
         JOIN users ON users.id = s.user_id \
         WHERE s.slot = 'login' AND walls.status = 'approved' \
           AND users.role = 'administrator' \
         ORDER BY s.updated_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
}
