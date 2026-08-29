use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Announcement {
    pub id: String,
    pub author_id: Option<String>,
    pub author_name: String,
    pub title: String,
    pub content: String,
    pub images: Vec<AnnouncementImage>,
    pub reactions: Vec<AnnouncementReaction>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct AnnouncementImage {
    pub id: String,
    pub file_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct AnnouncementReaction {
    pub emoji: String,
    pub count: i64,
    pub reacted_by_viewer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementDraft {
    pub title: String,
    pub content: String,
}
