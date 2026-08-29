use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct YoutubeDownloadJob {
    pub id: String,
    pub user_id: String,
    pub source_url: String,
    pub youtube_video_id: String,
    pub title: String,
    pub channel_name: String,
    pub duration_seconds: Option<i64>,
    pub media_kind: String,
    pub output_format: String,
    pub max_height: Option<i64>,
    pub status: String,
    pub progress_percent: Option<f64>,
    pub downloaded_bytes: i64,
    pub total_bytes: Option<i64>,
    pub speed_bytes_per_second: Option<f64>,
    pub eta_seconds: Option<i64>,
    pub storage_file_name: String,
    pub display_file_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub attempts: i64,
    pub error_code: Option<String>,
    pub last_error: Option<String>,
    pub lease_started_at: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewYoutubeDownloadJob {
    pub id: String,
    pub user_id: String,
    pub source_url: String,
    pub youtube_video_id: String,
    pub title: String,
    pub channel_name: String,
    pub duration_seconds: Option<i64>,
    pub media_kind: String,
    pub output_format: String,
    pub max_height: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct YoutubeDownloadSettings {
    pub member_downloads_enabled: bool,
    pub storage_budget_bytes: i64,
    pub per_user_budget_bytes: i64,
    pub max_output_bytes: i64,
    pub global_concurrency: i64,
    pub per_user_concurrency: i64,
    pub max_batch_urls: i64,
    pub max_queued_per_user: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct YoutubeDownloadFileRef {
    pub id: String,
    pub user_id: String,
    pub storage_file_name: String,
}
