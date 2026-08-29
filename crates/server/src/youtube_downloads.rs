use crate::{
    ApiError, AppState, authenticated_account, authenticated_administrator,
    network_policy::NetworkPolicy,
    ytdlp_proxy::YoutubePolicyProxy,
    ytdlp_runner::{RunFailure, RunnerEvent, ToolCapability, YoutubeInspection, YtDlpRunner},
};
use actix_files::NamedFile;
use actix_web::{
    HttpRequest, HttpResponse,
    http::header::{self, ContentDisposition, DispositionParam, DispositionType},
    web,
};
use db::entities::{NewYoutubeDownloadJob, YoutubeDownloadJob, YoutubeDownloadSettings};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{Mutex, Notify, broadcast, mpsc, watch},
    task::JoinSet,
    time::{MissedTickBehavior, interval, sleep, timeout},
};
use tracing::{info, warn};
use url::Url;
use uuid::Uuid;

const HISTORY_PAGE_SIZE: i64 = 50;
const SCHEDULER_IDLE: Duration = Duration::from_secs(1);
const SSE_KEEPALIVE: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
struct ActiveRun {
    user_id: String,
    cancel: watch::Sender<bool>,
    reserved_bytes: i64,
}

struct YoutubeDownloadInner {
    pool: SqlitePool,
    root: PathBuf,
    runner: YtDlpRunner,
    events: broadcast::Sender<YoutubeDownloadJob>,
    active: Mutex<HashMap<String, ActiveRun>>,
    wake: Notify,
}

#[derive(Clone)]
pub struct YoutubeDownloadService {
    inner: Arc<YoutubeDownloadInner>,
}

#[derive(Debug)]
struct PublishedFile {
    storage_file_name: String,
    display_file_name: String,
    mime_type: String,
    byte_size: i64,
    path: PathBuf,
}

impl YoutubeDownloadService {
    pub async fn from_env(pool: SqlitePool) -> Result<Self, String> {
        let root = std::env::var_os("PANDAN_DOWNLOAD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("data/downloads"));
        prepare_root(&root).await?;
        let root = tokio::fs::canonicalize(&root)
            .await
            .map_err(|error| format!("download media root could not be resolved: {error}"))?;
        let enabled = std::env::var("PANDAN_DOWNLOADS_ENABLED").map_or(true, |value| {
            matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes")
        });
        let proxy = YoutubePolicyProxy::start(NetworkPolicy::new(pool.clone())).await?;
        let runner = YtDlpRunner::from_env(proxy.url().to_owned(), enabled).await;
        let (events, _) = broadcast::channel(256);
        Ok(Self {
            inner: Arc::new(YoutubeDownloadInner {
                pool,
                root,
                runner,
                events,
                active: Mutex::new(HashMap::new()),
                wake: Notify::new(),
            }),
        })
    }

    #[cfg(test)]
    #[must_use]
    /// Creates an isolated service whose external tools are disabled.
    ///
    /// # Panics
    ///
    /// Panics if the process temporary directory cannot hold the test fixture.
    pub fn for_tests(pool: SqlitePool) -> Self {
        let root = std::env::temp_dir().join(format!("pandan-download-tests-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join(".partial")).expect("test partial root is created");
        std::fs::create_dir_all(root.join("files")).expect("test file root is created");
        let (events, _) = broadcast::channel(32);
        Self {
            inner: Arc::new(YoutubeDownloadInner {
                pool,
                root,
                runner: YtDlpRunner::disabled_for_tests(),
                events,
                active: Mutex::new(HashMap::new()),
                wake: Notify::new(),
            }),
        }
    }

    #[must_use]
    pub fn capability(&self) -> &ToolCapability {
        self.inner.runner.capability()
    }

    pub async fn inspect(&self, source_url: &str) -> Result<YoutubeInspection, RunFailure> {
        self.inner.runner.inspect(source_url).await
    }

    fn subscribe(&self) -> broadcast::Receiver<YoutubeDownloadJob> {
        self.inner.events.subscribe()
    }

    fn wake(&self) {
        self.inner.wake.notify_one();
    }

    async fn publish_job(&self, user_id: &str, job_id: &str) {
        match db::youtube_download_queries::get_job(&self.inner.pool, user_id, job_id).await {
            Ok(Some(job)) => {
                let _ = self.inner.events.send(job);
            }
            Ok(None) => {}
            Err(error) => warn!(%error, "download event state could not be loaded"),
        }
    }

    pub async fn cancel(&self, user_id: &str, job_id: &str) -> Result<bool, sqlx::Error> {
        let changed =
            db::youtube_download_queries::mark_cancelled(&self.inner.pool, user_id, job_id).await?;
        if let Some(active) = self.inner.active.lock().await.get(job_id) {
            let _ = active.cancel.send(true);
        }
        if changed {
            self.publish_job(user_id, job_id).await;
        }
        self.wake();
        Ok(changed)
    }

    pub async fn remove_job_files(&self, job: &YoutubeDownloadJob) -> Result<(), String> {
        let _ = self.cancel(&job.user_id, &job.id).await;
        remove_if_exists(self.partial_path(&job.id)).await?;
        if !job.storage_file_name.is_empty() {
            remove_if_exists(self.final_path(&job.user_id, &job.storage_file_name)?).await?;
        }
        Ok(())
    }

    pub async fn purge_user(&self, user_id: &str) -> Result<(), String> {
        let job_ids = db::youtube_download_queries::list_user_job_ids(&self.inner.pool, user_id)
            .await
            .map_err(|_| "download jobs could not be listed".to_owned())?;
        self.cancel_user_runs(user_id).await;
        for job_id in job_ids {
            remove_if_exists(self.partial_path(&job_id)).await?;
        }
        remove_if_exists(self.inner.root.join("files").join(user_id)).await?;
        Ok(())
    }

    pub async fn purge_deleted_user(&self, user_id: &str) {
        self.cancel_user_runs(user_id).await;
        if let Err(error) = remove_if_exists(self.inner.root.join("files").join(user_id)).await {
            warn!(%error, "deleted account download files could not be removed");
        }
        let active_ids = self
            .inner
            .active
            .lock()
            .await
            .iter()
            .filter(|(_, active)| active.user_id == user_id)
            .map(|(job_id, _)| job_id.clone())
            .collect::<Vec<_>>();
        for job_id in active_ids {
            let _ = remove_if_exists(self.partial_path(&job_id)).await;
        }
    }

    async fn cancel_user_runs(&self, user_id: &str) {
        let active = self.inner.active.lock().await;
        for run in active.values().filter(|run| run.user_id == user_id) {
            let _ = run.cancel.send(true);
        }
    }

    fn partial_path(&self, job_id: &str) -> PathBuf {
        self.inner.root.join(".partial").join(job_id)
    }

    fn final_path(&self, user_id: &str, file_name: &str) -> Result<PathBuf, String> {
        if Uuid::parse_str(user_id).is_err() || !safe_file_name(file_name) {
            return Err("download file identity is invalid".to_owned());
        }
        Ok(self.inner.root.join("files").join(user_id).join(file_name))
    }

    pub fn resolve_file(&self, job: &YoutubeDownloadJob) -> Result<PathBuf, String> {
        self.final_path(&job.user_id, &job.storage_file_name)
    }

    async fn publish_output(
        &self,
        job: &YoutubeDownloadJob,
        max_output_bytes: i64,
    ) -> Result<PublishedFile, String> {
        let stage = self.partial_path(&job.id);
        let mut entries = tokio::fs::read_dir(&stage)
            .await
            .map_err(|_| "download staging directory could not be read".to_owned())?;
        let mut output = None;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|_| "download staging directory could not be read".to_owned())?
        {
            let metadata = tokio::fs::symlink_metadata(entry.path())
                .await
                .map_err(|_| "download output could not be inspected".to_owned())?;
            if metadata.is_file() && !metadata.file_type().is_symlink() {
                if output.is_some() {
                    return Err("the downloader produced more than one output file".to_owned());
                }
                output = Some((entry.path(), metadata.len()));
            }
        }
        let (output, byte_size) =
            output.ok_or_else(|| "the downloader produced no output file".to_owned())?;
        let byte_size =
            i64::try_from(byte_size).map_err(|_| "download output is too large".to_owned())?;
        if byte_size <= 0 || byte_size > max_output_bytes {
            return Err("download output exceeded the configured size limit".to_owned());
        }
        let extension = output
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| "download output has no supported extension".to_owned())?;
        if extension != job.output_format || !supported_extension(&extension) {
            return Err("download output format did not match the selected profile".to_owned());
        }
        let final_dir = self.inner.root.join("files").join(&job.user_id);
        tokio::fs::create_dir_all(&final_dir)
            .await
            .map_err(|_| "download destination could not be created".to_owned())?;
        let storage_file_name = format!("{}.{}", Uuid::new_v4(), extension);
        let final_path = final_dir.join(&storage_file_name);
        tokio::fs::rename(&output, &final_path)
            .await
            .map_err(|_| "download output could not be published".to_owned())?;
        Ok(PublishedFile {
            storage_file_name,
            display_file_name: display_file_name(&job.title, &extension),
            mime_type: mime_type(&extension).to_owned(),
            byte_size,
            path: final_path,
        })
    }

    async fn reconcile(&self) {
        match db::youtube_download_queries::reset_interrupted(&self.inner.pool).await {
            Ok(0) => {}
            Ok(count) => info!(count, "reconciled interrupted YouTube downloads"),
            Err(error) => warn!(%error, "interrupted YouTube downloads could not be reconciled"),
        }
        if let Ok(mut entries) = tokio::fs::read_dir(self.inner.root.join(".partial")).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let _ = remove_if_exists(entry.path()).await;
            }
        }
        let references =
            match db::youtube_download_queries::list_complete_files(&self.inner.pool).await {
                Ok(references) => references,
                Err(error) => {
                    warn!(%error, "YouTube download files could not be reconciled");
                    return;
                }
            };
        let known = references
            .iter()
            .map(|reference| {
                (
                    reference.user_id.clone(),
                    reference.storage_file_name.clone(),
                )
            })
            .collect::<HashSet<_>>();
        for reference in references {
            let path = self
                .inner
                .root
                .join("files")
                .join(&reference.user_id)
                .join(&reference.storage_file_name);
            if !tokio::fs::try_exists(&path).await.unwrap_or(false)
                && let Err(error) = db::youtube_download_queries::invalidate_missing_file(
                    &self.inner.pool,
                    &reference.id,
                )
                .await
            {
                warn!(%error, "missing YouTube download could not be invalidated");
            }
        }
        self.remove_orphan_files(&known).await;
    }

    async fn remove_orphan_files(&self, known: &HashSet<(String, String)>) {
        let Ok(mut users) = tokio::fs::read_dir(self.inner.root.join("files")).await else {
            return;
        };
        while let Ok(Some(user)) = users.next_entry().await {
            let user_id = user.file_name().to_string_lossy().into_owned();
            let Ok(mut files) = tokio::fs::read_dir(user.path()).await else {
                let _ = remove_if_exists(user.path()).await;
                continue;
            };
            while let Ok(Some(file)) = files.next_entry().await {
                let name = file.file_name().to_string_lossy().into_owned();
                if !known.contains(&(user_id.clone(), name)) {
                    let _ = remove_if_exists(file.path()).await;
                }
            }
        }
    }
}

pub fn spawn_youtube_download_workers(state: web::Data<AppState>) {
    let service = state.youtube_downloads.clone();
    tokio::spawn(async move {
        service.reconcile().await;
        let mut running = JoinSet::new();
        loop {
            while running.try_join_next().is_some() {}
            let settings =
                match db::youtube_download_queries::get_settings(&service.inner.pool).await {
                    Ok(settings) => settings,
                    Err(error) => {
                        warn!(%error, "YouTube download policy could not be loaded");
                        sleep(SCHEDULER_IDLE).await;
                        continue;
                    }
                };
            if service.capability().available {
                while running.len() < settings.global_concurrency as usize {
                    let claim = db::youtube_download_queries::claim_next_job(
                        &service.inner.pool,
                        settings.per_user_concurrency,
                    )
                    .await;
                    let job = match claim {
                        Ok(Some(job)) => job,
                        Ok(None) => break,
                        Err(error) => {
                            warn!(%error, "YouTube download job could not be claimed");
                            break;
                        }
                    };
                    let job_service = service.clone();
                    let job_settings = settings.clone();
                    running.spawn(async move {
                        job_service.run_job(job, job_settings).await;
                    });
                }
            }
            if running.is_empty() {
                tokio::select! {
                    () = service.inner.wake.notified() => {}
                    () = sleep(SCHEDULER_IDLE) => {}
                }
            } else {
                tokio::select! {
                    _ = running.join_next() => {}
                    () = service.inner.wake.notified() => {}
                    () = sleep(SCHEDULER_IDLE) => {}
                }
            }
        }
    });
}

impl YoutubeDownloadService {
    async fn run_job(&self, job: YoutubeDownloadJob, settings: YoutubeDownloadSettings) {
        self.publish_job(&job.user_id, &job.id).await;
        let user_used =
            db::youtube_download_queries::user_storage_used(&self.inner.pool, &job.user_id)
                .await
                .unwrap_or(settings.per_user_budget_bytes);
        let instance_used = db::youtube_download_queries::instance_storage_used(&self.inner.pool)
            .await
            .unwrap_or(settings.storage_budget_bytes);
        let (cancel, cancel_receiver) = watch::channel(false);
        let mut active = self.inner.active.lock().await;
        let instance_reserved = active
            .values()
            .map(|run| run.reserved_bytes)
            .fold(0_i64, i64::saturating_add);
        let user_reserved = active
            .values()
            .filter(|run| run.user_id == job.user_id)
            .map(|run| run.reserved_bytes)
            .fold(0_i64, i64::saturating_add);
        if user_used
            .saturating_add(user_reserved)
            .saturating_add(settings.max_output_bytes)
            > settings.per_user_budget_bytes
            || instance_used
                .saturating_add(instance_reserved)
                .saturating_add(settings.max_output_bytes)
                > settings.storage_budget_bytes
        {
            drop(active);
            let _ = db::youtube_download_queries::mark_failed(
                &self.inner.pool,
                &job.id,
                "quota_exceeded",
                "Storage quota is full. Delete completed downloads or ask an administrator to raise the limit.",
            )
            .await;
            self.publish_job(&job.user_id, &job.id).await;
            return;
        }
        active.insert(
            job.id.clone(),
            ActiveRun {
                user_id: job.user_id.clone(),
                cancel: cancel.clone(),
                reserved_bytes: settings.max_output_bytes,
            },
        );
        drop(active);
        let stage = self.partial_path(&job.id);
        if let Err(error) = tokio::fs::create_dir_all(&stage).await {
            self.inner.active.lock().await.remove(&job.id);
            warn!(%error, "YouTube download staging directory could not be created");
            let _ = db::youtube_download_queries::mark_failed(
                &self.inner.pool,
                &job.id,
                "storage_unavailable",
                "Download storage is unavailable.",
            )
            .await;
            self.publish_job(&job.user_id, &job.id).await;
            return;
        }
        let (event_sender, mut event_receiver) = mpsc::channel(64);
        let run_stage = stage.clone();
        let run = self.inner.runner.download(
            &job,
            &run_stage,
            settings.max_output_bytes,
            cancel_receiver,
            event_sender,
        );
        tokio::pin!(run);
        let mut staging_check = interval(Duration::from_secs(1));
        staging_check.set_missed_tick_behavior(MissedTickBehavior::Skip);
        staging_check.tick().await;
        let mut events_open = true;
        let mut forced_failure = None;
        let result = loop {
            tokio::select! {
                result = &mut run => break result,
                _ = staging_check.tick(), if forced_failure.is_none() => {
                    match staging_size(&stage, settings.max_output_bytes).await {
                        Ok(size) if size > settings.max_output_bytes => {
                            forced_failure = Some(("quota_exceeded", "Download exceeded the configured output limit."));
                            let _ = cancel.send(true);
                        }
                        Ok(_) => {}
                        Err(_) => {
                            forced_failure = Some(("output_invalid", "Download staging failed validation."));
                            let _ = cancel.send(true);
                        }
                    }
                }
                event = event_receiver.recv(), if events_open => {
                    let Some(event) = event else {
                        events_open = false;
                        continue;
                    };
                    match event {
                        RunnerEvent::Progress(progress) => {
                            if progress.downloaded_bytes > settings.max_output_bytes {
                                forced_failure = Some(("quota_exceeded", "Download exceeded the configured output limit."));
                                let _ = cancel.send(true);
                            }
                            let _ = db::youtube_download_queries::update_progress(
                                &self.inner.pool,
                                &job.id,
                                progress.percent,
                                progress.downloaded_bytes,
                                progress.total_bytes,
                                progress.speed_bytes_per_second,
                                progress.eta_seconds,
                            ).await;
                            self.publish_job(&job.user_id, &job.id).await;
                        }
                        RunnerEvent::Postprocessing => {
                            let _ = db::youtube_download_queries::mark_postprocessing(
                                &self.inner.pool,
                                &job.id,
                            ).await;
                            self.publish_job(&job.user_id, &job.id).await;
                        }
                        RunnerEvent::OutputLimit => {
                            forced_failure = Some(("tool_output_invalid", "The downloader returned invalid diagnostic output."));
                            let _ = cancel.send(true);
                        }
                    }
                }
            }
        };
        self.inner.active.lock().await.remove(&job.id);
        if let Some((code, message)) = forced_failure {
            let _ =
                db::youtube_download_queries::mark_failed(&self.inner.pool, &job.id, code, message)
                    .await;
        } else {
            self.finish_run(&job, &settings, result).await;
        }
        let _ = remove_if_exists(stage).await;
        self.publish_job(&job.user_id, &job.id).await;
        self.wake();
    }

    async fn finish_run(
        &self,
        job: &YoutubeDownloadJob,
        settings: &YoutubeDownloadSettings,
        result: Result<(), RunFailure>,
    ) {
        match result {
            Ok(()) => {
                if !db::youtube_download_queries::mark_postprocessing(&self.inner.pool, &job.id)
                    .await
                    .unwrap_or(false)
                {
                    return;
                }
                self.publish_job(&job.user_id, &job.id).await;
                let published = match self.publish_output(job, settings.max_output_bytes).await {
                    Ok(published) => published,
                    Err(error) => {
                        warn!(%error, "YouTube download output failed validation");
                        let _ = db::youtube_download_queries::mark_failed(
                            &self.inner.pool,
                            &job.id,
                            "output_invalid",
                            "The downloaded file failed validation.",
                        )
                        .await;
                        return;
                    }
                };
                let stored = db::youtube_download_queries::mark_complete(
                    &self.inner.pool,
                    &job.id,
                    &published.storage_file_name,
                    &published.display_file_name,
                    &published.mime_type,
                    published.byte_size,
                )
                .await
                .unwrap_or(false);
                if !stored {
                    let _ = remove_if_exists(published.path).await;
                }
            }
            Err(RunFailure::Cancelled) => {
                let _ = db::youtube_download_queries::mark_cancelled(
                    &self.inner.pool,
                    &job.user_id,
                    &job.id,
                )
                .await;
            }
            Err(failure) => {
                let _ = db::youtube_download_queries::mark_failed(
                    &self.inner.pool,
                    &job.id,
                    failure.code(),
                    failure.message(),
                )
                .await;
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct PublicCapability {
    enabled: bool,
    available: bool,
    unavailable_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct DownloadOverview {
    capability: PublicCapability,
    policy: DownloadMemberPolicy,
    usage_bytes: i64,
    active_jobs: Vec<YoutubeDownloadJob>,
    history: Vec<YoutubeDownloadJob>,
}

#[derive(Debug, Serialize)]
struct DownloadMemberPolicy {
    member_downloads_enabled: bool,
    per_user_budget_bytes: i64,
    max_output_bytes: i64,
    max_batch_urls: i64,
    max_queued_per_user: i64,
}

#[derive(Debug, Serialize)]
struct DownloadAdminPolicy {
    #[serde(flatten)]
    settings: YoutubeDownloadSettings,
    storage_used_bytes: i64,
    capability: ToolCapability,
}

#[derive(Debug, Deserialize)]
struct InspectPayload {
    url: String,
}

#[derive(Debug, Deserialize)]
struct CreateJobsPayload {
    urls: Vec<String>,
    media_kind: String,
    output_format: String,
    max_height: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CreateJobsResponse {
    jobs: Vec<YoutubeDownloadJob>,
    rejected: Vec<RejectedUrl>,
}

#[derive(Debug, Serialize)]
struct RejectedUrl {
    url: String,
    code: &'static str,
    error: String,
}

#[derive(Debug, Deserialize)]
struct JobsQuery {
    status: Option<String>,
    before: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UpdatePolicyPayload {
    member_downloads_enabled: bool,
    storage_budget_bytes: i64,
    per_user_budget_bytes: i64,
    max_output_bytes: i64,
    global_concurrency: i64,
    per_user_concurrency: i64,
    max_batch_urls: i64,
    max_queued_per_user: i64,
}

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/downloads")
            .service(web::resource("").route(web::get().to(overview)))
            .route("/inspect", web::post().to(inspect))
            .service(
                web::resource("/jobs")
                    .route(web::get().to(list_jobs))
                    .route(web::post().to(create_jobs)),
            )
            .route("/events", web::get().to(events))
            .service(
                web::resource("/policy")
                    .route(web::get().to(get_policy))
                    .route(web::put().to(update_policy)),
            )
            .route("/jobs/{job_id}/cancel", web::post().to(cancel_job))
            .route("/jobs/{job_id}/retry", web::post().to(retry_job))
            .route("/jobs/{job_id}/preview", web::get().to(preview_file))
            .route("/jobs/{job_id}/file", web::get().to(download_file))
            .route("/jobs/{job_id}", web::delete().to(delete_job)),
    );
}

async fn overview(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<DownloadOverview>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (settings, usage_bytes, active_jobs, history) = tokio::try_join!(
        db::youtube_download_queries::get_settings(&state.pool),
        db::youtube_download_queries::user_storage_used(&state.pool, &account.id),
        db::youtube_download_queries::list_active_jobs(&state.pool, &account.id),
        db::youtube_download_queries::list_jobs(
            &state.pool,
            &account.id,
            None,
            None,
            HISTORY_PAGE_SIZE
        ),
    )?;
    Ok(web::Json(DownloadOverview {
        capability: public_capability(state.youtube_downloads.capability()),
        policy: member_policy(&settings),
        usage_bytes,
        active_jobs,
        history,
    }))
}

async fn inspect(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<InspectPayload>,
) -> Result<web::Json<YoutubeInspection>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let settings = db::youtube_download_queries::get_settings(&state.pool).await?;
    ensure_allowed(&state, &account.role, &settings)?;
    let source_url = normalize_youtube_url(&payload.url)?;
    let inspection = state
        .youtube_downloads
        .inspect(&source_url)
        .await
        .map_err(runner_error)?;
    if inspection.is_live {
        return Err(coded_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "live_not_supported",
            "live and upcoming videos are not supported",
        ));
    }
    Ok(web::Json(inspection))
}

async fn create_jobs(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateJobsPayload>,
) -> Result<(web::Json<CreateJobsResponse>, actix_web::http::StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let settings = db::youtube_download_queries::get_settings(&state.pool).await?;
    ensure_allowed(&state, &account.role, &settings)?;
    validate_profile(
        &payload.media_kind,
        &payload.output_format,
        payload.max_height,
    )?;
    if payload.urls.is_empty() || payload.urls.len() > settings.max_batch_urls as usize {
        return Err(coded_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_batch",
            "download batch size is invalid",
        ));
    }
    let unsettled =
        db::youtube_download_queries::count_unsettled_jobs(&state.pool, &account.id).await?;
    if unsettled + payload.urls.len() as i64 > settings.max_queued_per_user {
        return Err(coded_error(
            actix_web::http::StatusCode::CONFLICT,
            "queue_full",
            "download queue limit has been reached",
        ));
    }
    let mut jobs = Vec::new();
    let mut rejected = Vec::new();
    for raw_url in &payload.urls {
        let source_url = match normalize_youtube_url(raw_url) {
            Ok(url) => url,
            Err(error) => {
                rejected.push(RejectedUrl {
                    url: raw_url.chars().take(2048).collect(),
                    code: "unsupported_url",
                    error: error.to_string(),
                });
                continue;
            }
        };
        let inspection = match state.youtube_downloads.inspect(&source_url).await {
            Ok(inspection) if !inspection.is_live => inspection,
            Ok(_) => {
                rejected.push(RejectedUrl {
                    url: source_url,
                    code: "live_not_supported",
                    error: "Live and upcoming videos are not supported.".to_owned(),
                });
                continue;
            }
            Err(error) => {
                rejected.push(RejectedUrl {
                    url: source_url,
                    code: error.code(),
                    error: error.message().to_owned(),
                });
                continue;
            }
        };
        if payload.media_kind == "video"
            && payload
                .max_height
                .is_some_and(|height| !inspection.available_heights.contains(&height))
        {
            rejected.push(RejectedUrl {
                url: source_url,
                code: "format_unavailable",
                error: "The selected video height is no longer available.".to_owned(),
            });
            continue;
        }
        let formats = if payload.media_kind == "video" {
            &inspection.video_formats
        } else {
            &inspection.audio_formats
        };
        if !formats.contains(&payload.output_format) {
            rejected.push(RejectedUrl {
                url: source_url,
                code: "format_unavailable",
                error: "The selected output format is no longer available for this source."
                    .to_owned(),
            });
            continue;
        }
        let draft = NewYoutubeDownloadJob {
            id: Uuid::new_v4().to_string(),
            user_id: account.id.clone(),
            source_url,
            youtube_video_id: inspection.video_id,
            title: inspection.title,
            channel_name: inspection.channel_name,
            duration_seconds: inspection.duration_seconds,
            media_kind: payload.media_kind.clone(),
            output_format: payload.output_format.clone(),
            max_height: payload.max_height,
        };
        match db::youtube_download_queries::create_job(&state.pool, &draft).await {
            Ok(job) => jobs.push(job),
            Err(error)
                if error
                    .as_database_error()
                    .is_some_and(sqlx::error::DatabaseError::is_unique_violation) =>
            {
                rejected.push(RejectedUrl {
                    url: draft.source_url,
                    code: "duplicate_job",
                    error: "An equivalent download is already queued.".to_owned(),
                });
            }
            Err(error) => return Err(ApiError::Database(error)),
        }
    }
    state.youtube_downloads.wake();
    Ok((
        web::Json(CreateJobsResponse { jobs, rejected }),
        actix_web::http::StatusCode::CREATED,
    ))
}

async fn list_jobs(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<JobsQuery>,
) -> Result<web::Json<Vec<YoutubeDownloadJob>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if let Some(status) = query.status.as_deref()
        && !matches!(
            status,
            "queued"
                | "inspecting"
                | "downloading"
                | "postprocessing"
                | "complete"
                | "failed"
                | "cancelled"
        )
    {
        return Err(ApiError::BadRequest("download status filter is invalid"));
    }
    let limit = query.limit.unwrap_or(HISTORY_PAGE_SIZE).clamp(1, 100);
    Ok(web::Json(
        db::youtube_download_queries::list_jobs(
            &state.pool,
            &account.id,
            query.status.as_deref(),
            query.before.as_deref(),
            limit,
        )
        .await?,
    ))
}

async fn cancel_job(
    state: web::Data<AppState>,
    request: HttpRequest,
    job_id: web::Path<String>,
) -> Result<web::Json<YoutubeDownloadJob>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !state.youtube_downloads.cancel(&account.id, &job_id).await? {
        return Err(ApiError::NotFound("active download not found"));
    }
    db::youtube_download_queries::get_job(&state.pool, &account.id, &job_id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("download not found"))
}

async fn retry_job(
    state: web::Data<AppState>,
    request: HttpRequest,
    job_id: web::Path<String>,
) -> Result<(web::Json<YoutubeDownloadJob>, actix_web::http::StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let settings = db::youtube_download_queries::get_settings(&state.pool).await?;
    ensure_allowed(&state, &account.role, &settings)?;
    let old = db::youtube_download_queries::get_job(&state.pool, &account.id, &job_id)
        .await?
        .ok_or(ApiError::NotFound("download not found"))?;
    if !matches!(old.status.as_str(), "failed" | "cancelled") {
        return Err(ApiError::Conflict(
            "only failed or cancelled downloads can be retried",
        ));
    }
    if db::youtube_download_queries::count_unsettled_jobs(&state.pool, &account.id).await?
        >= settings.max_queued_per_user
    {
        return Err(coded_error(
            actix_web::http::StatusCode::CONFLICT,
            "queue_full",
            "download queue limit has been reached",
        ));
    }
    let job = db::youtube_download_queries::create_job(
        &state.pool,
        &NewYoutubeDownloadJob {
            id: Uuid::new_v4().to_string(),
            user_id: account.id,
            source_url: old.source_url,
            youtube_video_id: old.youtube_video_id,
            title: old.title,
            channel_name: old.channel_name,
            duration_seconds: old.duration_seconds,
            media_kind: old.media_kind,
            output_format: old.output_format,
            max_height: old.max_height,
        },
    )
    .await
    .map_err(|error| {
        if error
            .as_database_error()
            .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
        {
            ApiError::Conflict("an equivalent download is already queued")
        } else {
            ApiError::Database(error)
        }
    })?;
    state.youtube_downloads.wake();
    Ok((web::Json(job), actix_web::http::StatusCode::CREATED))
}

async fn delete_job(
    state: web::Data<AppState>,
    request: HttpRequest,
    job_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let job = db::youtube_download_queries::get_job(&state.pool, &account.id, &job_id)
        .await?
        .ok_or(ApiError::NotFound("download not found"))?;
    state
        .youtube_downloads
        .remove_job_files(&job)
        .await
        .map_err(|_| ApiError::Internal("download file could not be removed"))?;
    if db::youtube_download_queries::delete_job(&state.pool, &account.id, &job_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("download not found"))
    }
}

async fn download_file(
    state: web::Data<AppState>,
    request: HttpRequest,
    job_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    serve_file(state, request, job_id.into_inner(), true).await
}

async fn preview_file(
    state: web::Data<AppState>,
    request: HttpRequest,
    job_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    serve_file(state, request, job_id.into_inner(), false).await
}

async fn serve_file(
    state: web::Data<AppState>,
    request: HttpRequest,
    job_id: String,
    as_attachment: bool,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let job = db::youtube_download_queries::get_job(&state.pool, &account.id, &job_id)
        .await?
        .filter(|job| job.status == "complete" && !job.storage_file_name.is_empty())
        .ok_or(ApiError::NotFound("download file not found"))?;
    let path = state
        .youtube_downloads
        .resolve_file(&job)
        .map_err(|_| ApiError::NotFound("download file not found"))?;
    let file = NamedFile::open_async(path)
        .await
        .map_err(|_| ApiError::NotFound("download file not found"))?
        .set_content_disposition(file_disposition(&job.display_file_name, as_attachment));
    let mut response = file.into_response(&request);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("private, no-store"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

fn file_disposition(display_file_name: &str, as_attachment: bool) -> ContentDisposition {
    if as_attachment {
        ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(display_file_name.to_owned())],
        }
    } else {
        ContentDisposition {
            disposition: DispositionType::Inline,
            parameters: Vec::new(),
        }
    }
}

async fn events(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let receiver = state.youtube_downloads.subscribe();
    let user_id = account.id;
    let event_stream = stream::unfold((receiver, user_id), |(mut receiver, user_id)| async move {
        loop {
            match timeout(SSE_KEEPALIVE, receiver.recv()).await {
                Ok(Ok(job)) if job.user_id == user_id => {
                    let payload = serde_json::to_string(&job).unwrap_or_else(|_| "{}".to_owned());
                    let bytes = web::Bytes::from(format!("event: job\ndata: {payload}\n\n"));
                    return Some((Ok::<_, actix_web::Error>(bytes), (receiver, user_id)));
                }
                Ok(Ok(_)) | Ok(Err(broadcast::error::RecvError::Lagged(_))) => {}
                Ok(Err(broadcast::error::RecvError::Closed)) => return None,
                Err(_) => {
                    return Some((
                        Ok::<_, actix_web::Error>(web::Bytes::from_static(b": keepalive\n\n")),
                        (receiver, user_id),
                    ));
                }
            }
        }
    });
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/event-stream"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(event_stream))
}

async fn get_policy(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<DownloadAdminPolicy>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let (settings, storage_used_bytes) = tokio::try_join!(
        db::youtube_download_queries::get_settings(&state.pool),
        db::youtube_download_queries::instance_storage_used(&state.pool),
    )?;
    Ok(web::Json(DownloadAdminPolicy {
        settings,
        storage_used_bytes,
        capability: state.youtube_downloads.capability().clone(),
    }))
}

async fn update_policy(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdatePolicyPayload>,
) -> Result<web::Json<DownloadAdminPolicy>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    validate_policy(&payload)?;
    let settings = db::youtube_download_queries::update_settings(
        &state.pool,
        payload.member_downloads_enabled,
        payload.storage_budget_bytes,
        payload.per_user_budget_bytes,
        payload.max_output_bytes,
        payload.global_concurrency,
        payload.per_user_concurrency,
        payload.max_batch_urls,
        payload.max_queued_per_user,
    )
    .await?;
    let storage_used_bytes =
        db::youtube_download_queries::instance_storage_used(&state.pool).await?;
    state.youtube_downloads.wake();
    Ok(web::Json(DownloadAdminPolicy {
        settings,
        storage_used_bytes,
        capability: state.youtube_downloads.capability().clone(),
    }))
}

fn ensure_allowed(
    state: &AppState,
    role: &str,
    settings: &YoutubeDownloadSettings,
) -> Result<(), ApiError> {
    if !state.youtube_downloads.capability().available {
        return Err(coded_error(
            actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
            "tool_unavailable",
            "download tools are unavailable",
        ));
    }
    if role != "administrator" && !settings.member_downloads_enabled {
        return Err(ApiError::AccessDenied(
            "downloads are currently administrator-only",
        ));
    }
    Ok(())
}

fn public_capability(capability: &ToolCapability) -> PublicCapability {
    PublicCapability {
        enabled: capability.enabled,
        available: capability.available,
        unavailable_reason: capability.unavailable_reason.clone(),
    }
}

fn member_policy(settings: &YoutubeDownloadSettings) -> DownloadMemberPolicy {
    DownloadMemberPolicy {
        member_downloads_enabled: settings.member_downloads_enabled,
        per_user_budget_bytes: settings.per_user_budget_bytes,
        max_output_bytes: settings.max_output_bytes,
        max_batch_urls: settings.max_batch_urls,
        max_queued_per_user: settings.max_queued_per_user,
    }
}

fn runner_error(error: RunFailure) -> ApiError {
    match error {
        RunFailure::Unavailable => coded_error(
            actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
            "tool_unavailable",
            "download tools are unavailable",
        ),
        RunFailure::TimedOut => coded_error(
            actix_web::http::StatusCode::GATEWAY_TIMEOUT,
            "timed_out",
            "video inspection timed out",
        ),
        _ => coded_error(
            actix_web::http::StatusCode::BAD_GATEWAY,
            "download_failed",
            "video metadata could not be inspected",
        ),
    }
}

fn coded_error(
    status: actix_web::http::StatusCode,
    code: &'static str,
    message: &'static str,
) -> ApiError {
    ApiError::Coded {
        status,
        code,
        message,
    }
}

fn normalize_youtube_url(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 2048 {
        return Err(coded_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "unsupported_url",
            "YouTube URL is required",
        ));
    }
    let url = Url::parse(value).map_err(|_| {
        coded_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "unsupported_url",
            "YouTube URL is invalid",
        )
    })?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(coded_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "unsupported_url",
            "YouTube URL must use credential-free HTTPS",
        ));
    }
    let host = url.host_str().map(str::to_ascii_lowercase).ok_or_else(|| {
        coded_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "unsupported_url",
            "YouTube URL host is invalid",
        )
    })?;
    let video_id = if host == "youtu.be" {
        url.path_segments()
            .and_then(|mut segments| segments.next())
            .filter(|_| {
                url.path_segments()
                    .is_some_and(|segments| segments.count() == 1)
            })
            .map(str::to_owned)
    } else if matches!(
        host.as_str(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com"
    ) {
        match url.path() {
            "/watch" if url.query_pairs().all(|(key, _)| key != "list") => url
                .query_pairs()
                .find_map(|(key, value)| (key == "v").then(|| value.into_owned()))
                .as_deref()
                .map(str::to_owned),
            path if path.starts_with("/shorts/") => path
                .trim_start_matches("/shorts/")
                .split('/')
                .next()
                .map(str::to_owned),
            _ => None,
        }
    } else {
        None
    }
    .ok_or_else(|| {
        coded_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "unsupported_url",
            "only individual YouTube videos and Shorts are supported",
        )
    })?;
    if video_id.len() != 11
        || !video_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(coded_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "unsupported_url",
            "YouTube video ID is invalid",
        ));
    }
    Ok(format!("https://www.youtube.com/watch?v={video_id}"))
}

fn validate_profile(
    media_kind: &str,
    output_format: &str,
    max_height: Option<i64>,
) -> Result<(), ApiError> {
    match media_kind {
        "video" if matches!(output_format, "mp4" | "mkv" | "webm") => {
            if max_height.is_some_and(|height| !(144..=8_640).contains(&height)) {
                return Err(coded_error(
                    actix_web::http::StatusCode::BAD_REQUEST,
                    "format_unavailable",
                    "video height is invalid",
                ));
            }
        }
        "audio" if matches!(output_format, "m4a" | "mp3" | "opus") => {
            if max_height.is_some() {
                return Err(coded_error(
                    actix_web::http::StatusCode::BAD_REQUEST,
                    "format_unavailable",
                    "audio downloads cannot select a video height",
                ));
            }
        }
        _ => {
            return Err(coded_error(
                actix_web::http::StatusCode::BAD_REQUEST,
                "format_unavailable",
                "download format profile is invalid",
            ));
        }
    }
    Ok(())
}

fn validate_policy(payload: &UpdatePolicyPayload) -> Result<(), ApiError> {
    let instance_reservation = payload
        .max_output_bytes
        .checked_mul(payload.global_concurrency)
        .unwrap_or(i64::MAX);
    let account_reservation = payload
        .max_output_bytes
        .checked_mul(payload.per_user_concurrency)
        .unwrap_or(i64::MAX);
    if payload.storage_budget_bytes <= 0
        || payload.per_user_budget_bytes <= 0
        || payload.per_user_budget_bytes > payload.storage_budget_bytes
        || payload.max_output_bytes <= 0
        || payload.max_output_bytes > payload.per_user_budget_bytes
        || instance_reservation > payload.storage_budget_bytes
        || account_reservation > payload.per_user_budget_bytes
        || !(1..=8).contains(&payload.global_concurrency)
        || !(1..=4).contains(&payload.per_user_concurrency)
        || payload.per_user_concurrency > payload.global_concurrency
        || !(1..=50).contains(&payload.max_batch_urls)
        || !(1..=200).contains(&payload.max_queued_per_user)
    {
        return Err(ApiError::BadRequest("download policy value is invalid"));
    }
    Ok(())
}

async fn prepare_root(root: &Path) -> Result<(), String> {
    tokio::fs::create_dir_all(root.join(".partial"))
        .await
        .map_err(|error| format!("download partial directory could not be created: {error}"))?;
    tokio::fs::create_dir_all(root.join("files"))
        .await
        .map_err(|error| format!("download files directory could not be created: {error}"))
}

async fn remove_if_exists(path: PathBuf) -> Result<(), String> {
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("download storage entry could not be inspected".to_owned()),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        tokio::fs::remove_dir_all(path)
            .await
            .map_err(|_| "download directory could not be removed".to_owned())
    } else {
        tokio::fs::remove_file(path)
            .await
            .map_err(|_| "download file could not be removed".to_owned())
    }
}

async fn staging_size(root: &Path, stop_after: i64) -> Result<i64, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut total = 0_i64;
    while let Some(directory) = pending.pop() {
        let mut entries = tokio::fs::read_dir(directory)
            .await
            .map_err(|_| "download staging directory could not be read".to_owned())?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|_| "download staging directory could not be read".to_owned())?
        {
            let metadata = tokio::fs::symlink_metadata(entry.path())
                .await
                .map_err(|_| "download staging entry could not be inspected".to_owned())?;
            if metadata.file_type().is_symlink() {
                return Err("download staging contained a symbolic link".to_owned());
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                let size = i64::try_from(metadata.len())
                    .map_err(|_| "download staging size is invalid".to_owned())?;
                total = total.saturating_add(size);
                if total > stop_after {
                    return Ok(total);
                }
            } else {
                return Err("download staging contained an unsupported entry".to_owned());
            }
        }
    }
    Ok(total)
}

fn safe_file_name(value: &str) -> bool {
    let path = Path::new(value);
    path.components().count() == 1
        && path.file_name().and_then(|name| name.to_str()) == Some(value)
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn supported_extension(extension: &str) -> bool {
    matches!(extension, "mp4" | "mkv" | "webm" | "m4a" | "mp3" | "opus")
}

fn mime_type(extension: &str) -> &'static str {
    match extension {
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "m4a" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "opus" => "audio/ogg",
        _ => "application/octet-stream",
    }
}

fn display_file_name(title: &str, extension: &str) -> String {
    let mut sanitized = String::with_capacity(title.len().min(160));
    let mut separator = false;
    for character in title.chars().take(150) {
        if character.is_alphanumeric() || matches!(character, '.' | '-' | '_') {
            sanitized.push(character);
            separator = false;
        } else if !separator {
            sanitized.push(' ');
            separator = true;
        }
    }
    let sanitized = sanitized.trim().trim_matches('.').trim();
    let base = if sanitized.is_empty() {
        "youtube-download"
    } else {
        sanitized
    };
    format!("{base}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_urls_are_normalized_to_one_public_video_form() {
        assert_eq!(
            normalize_youtube_url("https://youtu.be/abcdefghijk").unwrap(),
            "https://www.youtube.com/watch?v=abcdefghijk"
        );
        assert_eq!(
            normalize_youtube_url("https://www.youtube.com/shorts/abcdefghijk").unwrap(),
            "https://www.youtube.com/watch?v=abcdefghijk"
        );
        assert!(normalize_youtube_url("https://www.youtube.com/playlist?list=PL123").is_err());
        assert!(normalize_youtube_url("http://www.youtube.com/watch?v=abcdefghijk").is_err());
        assert!(normalize_youtube_url("https://127.0.0.1/watch?v=abcdefghijk").is_err());
    }

    #[test]
    fn profiles_are_closed_enums() {
        assert!(validate_profile("video", "mp4", Some(1080)).is_ok());
        assert!(validate_profile("audio", "mp3", None).is_ok());
        assert!(validate_profile("audio", "mp3", Some(720)).is_err());
        assert!(validate_profile("video", "--exec", None).is_err());
    }

    #[test]
    fn display_names_cannot_create_paths_or_headers() {
        let name = display_file_name("../../evil\r\nattachment", "mp4");
        assert!(!name.contains('/'));
        assert!(!name.contains('\r'));
        assert!(!name.contains('\n'));
        assert!(
            Path::new(&name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
        );
    }

    #[test]
    fn media_delivery_uses_inline_preview_and_attachment_download() {
        let preview = file_disposition("song.opus", false);
        assert!(matches!(preview.disposition, DispositionType::Inline));
        assert!(preview.parameters.is_empty());

        let download = file_disposition("song.opus", true);
        assert!(matches!(download.disposition, DispositionType::Attachment));
        assert_eq!(
            download.parameters,
            vec![DispositionParam::Filename("song.opus".to_owned())]
        );
    }

    #[test]
    fn policy_reserves_worst_case_concurrent_outputs() {
        let mut payload = UpdatePolicyPayload {
            member_downloads_enabled: true,
            storage_budget_bytes: 20,
            per_user_budget_bytes: 10,
            max_output_bytes: 5,
            global_concurrency: 4,
            per_user_concurrency: 2,
            max_batch_urls: 10,
            max_queued_per_user: 50,
        };
        assert!(validate_policy(&payload).is_ok());
        payload.storage_budget_bytes = 19;
        assert!(validate_policy(&payload).is_err());
        payload.storage_budget_bytes = 20;
        payload.per_user_budget_bytes = 9;
        assert!(validate_policy(&payload).is_err());
    }

    #[actix_web::test]
    async fn staging_size_counts_nested_files_and_rejects_symlinks() {
        let root = std::env::temp_dir().join(format!("pandan-staging-size-{}", Uuid::new_v4()));
        let nested = root.join("nested");
        tokio::fs::create_dir_all(&nested)
            .await
            .expect("nested staging is created");
        tokio::fs::write(root.join("one.part"), [0_u8; 4])
            .await
            .expect("first fixture is written");
        tokio::fs::write(nested.join("two.part"), [0_u8; 7])
            .await
            .expect("second fixture is written");
        assert_eq!(staging_size(&root, 20).await.expect("size is measured"), 11);
        assert!(staging_size(&root, 10).await.expect("limit is measured") > 10);

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("one.part"), nested.join("escape"))
                .expect("symlink fixture is created");
            assert!(staging_size(&root, 20).await.is_err());
        }
        tokio::fs::remove_dir_all(root)
            .await
            .expect("staging fixture is removed");
    }
}
