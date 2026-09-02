use crate::entities::OllamaSettings;
use sqlx::SqlitePool;

/// Loads the instance-wide Ollama settings row.
///
/// # Errors
///
/// Returns a database error when the singleton cannot be read.
pub async fn get_ollama_settings(pool: &SqlitePool) -> Result<OllamaSettings, sqlx::Error> {
    sqlx::query_as::<_, OllamaSettings>(
        "SELECT id, enabled, base_url, model, prompt, tag_count, \
                configured_by_user_id, last_verified_at, created_at, updated_at \
         FROM ollama_settings WHERE id = 1",
    )
    .fetch_one(pool)
    .await
}

/// Replaces the editable Ollama settings and records the administrator responsible.
///
/// # Errors
///
/// Returns a database error when the singleton cannot be updated or reloaded.
#[allow(clippy::too_many_arguments)]
pub async fn update_ollama_settings(
    pool: &SqlitePool,
    enabled: bool,
    base_url: &str,
    model: &str,
    prompt: &str,
    tag_count: i64,
    configured_by_user_id: &str,
    last_verified_at: Option<&str>,
) -> Result<OllamaSettings, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE ollama_settings SET enabled = ?, base_url = ?, model = ?, prompt = ?, \
             tag_count = ?, configured_by_user_id = ?, last_verified_at = ?, updated_at = ? \
         WHERE id = 1",
    )
    .bind(enabled)
    .bind(base_url)
    .bind(model)
    .bind(prompt)
    .bind(tag_count)
    .bind(configured_by_user_id)
    .bind(last_verified_at)
    .bind(&now)
    .execute(pool)
    .await?;
    get_ollama_settings(pool).await
}
