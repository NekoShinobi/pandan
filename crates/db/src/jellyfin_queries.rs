use crate::entities::{JellyfinServerSettings, JellyfinUserConnection};
use sqlx::SqlitePool;

/// Loads the administrator-selected Jellyfin server, when configured.
///
/// # Errors
///
/// Returns a database error when the settings row cannot be read.
pub async fn get_jellyfin_server_settings(
    pool: &SqlitePool,
) -> Result<Option<JellyfinServerSettings>, sqlx::Error> {
    sqlx::query_as::<_, JellyfinServerSettings>(
        "SELECT id, base_url, server_id, server_name, server_version, \
                configured_by_user_id, created_at, updated_at \
         FROM jellyfin_server_settings WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
}

/// Atomically replaces the singleton server and invalidates every old account connection.
///
/// # Errors
///
/// Returns a database error when the replacement transaction cannot complete.
#[allow(clippy::too_many_arguments)]
pub async fn replace_jellyfin_server_settings(
    pool: &SqlitePool,
    base_url: &str,
    server_id: &str,
    server_name: &str,
    server_version: &str,
    configured_by_user_id: &str,
) -> Result<JellyfinServerSettings, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    sqlx::query("DELETE FROM jellyfin_server_settings WHERE id = 1")
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO jellyfin_server_settings (\
             id, base_url, server_id, server_name, server_version, \
             configured_by_user_id, created_at, updated_at\
         ) VALUES (1, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(base_url)
    .bind(server_id)
    .bind(server_name)
    .bind(server_version)
    .bind(configured_by_user_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    let settings = sqlx::query_as::<_, JellyfinServerSettings>(
        "SELECT id, base_url, server_id, server_name, server_version, \
                configured_by_user_id, created_at, updated_at \
         FROM jellyfin_server_settings WHERE id = 1",
    )
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(settings)
}

/// Deletes the singleton server and cascades every linked account.
///
/// # Errors
///
/// Returns a database error when the singleton cannot be deleted.
pub async fn delete_jellyfin_server_settings(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM jellyfin_server_settings WHERE id = 1")
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Loads one account-owned Jellyfin connection, including its encrypted token.
///
/// # Errors
///
/// Returns a database error when the account connection cannot be read.
pub async fn get_jellyfin_user_connection(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Option<JellyfinUserConnection>, sqlx::Error> {
    sqlx::query_as::<_, JellyfinUserConnection>(
        "SELECT user_id, server_setting_id, jellyfin_user_id, jellyfin_username, \
                token_ciphertext, device_id, last_verified_at, last_error, \
                created_at, updated_at \
         FROM jellyfin_user_connections WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Stores the Jellyfin identity returned by authentication for one Pandan account.
///
/// # Errors
///
/// Returns a database error when the account connection cannot be stored or reloaded.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_jellyfin_user_connection(
    pool: &SqlitePool,
    user_id: &str,
    jellyfin_user_id: &str,
    jellyfin_username: &str,
    token_ciphertext: &str,
    device_id: &str,
) -> Result<JellyfinUserConnection, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO jellyfin_user_connections (\
             user_id, server_setting_id, jellyfin_user_id, jellyfin_username, \
             token_ciphertext, device_id, last_verified_at, last_error, created_at, updated_at\
         ) VALUES (?, 1, ?, ?, ?, ?, ?, NULL, ?, ?) \
         ON CONFLICT(user_id) DO UPDATE SET \
             server_setting_id = 1, jellyfin_user_id = excluded.jellyfin_user_id, \
             jellyfin_username = excluded.jellyfin_username, \
             token_ciphertext = excluded.token_ciphertext, device_id = excluded.device_id, \
             last_verified_at = excluded.last_verified_at, last_error = NULL, \
             updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(jellyfin_user_id)
    .bind(jellyfin_username)
    .bind(token_ciphertext)
    .bind(device_id)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_jellyfin_user_connection(pool, user_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Records a successful identity verification and clears any stale error.
///
/// # Errors
///
/// Returns a database error when the connection health cannot be updated.
pub async fn mark_jellyfin_connection_verified(
    pool: &SqlitePool,
    user_id: &str,
    username: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE jellyfin_user_connections \
         SET jellyfin_username = ?, last_verified_at = ?, last_error = NULL, updated_at = ? \
         WHERE user_id = ?",
    )
    .bind(username)
    .bind(&now)
    .bind(&now)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Stores a bounded, user-safe connection error without deleting the credential.
///
/// # Errors
///
/// Returns a database error when the connection health cannot be updated.
pub async fn set_jellyfin_connection_error(
    pool: &SqlitePool,
    user_id: &str,
    error: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE jellyfin_user_connections SET last_error = ?, updated_at = ? WHERE user_id = ?",
    )
    .bind(error)
    .bind(now)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Removes one account's Jellyfin token.
///
/// # Errors
///
/// Returns a database error when the account connection cannot be deleted.
pub async fn delete_jellyfin_user_connection(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM jellyfin_user_connections WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}
