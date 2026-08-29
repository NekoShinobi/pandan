use super::{
    ApiError, AppState, authenticated_account, authenticated_administrator,
    jellyfin_client::{
        ItemQuery, JellyfinAuth, JellyfinClient, JellyfinClientError, JellyfinItem, JellyfinItems,
        PlaybackReport, safe_forwarded_request_headers,
    },
};
use actix_web::{
    HttpRequest, HttpResponse,
    http::{
        StatusCode,
        header::{self, ContentDisposition, DispositionParam, DispositionType},
    },
    web,
};
use db::entities::{JellyfinServerSettings, JellyfinUserConnection, SessionAccount};
use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;

const QUICK_CONNECT_LIFETIME: Duration = Duration::from_secs(10 * 60);
const MAX_QUICK_ATTEMPTS: usize = 500;
const TICKS_PER_SECOND: f64 = 10_000_000.0;

#[derive(Debug, Clone)]
struct QuickConnectAttempt {
    server_id: String,
    secret: String,
    code: String,
    device_id: String,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct ConnectionContext {
    server: JellyfinServerSettings,
    connection: JellyfinUserConnection,
    auth: JellyfinAuth,
}

#[derive(Debug, Clone)]
pub struct JellyfinService {
    client: JellyfinClient,
    quick_attempts: std::sync::Arc<Mutex<HashMap<String, QuickConnectAttempt>>>,
}

#[derive(Debug, Serialize)]
struct JellyfinStatusResponse {
    configured: bool,
    server_name: Option<String>,
    connected: bool,
    jellyfin_username: Option<String>,
    last_verified_at: Option<String>,
    last_error: Option<String>,
    secret_storage_enabled: bool,
}

#[derive(Debug, Serialize)]
struct JellyfinConfigResponse {
    configured: bool,
    base_url: Option<String>,
    server_id: Option<String>,
    server_name: Option<String>,
    server_version: Option<String>,
    secret_storage_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct UpdateConfigRequest {
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct PasswordLinkRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct QuickConnectResponse {
    code: String,
    expires_in_seconds: u64,
    approved: bool,
}

#[derive(Debug, Deserialize)]
struct MusicItemsQuery {
    library_id: String,
    kind: Option<String>,
    parent_id: Option<String>,
    query: Option<String>,
    start: Option<usize>,
    limit: Option<usize>,
    sort: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LibraryQuery {
    library_id: String,
    tag: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlaybackRequest {
    library_id: String,
    item_id: String,
    position_seconds: f64,
    #[serde(default)]
    is_paused: bool,
    play_session_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct PlaybackStartResponse {
    play_session_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MusicItem {
    id: String,
    library_id: String,
    kind: String,
    name: String,
    artist: Option<String>,
    album: Option<String>,
    album_id: Option<String>,
    duration_seconds: Option<f64>,
    track_number: Option<i64>,
    disc_number: Option<i64>,
    production_year: Option<i64>,
    image_item_id: Option<String>,
    image_tag: Option<String>,
    is_favorite: bool,
    played: bool,
}

#[derive(Debug, Serialize)]
struct MusicLibrary {
    id: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct MusicHomeResponse {
    libraries: Vec<MusicLibrary>,
    recent: Vec<MusicItem>,
    albums: Vec<MusicItem>,
    artists: Vec<MusicItem>,
    playlists: Vec<MusicItem>,
}

#[derive(Debug, Serialize)]
struct MusicItemsResponse {
    items: Vec<MusicItem>,
    start: usize,
    total: usize,
}

impl JellyfinService {
    #[must_use]
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            client: JellyfinClient::new(pool),
            quick_attempts: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn connection(
        &self,
        state: &AppState,
        user_id: &str,
    ) -> Result<ConnectionContext, ApiError> {
        let server = db::jellyfin_queries::get_jellyfin_server_settings(&state.pool)
            .await?
            .ok_or(ApiError::NotFound("Jellyfin is not configured"))?;
        let connection = db::jellyfin_queries::get_jellyfin_user_connection(&state.pool, user_id)
            .await?
            .ok_or(ApiError::NotFound("Jellyfin account is not linked"))?;
        let token = state
            .widget_integrations
            .decrypt_secret(&connection.token_ciphertext)
            .map_err(|_| ApiError::Internal("stored Jellyfin credential could not be opened"))?;
        let auth = JellyfinAuth {
            token,
            device_id: connection.device_id.clone(),
        };
        Ok(ConnectionContext {
            server,
            connection,
            auth,
        })
    }

    async fn music_roots(
        &self,
        context: &ConnectionContext,
    ) -> Result<Vec<JellyfinItem>, ApiError> {
        let views = self
            .client
            .user_views(
                &context.server.base_url,
                &context.connection.jellyfin_user_id,
                &context.auth,
            )
            .await
            .map_err(client_error)?;
        Ok(views
            .items
            .into_iter()
            .filter(is_music_root)
            .collect::<Vec<_>>())
    }

    async fn authorize_item(
        &self,
        context: &ConnectionContext,
        item_id: &str,
        expected_library_id: &str,
        audio_only: bool,
    ) -> Result<JellyfinItem, ApiError> {
        validate_identifier(item_id)?;
        validate_identifier(expected_library_id)?;
        let roots = self.music_roots(context).await?;
        if !roots.iter().any(|root| root.id == expected_library_id) {
            return Err(ApiError::NotFound("Jellyfin music item not found"));
        }
        let item = self
            .client
            .item(
                &context.server.base_url,
                &context.connection.jellyfin_user_id,
                item_id,
                &context.auth,
            )
            .await
            .map_err(client_error)?;
        if audio_only && !is_audio(&item) {
            return Err(ApiError::NotFound("Jellyfin music item not found"));
        }
        if item.item_type.eq_ignore_ascii_case("Playlist") && !audio_only {
            return Ok(item);
        }
        if item.id == expected_library_id {
            return Ok(item);
        }
        let ancestors = self
            .client
            .ancestors(
                &context.server.base_url,
                &context.connection.jellyfin_user_id,
                item_id,
                &context.auth,
            )
            .await
            .map_err(client_error)?;
        if !ancestors
            .iter()
            .any(|ancestor| ancestor.id == expected_library_id)
        {
            return Err(ApiError::NotFound("Jellyfin music item not found"));
        }
        Ok(item)
    }

    async fn store_authentication(
        &self,
        state: &AppState,
        account: &SessionAccount,
        device_id: &str,
        result: super::jellyfin_client::AuthenticationResult,
    ) -> Result<(), ApiError> {
        let ciphertext = state
            .widget_integrations
            .encrypt_secret(&result.access_token)
            .map_err(|_| {
                ApiError::Conflict("encrypted credential storage must be configured first")
            })?;
        db::jellyfin_queries::upsert_jellyfin_user_connection(
            &state.pool,
            &account.id,
            &result.user.id,
            &result.user.name,
            &ciphertext,
            device_id,
        )
        .await?;
        Ok(())
    }
}

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/jellyfin")
            .route("/status", web::get().to(status))
            .route("/config", web::get().to(get_config))
            .route("/config", web::put().to(update_config))
            .route("/config", web::delete().to(delete_config))
            .route(
                "/link/quick-connect",
                web::post().to(initiate_quick_connect),
            )
            .route("/link/quick-connect", web::get().to(poll_quick_connect))
            .route("/link/password", web::post().to(link_password))
            .route("/link/verify", web::post().to(verify_link))
            .route("/link", web::delete().to(unlink))
            .route("/music/home", web::get().to(music_home))
            .route("/music/items", web::get().to(music_items))
            .route("/music/items/{item_id}", web::get().to(music_item))
            .route("/music/items/{item_id}/image", web::get().to(music_image))
            .route("/music/items/{item_id}/audio", web::get().to(music_audio))
            .route(
                "/music/items/{item_id}/download",
                web::get().to(music_download),
            )
            .route("/music/playback/start", web::post().to(playback_start))
            .route("/music/playback/progress", web::put().to(playback_progress))
            .route("/music/playback/stop", web::post().to(playback_stop)),
    );
}

async fn status(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let server = db::jellyfin_queries::get_jellyfin_server_settings(&state.pool).await?;
    let connection =
        db::jellyfin_queries::get_jellyfin_user_connection(&state.pool, &account.id).await?;
    Ok(HttpResponse::Ok().json(JellyfinStatusResponse {
        configured: server.is_some(),
        server_name: server.map(|value| value.server_name),
        connected: connection.is_some(),
        jellyfin_username: connection
            .as_ref()
            .map(|value| value.jellyfin_username.clone()),
        last_verified_at: connection
            .as_ref()
            .and_then(|value| value.last_verified_at.clone()),
        last_error: connection.and_then(|value| value.last_error),
        secret_storage_enabled: state.widget_integrations.secrets_enabled(),
    }))
}

async fn get_config(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let server = db::jellyfin_queries::get_jellyfin_server_settings(&state.pool).await?;
    Ok(HttpResponse::Ok().json(JellyfinConfigResponse {
        configured: server.is_some(),
        base_url: server.as_ref().map(|value| value.base_url.clone()),
        server_id: server.as_ref().map(|value| value.server_id.clone()),
        server_name: server.as_ref().map(|value| value.server_name.clone()),
        server_version: server.map(|value| value.server_version),
        secret_storage_enabled: state.widget_integrations.secrets_enabled(),
    }))
}

async fn update_config(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateConfigRequest>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_administrator(&state, &request).await?;
    let base_url = JellyfinClient::normalize_base_url(&payload.base_url).map_err(client_error)?;
    let device_id = format!("pandan-probe-{}", Uuid::new_v4());
    let info = state
        .jellyfin
        .client
        .public_info(&base_url, &device_id)
        .await
        .map_err(client_error)?;
    let server = db::jellyfin_queries::replace_jellyfin_server_settings(
        &state.pool,
        &base_url,
        &info.id,
        &bounded(&info.server_name, 120),
        &bounded(&info.version, 64),
        &account.id,
    )
    .await?;
    state.jellyfin.quick_attempts.lock().await.clear();
    Ok(HttpResponse::Ok().json(JellyfinConfigResponse {
        configured: true,
        base_url: Some(server.base_url),
        server_id: Some(server.server_id),
        server_name: Some(server.server_name),
        server_version: Some(server.server_version),
        secret_storage_enabled: state.widget_integrations.secrets_enabled(),
    }))
}

async fn delete_config(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    db::jellyfin_queries::delete_jellyfin_server_settings(&state.pool).await?;
    state.jellyfin.quick_attempts.lock().await.clear();
    Ok(HttpResponse::NoContent().finish())
}

async fn initiate_quick_connect(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !state.widget_integrations.secrets_enabled() {
        return Err(ApiError::Conflict(
            "encrypted credential storage must be configured first",
        ));
    }
    let server = db::jellyfin_queries::get_jellyfin_server_settings(&state.pool)
        .await?
        .ok_or(ApiError::NotFound("Jellyfin is not configured"))?;
    let device_id = format!("pandan-{}", Uuid::new_v4());
    let result = state
        .jellyfin
        .client
        .initiate_quick_connect(&server.base_url, &device_id)
        .await
        .map_err(client_error)?;
    let attempt = QuickConnectAttempt {
        server_id: server.server_id,
        secret: result.secret,
        code: result.code.clone(),
        device_id,
        expires_at: Instant::now() + QUICK_CONNECT_LIFETIME,
    };
    let mut attempts = state.jellyfin.quick_attempts.lock().await;
    attempts.retain(|_, value| value.expires_at > Instant::now());
    if attempts.len() >= MAX_QUICK_ATTEMPTS && !attempts.contains_key(&account.id) {
        return Err(ApiError::Conflict(
            "too many Jellyfin links are pending; try again shortly",
        ));
    }
    attempts.insert(account.id, attempt);
    Ok(HttpResponse::Ok().json(QuickConnectResponse {
        code: result.code,
        expires_in_seconds: QUICK_CONNECT_LIFETIME.as_secs(),
        approved: false,
    }))
}

async fn poll_quick_connect(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let attempt = {
        let mut attempts = state.jellyfin.quick_attempts.lock().await;
        attempts.retain(|_, value| value.expires_at > Instant::now());
        attempts
            .get(&account.id)
            .cloned()
            .ok_or(ApiError::NotFound("Jellyfin link attempt expired"))?
    };
    let server = db::jellyfin_queries::get_jellyfin_server_settings(&state.pool)
        .await?
        .ok_or(ApiError::NotFound("Jellyfin is not configured"))?;
    if server.server_id != attempt.server_id {
        state
            .jellyfin
            .quick_attempts
            .lock()
            .await
            .remove(&account.id);
        return Err(ApiError::NotFound("Jellyfin link attempt expired"));
    }
    let result = state
        .jellyfin
        .client
        .quick_connect_status(&server.base_url, &attempt.secret, &attempt.device_id)
        .await
        .map_err(client_error)?;
    if !result.authenticated {
        return Ok(HttpResponse::Ok().json(QuickConnectResponse {
            code: attempt.code,
            expires_in_seconds: attempt
                .expires_at
                .saturating_duration_since(Instant::now())
                .as_secs(),
            approved: false,
        }));
    }
    let authentication = state
        .jellyfin
        .client
        .authenticate_quick_connect(&server.base_url, &attempt.secret, &attempt.device_id)
        .await
        .map_err(client_error)?;
    state
        .jellyfin
        .store_authentication(&state, &account, &attempt.device_id, authentication)
        .await?;
    state
        .jellyfin
        .quick_attempts
        .lock()
        .await
        .remove(&account.id);
    Ok(HttpResponse::Ok().json(QuickConnectResponse {
        code: attempt.code,
        expires_in_seconds: 0,
        approved: true,
    }))
}

async fn link_password(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<PasswordLinkRequest>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !state.widget_integrations.secrets_enabled() {
        return Err(ApiError::Conflict(
            "encrypted credential storage must be configured first",
        ));
    }
    let username = validate_text(&payload.username, 120, "Jellyfin username is required")?;
    if payload.password.chars().count() > 1_000 {
        return Err(ApiError::BadRequest("Jellyfin password is too long"));
    }
    let server = db::jellyfin_queries::get_jellyfin_server_settings(&state.pool)
        .await?
        .ok_or(ApiError::NotFound("Jellyfin is not configured"))?;
    let device_id = format!("pandan-{}", Uuid::new_v4());
    let authentication = state
        .jellyfin
        .client
        .authenticate_password(&server.base_url, username, &payload.password, &device_id)
        .await
        .map_err(client_error)?;
    state
        .jellyfin
        .store_authentication(&state, &account, &device_id, authentication)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn verify_link(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let context = state.jellyfin.connection(&state, &account.id).await?;
    match state
        .jellyfin
        .client
        .me(&context.server.base_url, &context.auth)
        .await
    {
        Ok(user) => {
            db::jellyfin_queries::mark_jellyfin_connection_verified(
                &state.pool,
                &account.id,
                &bounded(&user.name, 120),
            )
            .await?;
            Ok(HttpResponse::NoContent().finish())
        }
        Err(error) => {
            let safe = bounded(&error.to_string(), 500);
            db::jellyfin_queries::set_jellyfin_connection_error(&state.pool, &account.id, &safe)
                .await?;
            Err(client_error(error))
        }
    }
}

async fn unlink(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if let Ok(context) = state.jellyfin.connection(&state, &account.id).await {
        if let Err(error) = state
            .jellyfin
            .client
            .logout(&context.server.base_url, &context.auth)
            .await
        {
            tracing::warn!(
                user_id = %account.id,
                %error,
                "Jellyfin logout failed during unlink; removing the local connection"
            );
        }
    }
    db::jellyfin_queries::delete_jellyfin_user_connection(&state.pool, &account.id).await?;
    state
        .jellyfin
        .quick_attempts
        .lock()
        .await
        .remove(&account.id);
    Ok(HttpResponse::NoContent().finish())
}

async fn music_home(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let context = state.jellyfin.connection(&state, &account.id).await?;
    let roots = state.jellyfin.music_roots(&context).await?;
    let libraries = roots
        .iter()
        .map(|root| MusicLibrary {
            id: root.id.clone(),
            name: root.name.clone(),
        })
        .collect::<Vec<_>>();
    let mut recent = Vec::new();
    let mut albums = Vec::new();
    let mut artists = Vec::new();
    let mut playlists = Vec::new();
    let mut successful_queries = 0;
    let mut first_error = None;
    for root in roots.iter().take(12) {
        let recent_query = item_query(root.id.clone(), "Audio", Some("Audio"), 12, "DateCreated");
        let album_query = item_query(root.id.clone(), "MusicAlbum", None, 12, "SortName");
        let artist_query = item_query(root.id.clone(), "MusicArtist", None, 12, "SortName");
        let playlist_query = item_query(root.id.clone(), "Playlist", None, 12, "SortName");
        let (recent_result, album_result, artist_result, playlist_result) = tokio::join!(
            state.jellyfin.client.items(
                &context.server.base_url,
                &context.connection.jellyfin_user_id,
                &context.auth,
                &recent_query,
            ),
            state.jellyfin.client.items(
                &context.server.base_url,
                &context.connection.jellyfin_user_id,
                &context.auth,
                &album_query,
            ),
            state.jellyfin.client.items(
                &context.server.base_url,
                &context.connection.jellyfin_user_id,
                &context.auth,
                &artist_query,
            ),
            state.jellyfin.client.items(
                &context.server.base_url,
                &context.connection.jellyfin_user_id,
                &context.auth,
                &playlist_query,
            )
        );
        collect_music_home_group(
            &mut recent,
            recent_result,
            &root.id,
            "Audio",
            true,
            "recent tracks",
            &mut successful_queries,
            &mut first_error,
        );
        collect_music_home_group(
            &mut albums,
            album_result,
            &root.id,
            "MusicAlbum",
            false,
            "albums",
            &mut successful_queries,
            &mut first_error,
        );
        collect_music_home_group(
            &mut artists,
            artist_result,
            &root.id,
            "MusicArtist",
            false,
            "artists",
            &mut successful_queries,
            &mut first_error,
        );
        collect_music_home_group(
            &mut playlists,
            playlist_result,
            &root.id,
            "Playlist",
            false,
            "playlists",
            &mut successful_queries,
            &mut first_error,
        );
    }
    if !libraries.is_empty()
        && successful_queries == 0
        && let Some(error) = first_error
    {
        return Err(client_error(error));
    }
    recent.truncate(24);
    albums.truncate(24);
    artists.truncate(24);
    playlists.truncate(24);
    Ok(HttpResponse::Ok().json(MusicHomeResponse {
        libraries,
        recent,
        albums,
        artists,
        playlists,
    }))
}

async fn music_items(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<MusicItemsQuery>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let context = state.jellyfin.connection(&state, &account.id).await?;
    validate_identifier(&query.library_id)?;
    let roots = state.jellyfin.music_roots(&context).await?;
    if !roots.iter().any(|root| root.id == query.library_id) {
        return Err(ApiError::NotFound("Jellyfin music library not found"));
    }
    let kind = query.kind.as_deref().unwrap_or("tracks");
    let (include_types, media_types) = match kind {
        "tracks" => ("Audio", Some("Audio")),
        "albums" => ("MusicAlbum", None),
        "artists" => ("MusicArtist", None),
        "playlists" => ("Playlist", None),
        _ => return Err(ApiError::BadRequest("unsupported Jellyfin music item kind")),
    };
    let start = query.start.unwrap_or(0);
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let search = query
        .query
        .as_deref()
        .map(|value| validate_text(value, 160, "search query is required"))
        .transpose()?
        .map(str::to_owned);
    let parent_id = validated_parent_id(&query)?;
    if parent_id != query.library_id {
        state
            .jellyfin
            .authorize_item(&context, &parent_id, &query.library_id, false)
            .await?;
    }
    let (sort_by, sort_order) = music_sort(query.sort.as_deref())?;
    let upstream = state
        .jellyfin
        .client
        .items(
            &context.server.base_url,
            &context.connection.jellyfin_user_id,
            &context.auth,
            &ItemQuery {
                parent_id: Some(parent_id.clone()),
                include_item_types: include_types.to_owned(),
                media_types: media_types.map(str::to_owned),
                search_term: search,
                start_index: start,
                limit,
                // Music libraries commonly nest albums below artist or folder levels.
                // Keep every collection query inside the authorized parent while searching
                // all of its descendants; tracks are still independently re-authorized below.
                recursive: true,
                sort_by: sort_by.to_owned(),
                sort_order: sort_order.to_owned(),
                ids: None,
            },
        )
        .await
        .map_err(client_error)?;
    let items = if kind == "tracks" {
        let client = state.jellyfin.clone();
        let context = context.clone();
        let library_id = query.library_id.clone();
        stream::iter(upstream.items)
            .map(move |item| {
                let client = client.clone();
                let context = context.clone();
                let library_id = library_id.clone();
                async move {
                    if !is_audio(&item) {
                        return None;
                    }
                    client
                        .authorize_item(&context, &item.id, &library_id, true)
                        .await
                        .ok()
                        .map(|authorized| to_music_item(authorized, &library_id))
                }
            })
            // Authorization is concurrent, but output order must remain the
            // order Jellyfin returned (especially disc/track order for albums).
            .buffered(8)
            .filter_map(|item| async move { item })
            .collect::<Vec<_>>()
            .await
    } else {
        upstream
            .items
            .into_iter()
            .map(|item| to_music_item(item, &query.library_id))
            .collect()
    };
    Ok(HttpResponse::Ok().json(MusicItemsResponse {
        items,
        start,
        total: usize::try_from(upstream.total_record_count.max(0)).unwrap_or(usize::MAX),
    }))
}

async fn music_item(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
    query: web::Query<LibraryQuery>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let context = state.jellyfin.connection(&state, &account.id).await?;
    let item = state
        .jellyfin
        .authorize_item(&context, &path, &query.library_id, false)
        .await?;
    Ok(HttpResponse::Ok().json(to_music_item(item, &query.library_id)))
}

async fn music_image(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
    query: web::Query<LibraryQuery>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let context = state.jellyfin.connection(&state, &account.id).await?;
    state
        .jellyfin
        .authorize_item(&context, &path, &query.library_id, false)
        .await?;
    let (headers, bytes) = state
        .jellyfin
        .client
        .image(
            &context.server.base_url,
            &path,
            query.tag.as_deref(),
            &context.auth,
        )
        .await
        .map_err(client_error)?;
    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with("image/"))
        .unwrap_or("image/jpeg");
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, content_type))
        .insert_header((header::CACHE_CONTROL, "private, max-age=3600"))
        .body(bytes))
}

async fn music_audio(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
    query: web::Query<LibraryQuery>,
) -> Result<HttpResponse, ApiError> {
    serve_music_audio(state, request, path.into_inner(), query.into_inner(), false).await
}

/// Downloads one track through the same live, authorized music proxy as playback.
async fn music_download(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
    query: web::Query<LibraryQuery>,
) -> Result<HttpResponse, ApiError> {
    serve_music_audio(state, request, path.into_inner(), query.into_inner(), true).await
}

async fn serve_music_audio(
    state: web::Data<AppState>,
    request: HttpRequest,
    item_id: String,
    query: LibraryQuery,
    attachment: bool,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let context = state.jellyfin.connection(&state, &account.id).await?;
    let item = state
        .jellyfin
        .authorize_item(&context, &item_id, &query.library_id, true)
        .await?;
    let response = state
        .jellyfin
        .client
        .audio(
            &context.server.base_url,
            &context.connection.jellyfin_user_id,
            &item_id,
            &context.auth,
            safe_forwarded_request_headers(&request),
        )
        .await
        .map_err(client_error)?;
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|_| ApiError::Integration("Jellyfin returned an invalid status".to_owned()))?;
    let mut downstream = HttpResponse::build(status);
    for name in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
        "etag",
        "last-modified",
    ] {
        if let Some(value) = response.headers().get(name)
            && let Ok(value) = value.to_str()
        {
            downstream.append_header((name, value));
        }
    }
    downstream.insert_header((header::CACHE_CONTROL, "private, no-store"));
    downstream.insert_header(ContentDisposition {
        disposition: if attachment {
            DispositionType::Attachment
        } else {
            DispositionType::Inline
        },
        parameters: attachment
            .then(|| {
                DispositionParam::Filename(media_attachment_name(
                    &item.name,
                    "mp3",
                    "jellyfin-track",
                ))
            })
            .into_iter()
            .collect(),
    });
    let body = response
        .bytes_stream()
        .map(|chunk| chunk.map_err(actix_web::error::ErrorBadGateway));
    Ok(downstream.streaming(body))
}

fn media_attachment_name(title: &str, extension: &str, fallback: &str) -> String {
    let mut stem = title
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    stem = stem.trim_matches([' ', '_']).trim().to_owned();
    stem.truncate(120);
    if stem.is_empty() {
        stem = fallback.to_owned();
    }
    format!("{stem}.{extension}")
}

async fn playback_start(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<PlaybackRequest>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let context = state.jellyfin.connection(&state, &account.id).await?;
    state
        .jellyfin
        .authorize_item(&context, &payload.item_id, &payload.library_id, true)
        .await?;
    let play_session_id = Uuid::new_v4().to_string();
    let report = playback_report(&payload, play_session_id.clone())?;
    state
        .jellyfin
        .client
        .report_playback(
            &context.server.base_url,
            &["Sessions", "Playing"],
            &context.auth,
            &report,
        )
        .await
        .map_err(client_error)?;
    Ok(HttpResponse::Ok().json(PlaybackStartResponse { play_session_id }))
}

async fn playback_progress(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<PlaybackRequest>,
) -> Result<HttpResponse, ApiError> {
    playback_update(
        &state,
        &request,
        &payload,
        &["Sessions", "Playing", "Progress"],
    )
    .await
}

async fn playback_stop(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<PlaybackRequest>,
) -> Result<HttpResponse, ApiError> {
    playback_update(
        &state,
        &request,
        &payload,
        &["Sessions", "Playing", "Stopped"],
    )
    .await
}

async fn playback_update(
    state: &web::Data<AppState>,
    request: &HttpRequest,
    payload: &PlaybackRequest,
    path: &[&str],
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(state, request).await?;
    let context = state.jellyfin.connection(state, &account.id).await?;
    state
        .jellyfin
        .authorize_item(&context, &payload.item_id, &payload.library_id, true)
        .await?;
    let play_session_id = payload
        .play_session_id
        .as_deref()
        .map(validate_identifier)
        .transpose()?
        .ok_or(ApiError::BadRequest("Jellyfin play session is required"))?
        .to_owned();
    let report = playback_report(payload, play_session_id)?;
    state
        .jellyfin
        .client
        .report_playback(&context.server.base_url, path, &context.auth, &report)
        .await
        .map_err(client_error)?;
    Ok(HttpResponse::NoContent().finish())
}

fn playback_report(
    payload: &PlaybackRequest,
    play_session_id: String,
) -> Result<PlaybackReport, ApiError> {
    validate_identifier(&payload.item_id)?;
    validate_identifier(&payload.library_id)?;
    if !payload.position_seconds.is_finite()
        || !(0.0..=60.0 * 60.0 * 24.0 * 30.0).contains(&payload.position_seconds)
    {
        return Err(ApiError::BadRequest("invalid playback position"));
    }
    let ticks = i64::try_from(Duration::from_secs_f64(payload.position_seconds).as_nanos() / 100)
        .map_err(|_| ApiError::BadRequest("invalid playback position"))?;
    Ok(PlaybackReport {
        item_id: payload.item_id.clone(),
        play_session_id,
        position_ticks: ticks,
        is_paused: payload.is_paused,
        can_seek: true,
        play_method: "Transcode",
    })
}

fn item_query(
    parent_id: String,
    include_item_types: &str,
    media_types: Option<&str>,
    limit: usize,
    sort_by: &str,
) -> ItemQuery {
    ItemQuery {
        parent_id: Some(parent_id),
        include_item_types: include_item_types.to_owned(),
        media_types: media_types.map(str::to_owned),
        search_term: None,
        start_index: 0,
        limit,
        recursive: true,
        sort_by: sort_by.to_owned(),
        sort_order: if sort_by == "DateCreated" {
            "Descending".to_owned()
        } else {
            "Ascending".to_owned()
        },
        ids: None,
    }
}

fn music_sort(value: Option<&str>) -> Result<(&'static str, &'static str), ApiError> {
    match value.unwrap_or("name") {
        "name" => Ok(("SortName", "Ascending")),
        "newest" => Ok(("DateCreated", "Descending")),
        "year" => Ok(("ProductionYear,SortName", "Descending")),
        "track" => Ok(("ParentIndexNumber,IndexNumber,SortName", "Ascending")),
        _ => Err(ApiError::BadRequest("unsupported Jellyfin music sort")),
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_music_home_group(
    target: &mut Vec<MusicItem>,
    result: Result<JellyfinItems, JellyfinClientError>,
    library_id: &str,
    expected_type: &str,
    audio_only: bool,
    group: &'static str,
    successful_queries: &mut usize,
    first_error: &mut Option<JellyfinClientError>,
) {
    match result {
        Ok(response) => {
            *successful_queries += 1;
            target.extend(
                response
                    .items
                    .into_iter()
                    .filter(|item| {
                        if audio_only {
                            is_audio(item)
                        } else {
                            item.item_type.eq_ignore_ascii_case(expected_type)
                        }
                    })
                    .map(|item| to_music_item(item, library_id)),
            );
        }
        Err(error) => {
            tracing::warn!(
                jellyfin_group = group,
                error = %error,
                "Jellyfin music home group could not be loaded"
            );
            first_error.get_or_insert(error);
        }
    }
}

fn validated_parent_id(query: &MusicItemsQuery) -> Result<String, ApiError> {
    query.parent_id.as_deref().map_or_else(
        || Ok(query.library_id.clone()),
        |value| validate_identifier(value).map(str::to_owned),
    )
}

fn is_music_root(item: &JellyfinItem) -> bool {
    item.item_type.eq_ignore_ascii_case("CollectionFolder")
        && item
            .collection_type
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("music"))
}

fn is_audio(item: &JellyfinItem) -> bool {
    item.item_type.eq_ignore_ascii_case("Audio")
        && item
            .media_type
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("Audio"))
}

fn to_music_item(item: JellyfinItem, library_id: &str) -> MusicItem {
    let self_image = item.image_tags.get("Primary").cloned();
    let (image_item_id, image_tag) = if self_image.is_some() {
        (Some(item.id.clone()), self_image)
    } else if item.album_primary_image_tag.is_some() {
        (item.album_id.clone(), item.album_primary_image_tag.clone())
    } else {
        (None, None)
    };
    MusicItem {
        id: item.id,
        library_id: library_id.to_owned(),
        kind: item.item_type,
        name: item.name,
        artist: (!item.artists.is_empty()).then(|| item.artists.join(", ")),
        album: item.album,
        album_id: item.album_id,
        duration_seconds: item.run_time_ticks.and_then(ticks_to_seconds),
        track_number: item.index_number,
        disc_number: item.parent_index_number,
        production_year: item.production_year,
        image_item_id,
        image_tag,
        is_favorite: item.user_data.is_favorite,
        played: item.user_data.played,
    }
}

fn ticks_to_seconds(ticks: i64) -> Option<f64> {
    let ticks = u64::try_from(ticks).ok()?;
    let whole_seconds = ticks / 10_000_000;
    let subsecond_ticks = u32::try_from(ticks % 10_000_000).ok()?;
    Some(
        Duration::from_secs(whole_seconds).as_secs_f64()
            + f64::from(subsecond_ticks) / TICKS_PER_SECOND,
    )
}

fn validate_identifier(value: &str) -> Result<&str, ApiError> {
    if value.is_empty() || value.chars().count() > 128 || value.chars().any(char::is_control) {
        return Err(ApiError::BadRequest("invalid Jellyfin identifier"));
    }
    Ok(value)
}

fn validate_text<'a>(
    value: &'a str,
    max_chars: usize,
    empty_message: &'static str,
) -> Result<&'a str, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::BadRequest(empty_message));
    }
    if value.chars().count() > max_chars || value.chars().any(char::is_control) {
        return Err(ApiError::BadRequest("Jellyfin value is too long"));
    }
    Ok(value)
}

fn bounded(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn client_error(error: JellyfinClientError) -> ApiError {
    match error {
        JellyfinClientError::Unauthorized => {
            ApiError::AccessDenied("Jellyfin connection needs to be verified")
        }
        JellyfinClientError::NotFound => ApiError::NotFound("Jellyfin music item not found"),
        JellyfinClientError::Rejected(message) => ApiError::BadRequest(message),
        JellyfinClientError::Unavailable(message) => ApiError::Integration(message.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio_item() -> JellyfinItem {
        JellyfinItem {
            id: "track".to_owned(),
            item_type: "Audio".to_owned(),
            media_type: Some("Audio".to_owned()),
            name: "Track".to_owned(),
            ..JellyfinItem::default()
        }
    }

    #[test]
    fn audio_gate_requires_both_item_and_media_type() {
        let mut item = audio_item();
        assert!(is_audio(&item));
        item.media_type = Some("Video".to_owned());
        assert!(!is_audio(&item));
        item.media_type = Some("Audio".to_owned());
        item.item_type = "MusicVideo".to_owned();
        assert!(!is_audio(&item));
    }

    #[test]
    fn jellyfin_attachment_names_are_readable_and_header_safe() {
        assert_eq!(
            media_attachment_name("A track: live", "mp3", "track"),
            "A track_ live.mp3"
        );
        assert_eq!(media_attachment_name("東京", "mp3", "track"), "track.mp3");
    }

    #[test]
    fn music_root_gate_rejects_non_music_collection_folders() {
        let mut item = JellyfinItem {
            item_type: "CollectionFolder".to_owned(),
            collection_type: Some("music".to_owned()),
            ..JellyfinItem::default()
        };
        assert!(is_music_root(&item));
        item.collection_type = Some("movies".to_owned());
        assert!(!is_music_root(&item));
    }

    #[test]
    fn music_home_group_keeps_other_shelves_when_one_query_fails() {
        let mut target = Vec::new();
        let mut successful_queries = 2;
        let mut first_error = None;

        collect_music_home_group(
            &mut target,
            Err(JellyfinClientError::Unavailable("temporary failure")),
            "music",
            "Playlist",
            false,
            "playlists",
            &mut successful_queries,
            &mut first_error,
        );

        assert!(target.is_empty());
        assert_eq!(successful_queries, 2);
        assert!(matches!(
            first_error,
            Some(JellyfinClientError::Unavailable("temporary failure"))
        ));
    }

    #[test]
    fn music_home_group_filters_unexpected_item_types() {
        let mut target = Vec::new();
        let mut successful_queries = 0;
        let mut first_error = None;
        let response = JellyfinItems {
            items: vec![
                JellyfinItem {
                    id: "album".to_owned(),
                    item_type: "MusicAlbum".to_owned(),
                    name: "Album".to_owned(),
                    ..JellyfinItem::default()
                },
                audio_item(),
            ],
            total_record_count: 2,
            start_index: 0,
        };

        collect_music_home_group(
            &mut target,
            Ok(response),
            "music",
            "MusicAlbum",
            false,
            "albums",
            &mut successful_queries,
            &mut first_error,
        );

        assert_eq!(successful_queries, 1);
        assert!(first_error.is_none());
        assert_eq!(target.len(), 1);
        assert_eq!(target[0].id, "album");
    }

    #[test]
    fn playback_seconds_convert_to_checked_jellyfin_ticks() {
        let report = playback_report(
            &PlaybackRequest {
                library_id: "music".to_owned(),
                item_id: "track".to_owned(),
                position_seconds: 1.25,
                is_paused: false,
                play_session_id: None,
            },
            "session".to_owned(),
        )
        .unwrap();
        assert_eq!(report.position_ticks, 12_500_000);
    }

    #[test]
    fn album_track_sort_uses_disc_then_track_then_title() {
        let (sort_by, sort_order) = music_sort(Some("track")).unwrap();

        assert_eq!(sort_by, "ParentIndexNumber,IndexNumber,SortName");
        assert_eq!(sort_order, "Ascending");
    }
}
