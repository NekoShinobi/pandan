use serde::Serialize;
use sqlx::FromRow;

/// The singleton Ollama integration configured by an instance administrator.
#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct OllamaSettings {
    pub id: i64,
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub prompt: String,
    pub tag_count: i64,
    pub configured_by_user_id: Option<String>,
    pub last_verified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
