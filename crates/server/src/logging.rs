use crate::{ApiError, AppState, authenticated_administrator};
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::Utc;
use db::entities::LoggingSettings;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::{Duration, SystemTime},
};
use tracing::{Level, info, warn};
use tracing_subscriber::{
    EnvFilter,
    filter::filter_fn,
    fmt::{self, MakeWriter},
    layer::{Layer, SubscriberExt},
    util::SubscriberInitExt,
};

const ACTIVE_LOG_FILE: &str = "pandan.log";
const LOG_FILE_PREFIX: &str = "pandan-";
const LOG_FILE_SUFFIX: &str = ".log";
const LOG_CHANNEL_CAPACITY: usize = 8_192;
const DEFAULT_RETENTION_DAYS: i64 = 14;
const DEFAULT_MAX_FILE_SIZE_MB: i64 = 10;
const DEFAULT_MAX_FILES: i64 = 20;
const MAX_VIEW_LIMIT: usize = 500;
const DEFAULT_VIEW_LIMIT: usize = 200;
const MAX_TAIL_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct LoggingController {
    sender: SyncSender<WriterCommand>,
    enabled: Arc<AtomicBool>,
    threshold: Arc<AtomicU8>,
    directory: Arc<PathBuf>,
    dropped_entries: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<String>>>,
}

#[derive(Clone, Debug)]
struct RuntimeSettings {
    file_enabled: bool,
    retention_days: i64,
    max_file_size_mb: i64,
    max_files: i64,
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            file_enabled: true,
            retention_days: DEFAULT_RETENTION_DAYS,
            max_file_size_mb: DEFAULT_MAX_FILE_SIZE_MB,
            max_files: DEFAULT_MAX_FILES,
        }
    }
}

impl From<&LoggingSettings> for RuntimeSettings {
    fn from(settings: &LoggingSettings) -> Self {
        Self {
            file_enabled: settings.file_enabled,
            retention_days: settings.retention_days,
            max_file_size_mb: settings.max_file_size_mb,
            max_files: settings.max_files,
        }
    }
}

enum WriterCommand {
    Write(Vec<u8>),
    Configure(RuntimeSettings),
    Flush(mpsc::Sender<()>),
}

#[derive(Clone)]
struct ChannelMakeWriter {
    sender: SyncSender<WriterCommand>,
    dropped_entries: Arc<AtomicU64>,
}

struct ChannelWriter {
    sender: SyncSender<WriterCommand>,
    dropped_entries: Arc<AtomicU64>,
    bytes: Vec<u8>,
}

impl Write for ChannelWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for ChannelWriter {
    fn drop(&mut self) {
        if self.bytes.is_empty() {
            return;
        }
        let bytes = std::mem::take(&mut self.bytes);
        if let Err(error) = self.sender.try_send(WriterCommand::Write(bytes)) {
            if matches!(error, TrySendError::Full(_) | TrySendError::Disconnected(_)) {
                self.dropped_entries.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

impl<'writer> MakeWriter<'writer> for ChannelMakeWriter {
    type Writer = ChannelWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        ChannelWriter {
            sender: self.sender.clone(),
            dropped_entries: self.dropped_entries.clone(),
            bytes: Vec::with_capacity(512),
        }
    }
}

struct WriterState {
    directory: PathBuf,
    active_path: PathBuf,
    file: BufWriter<File>,
    active_bytes: u64,
    rotation_sequence: u64,
    settings: RuntimeSettings,
    last_error: Arc<Mutex<Option<String>>>,
    dropped_entries: Arc<AtomicU64>,
}

impl WriterState {
    fn open(
        directory: PathBuf,
        settings: RuntimeSettings,
        last_error: Arc<Mutex<Option<String>>>,
        dropped_entries: Arc<AtomicU64>,
    ) -> io::Result<Self> {
        fs::create_dir_all(&directory)?;
        let active_path = directory.join(ACTIVE_LOG_FILE);
        let active_bytes = active_path.metadata().map_or(0, |metadata| metadata.len());
        let file = open_log_file(&active_path)?;
        let mut state = Self {
            directory,
            active_path,
            file,
            active_bytes,
            rotation_sequence: 0,
            settings,
            last_error,
            dropped_entries,
        };
        state.apply_retention();
        Ok(state)
    }

    fn write(&mut self, bytes: &[u8]) {
        if !self.settings.file_enabled {
            return;
        }
        let max_bytes = self
            .settings
            .max_file_size_mb
            .max(1)
            .saturating_mul(1024 * 1024) as u64;
        if self.active_bytes > 0 && self.active_bytes.saturating_add(bytes.len() as u64) > max_bytes
        {
            if let Err(error) = self.rotate() {
                self.record_error(format!("log rotation failed: {error}"));
                self.dropped_entries.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
        if let Err(error) = self.file.write_all(bytes) {
            self.record_error(format!("log write failed: {error}"));
            self.dropped_entries.fetch_add(1, Ordering::Relaxed);
            return;
        }
        self.active_bytes = self.active_bytes.saturating_add(bytes.len() as u64);
    }

    fn configure(&mut self, settings: RuntimeSettings) {
        self.settings = settings;
        self.apply_retention();
    }

    fn flush(&mut self) {
        if let Err(error) = self.file.flush() {
            self.record_error(format!("log flush failed: {error}"));
        }
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        self.rotation_sequence = self.rotation_sequence.saturating_add(1);
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S");
        let rotated_path = self.directory.join(format!(
            "{LOG_FILE_PREFIX}{timestamp}-{:04}{LOG_FILE_SUFFIX}",
            self.rotation_sequence
        ));
        fs::rename(&self.active_path, rotated_path)?;
        self.file = open_log_file(&self.active_path)?;
        self.active_bytes = 0;
        self.apply_retention();
        Ok(())
    }

    fn apply_retention(&mut self) {
        let mut files = match rotated_files(&self.directory) {
            Ok(files) => files,
            Err(error) => {
                self.record_error(format!("log retention scan failed: {error}"));
                return;
            }
        };
        files.sort_by_key(|entry| std::cmp::Reverse(entry.modified));
        let max_files = usize::try_from(self.settings.max_files.max(1)).unwrap_or(usize::MAX);
        let cutoff = SystemTime::now()
            .checked_sub(Duration::from_secs(
                self.settings.retention_days.max(1) as u64 * 86_400,
            ))
            .unwrap_or(SystemTime::UNIX_EPOCH);
        for (index, entry) in files.into_iter().enumerate() {
            if index >= max_files || entry.modified < cutoff {
                if let Err(error) = fs::remove_file(&entry.path) {
                    self.record_error(format!("failed to remove retained log file: {error}"));
                }
            }
        }
    }

    fn record_error(&self, message: String) {
        if let Ok(mut error) = self.last_error.lock() {
            *error = Some(message);
        }
    }
}

fn open_log_file(path: &Path) -> io::Result<BufWriter<File>> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(BufWriter::new)
}

fn writer_loop(
    receiver: Receiver<WriterCommand>,
    directory: PathBuf,
    initial_settings: RuntimeSettings,
    last_error: Arc<Mutex<Option<String>>>,
    dropped_entries: Arc<AtomicU64>,
) {
    let startup_error = last_error.clone();
    let startup_dropped = dropped_entries.clone();
    let mut state =
        match WriterState::open(directory, initial_settings, last_error, dropped_entries) {
            Ok(state) => state,
            Err(error) => {
                if let Ok(mut last_error) = startup_error.lock() {
                    *last_error = Some(format!("log file initialization failed: {error}"));
                }
                while receiver.recv().is_ok() {
                    startup_dropped.fetch_add(1, Ordering::Relaxed);
                }
                return;
            }
        };

    loop {
        match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(WriterCommand::Write(bytes)) => state.write(&bytes),
            Ok(WriterCommand::Configure(settings)) => state.configure(settings),
            Ok(WriterCommand::Flush(acknowledge)) => {
                state.flush();
                let _ = acknowledge.send(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => state.flush(),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                state.flush();
                break;
            }
        }
    }
}

/// Installs console tracing and the non-blocking rotating JSON file layer.
///
/// # Errors
///
/// Returns an error when the log writer thread cannot be started or the global subscriber was
/// already installed.
pub fn initialize(settings: &LoggingSettings) -> Result<LoggingController, String> {
    let directory = std::env::var("PANDAN_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/logs"));
    let initial_settings = RuntimeSettings::from(settings);
    if settings.file_enabled {
        fs::create_dir_all(&directory)
            .map_err(|error| format!("failed to create log directory: {error}"))?;
        drop(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(directory.join(ACTIVE_LOG_FILE))
                .map_err(|error| format!("failed to open active log file: {error}"))?,
        );
    }
    let (sender, receiver) = mpsc::sync_channel(LOG_CHANNEL_CAPACITY);
    let enabled = Arc::new(AtomicBool::new(settings.file_enabled));
    let threshold = Arc::new(AtomicU8::new(level_rank(&settings.log_level)));
    let dropped_entries = Arc::new(AtomicU64::new(0));
    let last_error = Arc::new(Mutex::new(None));
    let thread_directory = directory.clone();
    let thread_error = last_error.clone();
    let thread_dropped = dropped_entries.clone();
    std::thread::Builder::new()
        .name("pandan-log-writer".to_owned())
        .spawn(move || {
            writer_loop(
                receiver,
                thread_directory,
                initial_settings,
                thread_error,
                thread_dropped,
            );
        })
        .map_err(|error| format!("failed to start log writer: {error}"))?;

    let make_writer = ChannelMakeWriter {
        sender: sender.clone(),
        dropped_entries: dropped_entries.clone(),
    };
    let filter_enabled = enabled.clone();
    let filter_threshold = threshold.clone();
    let file_filter = filter_fn(move |metadata| {
        filter_enabled.load(Ordering::Relaxed)
            && tracing_level_rank(metadata.level()) <= filter_threshold.load(Ordering::Relaxed)
    });
    let console_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_filter(console_filter))
        .with(
            fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_writer(make_writer)
                .with_filter(file_filter),
        )
        .try_init()
        .map_err(|error| format!("failed to install tracing subscriber: {error}"))?;

    Ok(LoggingController {
        sender,
        enabled,
        threshold,
        directory: Arc::new(directory),
        dropped_entries,
        last_error,
    })
}

impl LoggingController {
    #[cfg(test)]
    pub(crate) fn disabled_for_tests() -> Self {
        let directory =
            std::env::temp_dir().join(format!("pandan-test-logs-{}", uuid::Uuid::new_v4()));
        let (sender, receiver) = mpsc::sync_channel(LOG_CHANNEL_CAPACITY);
        let dropped_entries = Arc::new(AtomicU64::new(0));
        let last_error = Arc::new(Mutex::new(None));
        let thread_directory = directory.clone();
        let thread_error = last_error.clone();
        let thread_dropped = dropped_entries.clone();
        let initial_settings = RuntimeSettings {
            file_enabled: false,
            ..RuntimeSettings::default()
        };
        std::thread::Builder::new()
            .name("pandan-test-log-writer".to_owned())
            .spawn(move || {
                writer_loop(
                    receiver,
                    thread_directory,
                    initial_settings,
                    thread_error,
                    thread_dropped,
                );
            })
            .expect("test log writer starts");
        Self {
            sender,
            enabled: Arc::new(AtomicBool::new(false)),
            threshold: Arc::new(AtomicU8::new(level_rank("error"))),
            directory: Arc::new(directory),
            dropped_entries,
            last_error,
        }
    }

    /// Applies the persisted policy to future log events and the writer thread.
    ///
    /// # Errors
    ///
    /// Returns an error if the writer thread has stopped.
    pub fn configure(&self, settings: &LoggingSettings) -> Result<(), String> {
        self.enabled.store(settings.file_enabled, Ordering::Relaxed);
        self.threshold
            .store(level_rank(&settings.log_level), Ordering::Relaxed);
        self.sender
            .send(WriterCommand::Configure(RuntimeSettings::from(settings)))
            .map_err(|_| "the log writer is unavailable".to_owned())
    }

    fn flush(&self) {
        let (sender, receiver) = mpsc::channel();
        if self.sender.try_send(WriterCommand::Flush(sender)).is_ok() {
            let _ = receiver.recv_timeout(Duration::from_secs(2));
        }
    }

    fn snapshot(&self, limit: usize) -> io::Result<LogFileSnapshot> {
        self.flush();
        Ok(LogFileSnapshot {
            entries: read_entries(&self.directory, limit)?,
            storage: storage_status(
                &self.directory,
                self.dropped_entries.load(Ordering::Relaxed),
                self.last_error.lock().ok().and_then(|error| error.clone()),
            )?,
        })
    }
}

#[derive(Debug)]
struct RotatedFile {
    path: PathBuf,
    modified: SystemTime,
    bytes: u64,
}

fn rotated_files(directory: &Path) -> io::Result<Vec<RotatedFile>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(LOG_FILE_PREFIX) || !name.ends_with(LOG_FILE_SUFFIX) {
            continue;
        }
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            files.push(RotatedFile {
                path: entry.path(),
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                bytes: metadata.len(),
            });
        }
    }
    Ok(files)
}

#[derive(Debug, Serialize)]
struct LogEntry {
    id: String,
    timestamp: String,
    level: String,
    target: String,
    message: String,
    fields: Map<String, Value>,
    file: String,
}

#[derive(Debug, Serialize)]
struct LogStorageStatus {
    directory: String,
    active_file: String,
    active_bytes: u64,
    rotated_files: usize,
    retained_bytes: u64,
    dropped_entries: u64,
    last_error: Option<String>,
}

struct LogFileSnapshot {
    entries: Vec<LogEntry>,
    storage: LogStorageStatus,
}

#[derive(Debug, Serialize)]
struct LoggingSnapshotResponse {
    settings: LoggingSettings,
    storage: LogStorageStatus,
    entries: Vec<LogEntry>,
}

#[derive(Debug, Deserialize)]
struct LogQuery {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct UpdateLoggingSettingsRequest {
    file_enabled: bool,
    log_level: String,
    retention_days: i64,
    max_file_size_mb: i64,
    max_files: i64,
}

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::resource("/admin/logs")
            .route(web::get().to(get_logs))
            .route(web::put().to(update_logging_settings)),
    );
}

async fn get_logs(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<LogQuery>,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let settings = db::queries::get_logging_settings(&state.pool).await?;
    let controller = state.logging.clone();
    let limit = query
        .limit
        .unwrap_or(DEFAULT_VIEW_LIMIT)
        .clamp(1, MAX_VIEW_LIMIT);
    let snapshot = web::block(move || controller.snapshot(limit))
        .await
        .map_err(|error| {
            warn!(%error, "log snapshot task failed");
            ApiError::Internal("log snapshot task failed")
        })?
        .map_err(|error| {
            warn!(%error, "failed to read log snapshot");
            ApiError::Internal("logs could not be read")
        })?;
    Ok(HttpResponse::Ok().json(LoggingSnapshotResponse {
        settings,
        storage: snapshot.storage,
        entries: snapshot.entries,
    }))
}

async fn update_logging_settings(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<UpdateLoggingSettingsRequest>,
) -> Result<HttpResponse, ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    validate_settings(&payload)?;
    let settings = db::queries::update_logging_settings(
        &state.pool,
        payload.file_enabled,
        &payload.log_level,
        payload.retention_days,
        payload.max_file_size_mb,
        payload.max_files,
    )
    .await?;
    let controller = state.logging.clone();
    let runtime_settings = settings.clone();
    web::block(move || controller.configure(&runtime_settings))
        .await
        .map_err(|error| {
            warn!(%error, "logging reconfiguration task failed");
            ApiError::Internal("logging settings could not be applied")
        })?
        .map_err(|error| {
            warn!(%error, "failed to apply logging settings");
            ApiError::Internal("logging settings could not be applied")
        })?;
    info!(
        actor_user_id = %administrator.id,
        file_enabled = settings.file_enabled,
        log_level = %settings.log_level,
        retention_days = settings.retention_days,
        max_file_size_mb = settings.max_file_size_mb,
        max_files = settings.max_files,
        "administrator updated logging settings"
    );
    Ok(HttpResponse::Ok().json(settings))
}

fn validate_settings(payload: &UpdateLoggingSettingsRequest) -> Result<(), ApiError> {
    if !matches!(
        payload.log_level.as_str(),
        "error" | "warn" | "info" | "debug" | "trace"
    ) {
        return Err(ApiError::BadRequest("Select a valid log level."));
    }
    if !(1..=365).contains(&payload.retention_days) {
        return Err(ApiError::BadRequest(
            "Retention must be between 1 and 365 days.",
        ));
    }
    if !(1..=256).contains(&payload.max_file_size_mb) {
        return Err(ApiError::BadRequest(
            "File size must be between 1 and 256 MB.",
        ));
    }
    if !(1..=100).contains(&payload.max_files) {
        return Err(ApiError::BadRequest(
            "Retained files must be between 1 and 100.",
        ));
    }
    Ok(())
}

fn storage_status(
    directory: &Path,
    dropped_entries: u64,
    last_error: Option<String>,
) -> io::Result<LogStorageStatus> {
    let active_path = directory.join(ACTIVE_LOG_FILE);
    let active_bytes = active_path.metadata().map_or(0, |metadata| metadata.len());
    let rotated = rotated_files(directory)?;
    Ok(LogStorageStatus {
        directory: directory.display().to_string(),
        active_file: ACTIVE_LOG_FILE.to_owned(),
        active_bytes,
        rotated_files: rotated.len(),
        retained_bytes: active_bytes + rotated.iter().map(|entry| entry.bytes).sum::<u64>(),
        dropped_entries,
        last_error,
    })
}

fn read_entries(directory: &Path, limit: usize) -> io::Result<Vec<LogEntry>> {
    let mut paths = vec![directory.join(ACTIVE_LOG_FILE)];
    let mut rotated = rotated_files(directory)?;
    rotated.sort_by_key(|entry| std::cmp::Reverse(entry.modified));
    paths.extend(rotated.into_iter().map(|entry| entry.path));

    let mut entries = Vec::with_capacity(limit);
    for path in paths {
        if entries.len() >= limit || !path.exists() {
            break;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(ACTIVE_LOG_FILE)
            .to_owned();
        let bytes = read_tail(&path)?;
        for (line_index, line) in bytes.split(|byte| *byte == b'\n').rev().enumerate() {
            if entries.len() >= limit || line.is_empty() {
                continue;
            }
            if let Some(entry) = parse_entry(line, &file_name, line_index) {
                entries.push(entry);
            }
        }
    }
    Ok(entries)
}

fn read_tail(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let start = length.saturating_sub(MAX_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    if start > 0 {
        if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=newline);
        }
    }
    Ok(bytes)
}

fn parse_entry(line: &[u8], file_name: &str, line_index: usize) -> Option<LogEntry> {
    let mut value = serde_json::from_slice::<Value>(line).ok()?;
    let object = value.as_object_mut()?;
    let timestamp = object.remove("timestamp")?.as_str()?.to_owned();
    let level = object.remove("level")?.as_str()?.to_ascii_lowercase();
    let target = object
        .remove("target")
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default();
    let mut fields = object
        .remove("fields")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let message = fields
        .remove("message")
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default();
    Some(LogEntry {
        id: format!("{file_name}:{line_index}:{timestamp}"),
        timestamp,
        level,
        target,
        message,
        fields,
        file: file_name.to_owned(),
    })
}

fn level_rank(level: &str) -> u8 {
    match level {
        "error" => 1,
        "warn" => 2,
        "info" => 3,
        "debug" => 4,
        "trace" => 5,
        _ => 3,
    }
}

fn tracing_level_rank(level: &Level) -> u8 {
    if *level == Level::ERROR {
        1
    } else if *level == Level::WARN {
        2
    } else if *level == Level::INFO {
        3
    } else if *level == Level::DEBUG {
        4
    } else {
        5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_rotates_and_retains_the_configured_count() {
        let directory =
            std::env::temp_dir().join(format!("pandan-logging-{}", uuid::Uuid::new_v4()));
        let last_error = Arc::new(Mutex::new(None));
        let dropped = Arc::new(AtomicU64::new(0));
        let mut state = WriterState::open(
            directory.clone(),
            RuntimeSettings {
                max_file_size_mb: 1,
                max_files: 2,
                ..RuntimeSettings::default()
            },
            last_error,
            dropped,
        )
        .expect("writer opens");

        for _ in 0..4 {
            state.write(&vec![b'x'; 700_000]);
            state.write(b"\n");
            state.flush();
        }
        state.apply_retention();

        assert!(directory.join(ACTIVE_LOG_FILE).exists());
        assert!(rotated_files(&directory).expect("rotated files list").len() <= 2);
        fs::remove_dir_all(directory).expect("test log directory removes");
    }
}
