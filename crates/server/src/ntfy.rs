use actix_web::{HttpRequest, HttpResponse, web};
use db::entities::{NtfyNotification, NtfyNotificationDraft, NtfyTopic};
use futures_util::{
    Stream,
    stream::{self, StreamExt},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, pin::Pin, sync::Arc, time::Duration};
use tokio::{
    sync::{Notify, broadcast},
    task::JoinHandle,
    time::{Instant, Interval, MissedTickBehavior, interval_at},
};
use url::Url;

use crate::{
    ApiError, AppState, authenticated_account,
    widget_integrations::{NtfyAction, NtfyMessage, parse_ntfy_message_line},
};

const MAX_TOPIC_COUNT: usize = 32;
const MAX_NOTIFICATION_LIMIT: usize = 200;
const NTFY_SYNC_CONCURRENCY: usize = 4;
const MAX_NTFY_STREAM_LINE_BYTES: usize = 64 * 1_024;
const NTFY_EVENT_BUFFER: usize = 256;
const NTFY_RECONCILE_SECONDS: u64 = 5;
const NTFY_KEEPALIVE_SECONDS: u64 = 20;
const NTFY_RECONNECT_MAX_SECONDS: u64 = 60;
const NTFY_HEALTHY_STREAM_SECONDS: u64 = 60;
const NTFY_REPLAY_OVERLAP_SECONDS: u64 = 10;

#[derive(Debug, Serialize)]
struct NtfyConnectionResponse {
    base_url: String,
    has_token: bool,
    last_synced_at: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct NtfyNotificationResponse {
    id: String,
    topic_id: String,
    topic: String,
    topic_label: String,
    remote_id: String,
    occurred_at: i64,
    title: String,
    message: String,
    priority: i64,
    tags: Vec<String>,
    click_url: Option<String>,
    actions: Vec<NtfyAction>,
    seen: bool,
    received_at: String,
}

#[derive(Debug, Serialize)]
struct NtfyResponse {
    connection: Option<NtfyConnectionResponse>,
    topics: Vec<NtfyTopic>,
    notifications: Vec<NtfyNotificationResponse>,
    unread_count: i64,
    secret_storage_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct NtfyListQuery {
    topic_id: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct UpdateNtfyConnectionRequest {
    base_url: String,
    token: Option<String>,
    #[serde(default)]
    clear_token: bool,
}

#[derive(Debug, Deserialize)]
struct CreateNtfyTopicRequest {
    topic: String,
    #[serde(default)]
    label: String,
}

#[derive(Debug, Deserialize)]
struct UpdateNtfyTopicRequest {
    label: String,
}

#[derive(Debug, Serialize)]
struct NtfyActionResponse {
    status: u16,
    deleted: bool,
}

#[derive(Debug, Serialize)]
struct NtfyRealtimeEvent {
    kind: &'static str,
    notification: NtfyNotificationResponse,
    unread_count: i64,
}

#[derive(Debug, Serialize)]
struct NtfyDeletedEvent<'a> {
    kind: &'static str,
    notification_id: &'a str,
    unread_count: i64,
}

#[derive(Debug, Serialize)]
struct NtfyStatusEvent<'a> {
    kind: &'static str,
    last_error: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct NtfyBroadcastEvent {
    user_id: String,
    bytes: web::Bytes,
}

#[derive(Clone)]
pub struct NtfyEventHub {
    sender: broadcast::Sender<NtfyBroadcastEvent>,
    reconfigure: Arc<Notify>,
}

impl Default for NtfyEventHub {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(NTFY_EVENT_BUFFER);
        Self {
            sender,
            reconfigure: Arc::new(Notify::new()),
        }
    }
}

impl NtfyEventHub {
    fn publish(&self, user_id: &str, bytes: web::Bytes) {
        let _ = self.sender.send(NtfyBroadcastEvent {
            user_id: user_id.to_owned(),
            bytes,
        });
    }

    fn subscribe(&self) -> broadcast::Receiver<NtfyBroadcastEvent> {
        self.sender.subscribe()
    }

    fn connection_changed(&self) {
        self.reconfigure.notify_one();
    }
}

type NtfyUpstream =
    Pin<Box<dyn Stream<Item = Result<web::Bytes, reqwest::Error>> + Send + 'static>>;

struct NtfyRealtimeState {
    upstream: NtfyUpstream,
    state: web::Data<AppState>,
    user_id: String,
    topics: HashMap<String, NtfyTopic>,
    started_at: i64,
    buffer: Vec<u8>,
    finished: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NtfyWorkerConfig {
    connection: db::entities::NtfyConnection,
    topics: Vec<NtfyTopic>,
}

impl NtfyWorkerConfig {
    fn has_same_subscription(&self, other: &Self) -> bool {
        self.connection.user_id == other.connection.user_id
            && self.connection.base_url == other.connection.base_url
            && self.connection.token_ciphertext == other.connection.token_ciphertext
            && self.topics.len() == other.topics.len()
            && self.topics.iter().all(|topic| {
                other
                    .topics
                    .iter()
                    .any(|candidate| candidate.id == topic.id && candidate.topic == topic.topic)
            })
    }
}

struct NtfyWorkerTask {
    config: NtfyWorkerConfig,
    handle: JoinHandle<()>,
}

struct NtfyClientEventState {
    receiver: broadcast::Receiver<NtfyBroadcastEvent>,
    user_id: String,
    keepalive: Interval,
}

async fn ntfy(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<NtfyListQuery>,
) -> Result<web::Json<NtfyResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    ntfy_response(
        &state,
        &account.id,
        query.topic_id.as_deref(),
        query.limit.unwrap_or(50).clamp(1, MAX_NOTIFICATION_LIMIT),
    )
    .await
    .map(web::Json)
}

async fn ntfy_events(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let mut keepalive = interval_at(
        Instant::now() + Duration::from_secs(NTFY_KEEPALIVE_SECONDS),
        Duration::from_secs(NTFY_KEEPALIVE_SECONDS),
    );
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let events = stream::unfold(
        NtfyClientEventState {
            receiver: state.ntfy_events.subscribe(),
            user_id: account.id,
            keepalive,
        },
        next_client_ntfy_event,
    );
    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache, no-transform"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(events))
}

async fn next_client_ntfy_event(
    mut client: NtfyClientEventState,
) -> Option<(Result<web::Bytes, actix_web::Error>, NtfyClientEventState)> {
    loop {
        tokio::select! {
            message = client.receiver.recv() => {
                match message {
                    Ok(message) if message.user_id == client.user_id => {
                        return Some((Ok(message.bytes), client));
                    }
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
            _ = client.keepalive.tick() => {
                return Some((Ok(web::Bytes::from_static(b": keepalive\n\n")), client));
            }
        }
    }
}

async fn next_ntfy_event(
    mut realtime: NtfyRealtimeState,
) -> Option<(Result<web::Bytes, actix_web::Error>, NtfyRealtimeState)> {
    loop {
        if realtime.finished {
            return None;
        }
        if let Some(newline) = realtime.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = realtime.buffer.drain(..=newline).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = match std::str::from_utf8(&line) {
                Ok(line) => line,
                Err(_) => {
                    tracing::warn!(user_id = %realtime.user_id, "ntfy stream returned non-UTF-8 data");
                    realtime.finished = true;
                    return Some((Ok(ntfy_stream_error_event()), realtime));
                }
            };
            let message = match parse_ntfy_message_line(line) {
                Ok(Some(message)) => message,
                Ok(None) => {
                    return Some((Ok(web::Bytes::from_static(b": keepalive\n\n")), realtime));
                }
                Err(error) => {
                    tracing::warn!(user_id = %realtime.user_id, %error, "ntfy stream parse failed");
                    realtime.finished = true;
                    return Some((Ok(ntfy_stream_error_event()), realtime));
                }
            };
            let Some(topic) = realtime.topics.get(&message.topic).cloned() else {
                continue;
            };
            let occurred_at = message.time;
            let draft = notification_draft(message);
            let stored = db::ntfy_queries::store_ntfy_realtime_message(
                &realtime.state.pool,
                &realtime.user_id,
                &topic.id,
                &draft,
            )
            .await;
            let (inserted, notification) = match stored {
                Ok(stored) => stored,
                Err(error) => {
                    tracing::error!(user_id = %realtime.user_id, %error, "ntfy realtime message could not be stored");
                    realtime.finished = true;
                    return Some((Ok(ntfy_stream_error_event()), realtime));
                }
            };
            // A short replay may contain messages already recovered by the pre-stream sync.
            // A duplicate timestamped after this connection began is still sent so every open
            // browser receives the event even when another tab won the database insert race.
            if (!inserted && occurred_at < realtime.started_at)
                || notification.archived_at.is_some()
            {
                continue;
            }
            let notification = match notification_response(notification) {
                Ok(notification) => notification,
                Err(error) => {
                    tracing::error!(user_id = %realtime.user_id, %error, "stored ntfy realtime message was invalid");
                    realtime.finished = true;
                    return Some((Ok(ntfy_stream_error_event()), realtime));
                }
            };
            let unread_count = match db::ntfy_queries::count_unseen_ntfy_notifications(
                &realtime.state.pool,
                &realtime.user_id,
            )
            .await
            {
                Ok(count) => count,
                Err(error) => {
                    tracing::error!(user_id = %realtime.user_id, %error, "ntfy unread count failed");
                    realtime.finished = true;
                    return Some((Ok(ntfy_stream_error_event()), realtime));
                }
            };
            let payload = NtfyRealtimeEvent {
                kind: "notification",
                notification,
                unread_count,
            };
            let json = match serde_json::to_string(&payload) {
                Ok(json) => json,
                Err(error) => {
                    tracing::error!(user_id = %realtime.user_id, %error, "ntfy realtime event serialization failed");
                    realtime.finished = true;
                    return Some((Ok(ntfy_stream_error_event()), realtime));
                }
            };
            return Some((Ok(web::Bytes::from(format!("data: {json}\n\n"))), realtime));
        }

        match realtime.upstream.next().await {
            Some(Ok(chunk)) => {
                realtime.buffer.extend_from_slice(&chunk);
                let first_line_length = realtime
                    .buffer
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .unwrap_or(realtime.buffer.len());
                if first_line_length > MAX_NTFY_STREAM_LINE_BYTES {
                    tracing::warn!(user_id = %realtime.user_id, "ntfy stream line exceeded the size limit");
                    realtime.finished = true;
                    return Some((Ok(ntfy_stream_error_event()), realtime));
                }
            }
            Some(Err(error)) => {
                tracing::warn!(user_id = %realtime.user_id, %error, "ntfy upstream stream ended with an error");
                return None;
            }
            None => return None,
        }
    }
}

fn ntfy_stream_error_event() -> web::Bytes {
    web::Bytes::from_static(
        b"event: stream-error\ndata: {\"message\":\"Notification stream interrupted\"}\n\n",
    )
}

fn ntfy_status_event(error: Option<&str>) -> Result<web::Bytes, serde_json::Error> {
    let payload = NtfyStatusEvent {
        kind: "status",
        last_error: error,
    };
    serde_json::to_string(&payload).map(|json| web::Bytes::from(format!("data: {json}\n\n")))
}

async fn set_realtime_status(state: &AppState, user_id: &str, error: Option<&str>) {
    if let Err(error) = db::ntfy_queries::set_ntfy_sync_status(&state.pool, user_id, error).await {
        tracing::error!(%user_id, %error, "ntfy realtime status could not be stored");
        return;
    }
    match ntfy_status_event(error) {
        Ok(bytes) => state.ntfy_events.publish(user_id, bytes),
        Err(error) => {
            tracing::error!(%user_id, %error, "ntfy realtime status could not be encoded");
        }
    }
}

/// Keeps one guarded ntfy subscription open per configured account, independent of browsers.
pub fn spawn_ntfy_worker(state: web::Data<AppState>) {
    tokio::spawn(async move {
        let mut workers = HashMap::<String, NtfyWorkerTask>::new();
        loop {
            match load_ntfy_worker_configs(&state).await {
                Ok(configs) => reconcile_ntfy_workers(&state, &mut workers, configs),
                Err(error) => {
                    tracing::error!(%error, "ntfy realtime worker could not load connections");
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(NTFY_RECONCILE_SECONDS)) => {}
                _ = state.ntfy_events.reconfigure.notified() => {}
            }
        }
    });
}

async fn load_ntfy_worker_configs(state: &AppState) -> Result<Vec<NtfyWorkerConfig>, sqlx::Error> {
    let connections = db::ntfy_queries::list_ntfy_connections(&state.pool).await?;
    let mut configs = Vec::with_capacity(connections.len());
    for connection in connections {
        let topics = db::ntfy_queries::list_ntfy_topics(&state.pool, &connection.user_id).await?;
        if !topics.is_empty() {
            configs.push(NtfyWorkerConfig { connection, topics });
        }
    }
    Ok(configs)
}

fn reconcile_ntfy_workers(
    state: &web::Data<AppState>,
    workers: &mut HashMap<String, NtfyWorkerTask>,
    configs: Vec<NtfyWorkerConfig>,
) {
    let desired = configs
        .into_iter()
        .map(|config| (config.connection.user_id.clone(), config))
        .collect::<HashMap<_, _>>();
    let stale = workers
        .iter()
        .filter_map(|(user_id, worker)| {
            (worker.handle.is_finished()
                || desired
                    .get(user_id)
                    .is_none_or(|config| !config.has_same_subscription(&worker.config)))
            .then(|| user_id.clone())
        })
        .collect::<Vec<_>>();
    for user_id in stale {
        if let Some(worker) = workers.remove(&user_id) {
            worker.handle.abort();
        }
    }
    for (user_id, config) in desired {
        if workers.contains_key(&user_id) {
            continue;
        }
        let worker_state = state.clone();
        let worker_config = config.clone();
        let handle = tokio::spawn(async move {
            run_ntfy_account_worker(worker_state, worker_config).await;
        });
        workers.insert(user_id, NtfyWorkerTask { config, handle });
    }
}

async fn run_ntfy_account_worker(state: web::Data<AppState>, config: NtfyWorkerConfig) {
    let user_id = config.connection.user_id.clone();
    let topic_names = config
        .topics
        .iter()
        .map(|topic| topic.topic.as_str())
        .collect::<Vec<_>>();
    let topics = config
        .topics
        .iter()
        .cloned()
        .map(|topic| (topic.topic.clone(), topic))
        .collect::<HashMap<_, _>>();
    let mut reconnect_seconds = 1;
    tracing::info!(
        %user_id,
        topic_count = topic_names.len(),
        token_present = config.connection.token_ciphertext.is_some(),
        "ntfy account worker started"
    );

    match sync_account(&state, &user_id).await {
        Ok(inserted) => {
            tracing::debug!(%user_id, inserted, "ntfy recovery sync completed");
        }
        Err(error) => {
            tracing::warn!(%user_id, %error, "ntfy recovery sync failed");
        }
    }

    let mut disconnected_at = Instant::now();
    loop {
        let replay_since = ntfy_replay_since(disconnected_at.elapsed());
        let response = state
            .widget_integrations
            .open_ntfy_stream(
                &user_id,
                &config.connection.base_url,
                &topic_names,
                &replay_since,
                config.connection.token_ciphertext.as_deref(),
            )
            .await;
        match response {
            Ok(response) => {
                set_realtime_status(&state, &user_id, None).await;
                let connected_at = Instant::now();
                let mut forwarded_events = 0_u64;
                let mut control_events = 0_u64;
                let mut realtime = NtfyRealtimeState {
                    upstream: Box::pin(response.bytes_stream()),
                    state: state.clone(),
                    user_id: user_id.clone(),
                    topics: topics.clone(),
                    started_at: chrono::Utc::now().timestamp(),
                    buffer: Vec::new(),
                    finished: false,
                };
                while let Some((event, next)) = next_ntfy_event(realtime).await {
                    realtime = next;
                    match event {
                        Ok(bytes) if bytes.starts_with(b"data:") => {
                            state.ntfy_events.publish(&user_id, bytes);
                            forwarded_events = forwarded_events.saturating_add(1);
                        }
                        Ok(_) => {
                            control_events = control_events.saturating_add(1);
                        }
                        Err(error) => {
                            tracing::warn!(%user_id, %error, "ntfy browser event encoding failed");
                            break;
                        }
                    }
                }
                let connected_for_ms =
                    u64::try_from(connected_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                reconnect_seconds =
                    reconnect_delay_after_stream(reconnect_seconds, connected_at.elapsed());
                disconnected_at = Instant::now();
                tracing::warn!(
                    %user_id,
                    connected_for_ms,
                    forwarded_events,
                    control_events,
                    reconnect_in_seconds = reconnect_seconds,
                    "ntfy realtime stream ended; reconnecting"
                );
            }
            Err(error) => {
                tracing::warn!(
                    %user_id,
                    %error,
                    reconnect_in_seconds = reconnect_seconds,
                    "ntfy realtime connection failed"
                );
                let status = bounded_text(&error, 2_000, "Realtime connection unavailable");
                set_realtime_status(&state, &user_id, Some(&status)).await;
            }
        }
        tokio::time::sleep(Duration::from_secs(reconnect_seconds)).await;
        reconnect_seconds = (reconnect_seconds * 2).min(NTFY_RECONNECT_MAX_SECONDS);
    }
}

fn reconnect_delay_after_stream(current_seconds: u64, connected_for: Duration) -> u64 {
    if connected_for >= Duration::from_secs(NTFY_HEALTHY_STREAM_SECONDS) {
        1
    } else {
        current_seconds
    }
}

fn ntfy_replay_since(disconnected_for: Duration) -> String {
    format!(
        "{}s",
        disconnected_for
            .as_secs()
            .saturating_add(NTFY_REPLAY_OVERLAP_SECONDS)
    )
}

async fn update_connection(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateNtfyConnectionRequest>,
) -> Result<web::Json<NtfyResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if payload.clear_token
        && payload
            .token
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(ApiError::BadRequest(
            "an ntfy token cannot be set and cleared together",
        ));
    }
    let base_url = normalize_server_url(&payload.base_url)?;
    state
        .widget_integrations
        .validate_source(
            &base_url,
            crate::network_policy::NetworkAccessScope::Notifications,
        )
        .await
        .map_err(ApiError::Integration)?;
    let token = payload
        .token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if token.is_some_and(|value| value.len() > 4_096) {
        return Err(ApiError::BadRequest("ntfy token is too large"));
    }
    let encrypted = token
        .map(|value| {
            state
                .widget_integrations
                .encrypt_secret(value)
                .map_err(ApiError::Integration)
        })
        .transpose()?;
    let token_update = if let Some(ciphertext) = encrypted.as_deref() {
        Some(Some(ciphertext))
    } else if payload.clear_token {
        Some(None)
    } else {
        None
    };
    db::ntfy_queries::upsert_ntfy_connection(&state.pool, &account.id, &base_url, token_update)
        .await?;
    state.ntfy_events.connection_changed();
    ntfy_response(&state, &account.id, None, 50)
        .await
        .map(web::Json)
}

async fn delete_connection(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::ntfy_queries::delete_ntfy_connection(&state.pool, &account.id).await? {
        state.ntfy_events.connection_changed();
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("ntfy connection not found"))
    }
}

async fn create_topic(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateNtfyTopicRequest>,
) -> Result<web::Json<NtfyTopic>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::ntfy_queries::get_ntfy_connection(&state.pool, &account.id)
        .await?
        .is_none()
    {
        return Err(ApiError::Conflict("connect an ntfy server first"));
    }
    let topic = validate_topic(&payload.topic)?;
    let label = validate_label(&payload.label, topic)?;
    let topics = db::ntfy_queries::list_ntfy_topics(&state.pool, &account.id).await?;
    if topics.len() >= MAX_TOPIC_COUNT {
        return Err(ApiError::BadRequest("ntfy topic limit reached"));
    }
    if topics.iter().any(|candidate| candidate.topic == topic) {
        return Err(ApiError::Conflict("ntfy topic is already subscribed"));
    }
    let topic = db::ntfy_queries::create_ntfy_topic(&state.pool, &account.id, topic, label)
        .await
        .map_err(ApiError::from)?;
    state.ntfy_events.connection_changed();
    Ok(web::Json(topic))
}

async fn update_topic(
    state: web::Data<AppState>,
    request: HttpRequest,
    topic_id: web::Path<String>,
    payload: web::Json<UpdateNtfyTopicRequest>,
) -> Result<web::Json<NtfyTopic>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let label = validate_label(&payload.label, "Topic")?;
    db::ntfy_queries::update_ntfy_topic_label(&state.pool, &account.id, &topic_id, label)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("ntfy topic not found"))
}

async fn delete_topic(
    state: web::Data<AppState>,
    request: HttpRequest,
    topic_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::ntfy_queries::delete_ntfy_topic(&state.pool, &account.id, &topic_id).await? {
        state.ntfy_events.connection_changed();
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("ntfy topic not found"))
    }
}

async fn mark_seen(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::ntfy_queries::mark_ntfy_notifications_seen(&state.pool, &account.id).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_notification(
    state: web::Data<AppState>,
    request: HttpRequest,
    notification_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let notification =
        db::ntfy_queries::get_ntfy_notification(&state.pool, &account.id, &notification_id)
            .await?
            .ok_or(ApiError::NotFound("ntfy notification not found"))?;
    delete_notification_record(&state, &account.id, &notification).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn execute_action(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, usize)>,
) -> Result<web::Json<NtfyActionResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (notification_id, action_index) = path.into_inner();
    let notification =
        db::ntfy_queries::get_ntfy_notification(&state.pool, &account.id, &notification_id)
            .await?
            .ok_or(ApiError::NotFound("ntfy notification not found"))?;
    let actions: Vec<NtfyAction> = serde_json::from_str(&notification.actions_json)
        .map_err(|_| ApiError::Internal("stored ntfy action is invalid"))?;
    let action = actions
        .get(action_index)
        .ok_or(ApiError::NotFound("ntfy action not found"))?;
    let status = state
        .widget_integrations
        .execute_ntfy_http_action(action)
        .await
        .map_err(ApiError::Integration)?;
    let deleted = if action.clear {
        match delete_notification_record(&state, &account.id, &notification).await {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(
                    user_id = %account.id,
                    notification_id = %notification.id,
                    %error,
                    "ntfy action succeeded but its requested cleanup failed"
                );
                false
            }
        }
    } else {
        false
    };
    Ok(web::Json(NtfyActionResponse { status, deleted }))
}

async fn delete_notification_record(
    state: &AppState,
    user_id: &str,
    notification: &NtfyNotification,
) -> Result<(), ApiError> {
    let connection = db::ntfy_queries::get_ntfy_connection(&state.pool, user_id)
        .await?
        .ok_or(ApiError::Conflict("connect an ntfy server first"))?;
    state
        .widget_integrations
        .delete_ntfy_notification(
            user_id,
            &connection.base_url,
            &notification.topic,
            &notification.remote_id,
            connection.token_ciphertext.as_deref(),
        )
        .await
        .map_err(ApiError::Integration)?;
    if !db::ntfy_queries::delete_ntfy_notification(&state.pool, user_id, &notification.id).await? {
        return Err(ApiError::NotFound("ntfy notification not found"));
    }
    let unread_count =
        db::ntfy_queries::count_unseen_ntfy_notifications(&state.pool, user_id).await?;
    let json = serde_json::to_string(&NtfyDeletedEvent {
        kind: "deleted",
        notification_id: &notification.id,
        unread_count,
    })
    .map_err(|_| ApiError::Internal("ntfy deletion event could not be serialized"))?;
    state
        .ntfy_events
        .publish(user_id, web::Bytes::from(format!("data: {json}\n\n")));
    Ok(())
}

async fn ntfy_response(
    state: &AppState,
    user_id: &str,
    topic_id: Option<&str>,
    limit: usize,
) -> Result<NtfyResponse, ApiError> {
    let (connection, topics, notifications, unread_count) = tokio::try_join!(
        db::ntfy_queries::get_ntfy_connection(&state.pool, user_id),
        db::ntfy_queries::list_ntfy_topics(&state.pool, user_id),
        db::ntfy_queries::list_ntfy_notifications(&state.pool, user_id, topic_id, limit,),
        db::ntfy_queries::count_unseen_ntfy_notifications(&state.pool, user_id),
    )?;
    let notifications = notifications
        .into_iter()
        .map(notification_response)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NtfyResponse {
        connection: connection.map(|connection| NtfyConnectionResponse {
            base_url: connection.base_url,
            has_token: connection.token_ciphertext.is_some(),
            last_synced_at: connection.last_synced_at,
            last_error: connection.last_error,
        }),
        topics,
        notifications,
        unread_count,
        secret_storage_enabled: state.widget_integrations.secrets_enabled(),
    })
}

async fn sync_account(state: &AppState, user_id: &str) -> Result<u64, ApiError> {
    let Some(connection) = db::ntfy_queries::get_ntfy_connection(&state.pool, user_id).await?
    else {
        return Ok(0);
    };
    let topics = db::ntfy_queries::list_ntfy_topics(&state.pool, user_id).await?;
    let mut inserted = 0;
    let mut errors = Vec::new();
    let mut topic_groups = HashMap::<Option<String>, Vec<NtfyTopic>>::new();
    for topic in topics {
        topic_groups
            .entry(topic.last_message_id.clone())
            .or_default()
            .push(topic);
    }
    let integration = &state.widget_integrations;
    let base_url = &connection.base_url;
    let encrypted_token = connection.token_ciphertext.as_deref();
    let fetches = topic_groups.into_iter().map(|(since, topics)| async move {
        let topic_names = topics
            .iter()
            .map(|topic| topic.topic.as_str())
            .collect::<Vec<_>>();
        let result = integration
            .fetch_ntfy_topics(
                user_id,
                base_url,
                &topic_names,
                since.as_deref(),
                encrypted_token,
            )
            .await;
        (topics, result)
    });
    let results = stream::iter(fetches)
        .buffer_unordered(NTFY_SYNC_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    for (topics, result) in results {
        match result {
            Ok(messages) => {
                let mut messages_by_topic = HashMap::new();
                for message in messages {
                    messages_by_topic
                        .entry(message.topic.clone())
                        .or_insert_with(Vec::new)
                        .push(message);
                }
                for topic in topics {
                    let messages = messages_by_topic.remove(&topic.topic).unwrap_or_default();
                    let last_message_id = messages.last().map(|message| message.id.clone());
                    let drafts = messages
                        .into_iter()
                        .map(notification_draft)
                        .collect::<Vec<_>>();
                    inserted += db::ntfy_queries::store_ntfy_messages(
                        &state.pool,
                        user_id,
                        &topic.id,
                        &drafts,
                        last_message_id.as_deref(),
                    )
                    .await?;
                }
            }
            Err(error) => errors.push(format!(
                "{}: {error}",
                topics
                    .iter()
                    .map(|topic| topic.label.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
    let error =
        (!errors.is_empty()).then(|| bounded_text(&errors.join("; "), 2_000, "Sync failed"));
    db::ntfy_queries::set_ntfy_sync_status(&state.pool, user_id, error.as_deref()).await?;
    Ok(inserted)
}

fn notification_draft(message: NtfyMessage) -> NtfyNotificationDraft {
    NtfyNotificationDraft {
        remote_id: bounded_text(&message.id, 128, "unknown"),
        occurred_at: message.time,
        title: bounded_text(&message.title, 500, "Notification"),
        message: bounded_text(&message.message, 10_000, ""),
        priority: message.priority.clamp(1, 5),
        tags_json: serde_json::to_string(&message.tags).unwrap_or_else(|_| "[]".to_owned()),
        click_url: message
            .click
            .filter(|value| value.len() <= 2_048 && safe_browser_url(value)),
        actions_json: serde_json::to_string(&message.actions).unwrap_or_else(|_| "[]".to_owned()),
    }
}

fn notification_response(
    notification: NtfyNotification,
) -> Result<NtfyNotificationResponse, ApiError> {
    let tags = serde_json::from_str(&notification.tags_json)
        .map_err(|_| ApiError::Internal("stored ntfy tags are invalid"))?;
    let actions = serde_json::from_str(&notification.actions_json)
        .map_err(|_| ApiError::Internal("stored ntfy actions are invalid"))?;
    Ok(NtfyNotificationResponse {
        id: notification.id,
        topic_id: notification.topic_id,
        topic: notification.topic,
        topic_label: notification.topic_label,
        remote_id: notification.remote_id,
        occurred_at: notification.occurred_at,
        title: notification.title,
        message: notification.message,
        priority: notification.priority,
        tags,
        click_url: notification.click_url,
        actions,
        seen: notification.seen_at.is_some(),
        received_at: notification.received_at,
    })
}

fn normalize_server_url(value: &str) -> Result<String, ApiError> {
    let mut url =
        Url::parse(value.trim()).map_err(|_| ApiError::BadRequest("ntfy server URL is invalid"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::BadRequest(
            "ntfy server must be a credential-free HTTP(S) URL without a query or fragment",
        ));
    }
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(if path.is_empty() { "/" } else { &path });
    Ok(url.to_string().trim_end_matches('/').to_owned())
}

fn validate_topic(value: &str) -> Result<&str, ApiError> {
    let topic = value.trim();
    if topic.is_empty()
        || topic.len() > 64
        || !topic
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(ApiError::BadRequest(
            "ntfy topics must use 1-64 letters, numbers, dots, underscores, or hyphens",
        ));
    }
    Ok(topic)
}

fn validate_label<'a>(value: &'a str, fallback: &'a str) -> Result<&'a str, ApiError> {
    let label = value.trim();
    let label = if label.is_empty() { fallback } else { label };
    if label.chars().count() > 80 || label.chars().any(char::is_control) {
        return Err(ApiError::BadRequest(
            "ntfy topic labels must be 80 characters or fewer",
        ));
    }
    Ok(label)
}

fn safe_browser_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

fn bounded_text(value: &str, max: usize, fallback: &str) -> String {
    let value = value.trim();
    let value = if value.is_empty() { fallback } else { value };
    value.chars().take(max).collect()
}

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/ntfy", web::get().to(ntfy))
        .route("/ntfy/events", web::get().to(ntfy_events))
        .route("/ntfy/connection", web::put().to(update_connection))
        .route("/ntfy/connection", web::delete().to(delete_connection))
        .route("/ntfy/topics", web::post().to(create_topic))
        .route("/ntfy/topics/{topic_id}", web::patch().to(update_topic))
        .route("/ntfy/topics/{topic_id}", web::delete().to(delete_topic))
        .route("/ntfy/seen", web::post().to(mark_seen))
        .route(
            "/ntfy/notifications/{notification_id}",
            web::delete().to(delete_notification),
        )
        .route(
            "/ntfy/notifications/{notification_id}/actions/{action_index}",
            web::post().to(execute_action),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_config(
        base_url: &str,
        token: Option<&str>,
        label: &str,
        cursor: Option<&str>,
        sync_status: Option<(&str, &str)>,
    ) -> NtfyWorkerConfig {
        NtfyWorkerConfig {
            connection: db::entities::NtfyConnection {
                user_id: "user-1".to_owned(),
                base_url: base_url.to_owned(),
                token_ciphertext: token.map(str::to_owned),
                last_synced_at: sync_status.map(|(synced_at, _)| synced_at.to_owned()),
                last_error: sync_status.map(|(_, error)| error.to_owned()),
                created_at: "2026-08-21T00:00:00Z".to_owned(),
                updated_at: sync_status
                    .map_or("2026-08-21T00:00:00Z", |(synced_at, _)| synced_at)
                    .to_owned(),
            },
            topics: vec![NtfyTopic {
                id: "topic-1".to_owned(),
                topic: "alerts".to_owned(),
                label: label.to_owned(),
                last_message_id: cursor.map(str::to_owned),
                created_at: "2026-08-21T00:00:00Z".to_owned(),
                updated_at: "2026-08-21T00:00:00Z".to_owned(),
            }],
        }
    }

    #[test]
    fn worker_reconciliation_ignores_mutable_sync_metadata() {
        let running = worker_config(
            "https://ntfy.example.com",
            Some("encrypted-token"),
            "Alerts",
            Some("old-message"),
            None,
        );
        let refreshed = worker_config(
            "https://ntfy.example.com",
            Some("encrypted-token"),
            "Renamed alerts",
            Some("new-message"),
            Some(("2026-08-21T00:05:00Z", "temporary error")),
        );

        assert!(running.has_same_subscription(&refreshed));
    }

    #[test]
    fn worker_reconciliation_restarts_for_upstream_changes() {
        let running = worker_config("https://ntfy.example.com", None, "Alerts", None, None);
        let changed_server = worker_config("https://push.example.com", None, "Alerts", None, None);
        let changed_token = worker_config(
            "https://ntfy.example.com",
            Some("encrypted-token"),
            "Alerts",
            None,
            None,
        );
        let mut changed_topics = running.clone();
        changed_topics.topics[0].topic = "other-alerts".to_owned();

        assert!(!running.has_same_subscription(&changed_server));
        assert!(!running.has_same_subscription(&changed_token));
        assert!(!running.has_same_subscription(&changed_topics));
    }

    #[test]
    fn reconnect_backoff_resets_only_after_a_healthy_stream() {
        assert_eq!(
            reconnect_delay_after_stream(32, Duration::from_secs(12)),
            32
        );
        assert_eq!(
            reconnect_delay_after_stream(32, Duration::from_secs(NTFY_HEALTHY_STREAM_SECONDS),),
            1
        );
    }

    #[test]
    fn realtime_replay_window_covers_the_full_disconnect() {
        assert_eq!(ntfy_replay_since(Duration::ZERO), "10s");
        assert_eq!(ntfy_replay_since(Duration::from_secs(47)), "57s");
    }

    #[test]
    fn ntfy_topics_reject_paths_and_accept_documented_names() {
        assert_eq!(validate_topic("home.alerts_1").unwrap(), "home.alerts_1");
        assert!(validate_topic("home/alerts").is_err());
        assert!(validate_topic("alerts,other").is_err());
    }

    #[test]
    fn ntfy_server_urls_accept_http_syntax_for_policy_evaluation() {
        assert_eq!(
            normalize_server_url("https://ntfy.sh/").unwrap(),
            "https://ntfy.sh"
        );
        assert_eq!(
            normalize_server_url("http://ntfy.sh/").unwrap(),
            "http://ntfy.sh"
        );
        assert!(normalize_server_url("https://user:pass@ntfy.sh").is_err());
    }

    #[test]
    fn ntfy_status_events_clear_recovered_connection_errors() {
        let recovered = ntfy_status_event(None).expect("status event serializes");
        assert_eq!(
            recovered,
            web::Bytes::from_static(b"data: {\"kind\":\"status\",\"last_error\":null}\n\n")
        );
    }
}
