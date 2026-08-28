use sqlx::FromRow;

/// The singleton Jellyfin server selected by an instance administrator.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct JellyfinServerSettings {
    pub id: i64,
    pub base_url: String,
    pub server_id: String,
    pub server_name: String,
    pub server_version: String,
    pub configured_by_user_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// One Pandan account's private Jellyfin identity and encrypted access token.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct JellyfinUserConnection {
    pub user_id: String,
    pub server_setting_id: i64,
    pub jellyfin_user_id: String,
    pub jellyfin_username: String,
    pub token_ciphertext: String,
    pub device_id: String,
    pub last_verified_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
