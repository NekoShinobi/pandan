use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// One submission in the shared wallpaper collection.
///
/// Image and thumbnail bytes are deliberately absent: both are served from dedicated
/// endpoints so listing the collection never drags blobs through the API layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Wall {
    pub id: String,
    pub user_id: Option<String>,
    pub submitted_by_name: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub decision_note: String,
    pub decided_by_name: Option<String>,
    pub decided_at: Option<String>,
    pub mime_type: String,
    pub byte_size: i64,
    pub width: i64,
    pub height: i64,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A new submission as the server stores it after decoding the upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallDraft {
    pub user_id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub mime_type: String,
    pub width: i64,
    pub height: i64,
    pub image_data: Vec<u8>,
    pub thumbnail_data: Vec<u8>,
}

/// Stored image bytes for one wall, in either full or thumbnail size.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct WallImage {
    pub mime_type: String,
    pub image_data: Vec<u8>,
    pub updated_at: String,
}
