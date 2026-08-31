use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// One catalogue entry as an administrator manages it.
///
/// Artwork bytes are deliberately absent: they are served from a dedicated endpoint so
/// listing the catalogue never drags image blobs through the API layer.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct Podcast {
    pub id: String,
    pub feed_url: String,
    pub normalized_url: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub site_url: String,
    pub language: String,
    pub artwork_url: String,
    pub has_artwork: bool,
    pub auto_download_count: i64,
    pub max_retained_episodes: i64,
    pub added_by: Option<String>,
    pub last_fetched_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A catalogue entry as a member sees it, carrying their own subscription state.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct PodcastSummary {
    pub id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub site_url: String,
    pub feed_url: String,
    pub artwork_url: String,
    pub has_artwork: bool,
    pub auto_download_count: i64,
    pub max_retained_episodes: i64,
    pub subscribed: bool,
    pub ntfy_notifications_enabled: bool,
    pub ntfy_topic_id: Option<String>,
    pub episode_count: i64,
    pub downloaded_count: i64,
    pub latest_published_at: Option<String>,
    pub last_fetched_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
}

/// One listener's outbound ntfy route for a subscribed podcast.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct PodcastNotificationSettings {
    pub enabled: bool,
    pub topic_id: Option<String>,
    pub topic: Option<String>,
    pub topic_label: Option<String>,
}

/// One leased outbound notification, resolved against the listener's current ntfy route.
///
/// The encrypted token never crosses the server boundary; it is decrypted only by the guarded
/// integration client immediately before publishing.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct PodcastNotificationJob {
    pub user_id: String,
    pub episode_id: String,
    pub podcast_title: String,
    pub episode_title: String,
    pub episode_url: String,
    pub base_url: String,
    pub token_ciphertext: Option<String>,
    pub topic: String,
    pub attempts: i64,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct PodcastArtwork {
    pub content_type: String,
    pub data: Vec<u8>,
}

/// Channel-level metadata resolved from a feed, used both by the request preview and by
/// the refresh worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastFeedPreview {
    pub title: String,
    pub description: String,
    pub author: String,
    pub site_url: String,
    pub language: String,
    pub artwork_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastArtworkDraft {
    pub source_url: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct PodcastRequest {
    pub id: String,
    pub user_id: String,
    pub requester_name: String,
    pub feed_url: String,
    pub resolved_title: String,
    pub resolved_author: String,
    pub resolved_artwork_url: String,
    pub note: String,
    pub status: String,
    pub decision_note: String,
    pub decided_by_name: Option<String>,
    pub decided_at: Option<String>,
    pub podcast_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A new request as the server stores it after resolving the feed preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastRequestDraft {
    pub user_id: String,
    pub feed_url: String,
    pub normalized_url: String,
    pub resolved_title: String,
    pub resolved_author: String,
    pub resolved_artwork_url: String,
    pub note: String,
}

/// An indexed feed item joined with the caller's own listening state.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct PodcastEpisode {
    pub id: String,
    pub podcast_id: String,
    pub podcast_title: String,
    pub title: String,
    pub description: String,
    pub episode_url: String,
    pub enclosure_type: String,
    pub enclosure_bytes: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub published_at: String,
    pub download_status: Option<String>,
    pub download_progress: f64,
    pub position_seconds: i64,
    pub completed_at: Option<String>,
    pub saved_at: Option<String>,
    pub queue_position: Option<i64>,
}

/// One feed item as parsed, before it is matched against what is already indexed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastEpisodeDraft {
    pub guid: String,
    pub title: String,
    pub description: String,
    pub episode_url: String,
    pub enclosure_url: String,
    pub enclosure_type: String,
    pub enclosure_bytes: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub published_at: String,
}

/// A leased unit of work for the download worker.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct PodcastDownloadJob {
    pub episode_id: String,
    pub podcast_id: String,
    pub enclosure_url: String,
    pub enclosure_type: String,
    pub attempts: i64,
}

/// A cached file as the playback route and the reconciler see it.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct PodcastCachedFile {
    pub episode_id: String,
    pub file_name: String,
    pub content_type: String,
    pub byte_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct PodcastSettings {
    pub requests_enabled: bool,
    pub member_downloads_enabled: bool,
    pub max_pending_requests_per_user: i64,
    pub storage_budget_bytes: i64,
    pub max_episode_bytes: i64,
    pub default_auto_download_count: i64,
    pub updated_at: String,
}

/// Catalogue rows whose shared refresh window and lease have both elapsed.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct PodcastRefreshTarget {
    pub id: String,
    pub feed_url: String,
}

/// A new catalogue entry, assembled from a resolved feed preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastDraft {
    pub feed_url: String,
    pub normalized_url: String,
    pub preview: PodcastFeedPreview,
    pub added_by: String,
    pub auto_download_count: i64,
}
