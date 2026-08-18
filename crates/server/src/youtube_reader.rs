use super::{ApiError, AppState, authenticated_account};
use actix_web::{
    HttpRequest, HttpResponse,
    http::{StatusCode, header},
    web,
};
use chrono::{Duration as ChronoDuration, Utc};
use db::entities::{
    YoutubeChannelThumbnailDraft, YoutubeGroupRecord, YoutubeSubscription, YoutubeVideo,
    YoutubeVideoDraft,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::time::{Duration, sleep};
use tracing::{info, warn};

const REFRESH_HOURS: i64 = 2;
const REFRESH_LEASE_MINUTES: i64 = 10;
const PROFILE_CACHE_HOURS: i64 = 24;
const MAX_CHANNELS_PER_USER: usize = 128;
const MAX_GROUPS_PER_USER: usize = 32;
const MAX_GROUP_CHANNELS: usize = 128;
const REFRESH_BATCH_SIZE: usize = 100;

#[derive(Debug, Serialize)]
pub struct YoutubeReaderResponse {
    subscriptions: Vec<YoutubeSubscription>,
    groups: Vec<YoutubeGroup>,
    videos: Vec<YoutubeVideo>,
    display_mode: String,
}

#[derive(Debug, Serialize)]
struct YoutubeGroup {
    id: String,
    name: String,
    position: i64,
    channel_ids: Vec<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateSubscriptionRequest {
    channel_id: String,
}

#[derive(Debug, Deserialize)]
struct CreateGroupRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpdateGroupRequest {
    name: String,
    channel_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateDisplayModeRequest {
    display_mode: String,
}

pub(crate) fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/youtube", web::get().to(reader))
        .route(
            "/youtube/subscriptions",
            web::post().to(create_subscription),
        )
        .route(
            "/youtube/subscriptions/{channel_id}",
            web::delete().to(delete_subscription),
        )
        .route(
            "/youtube/subscriptions/{channel_id}/refresh",
            web::post().to(refresh_subscription),
        )
        .route(
            "/youtube/channels/{channel_id}/thumbnail",
            web::get().to(channel_thumbnail),
        )
        .route("/youtube/groups", web::post().to(create_group))
        .route("/youtube/groups/{group_id}", web::patch().to(update_group))
        .route("/youtube/groups/{group_id}", web::delete().to(delete_group))
        .route(
            "/youtube/display-mode",
            web::patch().to(update_display_mode),
        );
}

async fn reader(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<YoutubeReaderResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(load_reader(&state, &account.id).await?))
}

async fn create_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateSubscriptionRequest>,
) -> Result<(web::Json<YoutubeReaderResponse>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let channel_id = validate_channel_id(&payload.channel_id)?;
    let subscriptions = db::queries::list_youtube_subscriptions(&state.pool, &account.id).await?;
    if subscriptions
        .iter()
        .any(|subscription| subscription.channel_id == channel_id)
    {
        return Err(ApiError::Conflict(
            "this YouTube channel is already subscribed",
        ));
    }
    if subscriptions.len() >= MAX_CHANNELS_PER_USER {
        return Err(ApiError::Conflict(
            "the YouTube channel limit has been reached",
        ));
    }
    db::queries::ensure_youtube_channel(&state.pool, &channel_id).await?;
    if !db::queries::create_youtube_subscription(&state.pool, &account.id, &channel_id).await? {
        return Err(ApiError::Conflict(
            "this YouTube channel is already subscribed",
        ));
    }
    if let Err(error) = refresh_channel(&state, &channel_id).await {
        warn!(%channel_id, %error, "initial YouTube channel refresh failed");
    }
    Ok((
        web::Json(load_reader(&state, &account.id).await?),
        StatusCode::CREATED,
    ))
}

async fn delete_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    channel_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_youtube_subscription(&state.pool, &account.id, &channel_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("YouTube subscription not found"))
    }
}

async fn refresh_subscription(
    state: web::Data<AppState>,
    request: HttpRequest,
    channel_id: web::Path<String>,
) -> Result<web::Json<YoutubeReaderResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let subscriptions = db::queries::list_youtube_subscriptions(&state.pool, &account.id).await?;
    if !subscriptions
        .iter()
        .any(|subscription| subscription.channel_id == channel_id.as_str())
    {
        return Err(ApiError::NotFound("YouTube subscription not found"));
    }
    refresh_channel(&state, &channel_id).await?;
    Ok(web::Json(load_reader(&state, &account.id).await?))
}

async fn channel_thumbnail(
    state: web::Data<AppState>,
    request: HttpRequest,
    channel_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let channel_id = validate_channel_id(&channel_id)?;
    let thumbnail =
        db::queries::get_youtube_channel_thumbnail(&state.pool, &account.id, &channel_id).await?;
    let Some(thumbnail) = thumbnail else {
        return Ok(HttpResponse::NotFound()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .finish());
    };
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, thumbnail.content_type))
        .insert_header((header::CACHE_CONTROL, "private, max-age=86400"))
        .body(thumbnail.data))
}

async fn create_group(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateGroupRequest>,
) -> Result<(web::Json<YoutubeReaderResponse>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let name = validate_group_name(&payload.name)?;
    let groups = db::queries::list_youtube_groups(&state.pool, &account.id).await?;
    if groups.len() >= MAX_GROUPS_PER_USER {
        return Err(ApiError::Conflict(
            "the YouTube group limit has been reached",
        ));
    }
    if groups
        .iter()
        .any(|group| group.name.eq_ignore_ascii_case(&name))
    {
        return Err(ApiError::Conflict(
            "a YouTube group with this name already exists",
        ));
    }
    db::queries::create_youtube_group(&state.pool, &account.id, &name).await?;
    Ok((
        web::Json(load_reader(&state, &account.id).await?),
        StatusCode::CREATED,
    ))
}

async fn update_group(
    state: web::Data<AppState>,
    request: HttpRequest,
    group_id: web::Path<String>,
    payload: web::Json<UpdateGroupRequest>,
) -> Result<web::Json<YoutubeReaderResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let name = validate_group_name(&payload.name)?;
    let channel_ids = validate_group_channels(&payload.channel_ids)?;
    let (groups, subscriptions) = tokio::try_join!(
        db::queries::list_youtube_groups(&state.pool, &account.id),
        db::queries::list_youtube_subscriptions(&state.pool, &account.id),
    )?;
    if groups
        .iter()
        .any(|group| group.id != group_id.as_str() && group.name.eq_ignore_ascii_case(&name))
    {
        return Err(ApiError::Conflict(
            "a YouTube group with this name already exists",
        ));
    }
    let subscribed = subscriptions
        .into_iter()
        .map(|subscription| subscription.channel_id)
        .collect::<HashSet<_>>();
    if channel_ids
        .iter()
        .any(|channel_id| !subscribed.contains(channel_id))
    {
        return Err(ApiError::BadRequest(
            "groups can only contain subscribed channels",
        ));
    }
    db::queries::update_youtube_group(&state.pool, &account.id, &group_id, &name, &channel_ids)
        .await?
        .then_some(())
        .ok_or(ApiError::NotFound("YouTube group not found"))?;
    Ok(web::Json(load_reader(&state, &account.id).await?))
}

async fn delete_group(
    state: web::Data<AppState>,
    request: HttpRequest,
    group_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_youtube_group(&state.pool, &account.id, &group_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("YouTube group not found"))
    }
}

async fn update_display_mode(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateDisplayModeRequest>,
) -> Result<web::Json<YoutubeReaderResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !matches!(payload.display_mode.as_str(), "thumbnails" | "compact") {
        return Err(ApiError::BadRequest("YouTube display mode is invalid"));
    }
    db::queries::set_youtube_display_mode(&state.pool, &account.id, &payload.display_mode).await?;
    Ok(web::Json(load_reader(&state, &account.id).await?))
}

async fn load_reader(state: &AppState, user_id: &str) -> Result<YoutubeReaderResponse, ApiError> {
    let (subscriptions, group_records, memberships, videos, display_mode) = tokio::try_join!(
        db::queries::list_youtube_subscriptions(&state.pool, user_id),
        db::queries::list_youtube_groups(&state.pool, user_id),
        db::queries::list_youtube_group_channels(&state.pool, user_id),
        db::queries::list_youtube_videos(&state.pool, user_id),
        db::queries::get_youtube_display_mode(&state.pool, user_id),
    )?;
    let groups = group_records
        .into_iter()
        .map(|group| group_response(group, &memberships))
        .collect();
    Ok(YoutubeReaderResponse {
        subscriptions,
        groups,
        videos,
        display_mode,
    })
}

fn group_response(
    group: YoutubeGroupRecord,
    memberships: &[db::entities::YoutubeGroupChannel],
) -> YoutubeGroup {
    YoutubeGroup {
        channel_ids: memberships
            .iter()
            .filter(|membership| membership.group_id == group.id)
            .map(|membership| membership.channel_id.clone())
            .collect(),
        id: group.id,
        name: group.name,
        position: group.position,
        created_at: group.created_at,
        updated_at: group.updated_at,
    }
}

fn validate_channel_id(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.len() != 24
        || !value.starts_with("UC")
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(ApiError::BadRequest(
            "enter a valid 24-character YouTube Channel ID",
        ));
    }
    Ok(value.to_owned())
}

fn validate_group_name(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 40 {
        return Err(ApiError::BadRequest(
            "YouTube group names must be 1 to 40 characters",
        ));
    }
    Ok(value.to_owned())
}

fn validate_group_channels(values: &[String]) -> Result<Vec<String>, ApiError> {
    if values.len() > MAX_GROUP_CHANNELS {
        return Err(ApiError::BadRequest(
            "a YouTube group can contain up to 128 channels",
        ));
    }
    let mut seen = HashSet::new();
    let mut channel_ids = Vec::with_capacity(values.len());
    for value in values {
        let channel_id = validate_channel_id(value)?;
        if seen.insert(channel_id.clone()) {
            channel_ids.push(channel_id);
        }
    }
    Ok(channel_ids)
}

async fn refresh_channel(state: &AppState, channel_id: &str) -> Result<bool, ApiError> {
    let now = Utc::now();
    let due_before = (now - ChronoDuration::hours(REFRESH_HOURS)).to_rfc3339();
    let abandoned_before = (now - ChronoDuration::minutes(REFRESH_LEASE_MINUTES)).to_rfc3339();
    if !db::queries::claim_youtube_channel_refresh(
        &state.pool,
        channel_id,
        &due_before,
        &abandoned_before,
    )
    .await?
    {
        return Ok(false);
    }
    let channel = db::queries::get_youtube_channel(&state.pool, channel_id)
        .await?
        .ok_or(ApiError::NotFound("YouTube channel not found"))?;
    let profile_due_before = now - ChronoDuration::hours(PROFILE_CACHE_HOURS);
    let profile_is_due = channel.thumbnail_fetched_at.as_deref().is_none_or(|value| {
        chrono::DateTime::parse_from_rfc3339(value)
            .map_or(true, |fetched_at| fetched_at <= profile_due_before)
    });
    match state
        .widget_integrations
        .fetch_youtube_channel(channel_id)
        .await
    {
        Ok(snapshot) => {
            let thumbnail = if profile_is_due {
                let mut thumbnail = None;
                let mut last_error = None;
                let thumbnail_urls = if snapshot.thumbnail_urls.is_empty() {
                    match state
                        .widget_integrations
                        .fetch_youtube_channel_portrait_urls(channel_id)
                        .await
                    {
                        Ok(urls) => urls,
                        Err(error) => {
                            warn!(%channel_id, %error, "YouTube channel portrait discovery failed");
                            Vec::new()
                        }
                    }
                } else {
                    snapshot.thumbnail_urls.clone()
                };
                for source_url in &thumbnail_urls {
                    match state
                        .widget_integrations
                        .fetch_public_image(&source_url)
                        .await
                    {
                        Ok((content_type, data)) => {
                            thumbnail = Some(YoutubeChannelThumbnailDraft {
                                source_url: source_url.clone(),
                                content_type,
                                data,
                            });
                            break;
                        }
                        Err(error) => last_error = Some(error),
                    }
                }
                if thumbnail.is_none() {
                    if let Some(error) = last_error {
                        warn!(%channel_id, %error, "YouTube channel portrait fetch failed");
                    }
                }
                thumbnail
            } else {
                None
            };
            let title = truncate(&snapshot.title, 180, channel_id);
            let channel_url = if snapshot.channel_url.trim().is_empty() {
                format!("https://www.youtube.com/channel/{channel_id}")
            } else {
                truncate(&snapshot.channel_url, 2048, "")
            };
            let videos = snapshot
                .items
                .into_iter()
                .map(|item| YoutubeVideoDraft {
                    external_id: truncate(&item.external_id, 2048, &item.url),
                    url: truncate(&item.url, 2048, ""),
                    thumbnail_url: truncate(&item.thumbnail_url, 2048, ""),
                    title: truncate(&item.title, 500, "Untitled video"),
                    published_at: item.published_at,
                })
                .collect::<Vec<_>>();
            db::queries::store_youtube_channel_refresh(
                &state.pool,
                channel_id,
                &title,
                &channel_url,
                thumbnail.as_ref(),
                &videos,
            )
            .await?;
            Ok(true)
        }
        Err(message) => {
            db::queries::set_youtube_refresh_error(&state.pool, channel_id, &message).await?;
            Err(ApiError::Integration(message))
        }
    }
}

fn truncate(value: &str, max: usize, fallback: &str) -> String {
    let value = if value.trim().is_empty() {
        fallback
    } else {
        value.trim()
    };
    value.chars().take(max).collect()
}

pub fn spawn_youtube_refresh_worker(state: web::Data<AppState>) {
    tokio::spawn(async move {
        loop {
            loop {
                let due_before = (Utc::now() - ChronoDuration::hours(REFRESH_HOURS)).to_rfc3339();
                match db::queries::list_due_youtube_channel_ids(
                    &state.pool,
                    &due_before,
                    REFRESH_BATCH_SIZE,
                )
                .await
                {
                    Ok(channel_ids) if channel_ids.is_empty() => break,
                    Ok(channel_ids) => {
                        let final_batch = channel_ids.len() < REFRESH_BATCH_SIZE;
                        for channel_id in channel_ids {
                            match refresh_channel(&state, &channel_id).await {
                                Ok(true) => info!(%channel_id, "YouTube channel refreshed"),
                                Ok(false) => {}
                                Err(error) => warn!(%channel_id, %error, "YouTube refresh failed"),
                            }
                            sleep(Duration::from_secs(1)).await;
                        }
                        if final_batch {
                            break;
                        }
                    }
                    Err(error) => {
                        warn!(%error, "failed to load due YouTube channels");
                        break;
                    }
                }
            }
            sleep(Duration::from_secs(REFRESH_HOURS as u64 * 60 * 60)).await;
        }
    });
}
