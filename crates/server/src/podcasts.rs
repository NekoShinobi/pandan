//! Podcasts: an administrator-curated instance catalogue with a member request queue.
//!
//! The trust boundary is the point of this feature. Adding a podcast commits shared disk
//! and shared bandwidth, so it is an administrator act. Members ask; administrators
//! decide. Once a podcast is in the catalogue the decision has been made, so subscribing
//! and caching individual episodes are ordinary member actions bounded by quota.
//!
//! Every episode read — metadata, audio bytes, listening state — resolves through an
//! active subscription. Administrator routes resolve through `authenticated_administrator`
//! and never trust a client-supplied role.

use super::{
    ApiError, AppState, authenticated_account, authenticated_administrator, podcast_media,
};
use actix_files::NamedFile;
use actix_web::{
    HttpRequest, HttpResponse,
    http::header::{self, ContentDisposition, DispositionType},
    web,
};
use db::entities::{
    Podcast, PodcastDraft, PodcastEpisode, PodcastRequest, PodcastRequestDraft, PodcastSettings,
    PodcastSummary,
};
use serde::{Deserialize, Serialize};
use url::Url;

/// The longest resume position worth storing, so a corrupt client cannot write nonsense.
const MAX_POSITION_SECONDS: i64 = 24 * 60 * 60;
const MAX_EPISODE_PAGE: i64 = 200;
const RECENT_EPISODE_LIMIT: i64 = 60;
const IN_PROGRESS_LIMIT: i64 = 20;

#[derive(Debug, Serialize)]
pub struct PodcastOverview {
    podcasts: Vec<PodcastSummary>,
    queue: Vec<PodcastEpisode>,
    saved: Vec<PodcastEpisode>,
    recent: Vec<PodcastEpisode>,
    in_progress: Vec<PodcastEpisode>,
    requests: Vec<PodcastRequest>,
    policy: PodcastPolicy,
}

/// The part of the administrator policy a member is allowed to see.
///
/// Storage budget and usage stay on the administrator endpoint.
#[derive(Debug, Serialize)]
struct PodcastPolicy {
    requests_enabled: bool,
    member_downloads_enabled: bool,
    max_pending_requests_per_user: i64,
}

#[derive(Debug, Serialize)]
pub struct PodcastAdminSettings {
    #[serde(flatten)]
    settings: PodcastSettings,
    storage_used_bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct BulkDownloadOutcome {
    /// Episodes newly queued. Already-cached and in-flight episodes are not counted.
    queued: usize,
}

#[derive(Debug, Serialize)]
pub struct SubmitRequestResponse {
    /// `requested` when a review item was created, `subscribed` when the feed was already
    /// in the catalogue and the caller was simply subscribed to it.
    outcome: &'static str,
    request: Option<PodcastRequest>,
    podcast_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubmitRequestPayload {
    feed_url: String,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize)]
struct DecisionPayload {
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize)]
struct AddPodcastPayload {
    feed_url: String,
}

#[derive(Debug, Deserialize)]
struct RetentionPayload {
    auto_download_count: i64,
    max_retained_episodes: i64,
}

#[derive(Debug, Deserialize)]
struct EpisodePageQuery {
    #[serde(default)]
    limit: Option<i64>,
    #[serde(default)]
    offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RequestFilterQuery {
    #[serde(default)]
    status: String,
}

#[derive(Debug, Deserialize)]
struct ProgressPayload {
    position_seconds: i64,
    #[serde(default)]
    completed: bool,
}

#[derive(Debug, Deserialize)]
struct QueueAppendPayload {
    episode_id: String,
}

#[derive(Debug, Deserialize)]
struct QueueReorderPayload {
    episode_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AdminSettingsPayload {
    requests_enabled: bool,
    member_downloads_enabled: bool,
    max_pending_requests_per_user: i64,
    storage_budget_bytes: i64,
    max_episode_bytes: i64,
    default_auto_download_count: i64,
}

/// Registers the podcast routes.
///
/// Literal-prefix paths are registered before the `{podcast_id}` paths they would
/// otherwise be captured by: actix matches in registration order, and `/podcasts/queue`
/// is the same shape as `/podcasts/{podcast_id}`.
pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/podcasts")
            .service(
                web::resource("/requests")
                    .route(web::get().to(list_requests))
                    .route(web::post().to(submit_request)),
            )
            .route("/requests/{request_id}", web::delete().to(withdraw_request))
            .route(
                "/requests/{request_id}/approve",
                web::post().to(approve_request),
            )
            .route(
                "/requests/{request_id}/reject",
                web::post().to(reject_request),
            )
            .service(
                web::resource("/queue")
                    .route(web::post().to(append_queue))
                    .route(web::patch().to(reorder_queue)),
            )
            .route("/queue/{episode_id}", web::delete().to(remove_queue))
            .service(
                web::resource("/settings")
                    .route(web::get().to(get_settings))
                    .route(web::patch().to(update_settings)),
            )
            .service(
                web::resource("/episodes/{episode_id}/download")
                    .route(web::post().to(request_download))
                    .route(web::delete().to(remove_download)),
            )
            .route("/episodes/{episode_id}/audio", web::get().to(episode_audio))
            .route(
                "/episodes/{episode_id}/progress",
                web::put().to(save_progress),
            )
            .service(
                web::resource("/episodes/{episode_id}/saved")
                    .route(web::put().to(save_episode))
                    .route(web::delete().to(unsave_episode)),
            )
            .service(
                web::resource("")
                    .route(web::get().to(overview))
                    .route(web::post().to(add_podcast)),
            )
            .route("/{podcast_id}/artwork", web::get().to(podcast_artwork))
            .route("/{podcast_id}/episodes", web::get().to(podcast_episodes))
            .route(
                "/{podcast_id}/downloads",
                web::post().to(download_all_episodes),
            )
            .service(
                web::resource("/{podcast_id}/subscription")
                    .route(web::put().to(subscribe))
                    .route(web::delete().to(unsubscribe)),
            )
            .service(
                web::resource("/{podcast_id}")
                    .route(web::patch().to(update_retention))
                    .route(web::delete().to(delete_podcast)),
            ),
    );
}

// ---------------------------------------------------------------------------
// Feed URL handling
// ---------------------------------------------------------------------------

/// Parses and normalizes one submitted feed URL.
///
/// Normalizing before comparison is what stops the same show being requested three times
/// under three cosmetically different URLs. `Url` already lowercases the scheme and host
/// and drops a default port; this adds fragment and trailing-slash removal. The query is
/// deliberately preserved, because podcast hosts routinely use it to identify a feed.
///
/// The returned pair is the URL to display and the URL to compare on.
fn normalize_feed_url(value: &str) -> Result<(String, String), ApiError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 2048 {
        return Err(ApiError::BadRequest("feed address is required"));
    }
    let parsed = Url::parse(value).map_err(|_| ApiError::BadRequest("feed address is invalid"))?;
    if parsed.scheme() != "https" {
        return Err(ApiError::BadRequest("feed address must use HTTPS"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(ApiError::BadRequest(
            "feed address must not carry credentials",
        ));
    }
    if parsed.host_str().is_none() {
        return Err(ApiError::BadRequest("feed address is missing a host"));
    }

    let mut normalized = parsed.clone();
    normalized.set_fragment(None);
    let trimmed = normalized.path().trim_end_matches('/').to_owned();
    normalized.set_path(if trimmed.is_empty() { "/" } else { &trimmed });
    Ok((parsed.to_string(), normalized.to_string()))
}

/// Resolves one feed into a catalogue entry, reusing an existing entry for the same feed.
///
/// Publishing indexes the episodes from the same fetch that produced the preview, caches
/// the artwork, and queues the newest episodes. The refresh worker would eventually do all
/// three, but it sleeps between batches, and a show that reads as empty for ten minutes
/// after an administrator approves it looks broken.
async fn ensure_catalogued(
    state: &AppState,
    feed_url: &str,
    normalized_url: &str,
    added_by: &str,
) -> Result<Podcast, ApiError> {
    if let Some(existing) =
        db::queries::find_podcast_by_normalized_url(&state.pool, normalized_url).await?
    {
        return Ok(existing);
    }
    let settings = db::queries::get_podcast_settings(&state.pool).await?;
    let (preview, episodes) = state
        .podcast_media
        .fetch_feed(feed_url)
        .await
        .map_err(ApiError::Integration)?;
    let artwork_url = preview.artwork_url.clone();
    let podcast = db::queries::insert_podcast(
        &state.pool,
        &PodcastDraft {
            feed_url: feed_url.to_owned(),
            normalized_url: normalized_url.to_owned(),
            preview,
            added_by: added_by.to_owned(),
            auto_download_count: settings.default_auto_download_count,
        },
    )
    .await?;

    db::queries::upsert_podcast_episodes(&state.pool, &podcast.id, &episodes).await?;
    podcast_media::refresh_artwork(state, &podcast.id, &artwork_url).await;
    if podcast.auto_download_count > 0 {
        let newest = db::queries::list_newest_episode_ids(
            &state.pool,
            &podcast.id,
            podcast.auto_download_count,
        )
        .await?;
        for episode_id in newest {
            db::queries::enqueue_podcast_download(&state.pool, &episode_id, None).await?;
        }
    }
    db::queries::finish_podcast_refresh(&state.pool, &podcast.id, None).await?;

    db::queries::get_podcast(&state.pool, &podcast.id)
        .await?
        .ok_or(ApiError::Internal("podcast could not be reloaded"))
}

// ---------------------------------------------------------------------------
// Member surface
// ---------------------------------------------------------------------------

async fn overview(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<PodcastOverview>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let settings = db::queries::get_podcast_settings(&state.pool).await?;
    Ok(web::Json(PodcastOverview {
        podcasts: db::queries::list_podcast_summaries(&state.pool, &account.id).await?,
        queue: db::queries::list_podcast_queue(&state.pool, &account.id).await?,
        saved: db::queries::list_saved_podcast_episodes(&state.pool, &account.id).await?,
        recent: db::queries::list_recent_podcast_episodes(
            &state.pool,
            &account.id,
            RECENT_EPISODE_LIMIT,
        )
        .await?,
        in_progress: db::queries::list_in_progress_podcast_episodes(
            &state.pool,
            &account.id,
            IN_PROGRESS_LIMIT,
        )
        .await?,
        requests: db::queries::list_podcast_requests_for_user(&state.pool, &account.id).await?,
        policy: PodcastPolicy {
            requests_enabled: settings.requests_enabled,
            member_downloads_enabled: settings.member_downloads_enabled,
            max_pending_requests_per_user: settings.max_pending_requests_per_user,
        },
    }))
}

/// Submits one feed for administrator review.
///
/// A feed already in the catalogue never becomes a review item — the caller is simply
/// subscribed to it, because the decision this request would ask for has already been made.
async fn submit_request(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<SubmitRequestPayload>,
) -> Result<web::Json<SubmitRequestResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let settings = db::queries::get_podcast_settings(&state.pool).await?;
    if !settings.requests_enabled {
        return Err(ApiError::AccessDenied(
            "podcast requests are currently closed",
        ));
    }
    let (feed_url, normalized_url) = normalize_feed_url(&payload.feed_url)?;
    let note = payload.note.trim();
    if note.chars().count() > 500 {
        return Err(ApiError::BadRequest("note is too long"));
    }

    if let Some(existing) =
        db::queries::find_podcast_by_normalized_url(&state.pool, &normalized_url).await?
    {
        db::queries::subscribe_to_podcast(&state.pool, &account.id, &existing.id).await?;
        return Ok(web::Json(SubmitRequestResponse {
            outcome: "subscribed",
            request: None,
            podcast_id: Some(existing.id),
        }));
    }

    if db::queries::has_open_podcast_request(&state.pool, &account.id, &normalized_url).await? {
        return Err(ApiError::Conflict(
            "you already have an open request for this feed",
        ));
    }
    let pending = db::queries::count_pending_podcast_requests(&state.pool, &account.id).await?;
    if pending >= settings.max_pending_requests_per_user {
        return Err(ApiError::Conflict(
            "you have reached your open request limit",
        ));
    }

    // Resolve the feed so the requester and the reviewer both see what is being asked
    // for. This reads the channel head only; episodes are indexed after approval.
    let (preview, _) = state
        .podcast_media
        .fetch_feed(&feed_url)
        .await
        .map_err(ApiError::Integration)?;

    let record = db::queries::insert_podcast_request(
        &state.pool,
        &PodcastRequestDraft {
            user_id: account.id,
            feed_url,
            normalized_url,
            resolved_title: preview.title,
            resolved_author: preview.author,
            resolved_artwork_url: preview.artwork_url,
            note: note.to_owned(),
        },
    )
    .await?;
    Ok(web::Json(SubmitRequestResponse {
        outcome: "requested",
        request: Some(record),
        podcast_id: None,
    }))
}

async fn withdraw_request(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::withdraw_podcast_request(&state.pool, &account.id, &path.into_inner()).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("request not found"))
    }
}

async fn subscribe(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let podcast_id = path.into_inner();
    if db::queries::get_podcast(&state.pool, &podcast_id)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound("podcast not found"));
    }
    db::queries::subscribe_to_podcast(&state.pool, &account.id, &podcast_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn unsubscribe(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::queries::unsubscribe_from_podcast(&state.pool, &account.id, &path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn podcast_artwork(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    authenticated_account(&state, &request).await?;
    let artwork = db::queries::get_podcast_artwork(&state.pool, &path.into_inner())
        .await?
        .ok_or(ApiError::NotFound("artwork not found"))?;
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, artwork.content_type))
        .append_header(("x-content-type-options", "nosniff"))
        .append_header((header::CACHE_CONTROL, "private, max-age=86400"))
        .body(artwork.data))
}

async fn podcast_episodes(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
    query: web::Query<EpisodePageQuery>,
) -> Result<web::Json<Vec<PodcastEpisode>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, MAX_EPISODE_PAGE);
    let offset = query.offset.unwrap_or(0).max(0);
    Ok(web::Json(
        db::queries::list_podcast_episodes(
            &state.pool,
            &account.id,
            &path.into_inner(),
            limit,
            offset,
        )
        .await?,
    ))
}

// ---------------------------------------------------------------------------
// Episodes
// ---------------------------------------------------------------------------

/// Confirms the caller reaches this episode through an active subscription.
///
/// Returns `NotFound` rather than `Forbidden` so an unsubscribed caller cannot use the
/// response to learn which episodes exist.
async fn require_episode_access(
    state: &AppState,
    user_id: &str,
    episode_id: &str,
) -> Result<(), ApiError> {
    if db::queries::user_can_access_episode(&state.pool, user_id, episode_id).await? {
        Ok(())
    } else {
        Err(ApiError::NotFound("episode not found"))
    }
}

async fn request_download(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let episode_id = path.into_inner();
    require_episode_access(&state, &account.id, &episode_id).await?;
    let settings = db::queries::get_podcast_settings(&state.pool).await?;
    if !settings.member_downloads_enabled && account.role != "administrator" {
        return Err(ApiError::AccessDenied(
            "downloads are currently administrator-only",
        ));
    }
    db::queries::enqueue_podcast_download(&state.pool, &episode_id, Some(&account.id)).await?;
    Ok(HttpResponse::Accepted().finish())
}

/// Queues every episode of one show that is not cached yet.
///
/// Administrator-only, and deliberately not offered to members: a whole back catalogue is
/// a large commitment of shared disk and shared bandwidth, which is the same reason adding
/// a podcast is an administrator act. The transfer itself stays bounded — the download
/// worker still applies the storage budget and defers what does not fit rather than
/// exceeding it — so this queues work without widening any limit.
async fn download_all_episodes(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<web::Json<BulkDownloadOutcome>, ApiError> {
    let account = authenticated_administrator(&state, &request).await?;
    let podcast_id = path.into_inner();
    if db::queries::get_podcast(&state.pool, &podcast_id)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound("podcast not found"));
    }
    let episode_ids = db::queries::list_downloadable_episode_ids(&state.pool, &podcast_id).await?;
    for episode_id in &episode_ids {
        db::queries::enqueue_podcast_download(&state.pool, episode_id, Some(&account.id)).await?;
    }
    Ok(web::Json(BulkDownloadOutcome {
        queued: episode_ids.len(),
    }))
}

async fn remove_download(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let episode_id = path.into_inner();
    let Some(cached) = db::queries::delete_podcast_download(&state.pool, &episode_id).await? else {
        return Err(ApiError::NotFound("episode is not cached"));
    };
    if !cached.file_name.is_empty() {
        state.podcast_media.remove(&cached.file_name).await;
    }
    Ok(HttpResponse::NoContent().finish())
}

/// Streams one cached episode from local disk.
///
/// `NamedFile` answers Range requests, so seeking works without the handler ever holding
/// the file in memory. The media root is never mounted statically: the only way to a file
/// is through this handler, after the subscription check.
async fn episode_audio(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let episode_id = path.into_inner();
    require_episode_access(&state, &account.id, &episode_id).await?;

    let cached = db::queries::get_podcast_cached_file(&state.pool, &episode_id)
        .await?
        .ok_or(ApiError::Conflict("episode has not been downloaded yet"))?;
    let file_path = state
        .podcast_media
        .resolve(&cached.file_name)
        .map_err(|_| ApiError::Internal("cached episode is unreadable"))?;
    let file = NamedFile::open_async(&file_path)
        .await
        .map_err(|_| ApiError::Conflict("episode has not been downloaded yet"))?;

    // Eviction ranks on this, so playing an episode protects it from being reclaimed.
    let _ = db::queries::touch_podcast_download(&state.pool, &episode_id).await;

    Ok(file
        .set_content_disposition(ContentDisposition {
            disposition: DispositionType::Inline,
            parameters: Vec::new(),
        })
        .into_response(&request))
}

async fn save_progress(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
    payload: web::Json<ProgressPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let episode_id = path.into_inner();
    require_episode_access(&state, &account.id, &episode_id).await?;
    if !(0..=MAX_POSITION_SECONDS).contains(&payload.position_seconds) {
        return Err(ApiError::BadRequest("position is out of range"));
    }
    db::queries::upsert_podcast_progress(
        &state.pool,
        &account.id,
        &episode_id,
        payload.position_seconds,
        payload.completed,
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn save_episode(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let episode_id = path.into_inner();
    require_episode_access(&state, &account.id, &episode_id).await?;
    db::queries::set_podcast_episode_saved(&state.pool, &account.id, &episode_id, true).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn unsave_episode(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::queries::set_podcast_episode_saved(&state.pool, &account.id, &path.into_inner(), false)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

// ---------------------------------------------------------------------------
// Play queue
// ---------------------------------------------------------------------------

async fn append_queue(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<QueueAppendPayload>,
) -> Result<web::Json<Vec<PodcastEpisode>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    require_episode_access(&state, &account.id, &payload.episode_id).await?;
    if !db::queries::append_to_podcast_queue(
        &state.pool,
        &account.id,
        &payload.episode_id,
        db::queries::PODCAST_QUEUE_LIMIT,
    )
    .await?
    {
        return Err(ApiError::Conflict("your play queue is full"));
    }
    Ok(web::Json(
        db::queries::list_podcast_queue(&state.pool, &account.id).await?,
    ))
}

async fn reorder_queue(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<QueueReorderPayload>,
) -> Result<web::Json<Vec<PodcastEpisode>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !db::queries::reorder_podcast_queue(&state.pool, &account.id, &payload.episode_ids).await? {
        return Err(ApiError::Conflict(
            "the play queue changed; reload and try again",
        ));
    }
    Ok(web::Json(
        db::queries::list_podcast_queue(&state.pool, &account.id).await?,
    ))
}

async fn remove_queue(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<web::Json<Vec<PodcastEpisode>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::queries::remove_from_podcast_queue(&state.pool, &account.id, &path.into_inner()).await?;
    Ok(web::Json(
        db::queries::list_podcast_queue(&state.pool, &account.id).await?,
    ))
}

// ---------------------------------------------------------------------------
// Administrator surface
// ---------------------------------------------------------------------------

async fn list_requests(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<RequestFilterQuery>,
) -> Result<web::Json<Vec<PodcastRequest>>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let status = match query.status.trim() {
        "" | "all" => None,
        value @ ("pending" | "approved" | "rejected" | "withdrawn") => Some(value),
        _ => return Err(ApiError::BadRequest("request status filter is invalid")),
    };
    Ok(web::Json(
        db::queries::list_podcast_requests(&state.pool, status).await?,
    ))
}

/// Approves one request, publishing the podcast and subscribing its requester.
async fn approve_request(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
    payload: web::Json<DecisionPayload>,
) -> Result<web::Json<Podcast>, ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    let request_id = path.into_inner();
    let record = db::queries::get_podcast_request(&state.pool, &request_id)
        .await?
        .ok_or(ApiError::NotFound("request not found"))?;
    if record.status != "pending" {
        return Err(ApiError::Conflict("request has already been decided"));
    }
    let note = payload.note.trim();
    if note.chars().count() > 500 {
        return Err(ApiError::BadRequest("note is too long"));
    }

    let (feed_url, normalized_url) = normalize_feed_url(&record.feed_url)?;
    let podcast = ensure_catalogued(&state, &feed_url, &normalized_url, &administrator.id).await?;
    if !db::queries::approve_podcast_request(
        &state.pool,
        &request_id,
        &administrator.id,
        &podcast.id,
        note,
    )
    .await?
    {
        return Err(ApiError::Conflict("request has already been decided"));
    }
    Ok(web::Json(podcast))
}

async fn reject_request(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
    payload: web::Json<DecisionPayload>,
) -> Result<HttpResponse, ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    let note = payload.note.trim();
    if note.chars().count() > 500 {
        return Err(ApiError::BadRequest("note is too long"));
    }
    if db::queries::reject_podcast_request(&state.pool, &path.into_inner(), &administrator.id, note)
        .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("request not found"))
    }
}

/// Adds one podcast directly, bypassing the review queue.
async fn add_podcast(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<AddPodcastPayload>,
) -> Result<(web::Json<Podcast>, actix_web::http::StatusCode), ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    let (feed_url, normalized_url) = normalize_feed_url(&payload.feed_url)?;
    if db::queries::find_podcast_by_normalized_url(&state.pool, &normalized_url)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict("this feed is already in the catalogue"));
    }
    let podcast = ensure_catalogued(&state, &feed_url, &normalized_url, &administrator.id).await?;
    db::queries::subscribe_to_podcast(&state.pool, &administrator.id, &podcast.id).await?;
    Ok((web::Json(podcast), actix_web::http::StatusCode::CREATED))
}

async fn update_retention(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
    payload: web::Json<RetentionPayload>,
) -> Result<web::Json<Podcast>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    if !(0..=25).contains(&payload.auto_download_count) {
        return Err(ApiError::BadRequest(
            "automatic downloads must be between 0 and 25",
        ));
    }
    if !(1..=1000).contains(&payload.max_retained_episodes) {
        return Err(ApiError::BadRequest(
            "retained episodes must be between 1 and 1000",
        ));
    }
    db::queries::update_podcast_retention(
        &state.pool,
        &path.into_inner(),
        payload.auto_download_count,
        payload.max_retained_episodes,
    )
    .await?
    .map(web::Json)
    .ok_or(ApiError::NotFound("podcast not found"))
}

/// Removes one podcast, its episodes, and every file its removal orphans.
async fn delete_podcast(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let Some(orphaned) = db::queries::delete_podcast(&state.pool, &path.into_inner()).await? else {
        return Err(ApiError::NotFound("podcast not found"));
    };
    for file_name in &orphaned {
        state.podcast_media.remove(file_name).await;
    }
    Ok(HttpResponse::NoContent().finish())
}

async fn get_settings(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<PodcastAdminSettings>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    Ok(web::Json(PodcastAdminSettings {
        settings: db::queries::get_podcast_settings(&state.pool).await?,
        storage_used_bytes: db::queries::podcast_storage_used_bytes(&state.pool).await?,
    }))
}

async fn update_settings(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<AdminSettingsPayload>,
) -> Result<web::Json<PodcastAdminSettings>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    if !(0..=100).contains(&payload.max_pending_requests_per_user) {
        return Err(ApiError::BadRequest(
            "the open request limit must be between 0 and 100",
        ));
    }
    if !(0..=1_099_511_627_776).contains(&payload.storage_budget_bytes) {
        return Err(ApiError::BadRequest("the storage budget is out of range"));
    }
    if !(1_048_576..=5_368_709_120_i64).contains(&payload.max_episode_bytes) {
        return Err(ApiError::BadRequest(
            "the per-episode limit must be between 1 MB and 5 GB",
        ));
    }
    if !(0..=25).contains(&payload.default_auto_download_count) {
        return Err(ApiError::BadRequest(
            "automatic downloads must be between 0 and 25",
        ));
    }
    let settings = db::queries::update_podcast_settings(
        &state.pool,
        payload.requests_enabled,
        payload.member_downloads_enabled,
        payload.max_pending_requests_per_user,
        payload.storage_budget_bytes,
        payload.max_episode_bytes,
        payload.default_auto_download_count,
    )
    .await?;
    Ok(web::Json(PodcastAdminSettings {
        settings,
        storage_used_bytes: db::queries::podcast_storage_used_bytes(&state.pool).await?,
    }))
}

#[cfg(test)]
mod tests {
    use super::normalize_feed_url;

    #[test]
    fn feed_urls_normalize_to_one_comparable_form() {
        let variants = [
            "https://Example.COM:443/feed/",
            "https://example.com/feed",
            "https://example.com/feed/#latest",
            "  https://example.com/feed/  ",
        ];
        let normalized = variants
            .iter()
            .map(|value| normalize_feed_url(value).expect("variant normalizes").1)
            .collect::<Vec<_>>();
        assert!(
            normalized.windows(2).all(|pair| pair[0] == pair[1]),
            "cosmetic differences must collapse: {normalized:?}"
        );
    }

    #[test]
    fn a_meaningful_query_survives_normalization() {
        let (_, first) =
            normalize_feed_url("https://example.com/feed?show=one").expect("first normalizes");
        let (_, second) =
            normalize_feed_url("https://example.com/feed?show=two").expect("second normalizes");
        assert_ne!(
            first, second,
            "podcast hosts identify feeds by query, so it must not be stripped"
        );
    }

    #[test]
    fn insecure_and_credentialed_feed_addresses_are_refused() {
        for hostile in [
            "http://example.com/feed",
            "https://user:pass@example.com/feed",
            "ftp://example.com/feed",
            "not a url",
            "",
        ] {
            assert!(
                normalize_feed_url(hostile).is_err(),
                "{hostile} must be refused"
            );
        }
    }
}
