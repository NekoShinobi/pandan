use serde::{Deserialize, Serialize};
use sqlx::FromRow;

mod jellyfin;
mod kanban;
mod podcasts;
mod walls;
mod youtube_downloads;
pub use jellyfin::*;
pub use kanban::*;
pub use podcasts::*;
pub use walls::*;
pub use youtube_downloads::*;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct AppMetadata {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub completed: bool,
    pub priority: String,
    pub due_date: Option<String>,
    pub repeat_rule: String,
    pub repeat_interval: i64,
    pub repeat_unit: String,
    pub reschedule_from: String,
    pub completed_at: Option<String>,
    pub labels: Vec<String>,
    pub subtasks: Vec<TaskSubtask>,
    pub attachments: Vec<TaskAttachment>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct TaskSubtask {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct TaskAttachment {
    pub id: String,
    pub file_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDraft {
    pub title: String,
    pub description: String,
    pub priority: String,
    pub due_date: Option<String>,
    pub repeat_rule: String,
    pub repeat_interval: i64,
    pub repeat_unit: String,
    pub reschedule_from: String,
    pub labels: Vec<String>,
    pub subtasks: Vec<TaskSubtaskDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSubtaskDraft {
    pub id: Option<String>,
    pub title: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinePost {
    pub id: String,
    pub user_id: String,
    pub author_name: String,
    pub content: String,
    pub visibility: String,
    pub reply_to_post_id: Option<String>,
    pub reply_to_author_name: Option<String>,
    pub reply_to_content: Option<String>,
    pub tags: Vec<String>,
    pub attachments: Vec<LinePostAttachment>,
    pub reactions: Vec<LinePostReaction>,
    pub reply_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct LineAuthorProfile {
    pub user_id: String,
    pub display_name: String,
    pub post_count: i64,
    pub first_post_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineAuthorFeed {
    pub author: LineAuthorProfile,
    pub posts: Vec<LinePost>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineThread {
    pub parent: Option<LinePost>,
    pub post: LinePost,
    pub replies: Vec<LinePost>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct LinePostAttachment {
    pub id: String,
    pub file_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct LinePostReaction {
    pub emoji: String,
    pub count: i64,
    pub reacted_by_viewer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinePostDraft {
    pub content: String,
    pub visibility: String,
    pub reply_to_post_id: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct ManagedUser {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub created_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct AuthenticationSettings {
    pub password_login_enabled: bool,
    pub password_registration_enabled: bool,
    pub oidc_registration_enabled: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct LoggingSettings {
    pub file_enabled: bool,
    pub log_level: String,
    pub retention_days: i64,
    pub max_file_size_mb: i64,
    pub max_files: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct LoginAppearance {
    pub background_blur: i64,
    pub background_brightness: i64,
    pub background_contrast: i64,
    pub background_saturation: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct NetworkAccessRule {
    pub id: String,
    pub action: String,
    pub scheme: String,
    pub host: String,
    pub port: i64,
    pub integration: String,
    pub created_by_user_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct UserCredentials {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserSettings {
    pub user_id: String,
    pub display_name: String,
    pub location: String,
    pub timezone: String,
    pub sidebar_timezones: Vec<String>,
    pub calendar_week_start: String,
    pub temperature_unit: String,
    pub lines_default_visibility: String,
    pub podcast_playback_rate: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct UserBackground {
    pub mime_type: String,
    pub image_data: Vec<u8>,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct UserAvatar {
    pub mime_type: String,
    pub image_data: Vec<u8>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct UserAppearance {
    pub user_id: String,
    pub has_dashboard_wallpaper: bool,
    pub has_welcome_wallpaper: bool,
    pub has_loading_wallpaper: bool,
    pub has_login_wallpaper: bool,
    pub background_blur: i64,
    pub background_brightness: i64,
    pub background_contrast: i64,
    pub background_saturation: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct Workspace {
    pub workspace: i64,
    pub name: String,
    pub position: i64,
    pub has_custom_background: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq)]
pub struct SessionAccount {
    pub id: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
    pub display_name: String,
    pub location: String,
    pub timezone: String,
    pub sidebar_timezones_json: String,
    pub calendar_week_start: String,
    pub temperature_unit: String,
    pub lines_default_visibility: String,
    pub podcast_playback_rate: f64,
    pub settings_updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct AccountSession {
    pub id: String,
    pub token: String,
    pub user_agent: String,
    pub ip_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct EmbeddedPage {
    pub id: String,
    pub scope: String,
    pub owner_user_id: Option<String>,
    pub created_by_user_id: Option<String>,
    pub title: String,
    pub description: String,
    pub url: String,
    pub icon_url: Option<String>,
    pub allow_scripts: bool,
    pub allow_same_origin: bool,
    pub iframe_height: i64,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct OidcAuthorization {
    pub state: String,
    pub pkce_verifier: String,
    pub nonce: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct FeedItem {
    pub id: String,
    pub category: String,
    pub source: String,
    pub title: String,
    pub summary: String,
    pub reading_minutes: i64,
    pub published_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct RssSubscription {
    pub id: String,
    pub url: String,
    pub base_url: String,
    pub title: String,
    pub category: String,
    pub auto_delete_days: Option<i64>,
    pub auto_delete_mode: String,
    pub last_fetched_at: Option<String>,
    pub last_error: Option<String>,
    pub refresh_generation: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct RssItem {
    pub id: String,
    pub subscription_id: String,
    pub source: String,
    pub category: String,
    pub base_url: String,
    pub url: String,
    pub comments_url: String,
    pub title: String,
    pub summary: String,
    pub published_at: String,
    pub fetched_at: String,
    pub read_at: Option<String>,
    pub saved_at: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RssSubscriptionDraft {
    pub url: String,
    pub base_url: String,
    pub title: String,
    pub category: String,
    pub auto_delete_days: Option<i64>,
    pub auto_delete_mode: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct RssRefreshTarget {
    pub id: String,
    pub user_id: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RssItemDraft {
    pub external_id: String,
    pub url: String,
    pub comments_url: String,
    pub title: String,
    pub summary: String,
    pub published_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct YoutubeChannel {
    pub channel_id: String,
    pub title: String,
    pub channel_url: String,
    pub thumbnail_url: String,
    pub thumbnail_fetched_at: Option<String>,
    pub last_fetched_at: Option<String>,
    pub refresh_started_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct YoutubeSubscription {
    pub channel_id: String,
    pub title: String,
    pub channel_url: String,
    pub thumbnail_url: String,
    pub last_fetched_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct YoutubeChannelThumbnail {
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoutubeChannelThumbnailDraft {
    pub source_url: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct YoutubeVideo {
    pub id: String,
    pub channel_id: String,
    pub channel_title: String,
    pub url: String,
    pub thumbnail_url: String,
    pub title: String,
    pub published_at: String,
    pub fetched_at: String,
    pub watch_later_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct YoutubeGroupRecord {
    pub id: String,
    pub name: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct YoutubeGroupChannel {
    pub group_id: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoutubeVideoDraft {
    pub external_id: String,
    pub url: String,
    pub thumbnail_url: String,
    pub title: String,
    pub published_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct JournalNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub content: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct CalendarSubscription {
    pub id: String,
    pub url: String,
    pub name: String,
    pub color: String,
    pub last_fetched_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct CalendarEvent {
    pub id: String,
    pub subscription_id: String,
    pub calendar_name: String,
    pub calendar_color: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub url: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub all_day: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEventDraft {
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub url: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub all_day: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct PaymentSubscription {
    pub id: String,
    pub service: String,
    pub description: String,
    pub frequency: String,
    pub amount_micros: i64,
    pub currency: String,
    pub first_paid_on: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContactMethod {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContactAddress {
    pub label: String,
    pub street: String,
    pub city: String,
    pub region: String,
    pub postal_code: String,
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContactImportantDate {
    pub label: String,
    pub date: String,
    pub recurring: bool,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ContactPhoto {
    pub mime_type: String,
    pub image_data: Vec<u8>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactPhotoDraft {
    pub mime_type: String,
    pub image_data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Contact {
    pub id: String,
    pub dav_source_id: Option<String>,
    pub source_kind: String,
    pub source_reference: Option<String>,
    pub first_name: String,
    pub middle_name: String,
    pub last_name: String,
    pub nickname: String,
    pub pronouns: String,
    pub company: String,
    pub job_title: String,
    pub birthday: Option<String>,
    pub emails: Vec<ContactMethod>,
    pub phones: Vec<ContactMethod>,
    pub addresses: Vec<ContactAddress>,
    pub important_dates: Vec<ContactImportantDate>,
    pub tags: Vec<String>,
    pub relationship_context: String,
    pub notes: String,
    pub favorite: bool,
    pub archived: bool,
    #[serde(default)]
    pub has_photo: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContactDraft {
    pub dav_source_id: Option<String>,
    pub source_kind: String,
    pub source_reference: Option<String>,
    pub first_name: String,
    pub middle_name: String,
    pub last_name: String,
    pub nickname: String,
    pub pronouns: String,
    pub company: String,
    pub job_title: String,
    pub birthday: Option<String>,
    pub emails: Vec<ContactMethod>,
    pub phones: Vec<ContactMethod>,
    pub addresses: Vec<ContactAddress>,
    pub important_dates: Vec<ContactImportantDate>,
    pub tags: Vec<String>,
    pub relationship_context: String,
    pub notes: String,
    pub favorite: bool,
    pub archived: bool,
    #[serde(skip)]
    pub photo: Option<ContactPhotoDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct ContactDavSource {
    pub id: String,
    pub name: String,
    pub url: String,
    pub username: String,
    pub has_password: bool,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct CodingProject {
    pub id: String,
    pub provider: String,
    pub host: String,
    pub repository: String,
    pub has_credential: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct CodingCredential {
    pub provider: String,
    pub host: String,
    pub ciphertext: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardWidget {
    pub id: String,
    pub kind: String,
    pub workspace: i64,
    pub position: i64,
    pub size: String,
    pub grid_x: i64,
    pub grid_y: i64,
    pub grid_w: i64,
    pub grid_h: i64,
    pub config: serde_json::Value,
    pub has_secret: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct Bookmark {
    pub id: String,
    pub title: String,
    pub url: String,
    pub has_favicon: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct BookmarkFavicon {
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct BookmarkLibraryCategory {
    pub id: String,
    pub scope: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct BookmarkLibraryItem {
    pub id: String,
    pub category_id: String,
    pub title: String,
    pub url: String,
    pub icon_kind: String,
    pub icon_value: Option<String>,
    pub has_icon: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct BookmarkLibraryIcon {
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct NtfyConnection {
    pub user_id: String,
    pub base_url: String,
    pub token_ciphertext: Option<String>,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct NtfyTopic {
    pub id: String,
    pub topic: String,
    pub label: String,
    pub last_message_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct NtfyNotification {
    pub id: String,
    pub topic_id: String,
    pub topic: String,
    pub topic_label: String,
    pub remote_id: String,
    pub occurred_at: i64,
    pub title: String,
    pub message: String,
    pub priority: i64,
    pub tags_json: String,
    pub click_url: Option<String>,
    pub actions_json: String,
    pub seen_at: Option<String>,
    pub archived_at: Option<String>,
    pub received_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtfyNotificationDraft {
    pub remote_id: String,
    pub occurred_at: i64,
    pub title: String,
    pub message: String,
    pub priority: i64,
    pub tags_json: String,
    pub click_url: Option<String>,
    pub actions_json: String,
}
