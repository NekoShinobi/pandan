use crate::entities::{BookmarkLibraryCategory, BookmarkLibraryIcon, BookmarkLibraryItem};
use sqlx::SqlitePool;

/// Lists instance-wide bookmark categories.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_global_categories(
    pool: &SqlitePool,
) -> Result<Vec<BookmarkLibraryCategory>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, scope, name, created_at, updated_at \
         FROM bookmark_library_categories \
         WHERE scope = 'global' \
         ORDER BY name COLLATE NOCASE ASC, created_at ASC, id ASC",
    )
    .fetch_all(pool)
    .await
}

/// Lists bookmark categories owned by one account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_personal_categories(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<BookmarkLibraryCategory>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, scope, name, created_at, updated_at \
         FROM bookmark_library_categories \
         WHERE scope = 'personal' AND user_id = ? \
         ORDER BY name COLLATE NOCASE ASC, created_at ASC, id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Lists bookmark items in instance-wide categories.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_global_items(pool: &SqlitePool) -> Result<Vec<BookmarkLibraryItem>, sqlx::Error> {
    sqlx::query_as(
        "SELECT i.id, i.category_id, i.title, i.url, i.icon_kind, i.icon_value, \
                i.icon_data IS NOT NULL AS has_icon, i.created_at, i.updated_at \
         FROM bookmark_library_items i \
         INNER JOIN bookmark_library_categories c ON c.id = i.category_id \
         WHERE c.scope = 'global' \
         ORDER BY i.title COLLATE NOCASE ASC, i.created_at ASC, i.id ASC",
    )
    .fetch_all(pool)
    .await
}

/// Lists bookmark items in categories owned by one account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_personal_items(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<BookmarkLibraryItem>, sqlx::Error> {
    sqlx::query_as(
        "SELECT i.id, i.category_id, i.title, i.url, i.icon_kind, i.icon_value, \
                i.icon_data IS NOT NULL AS has_icon, i.created_at, i.updated_at \
         FROM bookmark_library_items i \
         INNER JOIN bookmark_library_categories c ON c.id = i.category_id \
         WHERE c.scope = 'personal' AND c.user_id = ? \
         ORDER BY i.title COLLATE NOCASE ASC, i.created_at ASC, i.id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Returns whether a category belongs to the requested scope and owner.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn category_is_accessible(
    pool: &SqlitePool,
    category_id: &str,
    scope: &str,
    user_id: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let found = if scope == "global" {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM bookmark_library_categories \
             WHERE id = ? AND scope = 'global'",
        )
        .bind(category_id)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM bookmark_library_categories \
             WHERE id = ? AND scope = 'personal' AND user_id = ?",
        )
        .bind(category_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?
    };
    Ok(found == 1)
}

/// Creates one personal bookmark category.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn create_personal_category(
    pool: &SqlitePool,
    user_id: &str,
    name: &str,
) -> Result<BookmarkLibraryCategory, sqlx::Error> {
    create_category(pool, "personal", Some(user_id), Some(user_id), name).await
}

/// Creates one instance-wide bookmark category.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn create_global_category(
    pool: &SqlitePool,
    administrator_id: &str,
    name: &str,
) -> Result<BookmarkLibraryCategory, sqlx::Error> {
    create_category(pool, "global", None, Some(administrator_id), name).await
}

async fn create_category(
    pool: &SqlitePool,
    scope: &str,
    user_id: Option<&str>,
    created_by_user_id: Option<&str>,
    name: &str,
) -> Result<BookmarkLibraryCategory, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO bookmark_library_categories \
         (id, scope, user_id, created_by_user_id, name, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(scope)
    .bind(user_id)
    .bind(created_by_user_id)
    .bind(name)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    find_category(pool, &id).await
}

/// Updates one personal category owned by the account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_personal_category(
    pool: &SqlitePool,
    user_id: &str,
    category_id: &str,
    name: &str,
) -> Result<Option<BookmarkLibraryCategory>, sqlx::Error> {
    update_category(pool, category_id, name, "personal", Some(user_id)).await
}

/// Updates one instance-wide category.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_global_category(
    pool: &SqlitePool,
    category_id: &str,
    name: &str,
) -> Result<Option<BookmarkLibraryCategory>, sqlx::Error> {
    update_category(pool, category_id, name, "global", None).await
}

async fn update_category(
    pool: &SqlitePool,
    category_id: &str,
    name: &str,
    scope: &str,
    user_id: Option<&str>,
) -> Result<Option<BookmarkLibraryCategory>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = if scope == "global" {
        sqlx::query(
            "UPDATE bookmark_library_categories SET name = ?, updated_at = ? \
             WHERE id = ? AND scope = 'global'",
        )
        .bind(name)
        .bind(&now)
        .bind(category_id)
        .execute(pool)
        .await?
    } else {
        sqlx::query(
            "UPDATE bookmark_library_categories SET name = ?, updated_at = ? \
             WHERE id = ? AND scope = 'personal' AND user_id = ?",
        )
        .bind(name)
        .bind(&now)
        .bind(category_id)
        .bind(user_id)
        .execute(pool)
        .await?
    };
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    find_category(pool, category_id).await.map(Some)
}

async fn find_category(
    pool: &SqlitePool,
    category_id: &str,
) -> Result<BookmarkLibraryCategory, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, scope, name, created_at, updated_at \
         FROM bookmark_library_categories WHERE id = ?",
    )
    .bind(category_id)
    .fetch_one(pool)
    .await
}

/// Deletes one category in the requested scope, cascading its items.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_category(
    pool: &SqlitePool,
    category_id: &str,
    scope: &str,
    user_id: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let result = if scope == "global" {
        sqlx::query(
            "DELETE FROM bookmark_library_categories \
             WHERE id = ? AND scope = 'global'",
        )
        .bind(category_id)
        .execute(pool)
        .await?
    } else {
        sqlx::query(
            "DELETE FROM bookmark_library_categories \
             WHERE id = ? AND scope = 'personal' AND user_id = ?",
        )
        .bind(category_id)
        .bind(user_id)
        .execute(pool)
        .await?
    };
    Ok(result.rows_affected() == 1)
}

/// Creates one bookmark item after the caller has authorized its category.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn create_item(
    pool: &SqlitePool,
    category_id: &str,
    title: &str,
    url: &str,
    icon_kind: &str,
    icon_value: Option<&str>,
    icon: Option<(&str, &[u8])>,
) -> Result<BookmarkLibraryItem, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let (icon_content_type, icon_data, icon_fetched_at) = icon
        .map_or((None, None, None), |(content_type, data)| {
            (Some(content_type), Some(data), Some(now.as_str()))
        });
    sqlx::query(
        "INSERT INTO bookmark_library_items \
         (id, category_id, title, url, icon_kind, icon_value, icon_content_type, \
          icon_data, icon_fetched_at, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(category_id)
    .bind(title)
    .bind(url)
    .bind(icon_kind)
    .bind(icon_value)
    .bind(icon_content_type)
    .bind(icon_data)
    .bind(icon_fetched_at)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    find_item(pool, &id).await
}

/// Updates one authorized bookmark item and may move it to another category in the same scope.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
#[allow(clippy::too_many_arguments)]
pub async fn update_item(
    pool: &SqlitePool,
    item_id: &str,
    category_id: &str,
    title: &str,
    url: &str,
    icon_kind: &str,
    icon_value: Option<&str>,
    icon: Option<(&str, &[u8])>,
    scope: &str,
    user_id: Option<&str>,
) -> Result<Option<BookmarkLibraryItem>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let (icon_content_type, icon_data, icon_fetched_at) = icon
        .map_or((None, None, None), |(content_type, data)| {
            (Some(content_type), Some(data), Some(now.as_str()))
        });
    let result = if scope == "global" {
        sqlx::query(
            "UPDATE bookmark_library_items \
             SET category_id = ?, title = ?, url = ?, icon_kind = ?, icon_value = ?, \
                 icon_content_type = ?, icon_data = ?, icon_fetched_at = ?, updated_at = ? \
             WHERE id = ? \
               AND EXISTS (SELECT 1 FROM bookmark_library_categories current \
                           WHERE current.id = bookmark_library_items.category_id \
                             AND current.scope = 'global') \
               AND EXISTS (SELECT 1 FROM bookmark_library_categories target \
                           WHERE target.id = ? AND target.scope = 'global')",
        )
        .bind(category_id)
        .bind(title)
        .bind(url)
        .bind(icon_kind)
        .bind(icon_value)
        .bind(icon_content_type)
        .bind(icon_data)
        .bind(icon_fetched_at)
        .bind(&now)
        .bind(item_id)
        .bind(category_id)
        .execute(pool)
        .await?
    } else {
        sqlx::query(
            "UPDATE bookmark_library_items \
             SET category_id = ?, title = ?, url = ?, icon_kind = ?, icon_value = ?, \
                 icon_content_type = ?, icon_data = ?, icon_fetched_at = ?, updated_at = ? \
             WHERE id = ? \
               AND EXISTS (SELECT 1 FROM bookmark_library_categories current \
                           WHERE current.id = bookmark_library_items.category_id \
                             AND current.scope = 'personal' AND current.user_id = ?) \
               AND EXISTS (SELECT 1 FROM bookmark_library_categories target \
                           WHERE target.id = ? AND target.scope = 'personal' \
                             AND target.user_id = ?)",
        )
        .bind(category_id)
        .bind(title)
        .bind(url)
        .bind(icon_kind)
        .bind(icon_value)
        .bind(icon_content_type)
        .bind(icon_data)
        .bind(icon_fetched_at)
        .bind(&now)
        .bind(item_id)
        .bind(user_id)
        .bind(category_id)
        .bind(user_id)
        .execute(pool)
        .await?
    };
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    find_item(pool, item_id).await.map(Some)
}

async fn find_item(pool: &SqlitePool, item_id: &str) -> Result<BookmarkLibraryItem, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, category_id, title, url, icon_kind, icon_value, \
                icon_data IS NOT NULL AS has_icon, created_at, updated_at \
         FROM bookmark_library_items WHERE id = ?",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
}

/// Deletes one bookmark item in the requested scope.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_item(
    pool: &SqlitePool,
    item_id: &str,
    scope: &str,
    user_id: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let result = if scope == "global" {
        sqlx::query(
            "DELETE FROM bookmark_library_items \
             WHERE id = ? AND EXISTS ( \
                 SELECT 1 FROM bookmark_library_categories c \
                 WHERE c.id = bookmark_library_items.category_id AND c.scope = 'global' \
             )",
        )
        .bind(item_id)
        .execute(pool)
        .await?
    } else {
        sqlx::query(
            "DELETE FROM bookmark_library_items \
             WHERE id = ? AND EXISTS ( \
                 SELECT 1 FROM bookmark_library_categories c \
                 WHERE c.id = bookmark_library_items.category_id \
                   AND c.scope = 'personal' AND c.user_id = ? \
             )",
        )
        .bind(item_id)
        .bind(user_id)
        .execute(pool)
        .await?
    };
    Ok(result.rows_affected() == 1)
}

/// Loads stored icon bytes only when the item is global or belongs to the account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_visible_icon(
    pool: &SqlitePool,
    user_id: &str,
    item_id: &str,
) -> Result<Option<BookmarkLibraryIcon>, sqlx::Error> {
    sqlx::query_as(
        "SELECT i.icon_content_type AS content_type, i.icon_data AS data \
         FROM bookmark_library_items i \
         INNER JOIN bookmark_library_categories c ON c.id = i.category_id \
         WHERE i.id = ? AND i.icon_data IS NOT NULL \
           AND (c.scope = 'global' OR (c.scope = 'personal' AND c.user_id = ?))",
    )
    .bind(item_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}
