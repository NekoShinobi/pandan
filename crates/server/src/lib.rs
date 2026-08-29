use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError,
    cookie::{Cookie, SameSite, time::Duration as CookieDuration},
    http::{StatusCode, header},
    web,
};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use chrono::{Datelike, Duration as ChronoDuration, Utc};
use db::entities::{
    AuthenticationSettings, Bookmark, CalendarEvent, CalendarSubscription, CodingProject, Contact,
    DashboardWidget, FeedItem, JournalNode, LoginAppearance, ManagedUser, PaymentSubscription,
    RssItem, RssItemDraft, RssRefreshTarget, RssSubscription, RssSubscriptionDraft, SessionAccount,
    Task, TaskDraft, TaskSubtaskDraft, User, UserAppearance, UserSettings,
};
use futures_util::future::join_all;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use tokio::time::{Duration as TokioDuration, sleep};
use tracing::{info, warn};

mod bible;
mod bookmark_library;
mod bookmarks;
pub mod calendar;
mod contacts;
pub mod document;
mod embedded_pages;
pub mod jellyfin;
mod jellyfin_client;
mod kanban;
mod lines;
pub mod logging;
pub mod network_policy;
pub mod ntfy;
pub mod oidc;
pub mod podcast_media;
mod podcasts;
mod walls;
pub mod widget_integrations;
mod youtube_downloads;
mod youtube_reader;
mod ytdlp_proxy;
mod ytdlp_runner;

pub use document::{SiteOrigin, service_worker, spa_document, web_app_manifest};
pub use embedded_pages::EmbeddedPagesResponse;
pub use podcast_media::{PodcastMedia, spawn_podcast_workers};
pub use youtube_downloads::{YoutubeDownloadService, spawn_youtube_download_workers};
pub use youtube_reader::spawn_youtube_refresh_worker;

pub const UI_BUILD_DIR: &str = "./ui/build";
const SESSION_COOKIE: &str = "pandan_session";
const OIDC_STATE_COOKIE: &str = "pandan_oidc_state";
const SESSION_DAYS: i64 = 30;
const OIDC_AUTHORIZATION_MINUTES: i64 = 10;
const MAX_WALLPAPER_BYTES: usize = 30 * 1024 * 1024;
const MAX_AVATAR_BYTES: usize = 10 * 1024 * 1024;
const MAX_TASK_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;
const RSS_REFRESH_MINUTES: i64 = 30;
const RSS_REFRESH_BATCH_SIZE: usize = 100;
const RSS_REFRESH_SPACING_SECONDS: u64 = 1;
const DEFAULT_RSS_RETENTION_DAYS: i64 = 7;
const DEFAULT_RSS_RETENTION_MODE: &str = "all";

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cookie_secure: bool,
    pub oidc: Option<oidc::OidcProvider>,
    pub widget_integrations: widget_integrations::WidgetIntegrationService,
    pub jellyfin: jellyfin::JellyfinService,
    pub youtube_downloads: youtube_downloads::YoutubeDownloadService,
    pub podcast_media: podcast_media::PodcastMedia,
    pub ntfy_events: ntfy::NtfyEventHub,
    pub site_origin: document::SiteOrigin,
    pub logging: logging::LoggingController,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    database: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OidcConfigResponse {
    pub enabled: bool,
    pub provider_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticationConfigResponse {
    pub password_login_enabled: bool,
    pub password_registration_enabled: bool,
    pub oidc_enabled: bool,
    pub oidc_registration_enabled: bool,
    pub oidc_provider_name: Option<String>,
    pub login_background_blur: i64,
    pub login_background_brightness: i64,
    pub login_background_contrast: i64,
    pub login_background_saturation: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetupStatusResponse {
    pub required: bool,
}

#[derive(Debug, Deserialize)]
struct OidcCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub user: User,
    pub settings: UserSettings,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserSessionResponse {
    pub id: String,
    pub user_agent: String,
    pub ip_address: String,
    pub is_current: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub user: User,
    pub settings: UserSettings,
    pub appearance: UserAppearance,
    pub tasks: Vec<Task>,
    pub archived_task_count: i64,
    pub feeds: Vec<FeedItem>,
    pub widgets: Vec<DashboardWidget>,
    pub bookmarks: Vec<Bookmark>,
    pub embedded_pages: EmbeddedPagesResponse,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateSettingsRequest {
    pub display_name: String,
    pub location: String,
    pub timezone: String,
    pub sidebar_timezones: Option<Vec<String>>,
    pub calendar_week_start: Option<String>,
    pub temperature_unit: String,
    pub lines_default_visibility: String,
    #[serde(default)]
    pub podcast_playback_rate: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateUserRoleRequest {
    pub role: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateAuthenticationSettingsRequest {
    pub password_login_enabled: bool,
    pub password_registration_enabled: bool,
    pub oidc_registration_enabled: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_task_priority")]
    pub priority: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub subtasks: Vec<TaskSubtaskRequest>,
    pub due_date: Option<String>,
    #[serde(default = "default_repeat_rule")]
    pub repeat_rule: String,
    #[serde(default = "default_repeat_interval")]
    pub repeat_interval: i64,
    #[serde(default = "default_repeat_unit")]
    pub repeat_unit: String,
    #[serde(default = "default_reschedule_from")]
    pub reschedule_from: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtasks: Option<Vec<TaskSubtaskRequest>>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub due_date: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_interval: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reschedule_from: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskSubtaskRequest {
    pub id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub completed: bool,
}

#[derive(Debug, Deserialize)]
struct TaskAttachmentQuery {
    file_name: String,
}

fn default_task_priority() -> String {
    "none".to_owned()
}

fn default_repeat_rule() -> String {
    "none".to_owned()
}

const fn default_repeat_interval() -> i64 {
    1
}

fn default_repeat_unit() -> String {
    "days".to_owned()
}

fn default_reschedule_from() -> String {
    "due_date".to_owned()
}

fn deserialize_optional_nullable<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateWidgetRequest {
    pub kind: String,
    pub workspace: i64,
    pub size: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateAppearanceRequest {
    pub background_blur: i64,
    pub background_brightness: i64,
    pub background_contrast: i64,
    pub background_saturation: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WidgetLayoutItem {
    pub id: String,
    pub workspace: i64,
    pub position: i64,
    pub size: String,
    pub grid_x: i64,
    pub grid_y: i64,
    pub grid_w: i64,
    pub grid_h: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateWidgetLayoutRequest {
    pub widgets: Vec<WidgetLayoutItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateWidgetConfigRequest {
    pub config: Value,
    pub secret: Option<String>,
    #[serde(default)]
    pub clear_secret: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WidgetCapabilitiesResponse {
    pub secret_storage_enabled: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateCodingProjectRequest {
    pub repository: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateCodingCredentialRequest {
    pub provider: String,
    pub host: String,
    pub token: Option<String>,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodingCredentialResponse {
    pub provider: String,
    pub host: String,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodingResponse {
    pub projects: Vec<CodingProject>,
    pub releases: Vec<widget_integrations::CodingRelease>,
    pub merge_requests: Vec<widget_integrations::CodingMergeRequest>,
    pub owned_repositories: Vec<widget_integrations::CodingOwnedRepository>,
    pub pipelines: Vec<widget_integrations::CodingPipeline>,
    pub credentials: Vec<CodingCredentialResponse>,
    pub secret_storage_enabled: bool,
    pub provider_errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WidgetDataQuery {
    #[serde(default)]
    refresh: bool,
}

#[derive(Debug, Deserialize)]
struct CodingDataQuery {
    #[serde(default)]
    refresh: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClearCompletedResponse {
    pub deleted: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteUserContentResponse {
    pub scope: String,
    pub deleted: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RssReaderResponse {
    pub subscriptions: Vec<RssSubscription>,
    pub items: Vec<RssItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateRssSubscriptionRequest {
    pub url: String,
    pub category: String,
    #[serde(default = "default_rss_auto_delete_days")]
    pub auto_delete_days: Option<i64>,
    #[serde(default = "default_rss_auto_delete_mode")]
    pub auto_delete_mode: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateRssSubscriptionRequest {
    pub category: String,
    pub auto_delete_days: Option<i64>,
    pub auto_delete_mode: String,
}

fn default_rss_auto_delete_days() -> Option<i64> {
    Some(DEFAULT_RSS_RETENTION_DAYS)
}

fn default_rss_auto_delete_mode() -> String {
    DEFAULT_RSS_RETENTION_MODE.to_owned()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SetRssItemReadRequest {
    pub read: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SetRssItemSavedRequest {
    pub saved: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PruneRssRequest {
    pub days: i64,
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PruneRssResponse {
    pub deleted: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JournalResponse {
    pub nodes: Vec<JournalNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarResponse {
    pub subscriptions: Vec<CalendarSubscription>,
    pub events: Vec<CalendarEvent>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateCalendarSubscriptionRequest {
    pub url: String,
    #[serde(default = "default_calendar_color")]
    pub color: String,
}

fn default_calendar_color() -> String {
    "#2DD4BF".to_owned()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymentSubscriptionRequest {
    pub service: String,
    #[serde(default)]
    pub description: String,
    pub frequency: String,
    #[serde(default)]
    pub amount_micros: i64,
    #[serde(default = "default_currency")]
    pub currency: String,
    pub first_paid_on: String,
}

fn default_currency() -> String {
    "USD".to_owned()
}

struct ValidatedPaymentSubscription<'a> {
    service: &'a str,
    description: &'a str,
    frequency: &'a str,
    amount_micros: i64,
    currency: String,
    first_paid_on: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateJournalNodeRequest {
    pub parent_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateJournalNodeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub parent_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'static str>,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(&'static str),
    #[error("email or password is incorrect")]
    Unauthorized,
    #[error("administrator access is required")]
    Forbidden,
    #[error("{0}")]
    AccessDenied(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    #[error("{0}")]
    NotFound(&'static str),
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Internal(&'static str),
    #[error("{0}")]
    Unavailable(&'static str),
    #[error("{message}")]
    Coded {
        status: StatusCode,
        code: &'static str,
        message: &'static str,
    },
    #[error("single sign-on is not configured")]
    OidcUnavailable,
    #[error("{0}")]
    AuthenticationDisabled(&'static str),
    #[error("complete administrator setup first")]
    SetupRequired,
    #[error("{0}")]
    Integration(String),
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden | Self::AccessDenied(_) | Self::AuthenticationDisabled(_) => {
                StatusCode::FORBIDDEN
            }
            Self::Conflict(_) | Self::SetupRequired => StatusCode::CONFLICT,
            Self::NotFound(_) | Self::OidcUnavailable => StatusCode::NOT_FOUND,
            Self::Integration(_) => StatusCode::BAD_GATEWAY,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Coded { status, .. } => *status,
            Self::Database(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        match self {
            Self::Database(error) => {
                tracing::error!(error = %error, "database operation failed");
            }
            Self::Internal(message) => {
                tracing::error!(error = %message, "internal server error");
            }
            Self::Integration(message) => {
                tracing::warn!(error = %message, "integration request failed");
            }
            _ => {}
        }

        let code = match self {
            Self::Coded { code, .. } => Some(*code),
            _ => None,
        };
        HttpResponse::build(self.status_code()).json(ErrorResponse {
            error: self.to_string(),
            code,
        })
    }
}

pub async fn health(state: web::Data<AppState>) -> impl Responder {
    match db::health_check(&state.pool).await {
        Ok(()) => HttpResponse::Ok().json(HealthResponse {
            status: "ok",
            database: "connected",
        }),
        Err(error) => {
            tracing::error!(%error, "database health check failed");
            HttpResponse::ServiceUnavailable().json(HealthResponse {
                status: "unavailable",
                database: "disconnected",
            })
        }
    }
}

async fn oidc_config(state: web::Data<AppState>) -> web::Json<OidcConfigResponse> {
    web::Json(OidcConfigResponse {
        enabled: state.oidc.is_some(),
        provider_name: state.oidc.as_ref().map(|provider| provider.name.clone()),
    })
}

fn authentication_config_response(
    state: &AppState,
    settings: AuthenticationSettings,
    appearance: LoginAppearance,
) -> AuthenticationConfigResponse {
    AuthenticationConfigResponse {
        password_login_enabled: settings.password_login_enabled || state.oidc.is_none(),
        password_registration_enabled: settings.password_registration_enabled,
        oidc_enabled: state.oidc.is_some(),
        oidc_registration_enabled: settings.oidc_registration_enabled,
        oidc_provider_name: state.oidc.as_ref().map(|provider| provider.name.clone()),
        login_background_blur: appearance.background_blur,
        login_background_brightness: appearance.background_brightness,
        login_background_contrast: appearance.background_contrast,
        login_background_saturation: appearance.background_saturation,
    }
}

async fn authentication_config(
    state: web::Data<AppState>,
) -> Result<web::Json<AuthenticationConfigResponse>, ApiError> {
    let (settings, appearance) = tokio::try_join!(
        db::queries::get_authentication_settings(&state.pool),
        db::queries::get_login_appearance(&state.pool),
    )?;
    Ok(web::Json(authentication_config_response(
        &state, settings, appearance,
    )))
}

async fn setup_status(
    state: web::Data<AppState>,
) -> Result<web::Json<SetupStatusResponse>, ApiError> {
    Ok(web::Json(SetupStatusResponse {
        required: !db::queries::is_onboarding_complete(&state.pool).await?,
    }))
}

async fn setup(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<RegisterRequest>,
) -> Result<HttpResponse, ApiError> {
    let email = normalize_email(&payload.email)?;
    let display_name = validate_short_text(&payload.display_name, "display name is required", 60)?;
    validate_password(&payload.password)?;
    let password = payload.password.clone();
    let password_hash = web::block(move || hash_password(&password))
        .await
        .map_err(|_| ApiError::Internal("password hashing failed"))?
        .map_err(|_| ApiError::Internal("password hashing failed"))?;
    let (user, settings) = db::queries::create_initial_administrator(
        &state.pool,
        &email,
        &password_hash,
        display_name,
    )
    .await?
    .ok_or(ApiError::Conflict(
        "administrator setup is already complete",
    ))?;
    let cookie = issue_session(&state, &request, &user.id).await?;
    info!(user_id = %user.id, "initial administrator account created");

    Ok(HttpResponse::Created()
        .cookie(cookie)
        .json(AuthResponse { user, settings }))
}

async fn oidc_start(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let provider = state.oidc.as_ref().ok_or(ApiError::OidcUnavailable)?;
    let attempt = provider.authorization_attempt();
    let expires_at =
        (chrono::Utc::now() + chrono::Duration::minutes(OIDC_AUTHORIZATION_MINUTES)).to_rfc3339();
    db::queries::create_oidc_authorization(
        &state.pool,
        &attempt.state,
        &attempt.pkce_verifier,
        &attempt.nonce,
        &expires_at,
    )
    .await?;

    let state_cookie = Cookie::build(OIDC_STATE_COOKIE, attempt.state)
        .path("/api/auth/oidc/callback")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.cookie_secure)
        .max_age(CookieDuration::minutes(OIDC_AUTHORIZATION_MINUTES))
        .finish();
    Ok(HttpResponse::SeeOther()
        .cookie(state_cookie)
        .append_header((header::LOCATION, attempt.url))
        .finish())
}

async fn oidc_callback(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<OidcCallbackQuery>,
) -> HttpResponse {
    if let Some(provider_error) = &query.error {
        tracing::warn!(error = %provider_error, "OIDC authorization was denied by provider");
        return oidc_error_redirect("access_denied", state.cookie_secure);
    }

    match complete_oidc_callback(&state, &request, &query).await {
        Ok(cookie) => HttpResponse::SeeOther()
            .cookie(cookie)
            .cookie(oidc_state_removal(state.cookie_secure))
            .append_header((header::LOCATION, "/"))
            .finish(),
        Err(error) => {
            tracing::warn!(%error, "OIDC callback failed");
            let reason = if error == "OIDC registration is disabled" {
                "registration_disabled"
            } else {
                "failed"
            };
            oidc_error_redirect(reason, state.cookie_secure)
        }
    }
}

async fn complete_oidc_callback(
    state: &AppState,
    request: &HttpRequest,
    query: &OidcCallbackQuery,
) -> Result<Cookie<'static>, String> {
    let provider = state
        .oidc
        .as_ref()
        .ok_or_else(|| "OIDC is not configured".to_owned())?;
    let state_value = query
        .state
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "OIDC callback omitted state".to_owned())?;
    let code = query
        .code
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "OIDC callback omitted authorization code".to_owned())?;
    let browser_state = request
        .cookie(OIDC_STATE_COOKIE)
        .map(|cookie| cookie.value().to_owned())
        .ok_or_else(|| "OIDC callback was not initiated by this browser".to_owned())?;
    if browser_state != state_value {
        return Err("OIDC browser state did not match callback state".to_owned());
    }
    let attempt = db::queries::consume_oidc_authorization(&state.pool, state_value)
        .await
        .map_err(|error| format!("failed to consume OIDC state: {error}"))?
        .ok_or_else(|| "OIDC state was invalid, expired, or already used".to_owned())?;
    let identity = provider
        .verify_code(code.to_owned(), attempt.pkce_verifier, attempt.nonce)
        .await
        .map_err(|error| error.to_string())?;
    let email = normalize_email(&identity.email).map_err(|error| error.to_string())?;
    let display_name: String = identity.display_name.chars().take(60).collect();
    let random_password = uuid::Uuid::new_v4().to_string();
    let unusable_password_hash = web::block(move || hash_password(&random_password))
        .await
        .map_err(|_| "OIDC account password hardening failed".to_owned())?
        .map_err(|_| "OIDC account password hardening failed".to_owned())?;
    let initial_user_id = db::queries::create_initial_oidc_administrator(
        &state.pool,
        &identity.issuer,
        &identity.subject,
        &email,
        &display_name,
        &unusable_password_hash,
    )
    .await
    .map_err(|error| format!("failed to create initial OIDC administrator: {error}"))?;
    let initial_administrator = initial_user_id.is_some();
    let user_id = match initial_user_id {
        Some(user_id) => user_id,
        None => {
            let authentication_settings = db::queries::get_authentication_settings(&state.pool)
                .await
                .map_err(|error| format!("failed to load authentication settings: {error}"))?;
            db::queries::find_or_create_oidc_user(
                &state.pool,
                &identity.issuer,
                &identity.subject,
                &email,
                &display_name,
                &unusable_password_hash,
                authentication_settings.oidc_registration_enabled,
            )
            .await
            .map_err(|error| format!("failed to link OIDC identity: {error}"))?
            .ok_or_else(|| "OIDC registration is disabled".to_owned())?
        }
    };
    if let Some(picture_url) = identity.picture_url.as_deref() {
        import_oidc_avatar_if_missing(state, &user_id, picture_url).await;
    }
    info!(
        %user_id,
        initial_administrator,
        "OIDC authentication completed"
    );
    issue_session(state, request, &user_id)
        .await
        .map_err(|error| error.to_string())
}

async fn import_oidc_avatar_if_missing(state: &AppState, user_id: &str, picture_url: &str) {
    match db::queries::has_user_avatar(&state.pool, user_id).await {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            tracing::warn!(%error, "OIDC avatar lookup failed");
            return;
        }
    }

    let (mime_type, image_data) = match state
        .widget_integrations
        .fetch_bounded_public_image(picture_url, MAX_AVATAR_BYTES)
        .await
    {
        Ok(image) => image,
        Err(error) => {
            tracing::warn!(%error, "OIDC avatar fetch failed");
            return;
        }
    };
    if let Err(error) = validate_image_upload(&mime_type, &image_data, "avatar") {
        tracing::warn!(%error, "OIDC avatar validation failed");
        return;
    }
    if let Err(error) =
        db::queries::insert_user_avatar_if_absent(&state.pool, user_id, &mime_type, &image_data)
            .await
    {
        tracing::warn!(%error, "OIDC avatar storage failed");
    }
}

fn oidc_error_redirect(reason: &str, cookie_secure: bool) -> HttpResponse {
    HttpResponse::SeeOther()
        .cookie(oidc_state_removal(cookie_secure))
        .append_header((header::LOCATION, format!("/?auth_error=oidc_{reason}")))
        .finish()
}

fn oidc_state_removal(cookie_secure: bool) -> Cookie<'static> {
    Cookie::build(OIDC_STATE_COOKIE, "")
        .path("/api/auth/oidc/callback")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(cookie_secure)
        .max_age(CookieDuration::ZERO)
        .finish()
}

async fn register(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<RegisterRequest>,
) -> Result<HttpResponse, ApiError> {
    ensure_onboarding_complete(&state).await?;
    if !db::queries::get_authentication_settings(&state.pool)
        .await?
        .password_registration_enabled
    {
        return Err(ApiError::AuthenticationDisabled(
            "password registration is disabled",
        ));
    }
    let email = normalize_email(&payload.email)?;
    let display_name = validate_short_text(&payload.display_name, "display name is required", 60)?;
    validate_password(&payload.password)?;

    let password = payload.password.clone();
    let password_hash = web::block(move || hash_password(&password))
        .await
        .map_err(|_| ApiError::Internal("password hashing failed"))?
        .map_err(|_| ApiError::Internal("password hashing failed"))?;

    let account = db::queries::create_account(&state.pool, &email, &password_hash, display_name)
        .await
        .map_err(|error| {
            if error
                .as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
            {
                ApiError::Conflict("an account already exists for this email")
            } else {
                ApiError::Database(error)
            }
        })?;
    let cookie = issue_session(&state, &request, &account.0.id).await?;
    info!(user_id = %account.0.id, "password account registered");

    Ok(HttpResponse::Created().cookie(cookie).json(AuthResponse {
        user: account.0,
        settings: account.1,
    }))
}

async fn login(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<LoginRequest>,
) -> Result<HttpResponse, ApiError> {
    let authentication_settings = db::queries::get_authentication_settings(&state.pool).await?;
    if !authentication_settings.password_login_enabled && state.oidc.is_some() {
        return Err(ApiError::AuthenticationDisabled(
            "password login is disabled",
        ));
    }
    let email = normalize_email(&payload.email)?;
    let Some(credentials) = db::queries::find_user_credentials(&state.pool, &email).await? else {
        warn!(reason = "unknown_account", "password authentication failed");
        return Err(ApiError::Unauthorized);
    };
    let password = payload.password.clone();
    let password_hash = credentials.password_hash.clone();
    let valid = web::block(move || verify_password(&password, &password_hash))
        .await
        .map_err(|_| ApiError::Internal("password verification failed"))?;
    if !valid {
        warn!(
            user_id = %credentials.id,
            reason = "invalid_password",
            "password authentication failed"
        );
        return Err(ApiError::Unauthorized);
    }

    let cookie = issue_session(&state, &request, &credentials.id).await?;
    let account = account_from_cookie_value(&state, cookie.value()).await?;
    info!(user_id = %credentials.id, "password authentication completed");

    Ok(HttpResponse::Ok()
        .cookie(cookie)
        .json(auth_response(account)))
}

async fn logout(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    if let Some(cookie) = request.cookie(SESSION_COOKIE) {
        db::queries::delete_session(&state.pool, cookie.value()).await?;
    }

    Ok(HttpResponse::NoContent()
        .cookie(session_removal_cookie(state.cookie_secure))
        .finish())
}

async fn session(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<AuthResponse>, ApiError> {
    Ok(web::Json(auth_response(
        authenticated_account(&state, &request).await?,
    )))
}

async fn list_sessions(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<Vec<BrowserSessionResponse>>, ApiError> {
    let current_token = session_token(&request)?;
    let account = authenticated_account(&state, &request).await?;
    let sessions = db::queries::list_account_sessions(&state.pool, &account.id)
        .await?
        .into_iter()
        .map(|session| BrowserSessionResponse {
            id: session.id,
            is_current: session.token == current_token,
            user_agent: session.user_agent,
            ip_address: session.ip_address,
        })
        .collect();
    Ok(web::Json(sessions))
}

async fn delete_session(
    state: web::Data<AppState>,
    request: HttpRequest,
    session_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let current_token = session_token(&request)?;
    let account = authenticated_account(&state, &request).await?;
    let deleted_token = db::queries::delete_account_session(&state.pool, &account.id, &session_id)
        .await?
        .ok_or(ApiError::NotFound("session not found"))?;
    let mut response = HttpResponse::NoContent();
    if deleted_token == current_token {
        response.cookie(session_removal_cookie(state.cookie_secure));
    }
    Ok(response.finish())
}

async fn dashboard(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<DashboardResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let user_id = account.id.clone();
    let (tasks, archived_task_count, feeds, widgets, bookmarks, appearance, embedded_pages) = tokio::try_join!(
        db::queries::list_tasks(&state.pool, &user_id),
        db::queries::count_archived_tasks(&state.pool, &user_id),
        db::queries::list_feed_items(&state.pool),
        db::queries::list_dashboard_widgets(&state.pool, &user_id),
        db::queries::list_bookmarks(&state.pool, &user_id),
        db::queries::find_user_appearance(&state.pool, &user_id),
        embedded_pages::load_visible_pages(&state.pool, &user_id),
    )?;
    let auth = auth_response(account);

    Ok(web::Json(DashboardResponse {
        user: auth.user,
        settings: auth.settings,
        appearance,
        tasks,
        archived_task_count,
        feeds,
        widgets,
        bookmarks,
        embedded_pages,
    }))
}

async fn create_widget(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateWidgetRequest>,
) -> Result<(web::Json<DashboardWidget>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    validate_widget_kind(&payload.kind)?;
    let (grid_w, grid_h) = default_grid_size(&payload.size)?;
    validate_widget_layout(payload.workspace, 0, &payload.size, 0, 0, grid_w, grid_h)?;
    if db::queries::list_dashboard_widgets(&state.pool, &account.id)
        .await?
        .len()
        >= 64
    {
        return Err(ApiError::BadRequest(
            "a dashboard can contain at most 64 widgets",
        ));
    }
    let widget = db::queries::create_dashboard_widget(
        &state.pool,
        &account.id,
        &payload.kind,
        payload.workspace,
        &payload.size,
    )
    .await?;
    Ok((web::Json(widget), StatusCode::CREATED))
}

async fn update_widget_layout(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateWidgetLayoutRequest>,
) -> Result<web::Json<Vec<DashboardWidget>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if payload.widgets.is_empty() || payload.widgets.len() > 64 {
        return Err(ApiError::BadRequest(
            "widget layout must contain between 1 and 64 items",
        ));
    }
    for (index, widget) in payload.widgets.iter().enumerate() {
        validate_widget_layout(
            widget.workspace,
            widget.position,
            &widget.size,
            widget.grid_x,
            widget.grid_y,
            widget.grid_w,
            widget.grid_h,
        )?;
        if payload.widgets[..index]
            .iter()
            .any(|candidate| candidate.id == widget.id)
        {
            return Err(ApiError::BadRequest(
                "widget layout contains duplicate identifiers",
            ));
        }
    }
    let layout = payload
        .widgets
        .iter()
        .map(|widget| {
            (
                widget.id.clone(),
                widget.workspace,
                widget.position,
                widget.size.clone(),
                widget.grid_x,
                widget.grid_y,
                widget.grid_w,
                widget.grid_h,
            )
        })
        .collect::<Vec<_>>();
    db::queries::update_dashboard_widget_layout(&state.pool, &account.id, &layout)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("widget not found"))
}

async fn widget_capabilities(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<WidgetCapabilitiesResponse>, ApiError> {
    authenticated_account(&state, &request).await?;
    Ok(web::Json(WidgetCapabilitiesResponse {
        secret_storage_enabled: state.widget_integrations.secrets_enabled(),
    }))
}

async fn update_widget_config(
    state: web::Data<AppState>,
    request: HttpRequest,
    widget_id: web::Path<String>,
    payload: web::Json<UpdateWidgetConfigRequest>,
) -> Result<web::Json<DashboardWidget>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let widget = db::queries::get_dashboard_widget(&state.pool, &account.id, &widget_id)
        .await?
        .ok_or(ApiError::NotFound("widget not found"))?;
    widget_integrations::validate_widget_config(&widget.kind, &payload.config)
        .map_err(ApiError::BadRequest)?;
    if payload.clear_secret
        && payload
            .secret
            .as_ref()
            .is_some_and(|value| !value.is_empty())
    {
        return Err(ApiError::BadRequest(
            "a credential cannot be set and cleared together",
        ));
    }
    let encrypted = payload
        .secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.len() > 4_096 {
                return Err(ApiError::BadRequest("widget credential is too large"));
            }
            state
                .widget_integrations
                .encrypt_secret(value)
                .map_err(ApiError::Integration)
        })
        .transpose()?;
    let secret_update = if let Some(ciphertext) = encrypted.as_deref() {
        Some(Some(ciphertext))
    } else if payload.clear_secret {
        Some(None)
    } else {
        None
    };
    let config_json = serde_json::to_string(&payload.config)
        .map_err(|_| ApiError::BadRequest("widget configuration is invalid"))?;
    let updated = db::queries::update_dashboard_widget_integration(
        &state.pool,
        &account.id,
        &widget_id,
        &config_json,
        secret_update,
    )
    .await?
    .ok_or(ApiError::NotFound("widget not found"))?;
    state.widget_integrations.clear_cache(&widget_id).await;
    Ok(web::Json(updated))
}

async fn widget_data(
    state: web::Data<AppState>,
    request: HttpRequest,
    widget_id: web::Path<String>,
    query: web::Query<WidgetDataQuery>,
) -> Result<web::Json<Value>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let widget = db::queries::get_dashboard_widget(&state.pool, &account.id, &widget_id)
        .await?
        .ok_or(ApiError::NotFound("widget not found"))?;
    if widget.kind == "rss" {
        return rss_widget_data(&state, &account.id, &widget)
            .await
            .map(web::Json);
    }
    if query.refresh {
        state.widget_integrations.clear_cache(&widget_id).await;
    }
    let encrypted_secret =
        db::queries::get_widget_secret(&state.pool, &account.id, &widget_id).await?;
    state
        .widget_integrations
        .fetch(&widget, encrypted_secret.as_deref())
        .await
        .map(web::Json)
        .map_err(ApiError::Integration)
}

async fn rss_widget_data(
    state: &AppState,
    user_id: &str,
    widget: &DashboardWidget,
) -> Result<Value, ApiError> {
    let subscriptions = db::queries::list_rss_subscriptions(&state.pool, user_id).await?;
    let configured_ids = rss_widget_config_strings(&widget.config, "subscription_ids", 32);
    let legacy_urls = rss_widget_config_strings(&widget.config, "urls", 32);
    let selected = subscriptions
        .into_iter()
        .filter(|subscription| {
            configured_ids.contains(&subscription.id)
                || (configured_ids.is_empty() && legacy_urls.contains(&subscription.url))
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(ApiError::Integration(
            "Choose at least one subscribed RSS source for this widget".to_owned(),
        ));
    }
    let selected_ids = selected
        .iter()
        .map(|subscription| subscription.id.clone())
        .collect::<Vec<_>>();
    let limit = widget.config["limit"]
        .as_u64()
        .map_or(24, |value| value.clamp(1, 40) as usize);
    let items =
        db::queries::list_current_rss_items(&state.pool, user_id, &selected_ids, limit).await?;
    let refreshed_at = selected
        .iter()
        .filter_map(|subscription| subscription.last_fetched_at.as_ref())
        .max()
        .cloned();
    let stale_source_count = selected
        .iter()
        .filter(|subscription| subscription.last_error.is_some())
        .count();
    let pending_source_count = selected
        .iter()
        .filter(|subscription| subscription.refresh_generation == 0)
        .count();
    Ok(serde_json::json!({
        "items": items,
        "refreshed_at": refreshed_at,
        "source_count": selected.len(),
        "stale_source_count": stale_source_count,
        "pending_source_count": pending_source_count,
    }))
}

fn rss_widget_config_strings(config: &Value, key: &str, max: usize) -> Vec<String> {
    config[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(max)
        .map(str::to_owned)
        .collect()
}

async fn delete_widget(
    state: web::Data<AppState>,
    request: HttpRequest,
    widget_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let widget = db::queries::get_dashboard_widget(&state.pool, &account.id, &widget_id)
        .await?
        .ok_or(ApiError::NotFound("widget not found"))?;
    if widget.kind == "streams" && widget.config["placement"] == "utility_rail" {
        return Err(ApiError::BadRequest(
            "the dashboard stream tracker cannot be removed",
        ));
    }
    if db::queries::delete_dashboard_widget(&state.pool, &account.id, &widget_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("widget not found"))
    }
}

async fn coding(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<CodingDataQuery>,
) -> Result<web::Json<CodingResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (cache_generation, cached_response) = if query.refresh {
        (
            state
                .widget_integrations
                .clear_coding_cache(&account.id)
                .await,
            None,
        )
    } else {
        state
            .widget_integrations
            .coding_cache_snapshot(&account.id)
            .await
    };
    if let Some(response) = cached_response {
        return Ok(web::Json(response));
    }
    let (projects, stored_credentials) = tokio::try_join!(
        db::queries::list_coding_projects(&state.pool, &account.id),
        db::queries::list_coding_credentials(&state.pool, &account.id)
    )?;
    let credential_map = stored_credentials
        .iter()
        .map(|credential| {
            (
                (credential.provider.clone(), credential.host.clone()),
                credential.ciphertext.clone(),
            )
        })
        .collect::<HashMap<_, _>>();

    let release_results = join_all(projects.iter().map(|project| {
        let service = state.widget_integrations.clone();
        let project = project.clone();
        let credential = credential_map
            .get(&(project.provider.clone(), project.host.clone()))
            .cloned();
        async move {
            let result = service
                .fetch_coding_release(&project, credential.as_deref())
                .await;
            (project.repository, result)
        }
    }))
    .await;
    let mut releases = Vec::new();
    let mut provider_errors = Vec::new();
    for (repository, result) in release_results {
        match result {
            Ok(release) => releases.push(release),
            Err(error) => provider_errors.push(format!("{repository}: {error}")),
        }
    }
    releases.sort_by(|left, right| right.published_at.cmp(&left.published_at));

    let pipeline_results = join_all(
        projects
            .iter()
            .filter(|project| project.provider == "gitlab")
            .filter_map(|project| {
                let credential = credential_map
                    .get(&(project.provider.clone(), project.host.clone()))?
                    .clone();
                let service = state.widget_integrations.clone();
                let project = project.clone();
                Some(async move {
                    let result = service.fetch_gitlab_pipeline(&project, &credential).await;
                    (project.repository, result)
                })
            }),
    )
    .await;
    let mut pipelines = Vec::new();
    for (repository, result) in pipeline_results {
        match result {
            Ok(Some(pipeline)) => pipelines.push(pipeline),
            Ok(None) => {}
            Err(error) => provider_errors.push(format!("{repository} pipeline: {error}")),
        }
    }

    let owned_repository_results = join_all(stored_credentials.iter().map(|credential| {
        let service = state.widget_integrations.clone();
        let provider = credential.provider.clone();
        let host = credential.host.clone();
        let ciphertext = credential.ciphertext.clone();
        async move {
            let result = service
                .fetch_owned_coding_repositories(&provider, &host, &ciphertext)
                .await;
            (provider, host, result)
        }
    }))
    .await;
    let mut owned_repositories = Vec::new();
    for (provider, host, result) in owned_repository_results {
        match result {
            Ok(mut snapshot) => {
                provider_errors.extend(
                    snapshot
                        .errors
                        .drain(..)
                        .map(|error| format!("{host}: {error}")),
                );
                owned_repositories.append(&mut snapshot.repositories);
            }
            Err(error) => {
                provider_errors.push(format!("{provider}@{host} repositories: {error}"));
            }
        }
    }
    owned_repositories.sort_by(|left, right| {
        right
            .open_pull_requests
            .unwrap_or_default()
            .cmp(&left.open_pull_requests.unwrap_or_default())
            .then_with(|| left.repository.cmp(&right.repository))
    });

    let credentials = stored_credentials
        .into_iter()
        .map(|credential| CodingCredentialResponse {
            provider: credential.provider,
            host: credential.host,
            connected: true,
        })
        .collect();
    let response = CodingResponse {
        projects,
        releases,
        merge_requests: Vec::new(),
        owned_repositories,
        pipelines,
        credentials,
        secret_storage_enabled: state.widget_integrations.secrets_enabled(),
        provider_errors,
    };
    state
        .widget_integrations
        .cache_coding_if_current(&account.id, cache_generation, &response)
        .await;
    Ok(web::Json(response))
}

async fn create_coding_project(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateCodingProjectRequest>,
) -> Result<(web::Json<CodingProject>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::list_coding_projects(&state.pool, &account.id)
        .await?
        .len()
        >= 64
    {
        return Err(ApiError::BadRequest(
            "a Coding workspace can contain at most 64 projects",
        ));
    }
    let repository = widget_integrations::parse_release_repository(&payload.repository)
        .map_err(|_| ApiError::BadRequest("repository syntax is invalid"))?;
    let project = db::queries::create_coding_project(
        &state.pool,
        &account.id,
        &repository.provider,
        &repository.host,
        &repository.repository,
    )
    .await
    .map_err(|error| {
        if error
            .as_database_error()
            .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
        {
            ApiError::Conflict("that project is already subscribed")
        } else {
            ApiError::Database(error)
        }
    })?;
    state
        .widget_integrations
        .clear_coding_cache(&account.id)
        .await;
    Ok((web::Json(project), StatusCode::CREATED))
}

async fn delete_coding_project(
    state: web::Data<AppState>,
    request: HttpRequest,
    project_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_coding_project(&state.pool, &account.id, &project_id).await? {
        state
            .widget_integrations
            .clear_coding_cache(&account.id)
            .await;
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Coding project not found"))
    }
}

async fn update_coding_credential(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateCodingCredentialRequest>,
) -> Result<web::Json<CodingCredentialResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    validate_coding_host(&payload.provider, &payload.host)?;
    if payload.clear
        && payload
            .token
            .as_ref()
            .is_some_and(|token| !token.trim().is_empty())
    {
        return Err(ApiError::BadRequest(
            "a provider credential cannot be set and cleared together",
        ));
    }
    if payload.clear {
        db::queries::delete_coding_credential(
            &state.pool,
            &account.id,
            &payload.provider,
            &payload.host,
        )
        .await?;
        state
            .widget_integrations
            .clear_coding_cache(&account.id)
            .await;
        return Ok(web::Json(CodingCredentialResponse {
            provider: payload.provider.clone(),
            host: payload.host.clone(),
            connected: false,
        }));
    }
    let token = payload
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or(ApiError::BadRequest("provider token is required"))?;
    if token.len() > 4_096 {
        return Err(ApiError::BadRequest("provider token is too large"));
    }
    let ciphertext = state
        .widget_integrations
        .encrypt_secret(token)
        .map_err(ApiError::Integration)?;
    db::queries::upsert_coding_credential(
        &state.pool,
        &account.id,
        &payload.provider,
        &payload.host,
        &ciphertext,
    )
    .await?;
    state
        .widget_integrations
        .clear_coding_cache(&account.id)
        .await;
    Ok(web::Json(CodingCredentialResponse {
        provider: payload.provider.clone(),
        host: payload.host.clone(),
        connected: true,
    }))
}

fn validate_coding_host(provider: &str, host: &str) -> Result<(), ApiError> {
    let valid = match provider {
        "github" => host == "github.com",
        "gitlab" => host == "gitlab.com",
        "codeberg" => host == "codeberg.org",
        "gitea" | "forgejo" => widget_integrations::parse_release_repository(&format!(
            "{provider}@{host}:owner/repository"
        ))
        .is_ok(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ApiError::BadRequest("code host is invalid"))
    }
}

fn validate_widget_kind(kind: &str) -> Result<(), ApiError> {
    if matches!(
        kind,
        "weather"
            | "task-summary"
            | "focus"
            | "task-list"
            | "feed-list"
            | "feed-sources"
            | "youtube"
            | "rss"
            | "reddit"
            | "stocks"
            | "calendar"
            | "clock"
            | "iframe"
            | "html"
            | "releases"
            | "streams"
            | "bible-verse"
    ) {
        Ok(())
    } else {
        Err(ApiError::BadRequest("widget type is invalid"))
    }
}

fn default_grid_size(size: &str) -> Result<(i64, i64), ApiError> {
    match size {
        "compact" => Ok((4, 4)),
        "standard" => Ok((6, 4)),
        "wide" => Ok((8, 5)),
        "full" => Ok((12, 6)),
        _ => Err(ApiError::BadRequest("widget size is invalid")),
    }
}

fn validate_widget_layout(
    workspace: i64,
    position: i64,
    size: &str,
    grid_x: i64,
    grid_y: i64,
    grid_w: i64,
    grid_h: i64,
) -> Result<(), ApiError> {
    if workspace != 0 {
        return Err(ApiError::BadRequest("widget workspace is invalid"));
    }
    if !(0..=127).contains(&position) {
        return Err(ApiError::BadRequest("widget position is invalid"));
    }
    default_grid_size(size)?;
    if !(0..=11).contains(&grid_x)
        || !(0..=255).contains(&grid_y)
        || !(1..=12).contains(&grid_w)
        || !(1..=12).contains(&grid_h)
        || grid_x + grid_w > 12
    {
        return Err(ApiError::BadRequest("widget grid position is invalid"));
    }
    Ok(())
}

async fn update_settings(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateSettingsRequest>,
) -> Result<web::Json<UserSettings>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let display_name = validate_short_text(&payload.display_name, "display name is required", 60)?;
    let location = validate_short_text(&payload.location, "location is required", 80)?;
    let timezone = validate_short_text(&payload.timezone, "timezone is required", 80)?;
    let sidebar_timezones = match payload.sidebar_timezones.as_deref() {
        Some(timezones) => validate_sidebar_timezones(timezones)?,
        None => parse_sidebar_timezones(&account.sidebar_timezones_json, timezone),
    };
    let sidebar_timezones_json = serde_json::to_string(&sidebar_timezones)
        .map_err(|_| ApiError::Internal("sidebar timezones could not be saved"))?;
    let calendar_week_start = payload
        .calendar_week_start
        .as_deref()
        .unwrap_or(&account.calendar_week_start);
    if !matches!(calendar_week_start, "sunday" | "monday") {
        return Err(ApiError::BadRequest("calendar week start is invalid"));
    }
    if !matches!(payload.temperature_unit.as_str(), "celsius" | "fahrenheit") {
        return Err(ApiError::BadRequest("temperature unit is invalid"));
    }
    if !matches!(
        payload.lines_default_visibility.as_str(),
        "private" | "public"
    ) {
        return Err(ApiError::BadRequest("Lines default visibility is invalid"));
    }
    let podcast_playback_rate = payload
        .podcast_playback_rate
        .unwrap_or(account.podcast_playback_rate);
    if !(0.5..=3.0).contains(&podcast_playback_rate) {
        return Err(ApiError::BadRequest(
            "podcast playback rate must be between 0.5 and 3.0",
        ));
    }

    Ok(web::Json(
        db::queries::update_user_settings(
            &state.pool,
            &account.id,
            display_name,
            location,
            timezone,
            &sidebar_timezones_json,
            calendar_week_start,
            &payload.temperature_unit,
            &payload.lines_default_visibility,
            podcast_playback_rate,
        )
        .await?,
    ))
}

async fn delete_user_content(
    state: web::Data<AppState>,
    request: HttpRequest,
    scope: web::Path<String>,
) -> Result<web::Json<DeleteUserContentResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let scope = scope.into_inner();
    if !matches!(
        scope.as_str(),
        "contacts"
            | "tasks"
            | "lines"
            | "calendar"
            | "rss"
            | "journal"
            | "youtube"
            | "downloads"
            | "podcasts"
            | "coding"
            | "subscriptions"
    ) {
        return Err(ApiError::BadRequest("content scope is invalid"));
    }
    if scope == "downloads" {
        state
            .youtube_downloads
            .purge_user(&account.id)
            .await
            .map_err(|_| ApiError::Internal("download files could not be removed"))?;
    }
    let deleted = db::queries::delete_user_content(&state.pool, &account.id, &scope).await?;
    info!(
        user_id = %account.id,
        %scope,
        deleted,
        "account content deleted"
    );
    Ok(web::Json(DeleteUserContentResponse { scope, deleted }))
}

async fn update_appearance(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateAppearanceRequest>,
) -> Result<web::Json<UserAppearance>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !(0..=24).contains(&payload.background_blur)
        || !(40..=140).contains(&payload.background_brightness)
        || !(50..=160).contains(&payload.background_contrast)
        || !(0..=180).contains(&payload.background_saturation)
    {
        return Err(ApiError::BadRequest(
            "background appearance value is invalid",
        ));
    }
    Ok(web::Json(
        db::queries::update_user_appearance(
            &state.pool,
            &account.id,
            payload.background_blur,
            payload.background_brightness,
            payload.background_contrast,
            payload.background_saturation,
        )
        .await?,
    ))
}

async fn update_login_appearance(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateAppearanceRequest>,
) -> Result<web::Json<LoginAppearance>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    if !(0..=24).contains(&payload.background_blur)
        || !(40..=140).contains(&payload.background_brightness)
        || !(50..=160).contains(&payload.background_contrast)
        || !(0..=180).contains(&payload.background_saturation)
    {
        return Err(ApiError::BadRequest(
            "background appearance value is invalid",
        ));
    }
    Ok(web::Json(
        db::queries::update_login_appearance(
            &state.pool,
            payload.background_blur,
            payload.background_brightness,
            payload.background_contrast,
            payload.background_saturation,
        )
        .await?,
    ))
}

async fn get_login_wallpaper(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let wallpaper = db::queries::find_login_wallpaper(&state.pool)
        .await?
        .ok_or(ApiError::NotFound("login wallpaper not found"))?;

    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, wallpaper.mime_type))
        .insert_header((header::CACHE_CONTROL, "public, no-cache"))
        .insert_header(("Cross-Origin-Resource-Policy", "same-origin"))
        .insert_header((header::ETAG, format!("\"{}\"", wallpaper.updated_at)))
        .body(wallpaper.image_data))
}

async fn get_wallpaper(
    state: web::Data<AppState>,
    request: HttpRequest,
    slot: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let slot = slot.into_inner();
    validate_wallpaper_slot(&slot)?;
    let wallpaper = if slot == "login" {
        db::queries::find_login_wallpaper(&state.pool).await?
    } else {
        db::queries::find_user_wallpaper(&state.pool, &account.id, &slot).await?
    }
    .ok_or(ApiError::NotFound("wallpaper not found"))?;

    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, wallpaper.mime_type))
        .insert_header((header::CACHE_CONTROL, "private, no-cache"))
        .insert_header((header::ETAG, format!("\"{}\"", wallpaper.updated_at)))
        .body(wallpaper.image_data))
}

async fn update_wallpaper(
    state: web::Data<AppState>,
    request: HttpRequest,
    slot: web::Path<String>,
    image_data: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let slot = slot.into_inner();
    validate_wallpaper_slot(&slot)?;
    if slot == "login" && account.role != "administrator" {
        return Err(ApiError::Forbidden);
    }
    if image_data.is_empty() || image_data.len() > MAX_WALLPAPER_BYTES {
        return Err(ApiError::BadRequest(
            "wallpaper image must be between 1 byte and 30 MB",
        ));
    }
    let mime_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .ok_or(ApiError::BadRequest("wallpaper image type is required"))?;
    validate_image_upload(mime_type, &image_data, "wallpaper")?;

    if slot == "login" {
        db::queries::replace_login_wallpaper(&state.pool, &account.id, mime_type, &image_data)
            .await?;
    } else {
        db::queries::upsert_user_wallpaper(&state.pool, &account.id, &slot, mime_type, &image_data)
            .await?;
    }
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_wallpaper(
    state: web::Data<AppState>,
    request: HttpRequest,
    slot: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let slot = slot.into_inner();
    validate_wallpaper_slot(&slot)?;
    if slot == "login" {
        if account.role != "administrator" {
            return Err(ApiError::Forbidden);
        }
        db::queries::delete_login_wallpaper(&state.pool).await?;
    } else {
        db::queries::delete_user_wallpaper(&state.pool, &account.id, &slot).await?;
    }
    Ok(HttpResponse::NoContent().finish())
}

fn validate_wallpaper_slot(slot: &str) -> Result<(), ApiError> {
    if matches!(slot, "dashboard" | "welcome" | "loading" | "login") {
        Ok(())
    } else {
        Err(ApiError::BadRequest("wallpaper slot is invalid"))
    }
}

async fn get_avatar(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let avatar = db::queries::find_user_avatar(&state.pool, &account.id)
        .await?
        .ok_or(ApiError::NotFound("avatar image not found"))?;

    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, avatar.mime_type))
        .insert_header((header::CACHE_CONTROL, "private, no-cache"))
        .insert_header((header::ETAG, format!("\"{}\"", avatar.updated_at)))
        .body(avatar.image_data))
}

async fn update_avatar(
    state: web::Data<AppState>,
    request: HttpRequest,
    image_data: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if image_data.is_empty() || image_data.len() > MAX_AVATAR_BYTES {
        return Err(ApiError::BadRequest(
            "avatar image must be between 1 byte and 10 MB",
        ));
    }
    let mime_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .ok_or(ApiError::BadRequest("avatar image type is required"))?;
    validate_image_upload(mime_type, &image_data, "avatar")?;

    db::queries::upsert_user_avatar(&state.pool, &account.id, mime_type, &image_data).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_avatar(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::queries::delete_user_avatar(&state.pool, &account.id).await?;
    Ok(HttpResponse::NoContent().finish())
}

fn validate_image_upload(
    mime_type: &str,
    image_data: &[u8],
    image_label: &'static str,
) -> Result<(), ApiError> {
    let valid_signature = match mime_type {
        "image/jpeg" => image_data.starts_with(&[0xff, 0xd8, 0xff]),
        "image/png" => image_data.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/webp" => {
            image_data.len() >= 12
                && image_data.starts_with(b"RIFF")
                && &image_data[8..12] == b"WEBP"
        }
        "image/avif" => image_data.len() >= 12 && &image_data[4..8] == b"ftyp",
        _ => {
            return Err(ApiError::BadRequest(match image_label {
                "avatar" => "avatar image type is not supported",
                "contact photo" => "contact photo type is not supported",
                "wall" => "wall image type is not supported",
                _ => "wallpaper image type is not supported",
            }));
        }
    };
    if valid_signature {
        Ok(())
    } else {
        Err(ApiError::BadRequest(match image_label {
            "avatar" => "avatar image content does not match its type",
            "contact photo" => "contact photo content does not match its type",
            "wall" => "wall image content does not match its type",
            _ => "wallpaper image content does not match its type",
        }))
    }
}

fn task_draft_from_create(payload: CreateTaskRequest) -> Result<TaskDraft, ApiError> {
    validate_task_draft(TaskDraft {
        title: payload.title,
        description: payload.description,
        priority: payload.priority,
        due_date: payload.due_date,
        repeat_rule: payload.repeat_rule,
        repeat_interval: payload.repeat_interval,
        repeat_unit: payload.repeat_unit,
        reschedule_from: payload.reschedule_from,
        labels: payload.labels,
        subtasks: payload
            .subtasks
            .into_iter()
            .map(|subtask| TaskSubtaskDraft {
                id: subtask.id,
                title: subtask.title,
                completed: subtask.completed,
            })
            .collect(),
    })
}

fn merge_task_update(task: &Task, payload: UpdateTaskRequest) -> Result<TaskDraft, ApiError> {
    validate_task_draft(TaskDraft {
        title: payload.title.unwrap_or_else(|| task.title.clone()),
        description: payload
            .description
            .unwrap_or_else(|| task.description.clone()),
        priority: payload.priority.unwrap_or_else(|| task.priority.clone()),
        due_date: payload.due_date.unwrap_or_else(|| task.due_date.clone()),
        repeat_rule: payload
            .repeat_rule
            .unwrap_or_else(|| task.repeat_rule.clone()),
        repeat_interval: payload.repeat_interval.unwrap_or(task.repeat_interval),
        repeat_unit: payload
            .repeat_unit
            .unwrap_or_else(|| task.repeat_unit.clone()),
        reschedule_from: payload
            .reschedule_from
            .unwrap_or_else(|| task.reschedule_from.clone()),
        labels: payload.labels.unwrap_or_else(|| task.labels.clone()),
        subtasks: payload.subtasks.map_or_else(
            || {
                task.subtasks
                    .iter()
                    .map(|subtask| TaskSubtaskDraft {
                        id: Some(subtask.id.clone()),
                        title: subtask.title.clone(),
                        completed: subtask.completed,
                    })
                    .collect()
            },
            |subtasks| {
                subtasks
                    .into_iter()
                    .map(|subtask| TaskSubtaskDraft {
                        id: subtask.id,
                        title: subtask.title,
                        completed: subtask.completed,
                    })
                    .collect()
            },
        ),
    })
}

fn validate_task_draft(mut draft: TaskDraft) -> Result<TaskDraft, ApiError> {
    draft.title = validate_short_text(&draft.title, "task name is required", 180)?.to_owned();
    draft.description = draft.description.trim().to_owned();
    if draft.description.chars().count() > 4000 {
        return Err(ApiError::BadRequest(
            "task description must be 4000 characters or fewer",
        ));
    }
    if !matches!(draft.priority.as_str(), "p1" | "p2" | "p3" | "p4" | "none") {
        return Err(ApiError::BadRequest("task priority is invalid"));
    }
    if !matches!(
        draft.repeat_rule.as_str(),
        "none" | "daily" | "weekly" | "monthly" | "yearly" | "custom"
    ) {
        return Err(ApiError::BadRequest("task repeat option is invalid"));
    }
    if !(1..=365).contains(&draft.repeat_interval) {
        return Err(ApiError::BadRequest(
            "task repeat interval must be between 1 and 365",
        ));
    }
    if !matches!(
        draft.repeat_unit.as_str(),
        "days" | "weeks" | "months" | "years"
    ) {
        return Err(ApiError::BadRequest("task repeat unit is invalid"));
    }
    if !matches!(
        draft.reschedule_from.as_str(),
        "due_date" | "completion_date"
    ) {
        return Err(ApiError::BadRequest(
            "task reschedule preference is invalid",
        ));
    }
    draft.due_date = draft
        .due_date
        .and_then(|value| (!value.trim().is_empty()).then(|| value.trim().to_owned()));
    if draft
        .due_date
        .as_deref()
        .is_some_and(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err())
    {
        return Err(ApiError::BadRequest("task due date is invalid"));
    }

    let mut labels = Vec::new();
    let mut seen_labels = HashSet::new();
    for label in draft.labels {
        let label = validate_short_text(&label, "task label cannot be empty", 40)?.to_owned();
        if seen_labels.insert(label.to_ascii_lowercase()) {
            labels.push(label);
        }
    }
    if labels.len() > 12 {
        return Err(ApiError::BadRequest("a task can have at most 12 labels"));
    }
    draft.labels = labels;

    let mut subtask_ids = HashSet::new();
    for subtask in &mut draft.subtasks {
        subtask.title =
            validate_short_text(&subtask.title, "subtask name is required", 180)?.to_owned();
        if let Some(id) = &subtask.id {
            if uuid::Uuid::parse_str(id).is_err() || !subtask_ids.insert(id.clone()) {
                return Err(ApiError::BadRequest("subtask identifier is invalid"));
            }
        }
    }
    if draft.subtasks.len() > 100 {
        return Err(ApiError::BadRequest("a task can have at most 100 subtasks"));
    }

    match draft.repeat_rule.as_str() {
        "daily" => {
            draft.repeat_interval = 1;
            draft.repeat_unit = "days".to_owned();
        }
        "weekly" => {
            draft.repeat_interval = 1;
            draft.repeat_unit = "weeks".to_owned();
        }
        "monthly" => {
            draft.repeat_interval = 1;
            draft.repeat_unit = "months".to_owned();
        }
        "yearly" => {
            draft.repeat_interval = 1;
            draft.repeat_unit = "years".to_owned();
        }
        _ => {}
    }
    Ok(draft)
}

fn next_task_due_date(draft: &TaskDraft) -> Result<String, ApiError> {
    let today = chrono::Utc::now().date_naive();
    let base = if draft.reschedule_from == "due_date" {
        draft
            .due_date
            .as_deref()
            .and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .unwrap_or(today)
    } else {
        today
    };
    let next = match draft.repeat_unit.as_str() {
        "days" => base.checked_add_signed(chrono::Duration::days(draft.repeat_interval)),
        "weeks" => base.checked_add_signed(chrono::Duration::weeks(draft.repeat_interval)),
        "months" => base.checked_add_months(chrono::Months::new(
            u32::try_from(draft.repeat_interval).unwrap_or(1),
        )),
        "years" => base.checked_add_months(chrono::Months::new(
            u32::try_from(draft.repeat_interval.saturating_mul(12)).unwrap_or(12),
        )),
        _ => None,
    }
    .ok_or(ApiError::BadRequest(
        "task recurrence exceeds the supported date range",
    ))?;
    Ok(next.format("%Y-%m-%d").to_string())
}

async fn create_task(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateTaskRequest>,
) -> Result<(web::Json<Task>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let draft = task_draft_from_create(payload.into_inner())?;
    let task = db::queries::create_task(&state.pool, &account.id, &draft).await?;
    Ok((web::Json(task), StatusCode::CREATED))
}

async fn archived_tasks(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<Vec<Task>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(
        db::queries::list_archived_tasks(&state.pool, &account.id).await?,
    ))
}

async fn update_task(
    state: web::Data<AppState>,
    request: HttpRequest,
    task_id: web::Path<String>,
    payload: web::Json<UpdateTaskRequest>,
) -> Result<web::Json<Task>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let existing = db::queries::get_task(&state.pool, &account.id, &task_id)
        .await?
        .ok_or(ApiError::NotFound("task not found"))?;
    let requested_completion = payload.completed;
    let mut draft = merge_task_update(&existing, payload.into_inner())?;
    let mut completed = requested_completion.unwrap_or(existing.completed);
    let mut completed_at = existing.completed_at.clone();

    if requested_completion == Some(true) {
        if draft.repeat_rule == "none" {
            completed_at = Some(chrono::Utc::now().to_rfc3339());
        } else {
            draft.due_date = Some(next_task_due_date(&draft)?);
            completed = false;
            completed_at = None;
        }
    } else if requested_completion == Some(false) {
        completed_at = None;
    }

    db::queries::update_task(
        &state.pool,
        &account.id,
        &task_id,
        &draft,
        completed,
        completed_at.as_deref(),
    )
    .await?
    .map(web::Json)
    .ok_or(ApiError::NotFound("task not found"))
}

async fn delete_task(
    state: web::Data<AppState>,
    request: HttpRequest,
    task_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_task(&state.pool, &account.id, &task_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("task not found"))
    }
}

async fn archive_task(
    state: web::Data<AppState>,
    request: HttpRequest,
    task_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::archive_task(&state.pool, &account.id, &task_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("task not found"))
    }
}

async fn restore_task(
    state: web::Data<AppState>,
    request: HttpRequest,
    task_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::restore_task(&state.pool, &account.id, &task_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("task not found"))
    }
}

async fn create_task_attachment(
    state: web::Data<AppState>,
    request: HttpRequest,
    task_id: web::Path<String>,
    query: web::Query<TaskAttachmentQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if body.is_empty() || body.len() > MAX_TASK_ATTACHMENT_BYTES {
        return Err(ApiError::BadRequest(
            "attachment must be between 1 byte and 10 MB",
        ));
    }
    let file_name = query
        .file_name
        .split(['/', '\\'])
        .next_back()
        .unwrap_or_default();
    let file_name = validate_short_text(file_name, "attachment name is required", 255)?;
    let mime_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream");
    if mime_type.len() > 120 || mime_type.contains(['\r', '\n']) {
        return Err(ApiError::BadRequest("attachment type is invalid"));
    }
    db::queries::create_task_attachment(
        &state.pool,
        &account.id,
        &task_id,
        file_name,
        mime_type,
        &body,
    )
    .await?
    .map(|attachment| HttpResponse::Created().json(attachment))
    .ok_or(ApiError::NotFound("task not found"))
}

async fn get_task_attachment(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (task_id, attachment_id) = path.into_inner();
    let (file_name, mime_type, data) =
        db::queries::get_task_attachment(&state.pool, &account.id, &task_id, &attachment_id)
            .await?
            .ok_or(ApiError::NotFound("attachment not found"))?;
    let safe_name = file_name.replace(['"', '\r', '\n'], "_");
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime_type))
        .append_header((
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{safe_name}\""),
        ))
        .body(data))
}

async fn delete_task_attachment(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (task_id, attachment_id) = path.into_inner();
    if db::queries::delete_task_attachment(&state.pool, &account.id, &task_id, &attachment_id)
        .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("attachment not found"))
    }
}

async fn clear_completed(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<ClearCompletedResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let deleted = db::queries::clear_completed_tasks(&state.pool, &account.id).await?;
    Ok(web::Json(ClearCompletedResponse { deleted }))
}

async fn list_users(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<Vec<ManagedUser>>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    Ok(web::Json(
        db::queries::list_managed_users(&state.pool).await?,
    ))
}

async fn get_authentication_settings(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<AuthenticationConfigResponse>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let (settings, appearance) = tokio::try_join!(
        db::queries::get_authentication_settings(&state.pool),
        db::queries::get_login_appearance(&state.pool),
    )?;
    Ok(web::Json(authentication_config_response(
        &state, settings, appearance,
    )))
}

async fn update_authentication_settings(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateAuthenticationSettingsRequest>,
) -> Result<web::Json<AuthenticationConfigResponse>, ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    if !payload.password_login_enabled && state.oidc.is_none() {
        return Err(ApiError::BadRequest(
            "password login cannot be disabled unless OIDC is configured",
        ));
    }
    let settings = db::queries::update_authentication_settings(
        &state.pool,
        payload.password_login_enabled,
        payload.password_registration_enabled,
        payload.oidc_registration_enabled,
    )
    .await?;
    info!(
        actor_user_id = %administrator.id,
        password_login_enabled = settings.password_login_enabled,
        password_registration_enabled = settings.password_registration_enabled,
        oidc_registration_enabled = settings.oidc_registration_enabled,
        "administrator updated authentication policy"
    );
    let appearance = db::queries::get_login_appearance(&state.pool).await?;
    Ok(web::Json(authentication_config_response(
        &state, settings, appearance,
    )))
}

async fn update_user_role(
    state: web::Data<AppState>,
    request: HttpRequest,
    user_id: web::Path<String>,
    payload: web::Json<UpdateUserRoleRequest>,
) -> Result<web::Json<ManagedUser>, ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    if !matches!(payload.role.as_str(), "administrator" | "member") {
        return Err(ApiError::BadRequest("role must be administrator or member"));
    }
    match db::queries::update_managed_user_role(
        &state.pool,
        &administrator.id,
        &user_id,
        &payload.role,
    )
    .await?
    {
        db::queries::UserMutationOutcome::Updated(user) => {
            info!(
                actor_user_id = %administrator.id,
                target_user_id = %user.id,
                role = %user.role,
                "administrator updated account role"
            );
            Ok(web::Json(user))
        }
        outcome => Err(user_mutation_error(&outcome)),
    }
}

async fn delete_user(
    state: web::Data<AppState>,
    request: HttpRequest,
    user_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    let user_id = user_id.into_inner();
    match db::queries::delete_managed_user(&state.pool, &administrator.id, &user_id).await? {
        db::queries::UserMutationOutcome::Deleted => {
            state.youtube_downloads.purge_deleted_user(&user_id).await;
            info!(
                actor_user_id = %administrator.id,
                target_user_id = %user_id,
                "administrator deleted account"
            );
            Ok(HttpResponse::NoContent().finish())
        }
        outcome => Err(user_mutation_error(&outcome)),
    }
}

fn user_mutation_error(outcome: &db::queries::UserMutationOutcome) -> ApiError {
    match outcome {
        db::queries::UserMutationOutcome::NotFound => ApiError::NotFound("user not found"),
        db::queries::UserMutationOutcome::SelfAction => {
            ApiError::Conflict("you cannot change or remove your own administrator account")
        }
        db::queries::UserMutationOutcome::LastAdministrator => {
            ApiError::Conflict("at least one administrator must remain")
        }
        db::queries::UserMutationOutcome::Updated(_)
        | db::queries::UserMutationOutcome::Deleted => {
            ApiError::Internal("unexpected user mutation outcome")
        }
    }
}

async fn rss_reader(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<RssReaderResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::queries::apply_rss_retention(&state.pool, &account.id).await?;
    Ok(web::Json(load_rss_reader(&state, &account.id).await?))
}

async fn create_rss_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateRssSubscriptionRequest>,
) -> Result<(web::Json<RssReaderResponse>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let url = validate_rss_url(&payload.url)?;
    if db::queries::list_rss_subscriptions(&state.pool, &account.id)
        .await?
        .iter()
        .any(|subscription| subscription.url == url)
    {
        return Err(ApiError::Conflict("this RSS feed is already subscribed"));
    }
    let category = validate_rss_settings(
        &payload.category,
        payload.auto_delete_days,
        &payload.auto_delete_mode,
    )?;
    let (title, items) = fetch_rss_snapshot(&state, &url).await?;
    let parsed =
        reqwest::Url::parse(&url).map_err(|_| ApiError::BadRequest("RSS URL is invalid"))?;
    let host = parsed
        .host_str()
        .ok_or(ApiError::BadRequest("RSS URL is invalid"))?;
    let base_url = parsed.port().map_or_else(
        || format!("{}://{host}", parsed.scheme()),
        |port| format!("{}://{host}:{port}", parsed.scheme()),
    );
    db::queries::create_rss_subscription(
        &state.pool,
        &account.id,
        &RssSubscriptionDraft {
            url,
            base_url,
            title,
            category,
            auto_delete_days: payload.auto_delete_days,
            auto_delete_mode: payload.auto_delete_mode.clone(),
        },
        &items,
    )
    .await?;
    Ok((
        web::Json(load_rss_reader(&state, &account.id).await?),
        StatusCode::CREATED,
    ))
}

async fn update_rss_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    subscription_id: web::Path<String>,
    payload: web::Json<UpdateRssSubscriptionRequest>,
) -> Result<web::Json<RssReaderResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let category = validate_rss_settings(
        &payload.category,
        payload.auto_delete_days,
        &payload.auto_delete_mode,
    )?;
    db::queries::update_rss_subscription(
        &state.pool,
        &account.id,
        &subscription_id,
        &category,
        payload.auto_delete_days,
        &payload.auto_delete_mode,
    )
    .await?
    .ok_or(ApiError::NotFound("RSS subscription not found"))?;
    db::queries::apply_rss_retention(&state.pool, &account.id).await?;
    Ok(web::Json(load_rss_reader(&state, &account.id).await?))
}

async fn delete_rss_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    subscription_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_rss_subscription(&state.pool, &account.id, &subscription_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("RSS subscription not found"))
    }
}

async fn refresh_rss_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    subscription_id: web::Path<String>,
) -> Result<web::Json<RssReaderResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let subscription =
        db::queries::get_rss_subscription(&state.pool, &account.id, &subscription_id)
            .await?
            .ok_or(ApiError::NotFound("RSS subscription not found"))?;
    match fetch_rss_snapshot(&state, &subscription.url).await {
        Ok((title, items)) => {
            db::queries::refresh_rss_subscription(
                &state.pool,
                &account.id,
                &subscription.id,
                &title,
                &items,
            )
            .await?;
            db::queries::apply_rss_retention(&state.pool, &account.id).await?;
            Ok(web::Json(load_rss_reader(&state, &account.id).await?))
        }
        Err(ApiError::Integration(message)) => {
            db::queries::set_rss_refresh_error(
                &state.pool,
                &account.id,
                &subscription.id,
                &message,
            )
            .await?;
            Err(ApiError::Integration(message))
        }
        Err(error) => Err(error),
    }
}

/// Refreshes every RSS subscription whose window has elapsed, then sleeps until the next sweep.
///
/// Subscriptions are otherwise refreshed only by hand, so a reader that stays closed never
/// collects new entries.
pub fn spawn_rss_refresh_worker(state: web::Data<AppState>) {
    tokio::spawn(async move {
        loop {
            refresh_due_rss_subscriptions(&state).await;
            sleep(TokioDuration::from_secs(RSS_REFRESH_MINUTES as u64 * 60)).await;
        }
    });
}

async fn refresh_due_rss_subscriptions(state: &AppState) {
    loop {
        let due_before = (Utc::now() - ChronoDuration::minutes(RSS_REFRESH_MINUTES)).to_rfc3339();
        let subscriptions = match db::queries::list_due_rss_subscriptions(
            &state.pool,
            &due_before,
            RSS_REFRESH_BATCH_SIZE,
        )
        .await
        {
            Ok(subscriptions) => subscriptions,
            Err(error) => {
                warn!(%error, "failed to load due RSS subscriptions");
                return;
            }
        };
        if subscriptions.is_empty() {
            return;
        }
        let final_batch = subscriptions.len() < RSS_REFRESH_BATCH_SIZE;
        for subscription in subscriptions {
            match refresh_due_rss_subscription(state, &subscription, &due_before).await {
                Ok(true) => info!(subscription = %subscription.id, "RSS subscription refreshed"),
                Ok(false) => {}
                Err(error) => {
                    warn!(subscription = %subscription.id, %error, "RSS refresh failed");
                }
            }
            sleep(TokioDuration::from_secs(RSS_REFRESH_SPACING_SECONDS)).await;
        }
        if final_batch {
            return;
        }
    }
}

async fn refresh_due_rss_subscription(
    state: &AppState,
    subscription: &RssRefreshTarget,
    due_before: &str,
) -> Result<bool, ApiError> {
    if !db::queries::claim_rss_subscription_refresh(&state.pool, &subscription.id, due_before)
        .await?
    {
        return Ok(false);
    }
    match fetch_rss_snapshot(state, &subscription.url).await {
        Ok((title, items)) => {
            db::queries::refresh_rss_subscription(
                &state.pool,
                &subscription.user_id,
                &subscription.id,
                &title,
                &items,
            )
            .await?;
            db::queries::apply_rss_retention(&state.pool, &subscription.user_id).await?;
            Ok(true)
        }
        Err(ApiError::Integration(message)) => {
            db::queries::set_rss_refresh_error(
                &state.pool,
                &subscription.user_id,
                &subscription.id,
                &message,
            )
            .await?;
            Err(ApiError::Integration(message))
        }
        Err(error) => Err(error),
    }
}

async fn set_rss_item_read(
    state: web::Data<AppState>,
    request: HttpRequest,
    item_id: web::Path<String>,
    payload: web::Json<SetRssItemReadRequest>,
) -> Result<web::Json<RssItem>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::queries::set_rss_item_read(&state.pool, &account.id, &item_id, payload.read)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("RSS item not found"))
}

async fn set_rss_item_saved(
    state: web::Data<AppState>,
    request: HttpRequest,
    item_id: web::Path<String>,
    payload: web::Json<SetRssItemSavedRequest>,
) -> Result<web::Json<RssItem>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::queries::set_rss_item_saved(&state.pool, &account.id, &item_id, payload.saved)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("RSS item not found"))
}

async fn prune_rss_items(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<PruneRssRequest>,
) -> Result<web::Json<PruneRssResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    validate_rss_retention(payload.days, &payload.mode)?;
    let deleted =
        db::queries::prune_rss_items(&state.pool, &account.id, payload.days, &payload.mode).await?;
    Ok(web::Json(PruneRssResponse { deleted }))
}

async fn load_rss_reader(state: &AppState, user_id: &str) -> Result<RssReaderResponse, ApiError> {
    let (subscriptions, items) = tokio::try_join!(
        db::queries::list_rss_subscriptions(&state.pool, user_id),
        db::queries::list_rss_items(&state.pool, user_id),
    )?;
    Ok(RssReaderResponse {
        subscriptions,
        items,
    })
}

async fn fetch_rss_snapshot(
    state: &AppState,
    url: &str,
) -> Result<(String, Vec<RssItemDraft>), ApiError> {
    let snapshot = state
        .widget_integrations
        .fetch_rss_feed(url)
        .await
        .map_err(ApiError::Integration)?;
    let title = truncate_text(snapshot.title.trim(), 180, "Untitled feed");
    let items = snapshot
        .items
        .into_iter()
        .map(|item| RssItemDraft {
            external_id: truncate_text(item.external_id.trim(), 2048, &item.published_at),
            url: truncate_text(item.url.trim(), 2048, ""),
            comments_url: truncate_text(item.comments_url.trim(), 2048, ""),
            title: truncate_text(item.title.trim(), 500, "Untitled"),
            summary: truncate_text(item.summary.trim(), 10_000, ""),
            published_at: item.published_at,
        })
        .collect();
    Ok((title, items))
}

fn validate_rss_url(value: &str) -> Result<String, ApiError> {
    let value = validate_short_text(value, "RSS feed URL is required", 2048)?;
    let parsed =
        reqwest::Url::parse(value).map_err(|_| ApiError::BadRequest("RSS URL is invalid"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || parsed.username() != ""
        || parsed.password().is_some()
    {
        return Err(ApiError::BadRequest(
            "RSS feeds must use a credential-free HTTP or HTTPS URL",
        ));
    }
    Ok(parsed.to_string())
}

fn validate_rss_settings(
    category: &str,
    auto_delete_days: Option<i64>,
    mode: &str,
) -> Result<String, ApiError> {
    let category = validate_short_text(category, "RSS category is required", 40)?.to_owned();
    if let Some(days) = auto_delete_days {
        validate_rss_retention(days, mode)?;
    } else if !matches!(mode, "read" | "all") {
        return Err(ApiError::BadRequest("RSS retention mode is invalid"));
    }
    Ok(category)
}

fn validate_rss_retention(days: i64, mode: &str) -> Result<(), ApiError> {
    if !(1..=3650).contains(&days) {
        return Err(ApiError::BadRequest(
            "RSS retention must be between 1 and 3650 days",
        ));
    }
    if !matches!(mode, "read" | "all") {
        return Err(ApiError::BadRequest("RSS retention mode is invalid"));
    }
    Ok(())
}

fn truncate_text(value: &str, max: usize, fallback: &str) -> String {
    let value = if value.is_empty() { fallback } else { value };
    value.chars().take(max).collect()
}

async fn calendar_reader(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<CalendarResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(load_calendar(&state, &account.id).await?))
}

async fn create_calendar_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateCalendarSubscriptionRequest>,
) -> Result<(web::Json<CalendarResponse>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let url = validate_calendar_url(&payload.url)?;
    let color = validate_calendar_color(&payload.color)?;
    if db::queries::list_calendar_subscriptions(&state.pool, &account.id)
        .await?
        .iter()
        .any(|subscription| subscription.url == url)
    {
        return Err(ApiError::Conflict("this calendar is already subscribed"));
    }
    let snapshot = fetch_calendar_snapshot(&state, &url).await?;
    db::queries::create_calendar_subscription(
        &state.pool,
        &account.id,
        &url,
        &snapshot.name,
        &color,
        &snapshot.events,
    )
    .await?;
    Ok((
        web::Json(load_calendar(&state, &account.id).await?),
        StatusCode::CREATED,
    ))
}

async fn refresh_calendar_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    subscription_id: web::Path<String>,
) -> Result<web::Json<CalendarResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let subscription =
        db::queries::get_calendar_subscription(&state.pool, &account.id, &subscription_id)
            .await?
            .ok_or(ApiError::NotFound("calendar subscription not found"))?;
    match fetch_calendar_snapshot(&state, &subscription.url).await {
        Ok(snapshot) => {
            db::queries::refresh_calendar_subscription(
                &state.pool,
                &account.id,
                &subscription.id,
                &snapshot.name,
                &snapshot.events,
            )
            .await?;
            Ok(web::Json(load_calendar(&state, &account.id).await?))
        }
        Err(ApiError::Integration(message)) => {
            db::queries::set_calendar_refresh_error(
                &state.pool,
                &account.id,
                &subscription.id,
                &message,
            )
            .await?;
            Err(ApiError::Integration(message))
        }
        Err(error) => Err(error),
    }
}

async fn delete_calendar_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    subscription_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_calendar_subscription(&state.pool, &account.id, &subscription_id).await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("calendar subscription not found"))
    }
}

async fn load_calendar(state: &AppState, user_id: &str) -> Result<CalendarResponse, ApiError> {
    let (subscriptions, mut events, contacts) = tokio::try_join!(
        db::queries::list_calendar_subscriptions(&state.pool, user_id),
        db::queries::list_calendar_events(&state.pool, user_id),
        db::contact_queries::list_contacts(&state.pool, user_id),
    )?;
    events.extend(contact_birthday_events(
        &contacts,
        chrono::Utc::now().year(),
    ));
    events.sort_by(|left, right| {
        left.start_at
            .cmp(&right.start_at)
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
    });
    Ok(CalendarResponse {
        subscriptions,
        events,
    })
}

fn contact_birthday_events(contacts: &[Contact], center_year: i32) -> Vec<CalendarEvent> {
    contacts
        .iter()
        .filter(|contact| !contact.archived)
        .filter_map(|contact| {
            let birthday = contact.birthday.as_deref()?;
            let (month, day) = contacts::birthday_month_day(birthday)?;
            let name = contact_display_name(contact);
            Some((contact, month, day, name))
        })
        .flat_map(|(contact, month, day, name)| {
            (center_year - 1..=center_year + 3).filter_map(move |year| {
                let date = chrono::NaiveDate::from_ymd_opt(year, month, day).or_else(|| {
                    if month == 2 && day == 29 {
                        chrono::NaiveDate::from_ymd_opt(year, 2, 28)
                    } else {
                        None
                    }
                })?;
                Some(CalendarEvent {
                    id: format!("contact-birthday-{}-{year}", contact.id),
                    subscription_id: "contacts-birthdays".to_owned(),
                    calendar_name: "Birthdays".to_owned(),
                    calendar_color: "#FB7185".to_owned(),
                    title: format!("Birthday · {name}"),
                    description: "From Contacts".to_owned(),
                    location: String::new(),
                    url: String::new(),
                    start_at: date.to_string(),
                    end_at: None,
                    all_day: true,
                })
            })
        })
        .collect()
}

fn contact_display_name(contact: &Contact) -> String {
    let name = [
        contact.first_name.as_str(),
        contact.middle_name.as_str(),
        contact.last_name.as_str(),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    if name.is_empty() {
        contact.nickname.clone()
    } else {
        name
    }
}

async fn fetch_calendar_snapshot(
    state: &AppState,
    url: &str,
) -> Result<calendar::CalendarSnapshot, ApiError> {
    let bytes = state
        .widget_integrations
        .fetch_calendar_file(url)
        .await
        .map_err(ApiError::Integration)?;
    calendar::parse_calendar(&bytes).map_err(ApiError::Integration)
}

fn validate_calendar_url(value: &str) -> Result<String, ApiError> {
    let value = validate_short_text(value, "calendar URL is required", 2048)?;
    let parsed =
        reqwest::Url::parse(value).map_err(|_| ApiError::BadRequest("calendar URL is invalid"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || parsed.username() != ""
        || parsed.password().is_some()
    {
        return Err(ApiError::BadRequest(
            "calendars must use a credential-free HTTP or HTTPS URL",
        ));
    }
    Ok(parsed.to_string())
}

fn validate_calendar_color(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    let valid = value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit);
    if !valid {
        return Err(ApiError::BadRequest(
            "calendar color must be a six-digit hex value",
        ));
    }
    Ok(value.to_ascii_uppercase())
}

async fn payment_subscriptions(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<Vec<PaymentSubscription>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(
        db::queries::list_payment_subscriptions(&state.pool, &account.id).await?,
    ))
}

async fn create_payment_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<PaymentSubscriptionRequest>,
) -> Result<(web::Json<PaymentSubscription>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let validated = validate_payment_subscription(&payload)?;
    let subscription = db::queries::create_payment_subscription(
        &state.pool,
        &account.id,
        validated.service,
        validated.description,
        validated.frequency,
        validated.amount_micros,
        &validated.currency,
        &validated.first_paid_on,
    )
    .await?;
    Ok((web::Json(subscription), StatusCode::CREATED))
}

async fn update_payment_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    subscription_id: web::Path<String>,
    payload: web::Json<PaymentSubscriptionRequest>,
) -> Result<web::Json<PaymentSubscription>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let validated = validate_payment_subscription(&payload)?;
    db::queries::update_payment_subscription(
        &state.pool,
        &account.id,
        &subscription_id,
        validated.service,
        validated.description,
        validated.frequency,
        validated.amount_micros,
        &validated.currency,
        &validated.first_paid_on,
    )
    .await?
    .map(web::Json)
    .ok_or(ApiError::NotFound("payment subscription not found"))
}

async fn delete_payment_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    subscription_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_payment_subscription(&state.pool, &account.id, &subscription_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("payment subscription not found"))
    }
}

fn validate_payment_subscription(
    payload: &PaymentSubscriptionRequest,
) -> Result<ValidatedPaymentSubscription<'_>, ApiError> {
    let service = validate_short_text(&payload.service, "service is required", 120)?;
    let frequency = validate_short_text(&payload.frequency, "frequency is required", 40)?;
    let description = payload.description.trim();
    if description.chars().count() > 2_000 {
        return Err(ApiError::BadRequest(
            "description must be 2000 characters or fewer",
        ));
    }
    if !(0..=1_000_000_000_000).contains(&payload.amount_micros) {
        return Err(ApiError::BadRequest("subscription amount is invalid"));
    }
    let currency = payload.currency.trim().to_ascii_uppercase();
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(ApiError::BadRequest("currency must be a three-letter code"));
    }
    let first_paid_on = chrono::NaiveDate::parse_from_str(payload.first_paid_on.trim(), "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest("first payment date is invalid"))?
        .format("%Y-%m-%d")
        .to_string();
    Ok(ValidatedPaymentSubscription {
        service,
        description,
        frequency,
        amount_micros: payload.amount_micros,
        currency,
        first_paid_on,
    })
}

async fn journal(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<JournalResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(JournalResponse {
        nodes: db::queries::list_journal_nodes(&state.pool, &account.id).await?,
    }))
}

async fn create_journal_node(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateJournalNodeRequest>,
) -> Result<(web::Json<JournalNode>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let name = validate_journal_name(&payload.name)?;
    let parent_id =
        validate_journal_parent(&state, &account.id, payload.parent_id.as_deref()).await?;
    if db::queries::journal_sibling_name_exists(
        &state.pool,
        &account.id,
        parent_id.as_deref(),
        &name,
        None,
    )
    .await?
    {
        return Err(ApiError::Conflict(
            "a journal item with this name already exists here",
        ));
    }
    let content = validate_journal_content(&payload.content)?;
    let node = db::queries::create_journal_node(
        &state.pool,
        &account.id,
        parent_id.as_deref(),
        &name,
        &content,
    )
    .await?;
    Ok((web::Json(node), StatusCode::CREATED))
}

async fn update_journal_node(
    state: web::Data<AppState>,
    request: HttpRequest,
    node_id: web::Path<String>,
    payload: web::Json<UpdateJournalNodeRequest>,
) -> Result<web::Json<JournalNode>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let node = db::queries::get_journal_node(&state.pool, &account.id, &node_id)
        .await?
        .ok_or(ApiError::NotFound("journal item not found"))?;
    let name = payload
        .name
        .as_deref()
        .map(validate_journal_name)
        .transpose()?
        .unwrap_or_else(|| node.name.clone());
    let parent_id = match &payload.parent_id {
        Some(parent) => validate_journal_parent(&state, &account.id, parent.as_deref()).await?,
        None => node.parent_id.clone(),
    };
    if let Some(position) = payload.position
        && !(0..=100_000).contains(&position)
    {
        return Err(ApiError::BadRequest(
            "journal position must be between 0 and 100000",
        ));
    }
    if parent_id.as_deref() == Some(node.id.as_str()) {
        return Err(ApiError::BadRequest(
            "a journal document cannot contain itself",
        ));
    }
    if let Some(parent) = parent_id.as_deref()
        && db::queries::journal_move_would_cycle(&state.pool, &account.id, &node.id, parent).await?
    {
        return Err(ApiError::BadRequest(
            "a journal document cannot move inside one of its descendants",
        ));
    }
    if db::queries::journal_sibling_name_exists(
        &state.pool,
        &account.id,
        parent_id.as_deref(),
        &name,
        Some(&node.id),
    )
    .await?
    {
        return Err(ApiError::Conflict(
            "a journal item with this name already exists here",
        ));
    }
    let content = payload
        .content
        .as_deref()
        .map(validate_journal_content)
        .transpose()?
        .unwrap_or_else(|| node.content.clone());
    db::queries::update_journal_node(
        &state.pool,
        &account.id,
        &node.id,
        parent_id.as_deref(),
        &name,
        &content,
        payload.position,
    )
    .await?
    .map(web::Json)
    .ok_or(ApiError::NotFound("journal item not found"))
}

async fn delete_journal_node(
    state: web::Data<AppState>,
    request: HttpRequest,
    node_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_journal_node(&state.pool, &account.id, &node_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("journal item not found"))
    }
}

async fn validate_journal_parent(
    state: &AppState,
    user_id: &str,
    parent_id: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let Some(parent_id) = parent_id else {
        return Ok(None);
    };
    let parent = db::queries::get_journal_node(&state.pool, user_id, parent_id)
        .await?
        .ok_or(ApiError::NotFound("journal parent document not found"))?;
    Ok(Some(parent.id))
}

fn validate_journal_name(value: &str) -> Result<String, ApiError> {
    let name = validate_short_text(value, "journal name is required", 120)?;
    if matches!(name, "." | "..")
        || name.contains(['/', '\\'])
        || name.chars().any(char::is_control)
    {
        return Err(ApiError::BadRequest(
            "journal name contains invalid characters",
        ));
    }
    Ok(name.to_owned())
}

fn validate_journal_content(value: &str) -> Result<String, ApiError> {
    if value.chars().count() > 1_000_000 {
        return Err(ApiError::BadRequest(
            "journal documents must be 1000000 characters or fewer",
        ));
    }
    Ok(value.to_owned())
}

pub fn configure_api(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/api")
            .route("/health", web::get().to(health))
            .route("/setup", web::get().to(setup_status))
            .route("/setup", web::post().to(setup))
            .route("/auth/config", web::get().to(authentication_config))
            .route("/auth/oidc/config", web::get().to(oidc_config))
            .route("/auth/oidc/start", web::get().to(oidc_start))
            .route("/auth/oidc/callback", web::get().to(oidc_callback))
            .route("/auth/register", web::post().to(register))
            .route("/auth/login", web::post().to(login))
            .route("/auth/logout", web::post().to(logout))
            .route("/auth/session", web::get().to(session))
            .route(
                "/appearance/login-wallpaper",
                web::get().to(get_login_wallpaper),
            )
            .route("/dashboard", web::get().to(dashboard))
            .configure(bookmark_library::configure)
            .configure(bookmarks::configure)
            .configure(embedded_pages::configure)
            .configure(jellyfin::configure)
            .configure(network_policy::configure)
            .configure(logging::configure)
            .route("/widgets", web::post().to(create_widget))
            .route("/widgets/capabilities", web::get().to(widget_capabilities))
            .route("/widgets/layout", web::put().to(update_widget_layout))
            .route("/widgets/{widget_id}", web::put().to(update_widget_config))
            .route("/widgets/{widget_id}/data", web::get().to(widget_data))
            .route("/widgets/{widget_id}", web::delete().to(delete_widget))
            .route("/coding", web::get().to(coding))
            .route("/coding/projects", web::post().to(create_coding_project))
            .route(
                "/coding/projects/{project_id}",
                web::delete().to(delete_coding_project),
            )
            .route(
                "/coding/credential",
                web::put().to(update_coding_credential),
            )
            .route("/settings", web::put().to(update_settings))
            .route("/settings/sessions", web::get().to(list_sessions))
            .route(
                "/settings/sessions/{session_id}",
                web::delete().to(delete_session),
            )
            .route(
                "/settings/data/{scope}",
                web::delete().to(delete_user_content),
            )
            .route("/settings/appearance", web::put().to(update_appearance))
            .service(
                web::resource("/settings/avatar")
                    .app_data(web::PayloadConfig::new(MAX_AVATAR_BYTES))
                    .route(web::get().to(get_avatar))
                    .route(web::put().to(update_avatar))
                    .route(web::delete().to(delete_avatar)),
            )
            .service(
                web::resource("/settings/wallpapers/{slot}")
                    .app_data(web::PayloadConfig::new(MAX_WALLPAPER_BYTES))
                    .route(web::get().to(get_wallpaper))
                    .route(web::put().to(update_wallpaper))
                    .route(web::delete().to(delete_wallpaper)),
            )
            .route("/admin/users", web::get().to(list_users))
            .route(
                "/admin/authentication",
                web::get().to(get_authentication_settings),
            )
            .route(
                "/admin/authentication",
                web::put().to(update_authentication_settings),
            )
            .route(
                "/admin/appearance/login",
                web::put().to(update_login_appearance),
            )
            .route("/admin/users/{user_id}", web::patch().to(update_user_role))
            .route("/admin/users/{user_id}", web::delete().to(delete_user))
            .route("/tasks", web::post().to(create_task))
            .route("/tasks/archived", web::get().to(archived_tasks))
            .route("/tasks/completed", web::delete().to(clear_completed))
            .route("/tasks/{task_id}", web::patch().to(update_task))
            .route("/tasks/{task_id}", web::delete().to(delete_task))
            .route("/tasks/{task_id}/archive", web::patch().to(archive_task))
            .route("/tasks/{task_id}/restore", web::patch().to(restore_task))
            .configure(kanban::configure)
            .configure(lines::configure)
            .configure(ntfy::configure)
            .configure(walls::configure)
            .configure(podcasts::configure)
            .configure(youtube_downloads::configure)
            .route("/rss", web::get().to(rss_reader))
            .route(
                "/rss/subscriptions",
                web::post().to(create_rss_subscription),
            )
            .route(
                "/rss/subscriptions/{subscription_id}",
                web::patch().to(update_rss_subscription),
            )
            .route(
                "/rss/subscriptions/{subscription_id}",
                web::delete().to(delete_rss_subscription),
            )
            .route(
                "/rss/subscriptions/{subscription_id}/refresh",
                web::post().to(refresh_rss_subscription),
            )
            .route("/rss/items/{item_id}", web::patch().to(set_rss_item_read))
            .route(
                "/rss/items/{item_id}/read-later",
                web::put().to(set_rss_item_saved),
            )
            .route("/rss/prune", web::post().to(prune_rss_items))
            .configure(youtube_reader::configure)
            .route("/calendar", web::get().to(calendar_reader))
            .route(
                "/calendar/subscriptions",
                web::post().to(create_calendar_subscription),
            )
            .route(
                "/calendar/subscriptions/{subscription_id}/refresh",
                web::post().to(refresh_calendar_subscription),
            )
            .route(
                "/calendar/subscriptions/{subscription_id}",
                web::delete().to(delete_calendar_subscription),
            )
            .route(
                "/payment-subscriptions",
                web::get().to(payment_subscriptions),
            )
            .route(
                "/payment-subscriptions",
                web::post().to(create_payment_subscription),
            )
            .route(
                "/payment-subscriptions/{subscription_id}",
                web::put().to(update_payment_subscription),
            )
            .route(
                "/payment-subscriptions/{subscription_id}",
                web::delete().to(delete_payment_subscription),
            )
            .route("/journal", web::get().to(journal))
            .route("/journal/nodes", web::post().to(create_journal_node))
            .route(
                "/journal/nodes/{node_id}",
                web::patch().to(update_journal_node),
            )
            .route(
                "/journal/nodes/{node_id}",
                web::delete().to(delete_journal_node),
            )
            .configure(contacts::configure)
            .service(
                web::resource("/tasks/{task_id}/attachments")
                    .app_data(web::PayloadConfig::new(MAX_TASK_ATTACHMENT_BYTES))
                    .route(web::post().to(create_task_attachment)),
            )
            .route(
                "/tasks/{task_id}/attachments/{attachment_id}",
                web::get().to(get_task_attachment),
            )
            .route(
                "/tasks/{task_id}/attachments/{attachment_id}",
                web::delete().to(delete_task_attachment),
            ),
    );
}

async fn authenticated_account(
    state: &AppState,
    request: &HttpRequest,
) -> Result<SessionAccount, ApiError> {
    let token = session_token(request)?;
    let account = account_from_cookie_value(state, &token).await?;
    let metadata = session_metadata(request);
    if let Err(error) = db::queries::touch_session(
        &state.pool,
        &token,
        &metadata.user_agent,
        &metadata.ip_address,
    )
    .await
    {
        warn!(%error, "failed to refresh session metadata");
    }
    Ok(account)
}

async fn authenticated_administrator(
    state: &AppState,
    request: &HttpRequest,
) -> Result<SessionAccount, ApiError> {
    let account = authenticated_account(state, request).await?;
    if account.role != "administrator" {
        return Err(ApiError::Forbidden);
    }
    Ok(account)
}

async fn account_from_cookie_value(
    state: &AppState,
    token: &str,
) -> Result<SessionAccount, ApiError> {
    db::queries::find_session_account(&state.pool, token)
        .await?
        .ok_or(ApiError::Unauthorized)
}

async fn issue_session(
    state: &AppState,
    request: &HttpRequest,
    user_id: &str,
) -> Result<Cookie<'static>, ApiError> {
    let token = uuid::Uuid::new_v4().to_string();
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(SESSION_DAYS)).to_rfc3339();
    let metadata = session_metadata(request);
    db::queries::create_session(
        &state.pool,
        &token,
        user_id,
        &metadata.user_agent,
        &metadata.ip_address,
        &expires_at,
    )
    .await?;

    Ok(Cookie::build(SESSION_COOKIE, token)
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(state.cookie_secure)
        .max_age(CookieDuration::days(SESSION_DAYS))
        .finish())
}

struct SessionMetadata {
    user_agent: String,
    ip_address: String,
}

fn session_token(request: &HttpRequest) -> Result<String, ApiError> {
    request
        .cookie(SESSION_COOKIE)
        .map(|cookie| cookie.value().to_owned())
        .ok_or(ApiError::Unauthorized)
}

fn session_metadata(request: &HttpRequest) -> SessionMetadata {
    let user_agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(512).collect())
        .unwrap_or_else(|| "Unavailable".to_owned());
    let ip_address = request
        .connection_info()
        .realip_remote_addr()
        .and_then(normalize_ip_address)
        .or_else(|| request.peer_addr().map(|address| address.ip().to_string()))
        .unwrap_or_else(|| "Unavailable".to_owned());
    SessionMetadata {
        user_agent,
        ip_address,
    }
}

fn normalize_ip_address(value: &str) -> Option<String> {
    value
        .parse::<std::net::IpAddr>()
        .map(|address| address.to_string())
        .or_else(|_| {
            value
                .parse::<std::net::SocketAddr>()
                .map(|address| address.ip().to_string())
        })
        .ok()
}

fn session_removal_cookie(cookie_secure: bool) -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE, "")
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(cookie_secure)
        .max_age(CookieDuration::ZERO)
        .finish()
}

fn auth_response(account: SessionAccount) -> AuthResponse {
    let sidebar_timezones =
        parse_sidebar_timezones(&account.sidebar_timezones_json, &account.timezone);
    AuthResponse {
        user: User {
            id: account.id.clone(),
            email: account.email,
            role: account.role,
            created_at: account.created_at,
        },
        settings: UserSettings {
            user_id: account.id,
            display_name: account.display_name,
            location: account.location,
            timezone: account.timezone,
            sidebar_timezones,
            calendar_week_start: account.calendar_week_start,
            temperature_unit: account.temperature_unit,
            lines_default_visibility: account.lines_default_visibility,
            podcast_playback_rate: account.podcast_playback_rate,
            updated_at: account.settings_updated_at,
        },
    }
}

async fn ensure_onboarding_complete(state: &AppState) -> Result<(), ApiError> {
    if db::queries::is_onboarding_complete(&state.pool).await? {
        Ok(())
    } else {
        Err(ApiError::SetupRequired)
    }
}

fn normalize_email(value: &str) -> Result<String, ApiError> {
    let email = value.trim().to_ascii_lowercase();
    let mut parts = email.split('@');
    let valid = matches!((parts.next(), parts.next(), parts.next()), (Some(local), Some(domain), None)
        if !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.'));
    if email.len() > 254 || !valid {
        return Err(ApiError::BadRequest("enter a valid email address"));
    }
    Ok(email)
}

fn validate_password(password: &str) -> Result<(), ApiError> {
    let length = password.chars().count();
    if !(10..=128).contains(&length) {
        return Err(ApiError::BadRequest(
            "password must be between 10 and 128 characters",
        ));
    }
    Ok(())
}

fn validate_short_text<'a>(
    value: &'a str,
    empty_message: &'static str,
    max_length: usize,
) -> Result<&'a str, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::BadRequest(empty_message));
    }
    if value.chars().count() > max_length {
        return Err(ApiError::BadRequest("value is too long"));
    }
    Ok(value)
}

fn validate_sidebar_timezones(values: &[String]) -> Result<Vec<String>, ApiError> {
    if values.is_empty() || values.len() > 5 {
        return Err(ApiError::BadRequest(
            "sidebar timezones must contain between one and five entries",
        ));
    }
    let mut seen = HashSet::new();
    let mut validated = Vec::with_capacity(values.len());
    for value in values {
        let timezone = validate_short_text(value, "sidebar timezone is required", 80)?;
        if !seen.insert(timezone.to_owned()) {
            return Err(ApiError::BadRequest(
                "sidebar timezones must not contain duplicates",
            ));
        }
        validated.push(timezone.to_owned());
    }
    Ok(validated)
}

fn parse_sidebar_timezones(value: &str, fallback: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value)
        .ok()
        .filter(|timezones| !timezones.is_empty())
        .unwrap_or_else(|| vec![fallback.to_owned()])
}

fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
}

fn verify_password(password: &str, password_hash: &str) -> bool {
    PasswordHash::new(password_hash).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, cookie::Cookie, http::StatusCode, test};

    async fn test_pool() -> SqlitePool {
        db::connect("sqlite::memory:")
            .await
            .expect("database connects")
    }

    #[actix_web::test]
    async fn new_rss_subscriptions_default_to_seven_day_all_item_retention() {
        let request: CreateRssSubscriptionRequest = serde_json::from_value(serde_json::json!({
            "url": "https://example.com/feed.xml",
            "category": "General"
        }))
        .expect("request deserializes");

        assert_eq!(request.auto_delete_days, Some(7));
        assert_eq!(request.auto_delete_mode, "all");
    }

    fn state(pool: SqlitePool) -> web::Data<AppState> {
        // Each test gets its own media root so cached-file assertions cannot collide.
        let media_root =
            std::env::temp_dir().join(format!("pandan-test-media-{}", uuid::Uuid::new_v4()));
        web::Data::new(AppState {
            pool: pool.clone(),
            cookie_secure: false,
            oidc: None,
            widget_integrations: widget_integrations::WidgetIntegrationService::for_tests(
                pool.clone(),
            )
            .expect("test widget integrations initialize"),
            jellyfin: jellyfin::JellyfinService::new(pool.clone()),
            youtube_downloads: youtube_downloads::YoutubeDownloadService::for_tests(pool.clone()),
            podcast_media: podcast_media::PodcastMedia::with_root_and_pool(media_root, pool)
                .expect("test podcast media initializes"),
            ntfy_events: ntfy::NtfyEventHub::default(),
            site_origin: document::SiteOrigin::default(),
            logging: logging::LoggingController::disabled_for_tests(),
        })
    }

    fn session_cookie(response: &actix_web::dev::ServiceResponse) -> Cookie<'static> {
        response
            .response()
            .cookies()
            .find(|cookie| cookie.name() == SESSION_COOKIE)
            .expect("session cookie is set")
            .clone()
            .into_owned()
    }

    #[actix_web::test]
    async fn database_errors_keep_their_source_and_expose_a_safe_message() {
        let error = ApiError::from(sqlx::Error::RowNotFound);

        assert_eq!(error.to_string(), "database operation failed");
        assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(std::error::Error::source(&error).is_some());
    }

    #[actix_web::test]
    async fn calendar_colors_are_normalized_and_validated() {
        assert_eq!(
            validate_calendar_color(" #a1b2c3 ").expect("hex color is valid"),
            "#A1B2C3"
        );
        assert!(validate_calendar_color("teal").is_err());
        assert!(validate_calendar_color("#12345G").is_err());
        assert!(validate_calendar_color("#12345678").is_err());
    }

    #[actix_web::test]
    async fn contact_birthdays_become_annual_all_day_calendar_events() {
        let contact = Contact {
            id: "contact-1".to_owned(),
            dav_source_id: None,
            source_kind: "manual".to_owned(),
            source_reference: None,
            first_name: "Ari".to_owned(),
            middle_name: String::new(),
            last_name: "Stone".to_owned(),
            nickname: String::new(),
            pronouns: String::new(),
            company: String::new(),
            job_title: String::new(),
            birthday: Some("1992-02-29".to_owned()),
            emails: Vec::new(),
            phones: Vec::new(),
            addresses: Vec::new(),
            important_dates: Vec::new(),
            tags: Vec::new(),
            relationship_context: String::new(),
            notes: String::new(),
            favorite: false,
            archived: false,
            has_photo: false,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let events = contact_birthday_events(&[contact], 2025);

        assert_eq!(events.len(), 5);
        assert_eq!(events[1].start_at, "2025-02-28");
        assert_eq!(events[1].title, "Birthday · Ari Stone");
        assert!(events.iter().all(|event| event.all_day));
    }

    #[actix_web::test]
    async fn yearless_contact_birthdays_become_calendar_events() {
        let mut contact = Contact {
            id: "contact-2".to_owned(),
            dav_source_id: None,
            source_kind: "monica".to_owned(),
            source_reference: Some("remote-2".to_owned()),
            first_name: "Sean".to_owned(),
            middle_name: String::new(),
            last_name: "Choi".to_owned(),
            nickname: String::new(),
            pronouns: String::new(),
            company: String::new(),
            job_title: String::new(),
            birthday: Some("--08-20".to_owned()),
            emails: Vec::new(),
            phones: Vec::new(),
            addresses: Vec::new(),
            important_dates: Vec::new(),
            tags: Vec::new(),
            relationship_context: String::new(),
            notes: String::new(),
            favorite: false,
            archived: false,
            has_photo: false,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let events = contact_birthday_events(std::slice::from_ref(&contact), 2026);
        assert_eq!(events[1].start_at, "2026-08-20");
        assert_eq!(events[1].title, "Birthday · Sean Choi");

        contact.archived = true;
        assert!(contact_birthday_events(&[contact], 2026).is_empty());
    }

    #[actix_web::test]
    async fn health_reports_database_connection() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::get().uri("/api/health").to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn dashboard_requires_a_session() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::get().uri("/api/dashboard").to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn sessions_can_be_listed_and_forced_to_sign_out() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let first_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .peer_addr("192.0.2.10:4100".parse().expect("valid peer address"))
                .insert_header(("x-forwarded-for", "203.0.113.10"))
                .insert_header((header::USER_AGENT, "First Browser/1.0"))
                .set_json(RegisterRequest {
                    email: "sessions@example.com".to_owned(),
                    password: "correct horse battery staple".to_owned(),
                    display_name: "Session Owner".to_owned(),
                })
                .to_request(),
        )
        .await;
        let first_cookie = session_cookie(&first_response);

        let second_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/login")
                .peer_addr("198.51.100.8:4200".parse().expect("valid peer address"))
                .insert_header(("x-forwarded-for", "203.0.113.20"))
                .insert_header((header::USER_AGENT, "Second Browser/2.0"))
                .set_json(LoginRequest {
                    email: "sessions@example.com".to_owned(),
                    password: "correct horse battery staple".to_owned(),
                })
                .to_request(),
        )
        .await;
        let second_cookie = session_cookie(&second_response);

        let sessions: Vec<BrowserSessionResponse> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/settings/sessions")
                .peer_addr("198.51.100.8:4200".parse().expect("valid peer address"))
                .insert_header(("x-forwarded-for", "203.0.113.20"))
                .insert_header((header::USER_AGENT, "Second Browser/2.0"))
                .cookie(second_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(sessions.len(), 2);
        let current = sessions
            .iter()
            .find(|session| session.is_current)
            .expect("current session is labeled");
        assert_eq!(current.user_agent, "Second Browser/2.0");
        assert_eq!(current.ip_address, "203.0.113.20");
        let remote = sessions
            .iter()
            .find(|session| !session.is_current)
            .expect("remote session is listed");
        assert_eq!(remote.user_agent, "First Browser/1.0");
        assert_eq!(remote.ip_address, "203.0.113.10");

        let forced = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/settings/sessions/{}", remote.id))
                .cookie(second_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(forced.status(), StatusCode::NO_CONTENT);

        let rejected = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(first_cookie)
                .to_request(),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

        let current_id = current.id.clone();
        let signed_out = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/settings/sessions/{current_id}"))
                .cookie(second_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(signed_out.status(), StatusCode::NO_CONTENT);
        assert!(
            signed_out
                .response()
                .cookies()
                .any(|cookie| cookie.name() == SESSION_COOKIE)
        );

        let current_rejected = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(second_cookie)
                .to_request(),
        )
        .await;
        assert_eq!(current_rejected.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn oidc_endpoints_report_disabled_configuration_safely() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let config: OidcConfigResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/auth/oidc/config")
                .to_request(),
        )
        .await;
        assert!(!config.enabled);
        assert!(config.provider_name.is_none());

        let authentication: AuthenticationConfigResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/auth/config")
                .to_request(),
        )
        .await;
        assert!(authentication.password_login_enabled);
        assert!(authentication.password_registration_enabled);
        assert!(!authentication.oidc_enabled);
        assert!(authentication.oidc_registration_enabled);

        let start_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/auth/oidc/start")
                .to_request(),
        )
        .await;
        assert_eq!(start_response.status(), StatusCode::NOT_FOUND);

        let denied_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/auth/oidc/callback?error=access_denied")
                .to_request(),
        )
        .await;
        assert_eq!(denied_response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            denied_response
                .headers()
                .get(header::LOCATION)
                .expect("redirect location"),
            "/?auth_error=oidc_access_denied"
        );
    }

    #[actix_web::test]
    async fn administrator_controls_password_and_oidc_registration_policy() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let setup_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "admin@example.com".to_owned(),
                    password: "correct horse battery staple".to_owned(),
                    display_name: "Admin".to_owned(),
                })
                .to_request(),
        )
        .await;
        let cookie = session_cookie(&setup_response);

        let updated: AuthenticationConfigResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::put()
                .uri("/api/admin/authentication")
                .cookie(cookie.clone())
                .set_json(UpdateAuthenticationSettingsRequest {
                    password_login_enabled: true,
                    password_registration_enabled: false,
                    oidc_registration_enabled: false,
                })
                .to_request(),
        )
        .await;
        assert!(updated.password_login_enabled);
        assert!(!updated.password_registration_enabled);
        assert!(!updated.oidc_registration_enabled);

        let registration = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/register")
                .set_json(RegisterRequest {
                    email: "member@example.com".to_owned(),
                    password: "another secure password".to_owned(),
                    display_name: "Member".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(registration.status(), StatusCode::FORBIDDEN);

        let login = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/login")
                .set_json(LoginRequest {
                    email: "admin@example.com".to_owned(),
                    password: "correct horse battery staple".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(login.status(), StatusCode::OK);

        let lockout_attempt = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/admin/authentication")
                .cookie(cookie)
                .set_json(UpdateAuthenticationSettingsRequest {
                    password_login_enabled: false,
                    password_registration_enabled: false,
                    oidc_registration_enabled: false,
                })
                .to_request(),
        )
        .await;
        assert_eq!(lockout_attempt.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn network_access_rules_are_administrator_only_and_round_trip() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let setup_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "admin@example.com".to_owned(),
                    password: "correct horse battery staple".to_owned(),
                    display_name: "Admin".to_owned(),
                })
                .to_request(),
        )
        .await;
        let administrator_cookie = session_cookie(&setup_response);
        let member_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/register")
                .set_json(RegisterRequest {
                    email: "member@example.com".to_owned(),
                    password: "another secure password".to_owned(),
                    display_name: "Member".to_owned(),
                })
                .to_request(),
        )
        .await;
        let member_cookie = session_cookie(&member_response);

        let forbidden = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/admin/network-access")
                .cookie(member_cookie)
                .to_request(),
        )
        .await;
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        let created: db::entities::NetworkAccessRule = test::call_and_read_body_json(
            &app,
            test::TestRequest::post()
                .uri("/api/admin/network-access")
                .cookie(administrator_cookie.clone())
                .set_json(serde_json::json!({
                    "action": "allow",
                    "origin": "http://192.168.10.20:3000",
                    "integration": "rss"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(created.action, "allow");
        assert_eq!(created.scheme, "http");
        assert_eq!(created.host, "192.168.10.20");
        assert_eq!(created.port, 3000);
        assert_eq!(created.integration, "rss");

        let rules: Vec<db::entities::NetworkAccessRule> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/admin/network-access")
                .cookie(administrator_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(rules, vec![created.clone()]);

        let deleted = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/admin/network-access/{}", created.id))
                .cookie(administrator_cookie)
                .to_request(),
        )
        .await;
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    }

    #[actix_web::test]
    async fn onboarding_creates_the_administrator_once_and_persists_settings() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let initial_status: SetupStatusResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get().uri("/api/setup").to_request(),
        )
        .await;
        assert!(initial_status.required);

        let premature_registration = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/register")
                .set_json(RegisterRequest {
                    email: "member@example.com".to_owned(),
                    password: "a secure member password".to_owned(),
                    display_name: "Member".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(premature_registration.status(), StatusCode::CONFLICT);

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "Ada@Example.com".to_owned(),
                    password: "correct horse battery staple".to_owned(),
                    display_name: "Ada".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let cookie = session_cookie(&response);

        let dashboard: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(dashboard.user.email, "ada@example.com");
        assert_eq!(dashboard.user.role, "administrator");
        assert_eq!(dashboard.settings.display_name, "Ada");
        assert_eq!(dashboard.tasks.len(), 3);
        assert_eq!(dashboard.widgets.len(), 7);
        assert!(
            dashboard
                .widgets
                .iter()
                .all(|widget| widget.kind != "task-progress")
        );
        assert!(dashboard.widgets.iter().any(|widget| {
            widget.kind == "streams" && widget.config["placement"] == "utility_rail"
        }));

        let created_widget: DashboardWidget = test::call_and_read_body_json(
            &app,
            test::TestRequest::post()
                .uri("/api/widgets")
                .cookie(cookie.clone())
                .set_json(CreateWidgetRequest {
                    kind: "youtube".to_owned(),
                    workspace: 0,
                    size: "compact".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(created_widget.workspace, 0);
        assert_eq!(created_widget.size, "compact");
        assert_eq!(created_widget.config, serde_json::json!({}));
        assert!(!created_widget.has_secret);

        let capabilities: WidgetCapabilitiesResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/widgets/capabilities")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert!(!capabilities.secret_storage_enabled);

        let configured_widget: DashboardWidget = test::call_and_read_body_json(
            &app,
            test::TestRequest::put()
                .uri(&format!("/api/widgets/{}", created_widget.id))
                .cookie(cookie.clone())
                .set_json(UpdateWidgetConfigRequest {
                    config: serde_json::json!({
                        "channels": ["UCXuqSBlHAE6Xw-yeJA0Tunw"],
                        "include_shorts": false
                    }),
                    secret: None,
                    clear_secret: false,
                })
                .to_request(),
        )
        .await;
        assert_eq!(
            configured_widget.config["channels"][0],
            "UCXuqSBlHAE6Xw-yeJA0Tunw"
        );
        assert!(!configured_widget.has_secret);

        let updated_widgets: Vec<DashboardWidget> = test::call_and_read_body_json(
            &app,
            test::TestRequest::put()
                .uri("/api/widgets/layout")
                .cookie(cookie.clone())
                .set_json(UpdateWidgetLayoutRequest {
                    widgets: vec![WidgetLayoutItem {
                        id: created_widget.id.clone(),
                        workspace: 0,
                        position: 3,
                        size: "full".to_owned(),
                        grid_x: 0,
                        grid_y: 12,
                        grid_w: 12,
                        grid_h: 6,
                    }],
                })
                .to_request(),
        )
        .await;
        let updated_widget = updated_widgets
            .iter()
            .find(|widget| widget.id == created_widget.id)
            .expect("updated widget remains in dashboard");
        assert_eq!(updated_widget.workspace, 0);
        assert_eq!(updated_widget.size, "full");

        let deleted_widget = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/widgets/{}", created_widget.id))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(deleted_widget.status(), StatusCode::NO_CONTENT);

        let completed_status: SetupStatusResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get().uri("/api/setup").to_request(),
        )
        .await;
        assert!(!completed_status.required);

        let replay = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "second-admin@example.com".to_owned(),
                    password: "another secure password".to_owned(),
                    display_name: "Second Admin".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(replay.status(), StatusCode::CONFLICT);

        let updated: UserSettings = test::call_and_read_body_json(
            &app,
            test::TestRequest::put()
                .uri("/api/settings")
                .cookie(cookie.clone())
                .set_json(UpdateSettingsRequest {
                    display_name: "Ada Lovelace".to_owned(),
                    location: "London".to_owned(),
                    timezone: "Europe/London".to_owned(),
                    sidebar_timezones: Some(vec![
                        "Europe/London".to_owned(),
                        "Asia/Tokyo".to_owned(),
                    ]),
                    calendar_week_start: None,
                    temperature_unit: "fahrenheit".to_owned(),
                    lines_default_visibility: "public".to_owned(),
                    podcast_playback_rate: None,
                })
                .to_request(),
        )
        .await;
        assert_eq!(updated.display_name, "Ada Lovelace");
        assert_eq!(updated.temperature_unit, "fahrenheit");
        assert_eq!(updated.lines_default_visibility, "public");
        assert_eq!(updated.sidebar_timezones, ["Europe/London", "Asia/Tokyo"]);

        let preserved: UserSettings = test::call_and_read_body_json(
            &app,
            test::TestRequest::put()
                .uri("/api/settings")
                .cookie(cookie.clone())
                .set_json(UpdateSettingsRequest {
                    display_name: "Ada Lovelace".to_owned(),
                    location: "London".to_owned(),
                    timezone: "Europe/London".to_owned(),
                    sidebar_timezones: None,
                    calendar_week_start: None,
                    temperature_unit: "fahrenheit".to_owned(),
                    lines_default_visibility: "public".to_owned(),
                    podcast_playback_rate: None,
                })
                .to_request(),
        )
        .await;
        assert_eq!(preserved.sidebar_timezones, ["Europe/London", "Asia/Tokyo"]);

        let appearance: UserAppearance = test::call_and_read_body_json(
            &app,
            test::TestRequest::put()
                .uri("/api/settings/appearance")
                .cookie(cookie.clone())
                .set_json(UpdateAppearanceRequest {
                    background_blur: 6,
                    background_brightness: 84,
                    background_contrast: 116,
                    background_saturation: 64,
                })
                .to_request(),
        )
        .await;
        assert_eq!(appearance.background_blur, 6);
        assert_eq!(appearance.background_contrast, 116);

        let login_appearance: LoginAppearance = test::call_and_read_body_json(
            &app,
            test::TestRequest::put()
                .uri("/api/admin/appearance/login")
                .cookie(cookie.clone())
                .set_json(UpdateAppearanceRequest {
                    background_blur: 4,
                    background_brightness: 88,
                    background_contrast: 112,
                    background_saturation: 68,
                })
                .to_request(),
        )
        .await;
        assert_eq!(login_appearance.background_blur, 4);
        assert_eq!(login_appearance.background_saturation, 68);

        let public_appearance: AuthenticationConfigResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/auth/config")
                .to_request(),
        )
        .await;
        assert_eq!(public_appearance.login_background_blur, 4);
        assert_eq!(public_appearance.login_background_brightness, 88);
        assert_eq!(public_appearance.login_background_contrast, 112);
        assert_eq!(public_appearance.login_background_saturation, 68);

        let png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let dashboard_wallpaper_update = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/settings/wallpapers/dashboard")
                .cookie(cookie.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(png.clone())
                .to_request(),
        )
        .await;
        assert_eq!(dashboard_wallpaper_update.status(), StatusCode::NO_CONTENT);

        let dashboard_wallpaper_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/settings/wallpapers/dashboard")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(dashboard_wallpaper_response.status(), StatusCode::OK);
        assert_eq!(
            dashboard_wallpaper_response
                .headers()
                .get(header::CONTENT_TYPE),
            Some(&header::HeaderValue::from_static("image/png")),
        );
        assert_eq!(
            test::read_body(dashboard_wallpaper_response).await.as_ref(),
            png
        );

        let welcome_wallpaper_update = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/settings/wallpapers/welcome")
                .cookie(cookie.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(png.clone())
                .to_request(),
        )
        .await;
        assert_eq!(welcome_wallpaper_update.status(), StatusCode::NO_CONTENT);

        let loading_wallpaper_update = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/settings/wallpapers/loading")
                .cookie(cookie.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(png.clone())
                .to_request(),
        )
        .await;
        assert_eq!(loading_wallpaper_update.status(), StatusCode::NO_CONTENT);

        let loading_wallpaper_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/settings/wallpapers/loading")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(loading_wallpaper_response.status(), StatusCode::OK);
        assert_eq!(
            test::read_body(loading_wallpaper_response).await.as_ref(),
            png
        );

        let login_wallpaper_update = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/settings/wallpapers/login")
                .cookie(cookie.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(png.clone())
                .to_request(),
        )
        .await;
        assert_eq!(login_wallpaper_update.status(), StatusCode::NO_CONTENT);

        let public_login_wallpaper = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/appearance/login-wallpaper")
                .to_request(),
        )
        .await;
        assert_eq!(public_login_wallpaper.status(), StatusCode::OK);
        assert_eq!(
            public_login_wallpaper
                .headers()
                .get("Cross-Origin-Resource-Policy"),
            Some(&header::HeaderValue::from_static("same-origin")),
        );
        assert_eq!(test::read_body(public_login_wallpaper).await.as_ref(), png);

        let wallpaper_dashboard: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert!(wallpaper_dashboard.appearance.has_dashboard_wallpaper);
        assert!(wallpaper_dashboard.appearance.has_welcome_wallpaper);
        assert!(wallpaper_dashboard.appearance.has_loading_wallpaper);
        assert!(wallpaper_dashboard.appearance.has_login_wallpaper);

        for slot in ["dashboard", "welcome", "loading", "login"] {
            let deleted = test::call_service(
                &app,
                test::TestRequest::delete()
                    .uri(&format!("/api/settings/wallpapers/{slot}"))
                    .cookie(cookie.clone())
                    .to_request(),
            )
            .await;
            assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
        }

        let missing_login_wallpaper = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/appearance/login-wallpaper")
                .to_request(),
        )
        .await;
        assert_eq!(missing_login_wallpaper.status(), StatusCode::NOT_FOUND);

        let avatar_update = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/settings/avatar")
                .cookie(cookie.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(png.clone())
                .to_request(),
        )
        .await;
        assert_eq!(avatar_update.status(), StatusCode::NO_CONTENT);

        let avatar_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/settings/avatar")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(avatar_response.status(), StatusCode::OK);
        assert_eq!(
            avatar_response.headers().get(header::CONTENT_TYPE),
            Some(&header::HeaderValue::from_static("image/png")),
        );
        assert_eq!(test::read_body(avatar_response).await.as_ref(), png);

        let avatar_delete = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri("/api/settings/avatar")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(avatar_delete.status(), StatusCode::NO_CONTENT);

        let missing_avatar = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/settings/avatar")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(missing_avatar.status(), StatusCode::NOT_FOUND);

        let logout_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/logout")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

        let rejected = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(cookie)
                .to_request(),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

        let login_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/login")
                .set_json(LoginRequest {
                    email: "ada@example.com".to_owned(),
                    password: "correct horse battery staple".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(login_response.status(), StatusCode::OK);
        let login_cookie = session_cookie(&login_response);
        let restored: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(login_cookie)
                .to_request(),
        )
        .await;
        assert_eq!(restored.settings.display_name, "Ada Lovelace");
        assert_eq!(restored.settings.temperature_unit, "fahrenheit");
        assert_eq!(
            restored.settings.sidebar_timezones,
            ["Europe/London", "Asia/Tokyo"]
        );
    }

    #[actix_web::test]
    async fn bible_verse_widget_can_be_created_and_returns_bundled_data() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let setup = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "verse@example.com".to_owned(),
                    password: "a secure verse password".to_owned(),
                    display_name: "Verse Reader".to_owned(),
                })
                .to_request(),
        )
        .await;
        let cookie = session_cookie(&setup);

        let created_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/widgets")
                .cookie(cookie.clone())
                .set_json(CreateWidgetRequest {
                    kind: "bible-verse".to_owned(),
                    workspace: 0,
                    size: "standard".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(created_response.status(), StatusCode::CREATED);
        let widget: DashboardWidget = test::read_body_json(created_response).await;

        let data_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/widgets/{}/data", widget.id))
                .cookie(cookie)
                .to_request(),
        )
        .await;
        assert_eq!(data_response.status(), StatusCode::OK);
        let data: Value = test::read_body_json(data_response).await;
        let verse = &data["items"][0];
        assert!(verse["title"].as_str().is_some_and(|text| !text.is_empty()));
        assert!(
            verse["source"]
                .as_str()
                .is_some_and(|reference| reference.contains(':'))
        );
        assert_eq!(verse["version"], "English Revised Version");
    }

    #[actix_web::test]
    async fn rich_tasks_recur_archive_and_keep_attachments_private() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let setup = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "tasks@example.com".to_owned(),
                    password: "a secure password".to_owned(),
                    display_name: "Task Tester".to_owned(),
                })
                .to_request(),
        )
        .await;
        let cookie = session_cookie(&setup);
        let created: Task = test::call_and_read_body_json(
            &app,
            test::TestRequest::post()
                .uri("/api/tasks")
                .cookie(cookie.clone())
                .set_json(CreateTaskRequest {
                    title: "Prepare release".to_owned(),
                    description: "Collect the final notes.".to_owned(),
                    priority: "p2".to_owned(),
                    labels: vec!["release".to_owned(), "team".to_owned()],
                    subtasks: vec![TaskSubtaskRequest {
                        id: None,
                        title: "Write changelog".to_owned(),
                        completed: false,
                    }],
                    due_date: Some("2026-08-20".to_owned()),
                    repeat_rule: "daily".to_owned(),
                    repeat_interval: 1,
                    repeat_unit: "days".to_owned(),
                    reschedule_from: "due_date".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(created.priority, "p2");
        assert_eq!(created.labels, ["release", "team"]);
        assert_eq!(created.subtasks.len(), 1);

        let recurring: Task = test::call_and_read_body_json(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/tasks/{}", created.id))
                .cookie(cookie.clone())
                .set_json(UpdateTaskRequest {
                    title: None,
                    description: None,
                    completed: Some(true),
                    priority: None,
                    labels: None,
                    subtasks: None,
                    due_date: None,
                    repeat_rule: None,
                    repeat_interval: None,
                    repeat_unit: None,
                    reschedule_from: None,
                })
                .to_request(),
        )
        .await;
        assert!(!recurring.completed);
        assert_eq!(recurring.due_date.as_deref(), Some("2026-08-21"));

        let attachment_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!(
                    "/api/tasks/{}/attachments?file_name=release-notes.txt",
                    created.id
                ))
                .cookie(cookie.clone())
                .insert_header((header::CONTENT_TYPE, "text/plain"))
                .set_payload("private notes")
                .to_request(),
        )
        .await;
        assert_eq!(attachment_response.status(), StatusCode::CREATED);
        let attachment: serde_json::Value = test::read_body_json(attachment_response).await;
        let attachment_id = attachment["id"].as_str().expect("attachment id exists");

        let archive_response = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/tasks/{}/archive", created.id))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(archive_response.status(), StatusCode::NO_CONTENT);

        let dashboard: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert!(dashboard.tasks.iter().all(|task| task.id != created.id));
        assert_eq!(dashboard.archived_task_count, 1);

        let archived: Vec<Task> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/tasks/archived")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, created.id);
        assert_eq!(archived[0].attachments.len(), 1);

        let restore_response = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/tasks/{}/restore", created.id))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(restore_response.status(), StatusCode::NO_CONTENT);

        let archived_after_restore: Vec<Task> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/tasks/archived")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert!(archived_after_restore.is_empty());

        let restored_dashboard: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert!(
            restored_dashboard
                .tasks
                .iter()
                .any(|task| task.id == created.id)
        );
        assert_eq!(restored_dashboard.archived_task_count, 0);

        let denied = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/tasks/{}/attachments/{attachment_id}",
                    created.id
                ))
                .to_request(),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

        let download = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/tasks/{}/attachments/{attachment_id}",
                    created.id
                ))
                .cookie(cookie)
                .to_request(),
        )
        .await;
        assert_eq!(download.status(), StatusCode::OK);
        assert_eq!(test::read_body(download).await, "private notes");
    }

    #[actix_web::test]
    async fn users_cannot_mutate_each_others_tasks() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let first = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "first@example.com".to_owned(),
                    password: "a secure password".to_owned(),
                    display_name: "First".to_owned(),
                })
                .to_request(),
        )
        .await;
        let first_cookie = session_cookie(&first);
        let created_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/tasks")
                .cookie(first_cookie.clone())
                .set_json(CreateTaskRequest {
                    title: "Private task".to_owned(),
                    description: String::new(),
                    priority: default_task_priority(),
                    labels: Vec::new(),
                    subtasks: Vec::new(),
                    due_date: None,
                    repeat_rule: default_repeat_rule(),
                    repeat_interval: default_repeat_interval(),
                    repeat_unit: default_repeat_unit(),
                    reschedule_from: default_reschedule_from(),
                })
                .to_request(),
        )
        .await;
        let created_status = created_response.status();
        let created_body = test::read_body(created_response).await;
        assert_eq!(
            created_status,
            StatusCode::CREATED,
            "{}",
            String::from_utf8_lossy(&created_body)
        );
        let created: Task = serde_json::from_slice(&created_body).expect("task response parses");
        let archive_response = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/tasks/{}/archive", created.id))
                .cookie(first_cookie)
                .to_request(),
        )
        .await;
        assert_eq!(archive_response.status(), StatusCode::NO_CONTENT);

        let second = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/register")
                .set_json(RegisterRequest {
                    email: "second@example.com".to_owned(),
                    password: "another secure password".to_owned(),
                    display_name: "Second".to_owned(),
                })
                .to_request(),
        )
        .await;
        let second_cookie = session_cookie(&second);
        let second_dashboard: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(second_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(second_dashboard.user.role, "member");
        assert_eq!(second_dashboard.archived_task_count, 0);
        let second_archived: Vec<Task> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/tasks/archived")
                .cookie(second_cookie.clone())
                .to_request(),
        )
        .await;
        assert!(second_archived.is_empty());
        let restore_response = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/tasks/{}/restore", created.id))
                .cookie(second_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(restore_response.status(), StatusCode::NOT_FOUND);
        let response = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/tasks/{}", created.id))
                .cookie(second_cookie)
                .set_json(UpdateTaskRequest {
                    title: None,
                    description: None,
                    completed: Some(true),
                    priority: None,
                    labels: None,
                    subtasks: None,
                    due_date: None,
                    repeat_rule: None,
                    repeat_interval: None,
                    repeat_unit: None,
                    reschedule_from: None,
                })
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn administrators_can_manage_users_without_losing_the_final_administrator() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let administrator = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "admin@example.com".to_owned(),
                    password: "a secure administrator password".to_owned(),
                    display_name: "Admin".to_owned(),
                })
                .to_request(),
        )
        .await;
        let administrator_cookie = session_cookie(&administrator);
        let administrator_dashboard: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(administrator_cookie.clone())
                .to_request(),
        )
        .await;

        let member = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/register")
                .set_json(RegisterRequest {
                    email: "member@example.com".to_owned(),
                    password: "a secure member password".to_owned(),
                    display_name: "Member".to_owned(),
                })
                .to_request(),
        )
        .await;
        let member_cookie = session_cookie(&member);
        let member_dashboard: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(member_cookie.clone())
                .to_request(),
        )
        .await;

        let forbidden = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/admin/users")
                .cookie(member_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
        let forbidden_logs = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/admin/logs")
                .cookie(member_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(forbidden_logs.status(), StatusCode::FORBIDDEN);

        let administrator_logs = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/admin/logs?limit=25")
                .cookie(administrator_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(administrator_logs.status(), StatusCode::OK);

        let users: Vec<ManagedUser> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/admin/users")
                .cookie(administrator_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(users.len(), 2);
        assert!(users.iter().all(|user| user.last_login_at.is_some()));

        let self_change = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!(
                    "/api/admin/users/{}",
                    administrator_dashboard.user.id
                ))
                .cookie(administrator_cookie.clone())
                .set_json(UpdateUserRoleRequest {
                    role: "member".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(self_change.status(), StatusCode::CONFLICT);

        let promoted: ManagedUser = test::call_and_read_body_json(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/admin/users/{}", member_dashboard.user.id))
                .cookie(administrator_cookie)
                .set_json(UpdateUserRoleRequest {
                    role: "administrator".to_owned(),
                })
                .to_request(),
        )
        .await;
        assert_eq!(promoted.role, "administrator");
        assert!(promoted.last_login_at.is_some());

        let delete_previous_administrator = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!(
                    "/api/admin/users/{}",
                    administrator_dashboard.user.id
                ))
                .cookie(member_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(
            delete_previous_administrator.status(),
            StatusCode::NO_CONTENT
        );

        let delete_self = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/admin/users/{}", member_dashboard.user.id))
                .cookie(member_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(delete_self.status(), StatusCode::CONFLICT);

        let remaining: Vec<ManagedUser> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/admin/users")
                .cookie(member_cookie)
                .to_request(),
        )
        .await;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, member_dashboard.user.id);
    }

    /// The podcast trust boundary, exercised over HTTP.
    ///
    /// A member must not be able to reach any route that publishes a podcast, decides a
    /// request, evicts a cached file, or changes storage policy. Only the catalogue an
    /// administrator has approved may be subscribed to.
    #[tokio::test]
    async fn members_cannot_reach_any_podcast_administrator_route() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let administrator = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "podcast-admin@example.com".to_owned(),
                    password: "a secure administrator password".to_owned(),
                    display_name: "Podcast Admin".to_owned(),
                })
                .to_request(),
        )
        .await;
        let administrator_cookie = session_cookie(&administrator);
        let member = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/register")
                .set_json(RegisterRequest {
                    email: "podcast-member@example.com".to_owned(),
                    password: "a secure member password".to_owned(),
                    display_name: "Podcast Member".to_owned(),
                })
                .to_request(),
        )
        .await;
        let member_cookie = session_cookie(&member);

        // Every administrator-only route, refused for a member.
        for request in [
            test::TestRequest::get().uri("/api/podcasts/requests"),
            test::TestRequest::get().uri("/api/podcasts/settings"),
        ] {
            let response =
                test::call_service(&app, request.cookie(member_cookie.clone()).to_request()).await;
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "a member reached an administrator podcast route"
            );
        }
        let refused = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/podcasts")
                .cookie(member_cookie.clone())
                .set_json(serde_json::json!({ "feed_url": "https://example.com/feed.xml" }))
                .to_request(),
        )
        .await;
        assert_eq!(
            refused.status(),
            StatusCode::FORBIDDEN,
            "a member published a podcast directly"
        );
        let refused = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri("/api/podcasts/settings")
                .cookie(member_cookie.clone())
                .set_json(serde_json::json!({
                    "requests_enabled": false,
                    "member_downloads_enabled": false,
                    "max_pending_requests_per_user": 1,
                    "storage_budget_bytes": 1_073_741_824_i64,
                    "max_episode_bytes": 52_428_800_i64,
                    "default_auto_download_count": 1
                }))
                .to_request(),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);
        let refused = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri("/api/podcasts/episodes/anything/download")
                .cookie(member_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);
        let refused = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/podcasts/anything/downloads")
                .cookie(member_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(
            refused.status(),
            StatusCode::FORBIDDEN,
            "a member queued a whole show for download"
        );

        // Anonymous callers reach nothing at all.
        let anonymous = test::call_service(
            &app,
            test::TestRequest::get().uri("/api/podcasts").to_request(),
        )
        .await;
        assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

        // A member sees an empty catalogue and their own (empty) request list.
        let overview: serde_json::Value = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/podcasts")
                .cookie(member_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(overview["podcasts"].as_array().map(Vec::len), Some(0));
        assert_eq!(overview["requests"].as_array().map(Vec::len), Some(0));
        assert_eq!(
            overview["policy"]["requests_enabled"],
            serde_json::json!(true)
        );

        // An administrator reaches the review queue and the storage policy.
        let queue: Vec<serde_json::Value> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/podcasts/requests")
                .cookie(administrator_cookie.clone())
                .to_request(),
        )
        .await;
        assert!(queue.is_empty());
        let settings: serde_json::Value = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/podcasts/settings")
                .cookie(administrator_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(settings["requests_enabled"], serde_json::json!(true));
        assert_eq!(settings["storage_used_bytes"], serde_json::json!(0));

        // The bulk download route is reachable for an administrator, and still resolves
        // the show before queueing anything.
        let missing = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/podcasts/not-a-show/downloads")
                .cookie(administrator_cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        // Closing requests refuses submissions at the door, before any outbound fetch.
        let closed = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri("/api/podcasts/settings")
                .cookie(administrator_cookie.clone())
                .set_json(serde_json::json!({
                    "requests_enabled": false,
                    "member_downloads_enabled": true,
                    "max_pending_requests_per_user": 5,
                    "storage_budget_bytes": 1_073_741_824_i64,
                    "max_episode_bytes": 52_428_800_i64,
                    "default_auto_download_count": 3
                }))
                .to_request(),
        )
        .await;
        assert_eq!(closed.status(), StatusCode::OK);
        let refused = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/podcasts/requests")
                .cookie(member_cookie.clone())
                .set_json(serde_json::json!({ "feed_url": "https://example.com/feed.xml" }))
                .to_request(),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    }

    /// A malformed or private feed address is refused before anything is fetched.
    #[tokio::test]
    async fn podcast_requests_refuse_unsafe_feed_addresses() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let administrator = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/setup")
                .set_json(RegisterRequest {
                    email: "feedguard@example.com".to_owned(),
                    password: "a secure administrator password".to_owned(),
                    display_name: "Feed Guard".to_owned(),
                })
                .to_request(),
        )
        .await;
        let cookie = session_cookie(&administrator);

        for hostile in ["https://user:pass@example.com/feed.xml", "not a url", ""] {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/podcasts/requests")
                    .cookie(cookie.clone())
                    .set_json(serde_json::json!({ "feed_url": hostile }))
                    .to_request(),
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "{hostile} was not refused"
            );
        }

        let plain_http = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/podcasts/requests")
                .cookie(cookie.clone())
                .set_json(serde_json::json!({
                    "feed_url": "http://example.com/feed.xml"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(
            plain_http.status(),
            StatusCode::BAD_GATEWAY,
            "plain HTTP syntax should reach and be refused by the network policy"
        );

        // Loopback and private destinations are refused by the shared SSRF guard, which
        // reports a provider error rather than accepting the feed.
        for private in [
            "https://localhost/feed.xml",
            "https://127.0.0.1/feed.xml",
            "https://192.168.1.10/feed.xml",
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/podcasts/requests")
                    .cookie(cookie.clone())
                    .set_json(serde_json::json!({ "feed_url": private }))
                    .to_request(),
            )
            .await;
            assert_ne!(
                response.status(),
                StatusCode::OK,
                "{private} must never be accepted"
            );
        }
    }

    /// Encodes a real, decodable PNG so the thumbnail pipeline has something to work on.
    fn sample_png(width: u32, height: u32) -> Vec<u8> {
        let buffer = image::RgbImage::from_pixel(width, height, image::Rgb([28, 96, 64]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(buffer)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("sample png encodes");
        bytes.into_inner()
    }

    /// Registers an account and returns its session cookie.
    ///
    /// A macro rather than a function because the initialized test service has no
    /// nameable type without depending on `actix-http` directly.
    macro_rules! account {
        ($app:expr, $uri:expr, $email:expr, $display_name:expr) => {{
            let response = test::call_service(
                &$app,
                test::TestRequest::post()
                    .uri($uri)
                    .set_json(RegisterRequest {
                        email: $email.to_owned(),
                        password: "a sufficiently long password".to_owned(),
                        display_name: $display_name.to_owned(),
                    })
                    .to_request(),
            )
            .await;
            assert!(
                response.status().is_success(),
                "{} registers: {:?}",
                $email,
                response.status()
            );
            session_cookie(&response)
        }};
    }

    /// Submits one wall as the given account and returns it, still pending.
    macro_rules! submit_wall {
        ($app:expr, $cookie:expr, $title:expr) => {{
            let response = test::call_service(
                &$app,
                test::TestRequest::post()
                    .uri(&format!(
                        "/api/walls?title={}&tags=dark,terminal",
                        $title.replace(' ', "%20")
                    ))
                    .cookie($cookie.clone())
                    .insert_header((header::CONTENT_TYPE, "image/png"))
                    .set_payload(sample_png(24, 16))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::CREATED, "wall is submitted");
            test::read_body_json::<db::entities::Wall, _>(response).await
        }};
    }

    #[actix_web::test]
    async fn walls_run_from_submission_through_approval_to_an_applied_wallpaper() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let administrator = account!(app, "/api/setup", "admin@example.com", "Admin");
        let submitter = account!(app, "/api/auth/register", "sub@example.com", "Submitter");
        let bystander = account!(app, "/api/auth/register", "by@example.com", "Bystander");

        let wall = submit_wall!(app, submitter, "Dark Terminal");
        assert_eq!(wall.status, "pending");
        assert_eq!(wall.width, 24);
        assert_eq!(wall.height, 16);
        assert_eq!(wall.tags, vec!["dark".to_owned(), "terminal".to_owned()]);
        assert_eq!(wall.submitted_by_name, "Submitter");

        // A pending wall is invisible to everyone but its submitter and administrators.
        for (cookie, expected) in [
            (&bystander, StatusCode::NOT_FOUND),
            (&submitter, StatusCode::OK),
            (&administrator, StatusCode::OK),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/walls/{}", wall.id))
                    .cookie(cookie.clone())
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), expected);
        }

        let collection: Vec<db::entities::Wall> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/walls")
                .cookie(bystander.clone())
                .to_request(),
        )
        .await;
        assert!(
            collection.is_empty(),
            "pending walls stay out of the collection"
        );

        // Only an administrator may decide.
        let refused = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/walls/{}/approve", wall.id))
                .cookie(submitter.clone())
                .set_json(serde_json::json!({ "note": "" }))
                .to_request(),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);

        let approved: db::entities::Wall = test::call_and_read_body_json(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/walls/{}/approve", wall.id))
                .cookie(administrator.clone())
                .set_json(serde_json::json!({ "note": "Looks good" }))
                .to_request(),
        )
        .await;
        assert_eq!(approved.status, "approved");
        assert_eq!(approved.decided_by_name.as_deref(), Some("Admin"));

        let collection: Vec<db::entities::Wall> = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/walls")
                .cookie(bystander.clone())
                .to_request(),
        )
        .await;
        assert_eq!(collection.len(), 1, "approved walls reach everyone");

        // A thumbnail was generated and is smaller than the original.
        let thumbnail = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/walls/{}/thumbnail", wall.id))
                .cookie(bystander.clone())
                .to_request(),
        )
        .await;
        assert_eq!(thumbnail.status(), StatusCode::OK);
        assert_eq!(
            thumbnail
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("image/jpeg")
        );
        assert!(!test::read_body(thumbnail).await.is_empty());

        let applied = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!("/api/walls/{}/apply", wall.id))
                .cookie(bystander.clone())
                .set_json(serde_json::json!({ "slot": "welcome" }))
                .to_request(),
        )
        .await;
        assert_eq!(applied.status(), StatusCode::NO_CONTENT);

        // The wallpaper endpoint now serves the wall's own bytes.
        let served = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/settings/wallpapers/welcome")
                .cookie(bystander.clone())
                .to_request(),
        )
        .await;
        assert_eq!(served.status(), StatusCode::OK);
        assert_eq!(test::read_body(served).await.as_ref(), sample_png(24, 16));

        let dashboard: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(bystander.clone())
                .to_request(),
        )
        .await;
        assert!(
            dashboard.appearance.has_welcome_wallpaper,
            "an applied wall counts as a custom wallpaper"
        );
    }

    #[actix_web::test]
    async fn a_rejected_or_deleted_wall_releases_the_slots_that_used_it() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let administrator = account!(app, "/api/setup", "admin@example.com", "Admin");

        let first = submit_wall!(app, administrator, "First");
        let second = submit_wall!(app, administrator, "Second");
        for wall in [&first, &second] {
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&format!("/api/walls/{}/approve", wall.id))
                    .cookie(administrator.clone())
                    .set_json(serde_json::json!({ "note": "" }))
                    .to_request(),
            )
            .await;
        }

        // Upload an image first, then apply a wall over it.
        test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/settings/wallpapers/welcome")
                .cookie(administrator.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(sample_png(8, 8))
                .to_request(),
        )
        .await;
        test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!("/api/walls/{}/apply", first.id))
                .cookie(administrator.clone())
                .set_json(serde_json::json!({ "slot": "welcome" }))
                .to_request(),
        )
        .await;

        let served = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/settings/wallpapers/welcome")
                .cookie(administrator.clone())
                .to_request(),
        )
        .await;
        assert_eq!(
            test::read_body(served).await.as_ref(),
            sample_png(24, 16),
            "an applied wall wins over a previously uploaded image"
        );

        // Deleting the applied wall cascades the selection away and leaves no wallpaper,
        // because applying replaced the upload rather than shadowing it.
        test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/walls/{}", first.id))
                .cookie(administrator.clone())
                .to_request(),
        )
        .await;
        let served = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/settings/wallpapers/welcome")
                .cookie(administrator.clone())
                .to_request(),
        )
        .await;
        assert_eq!(served.status(), StatusCode::NOT_FOUND);

        // Uploading again clears an applied wall in the other direction.
        test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!("/api/walls/{}/apply", second.id))
                .cookie(administrator.clone())
                .set_json(serde_json::json!({ "slot": "welcome" }))
                .to_request(),
        )
        .await;
        test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/settings/wallpapers/welcome")
                .cookie(administrator.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(sample_png(8, 8))
                .to_request(),
        )
        .await;
        let served = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/settings/wallpapers/welcome")
                .cookie(administrator.clone())
                .to_request(),
        )
        .await;
        assert_eq!(
            test::read_body(served).await.as_ref(),
            sample_png(8, 8),
            "a fresh upload replaces the applied wall"
        );
    }

    #[actix_web::test]
    async fn the_login_wall_is_administrator_only_and_stays_a_singleton() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let administrator = account!(app, "/api/setup", "admin@example.com", "Admin");
        let member = account!(app, "/api/auth/register", "member@example.com", "Member");

        let wall = submit_wall!(app, administrator, "Login");
        test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/walls/{}/approve", wall.id))
                .cookie(administrator.clone())
                .set_json(serde_json::json!({ "note": "" }))
                .to_request(),
        )
        .await;

        let refused = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!("/api/walls/{}/apply", wall.id))
                .cookie(member.clone())
                .set_json(serde_json::json!({ "slot": "login" }))
                .to_request(),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);

        let processing_refused = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/admin/appearance/login")
                .cookie(member.clone())
                .set_json(UpdateAppearanceRequest {
                    background_blur: 12,
                    background_brightness: 90,
                    background_contrast: 110,
                    background_saturation: 80,
                })
                .to_request(),
        )
        .await;
        assert_eq!(processing_refused.status(), StatusCode::FORBIDDEN);

        test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!("/api/walls/{}/apply", wall.id))
                .cookie(administrator.clone())
                .set_json(serde_json::json!({ "slot": "login" }))
                .to_request(),
        )
        .await;

        // The login image is readable without a session.
        let public = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/appearance/login-wallpaper")
                .to_request(),
        )
        .await;
        assert_eq!(public.status(), StatusCode::OK);
        assert_eq!(test::read_body(public).await.as_ref(), sample_png(24, 16));

        // Uploading a login image afterwards leaves exactly one winner.
        test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/settings/wallpapers/login")
                .cookie(administrator.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(sample_png(8, 8))
                .to_request(),
        )
        .await;
        let public = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/appearance/login-wallpaper")
                .to_request(),
        )
        .await;
        assert_eq!(
            test::read_body(public).await.as_ref(),
            sample_png(8, 8),
            "an uploaded login image replaces the applied wall"
        );
    }

    #[actix_web::test]
    async fn a_wall_stays_editable_by_its_submitter_and_administrators_at_any_status() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let administrator = account!(app, "/api/setup", "admin@example.com", "Admin");
        let submitter = account!(app, "/api/auth/register", "sub@example.com", "Submitter");
        let bystander = account!(app, "/api/auth/register", "by@example.com", "Bystander");

        let wall = submit_wall!(app, submitter, "Draft");
        let edit = |cookie: Cookie<'static>, id: String, title: &str, tags: Vec<&str>| {
            let body = serde_json::json!({
                "title": title,
                "description": "edited description",
                "tags": tags,
            });
            test::TestRequest::patch()
                .uri(&format!("/api/walls/{id}"))
                .cookie(cookie)
                .set_json(body)
                .to_request()
        };

        // Pending: the submitter may edit.
        let updated: db::entities::Wall = test::call_and_read_body_json(
            &app,
            edit(submitter.clone(), wall.id.clone(), "Renamed", vec!["one"]),
        )
        .await;
        assert_eq!(updated.title, "Renamed");
        assert_eq!(updated.tags, vec!["one".to_owned()]);
        assert_eq!(updated.status, "pending", "editing never decides a wall");

        // Someone else never may. While pending it is not even visible to them.
        let refused = test::call_service(
            &app,
            edit(bystander.clone(), wall.id.clone(), "Hijacked", vec![]),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::NOT_FOUND);

        test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/walls/{}/approve", wall.id))
                .cookie(administrator.clone())
                .set_json(serde_json::json!({ "note": "in" }))
                .to_request(),
        )
        .await;

        // Approved: the submitter still may, and the decision survives the edit.
        let updated: db::entities::Wall = test::call_and_read_body_json(
            &app,
            edit(
                submitter.clone(),
                wall.id.clone(),
                "Renamed again",
                vec!["two", "three"],
            ),
        )
        .await;
        assert_eq!(updated.title, "Renamed again");
        assert_eq!(updated.status, "approved");
        assert_eq!(updated.decision_note, "in");
        assert_eq!(updated.decided_by_name.as_deref(), Some("Admin"));

        // An administrator may edit someone else's wall.
        let updated: db::entities::Wall = test::call_and_read_body_json(
            &app,
            edit(
                administrator.clone(),
                wall.id.clone(),
                "Curated",
                vec!["curated"],
            ),
        )
        .await;
        assert_eq!(updated.title, "Curated");
        assert_eq!(updated.tags, vec!["curated".to_owned()]);

        // An approved wall is visible to everyone, but still not editable by them.
        let refused = test::call_service(
            &app,
            edit(bystander.clone(), wall.id.clone(), "Hijacked", vec![]),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);

        // A rejected wall stays editable by its submitter too.
        let rejected = submit_wall!(app, submitter, "Second");
        test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/walls/{}/reject", rejected.id))
                .cookie(administrator.clone())
                .set_json(serde_json::json!({ "note": "too small" }))
                .to_request(),
        )
        .await;
        let updated: db::entities::Wall = test::call_and_read_body_json(
            &app,
            edit(submitter.clone(), rejected.id.clone(), "Reworked", vec![]),
        )
        .await;
        assert_eq!(updated.title, "Reworked");
        assert_eq!(updated.status, "rejected");
        assert_eq!(updated.decision_note, "too small");
    }

    #[actix_web::test]
    async fn wall_thumbnails_shrink_large_images_and_never_inflate_small_ones() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let cookie = account!(app, "/api/setup", "admin@example.com", "Admin");

        for (width, height, expected_width, expected_height) in
            [(1920_u32, 1080_u32, 640_u32, 360_u32), (64, 48, 64, 48)]
        {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(&format!("/api/walls?title=Size{width}"))
                    .cookie(cookie.clone())
                    .insert_header((header::CONTENT_TYPE, "image/png"))
                    .set_payload(sample_png(width, height))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::CREATED);
            let wall: db::entities::Wall = test::read_body_json(response).await;
            assert_eq!(wall.width, i64::from(width));
            assert_eq!(wall.height, i64::from(height));

            let thumbnail = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/walls/{}/thumbnail", wall.id))
                    .cookie(cookie.clone())
                    .to_request(),
            )
            .await;
            let bytes = test::read_body(thumbnail).await;
            let decoded = image::ImageReader::new(std::io::Cursor::new(bytes.as_ref()))
                .with_guessed_format()
                .expect("thumbnail format is readable")
                .decode()
                .expect("thumbnail decodes");
            assert_eq!(
                (decoded.width(), decoded.height()),
                (expected_width, expected_height),
                "{width}x{height} thumbnail is sized correctly"
            );
        }
    }

    #[actix_web::test]
    async fn wall_submissions_reject_images_the_server_cannot_trust() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;
        let cookie = account!(app, "/api/setup", "admin@example.com", "Admin");

        // A bare PNG signature passes the magic-byte check but cannot be decoded, so the
        // thumbnail stage is what has to reject it.
        let undecodable = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/walls?title=Broken")
                .cookie(cookie.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
                .to_request(),
        )
        .await;
        assert_eq!(undecodable.status(), StatusCode::BAD_REQUEST);

        let wrong_type = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/walls?title=Text")
                .cookie(cookie.clone())
                .insert_header((header::CONTENT_TYPE, "text/plain"))
                .set_payload(b"not an image".to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(wrong_type.status(), StatusCode::BAD_REQUEST);

        let untitled = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/walls?title=%20")
                .cookie(cookie.clone())
                .insert_header((header::CONTENT_TYPE, "image/png"))
                .set_payload(sample_png(8, 8))
                .to_request(),
        )
        .await;
        assert_eq!(untitled.status(), StatusCode::BAD_REQUEST);

        // A pending wall cannot be used as a wallpaper.
        let pending = submit_wall!(app, cookie, "Pending");
        let refused = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!("/api/walls/{}/apply", pending.id))
                .cookie(cookie.clone())
                .set_json(serde_json::json!({ "slot": "welcome" }))
                .to_request(),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::CONFLICT);
    }

    #[actix_web::test]
    async fn embedded_pages_keep_global_and_personal_scopes_separate() {
        let app = test::init_service(
            App::new()
                .app_data(state(test_pool().await))
                .configure(configure_api),
        )
        .await;

        let unauthenticated = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/embedded-pages")
                .to_request(),
        )
        .await;
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

        let administrator = account!(app, "/api/setup", "admin@example.com", "Admin");
        let member = account!(app, "/api/auth/register", "member@example.com", "Member");
        let other = account!(app, "/api/auth/register", "other@example.com", "Other");

        let personal: db::entities::EmbeddedPage = test::call_and_read_body_json(
            &app,
            test::TestRequest::post()
                .uri("/api/embedded-pages")
                .cookie(member.clone())
                .set_json(serde_json::json!({
                    "title": "My reports",
                    "description": "Private reporting console",
                    "url": "https://reports.example.com/embed"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(personal.scope, "user");

        let global: db::entities::EmbeddedPage = test::call_and_read_body_json(
            &app,
            test::TestRequest::post()
                .uri("/api/admin/embedded-pages")
                .cookie(administrator.clone())
                .set_json(serde_json::json!({
                    "title": "Status",
                    "description": "Instance status",
                    "url": "https://status.example.com/"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(global.scope, "global");

        let forbidden = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/admin/embedded-pages")
                .cookie(member.clone())
                .set_json(serde_json::json!({
                    "title": "Not global",
                    "description": "",
                    "url": "https://example.com/"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        let invalid = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/embedded-pages")
                .cookie(member.clone())
                .set_json(serde_json::json!({
                    "title": "Unsafe",
                    "description": "",
                    "url": "http://internal.example.com/"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

        let member_pages: EmbeddedPagesResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/embedded-pages")
                .cookie(member.clone())
                .to_request(),
        )
        .await;
        assert_eq!(member_pages.global, vec![global.clone()]);
        assert_eq!(member_pages.personal, vec![personal.clone()]);

        let other_pages: EmbeddedPagesResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/embedded-pages")
                .cookie(other)
                .to_request(),
        )
        .await;
        assert_eq!(other_pages.global, vec![global]);
        assert!(other_pages.personal.is_empty());

        let hidden_from_administrator = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/embedded-pages/{}", personal.id))
                .cookie(administrator.clone())
                .set_json(serde_json::json!({
                    "title": "Admin edit",
                    "description": "",
                    "url": "https://example.com/"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(hidden_from_administrator.status(), StatusCode::NOT_FOUND);

        let dashboard: DashboardResponse = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/dashboard")
                .cookie(member)
                .to_request(),
        )
        .await;
        assert_eq!(dashboard.embedded_pages.global.len(), 1);
        assert_eq!(dashboard.embedded_pages.personal.len(), 1);
    }
}
