use db::entities::YoutubeDownloadJob;
use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::{mpsc, watch},
    time::{sleep, timeout},
};
use tracing::{info, warn};

const INSPECTION_TIMEOUT: Duration = Duration::from_secs(75);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const CAPTURE_LIMIT: usize = 2 * 1024 * 1024;
const MACHINE_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
const MACHINE_LINE_LIMIT: usize = 8 * 1024;
const PROGRESS_PREFIX: &str = "PANDAN_PROGRESS:";
const POSTPROCESS_PREFIX: &str = "PANDAN_POST:";

#[derive(Debug, Clone, Serialize)]
pub struct ToolCapability {
    pub enabled: bool,
    pub available: bool,
    pub yt_dlp_version: Option<String>,
    pub ffmpeg_version: Option<String>,
    pub ffprobe_version: Option<String>,
    pub deno_version: Option<String>,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct YoutubeInspection {
    pub source_url: String,
    pub video_id: String,
    pub title: String,
    pub channel_name: String,
    pub duration_seconds: Option<i64>,
    pub is_live: bool,
    pub available_heights: Vec<i64>,
    pub video_formats: Vec<String>,
    pub audio_formats: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub percent: Option<f64>,
    pub downloaded_bytes: i64,
    pub total_bytes: Option<i64>,
    pub speed_bytes_per_second: Option<f64>,
    pub eta_seconds: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub enum RunnerEvent {
    Progress(DownloadProgress),
    Postprocessing,
    OutputLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunFailure {
    Cancelled,
    TimedOut,
    OutputLimit,
    Failed,
    Unavailable,
}

impl RunFailure {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::OutputLimit => "tool_output_invalid",
            Self::Failed => "download_failed",
            Self::Unavailable => "tool_unavailable",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Cancelled => "Download was cancelled.",
            Self::TimedOut => "Download exceeded the two-hour time limit.",
            Self::OutputLimit => "The downloader returned too much diagnostic output.",
            Self::Failed => "The video could not be downloaded with the selected format.",
            Self::Unavailable => "The download tools are unavailable.",
        }
    }
}

#[derive(Debug, Clone)]
pub struct YtDlpRunner {
    yt_dlp_bin: PathBuf,
    ffmpeg_bin: PathBuf,
    deno_bin: PathBuf,
    proxy_url: String,
    capability: ToolCapability,
}

impl YtDlpRunner {
    #[cfg(test)]
    pub fn disabled_for_tests() -> Self {
        Self {
            yt_dlp_bin: PathBuf::from("yt-dlp"),
            ffmpeg_bin: PathBuf::from("ffmpeg"),
            deno_bin: PathBuf::from("deno"),
            proxy_url: "http://127.0.0.1:1".to_owned(),
            capability: ToolCapability {
                enabled: false,
                available: false,
                yt_dlp_version: None,
                ffmpeg_version: None,
                ffprobe_version: None,
                deno_version: None,
                unavailable_reason: Some("Downloads are disabled in tests.".to_owned()),
            },
        }
    }

    pub async fn from_env(proxy_url: String, enabled: bool) -> Self {
        let yt_dlp_bin = std::env::var_os("PANDAN_YTDLP_BIN")
            .filter(|value| !value.is_empty())
            .map_or_else(|| PathBuf::from("yt-dlp"), PathBuf::from);
        let ffmpeg_bin = std::env::var_os("PANDAN_FFMPEG_BIN")
            .filter(|value| !value.is_empty())
            .map_or_else(|| PathBuf::from("ffmpeg"), PathBuf::from);
        let deno_bin = std::env::var_os("PANDAN_DENO_BIN")
            .filter(|value| !value.is_empty())
            .map_or_else(|| PathBuf::from("deno"), PathBuf::from);
        let ffprobe_bin = ffmpeg_bin.with_file_name("ffprobe");
        let yt_dlp_version = probe_version(&yt_dlp_bin, &["--version"]).await;
        let ffmpeg_version = probe_version(&ffmpeg_bin, &["-version"]).await;
        let ffprobe_version = probe_version(&ffprobe_bin, &["-version"]).await;
        let deno_version = probe_version(&deno_bin, &["--version"]).await;
        let unavailable_reason = if !enabled {
            Some("Downloads are disabled by the operator.".to_owned())
        } else if yt_dlp_version.is_none() {
            Some("yt-dlp is not installed or could not be started.".to_owned())
        } else if ffmpeg_version.is_none() {
            Some("ffmpeg is not installed or could not be started.".to_owned())
        } else if ffprobe_version.is_none() {
            Some("ffprobe is not installed beside ffmpeg or could not be started.".to_owned())
        } else if deno_version.is_none() {
            Some("Deno is not installed or could not be started.".to_owned())
        } else {
            None
        };
        let capability = ToolCapability {
            enabled,
            available: unavailable_reason.is_none(),
            yt_dlp_version,
            ffmpeg_version,
            ffprobe_version,
            deno_version,
            unavailable_reason,
        };
        Self {
            yt_dlp_bin,
            ffmpeg_bin,
            deno_bin,
            proxy_url,
            capability,
        }
    }

    #[must_use]
    pub fn capability(&self) -> &ToolCapability {
        &self.capability
    }

    pub async fn inspect(&self, source_url: &str) -> Result<YoutubeInspection, RunFailure> {
        if !self.capability.available {
            return Err(RunFailure::Unavailable);
        }
        let mut command = self.base_command();
        command.args([
            "--skip-download",
            "--dump-single-json",
            "--no-warnings",
            "--",
            source_url,
        ]);
        let output = capture_command(command, INSPECTION_TIMEOUT).await?;
        if !output.status_success || output.stdout_overflow || output.stderr_overflow {
            return Err(RunFailure::Failed);
        }
        let raw: RawInspection =
            serde_json::from_slice(&output.stdout).map_err(|_| RunFailure::Failed)?;
        normalize_inspection(source_url, raw)
    }

    pub async fn download(
        &self,
        job: &YoutubeDownloadJob,
        stage_dir: &Path,
        max_output_bytes: i64,
        mut cancel: watch::Receiver<bool>,
        events: mpsc::Sender<RunnerEvent>,
    ) -> Result<(), RunFailure> {
        if !self.capability.available {
            return Err(RunFailure::Unavailable);
        }
        let mut command = self.base_command();
        let output_template = stage_dir.join("output.%(ext)s");
        let home_path = format!("home:{}", stage_dir.display());
        let temp_path = format!("temp:{}", stage_dir.display());
        command
            .args(["--newline", "--progress-delta", "1", "--max-filesize"])
            .arg(max_output_bytes.to_string())
            .args(["--paths", &home_path])
            .args(["--paths", &temp_path])
            .arg("--output")
            .arg(output_template)
            .args([
                "--progress-template",
                "download:PANDAN_PROGRESS:%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s|%(progress._percent_str)s",
                "--progress-template",
                "postprocess:PANDAN_POST:%(progress.status)s",
            ]);
        apply_profile(&mut command, job);
        command.arg("--").arg(&job.source_url);
        configure_process(&mut command, Some(stage_dir));
        let started = Instant::now();
        let mut child = command.spawn().map_err(|error| {
            warn!(
                job_id = %job.id,
                user_id = %job.user_id,
                %error,
                "YouTube downloader process could not start"
            );
            RunFailure::Unavailable
        })?;
        let process_id = child.id().ok_or(RunFailure::Unavailable)?;
        log_download_started(job, process_id);
        let stdout = child.stdout.take().ok_or(RunFailure::Failed)?;
        let stderr = child.stderr.take().ok_or(RunFailure::Failed)?;
        let stdout_task = tokio::spawn(read_machine_output(stdout, events));
        let stderr_task = tokio::spawn(drain_output(stderr, 64 * 1024));
        let wait = child.wait();
        tokio::pin!(wait);
        let deadline = sleep(DOWNLOAD_TIMEOUT);
        tokio::pin!(deadline);
        let mut termination = None;
        let status = loop {
            tokio::select! {
                result = &mut wait => break result.map_err(|_| RunFailure::Failed)?,
                changed = cancel.changed(), if termination.is_none() => {
                    if changed.is_err() || *cancel.borrow() {
                        terminate_process_group(process_id);
                        termination = Some(RunFailure::Cancelled);
                    }
                }
                () = &mut deadline, if termination.is_none() => {
                    terminate_process_group(process_id);
                    termination = Some(RunFailure::TimedOut);
                }
            }
        };
        let output_overflow = stdout_task
            .await
            .map_err(|_| RunFailure::Failed)?
            .map_err(|_| RunFailure::Failed)?;
        let _ = stderr_task.await;
        if let Some(termination) = termination {
            log_download_terminated(job, termination, started.elapsed());
            return Err(termination);
        }
        if output_overflow {
            log_download_output_limit(job, started.elapsed());
            return Err(RunFailure::OutputLimit);
        }
        if status.success() {
            log_download_completed(job, started.elapsed());
            Ok(())
        } else {
            log_download_failed(job, status.code(), started.elapsed());
            Err(RunFailure::Failed)
        }
    }

    fn base_command(&self) -> Command {
        let mut command = Command::new(&self.yt_dlp_bin);
        command
            .args([
                "--ignore-config",
                "--no-plugin-dirs",
                "--no-update",
                "--no-cache-dir",
                "--no-playlist",
                "--no-remote-components",
                "--use-extractors",
                "youtube",
                "--downloader",
                "native",
                "--proxy",
            ])
            .arg(&self.proxy_url)
            .args([
                "--socket-timeout",
                "20",
                "--retries",
                "3",
                "--fragment-retries",
                "3",
                "--ffmpeg-location",
            ])
            .arg(&self.ffmpeg_bin);
        command
            .arg("--js-runtimes")
            .arg(format!("deno:{}", self.deno_bin.display()));
        configure_process(&mut command, None);
        command
    }
}

fn log_download_started(job: &YoutubeDownloadJob, process_id: u32) {
    info!(
        job_id = %job.id,
        user_id = %job.user_id,
        media_kind = %job.media_kind,
        output_format = %job.output_format,
        max_height = ?job.max_height,
        process_id,
        "YouTube downloader process started"
    );
}

fn log_download_terminated(job: &YoutubeDownloadJob, failure: RunFailure, elapsed: Duration) {
    warn!(
        job_id = %job.id,
        user_id = %job.user_id,
        failure = failure.code(),
        elapsed_ms = elapsed.as_millis(),
        "YouTube downloader process terminated"
    );
}

fn log_download_output_limit(job: &YoutubeDownloadJob, elapsed: Duration) {
    warn!(
        job_id = %job.id,
        user_id = %job.user_id,
        elapsed_ms = elapsed.as_millis(),
        "YouTube downloader diagnostic output exceeded its limit"
    );
}

fn log_download_completed(job: &YoutubeDownloadJob, elapsed: Duration) {
    info!(
        job_id = %job.id,
        user_id = %job.user_id,
        elapsed_ms = elapsed.as_millis(),
        "YouTube downloader process completed"
    );
}

fn log_download_failed(job: &YoutubeDownloadJob, exit_code: Option<i32>, elapsed: Duration) {
    warn!(
        job_id = %job.id,
        user_id = %job.user_id,
        ?exit_code,
        elapsed_ms = elapsed.as_millis(),
        "YouTube downloader process failed"
    );
}

fn apply_profile(command: &mut Command, job: &YoutubeDownloadJob) {
    match (job.media_kind.as_str(), job.output_format.as_str()) {
        ("video", container) => {
            let height = job
                .max_height
                .map(|height| format!("[height<={height}]"))
                .unwrap_or_default();
            let selector = match container {
                "mp4" => {
                    format!("bestvideo*[ext=mp4]{height}+bestaudio[ext=m4a]/best[ext=mp4]{height}")
                }
                "webm" => format!(
                    "bestvideo*[ext=webm]{height}+bestaudio[ext=webm]/best[ext=webm]{height}"
                ),
                _ => format!("bestvideo*{height}+bestaudio/best{height}"),
            };
            command
                .args(["--format", &selector, "--merge-output-format", container])
                .args(["--remux-video", container]);
        }
        ("audio", format) => {
            let selector = if format == "opus" {
                "bestaudio[acodec*=opus]"
            } else if format == "m4a" {
                "bestaudio[ext=m4a]"
            } else {
                "bestaudio"
            };
            command.args([
                "--format",
                selector,
                "--extract-audio",
                "--audio-format",
                format,
            ]);
            if format == "mp3" {
                command.args(["--audio-quality", "0"]);
            }
        }
        _ => {
            command.args(["--format", "best"]);
        }
    }
}

fn configure_process(command: &mut Command, working_dir: Option<&Path>) {
    command
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(working_dir) = working_dir {
        command
            .current_dir(working_dir)
            .env("HOME", working_dir)
            .env("TMPDIR", working_dir);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
}

fn terminate_process_group(process_id: u32) {
    let Ok(process_id) = i32::try_from(process_id) else {
        return;
    };
    let _ = killpg(Pid::from_raw(process_id), Signal::SIGKILL);
}

struct CapturedCommand {
    status_success: bool,
    stdout: Vec<u8>,
    stdout_overflow: bool,
    stderr_overflow: bool,
}

async fn capture_command(
    mut command: Command,
    deadline: Duration,
) -> Result<CapturedCommand, RunFailure> {
    configure_process(&mut command, None);
    let mut child = command.spawn().map_err(|_| RunFailure::Unavailable)?;
    let process_id = child.id().ok_or(RunFailure::Unavailable)?;
    let stdout = child.stdout.take().ok_or(RunFailure::Failed)?;
    let stderr = child.stderr.take().ok_or(RunFailure::Failed)?;
    let operation = async move {
        let (status, stdout, stderr) = tokio::join!(
            child.wait(),
            drain_output(stdout, CAPTURE_LIMIT),
            drain_output(stderr, 64 * 1024)
        );
        let status = status.map_err(|_| RunFailure::Failed)?;
        let (stdout, stdout_overflow) = stdout.map_err(|_| RunFailure::Failed)?;
        let (_, stderr_overflow) = stderr.map_err(|_| RunFailure::Failed)?;
        Ok(CapturedCommand {
            status_success: status.success(),
            stdout,
            stdout_overflow,
            stderr_overflow,
        })
    };
    match timeout(deadline, operation).await {
        Ok(result) => result,
        Err(_) => {
            terminate_process_group(process_id);
            Err(RunFailure::TimedOut)
        }
    }
}

async fn probe_version(executable: &Path, arguments: &[&str]) -> Option<String> {
    let mut command = Command::new(executable);
    command.args(arguments);
    let output = capture_command(command, Duration::from_secs(10))
        .await
        .ok()?;
    if !output.status_success || output.stdout_overflow {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let summary = text
        .lines()
        .take(3)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    (!summary.is_empty()).then(|| summary.chars().take(240).collect())
}

async fn drain_output<R: AsyncRead + Unpin>(
    mut reader: R,
    limit: usize,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut captured = Vec::with_capacity(limit.min(16 * 1024));
    let mut buffer = [0_u8; 4096];
    let mut overflow = false;
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            return Ok((captured, overflow));
        }
        let remaining = limit.saturating_sub(captured.len());
        if remaining > 0 {
            captured.extend_from_slice(&buffer[..count.min(remaining)]);
        }
        overflow |= count > remaining;
    }
}

async fn read_machine_output<R: AsyncRead + Unpin>(
    mut reader: R,
    events: mpsc::Sender<RunnerEvent>,
) -> std::io::Result<bool> {
    let mut chunk = [0_u8; 4096];
    let mut line = Vec::with_capacity(512);
    let mut total = 0_usize;
    let mut output_limit_sent = false;
    let mut line_overflow = false;
    loop {
        let count = reader.read(&mut chunk).await?;
        if count == 0 {
            if !line.is_empty() && !line_overflow {
                dispatch_machine_line(&line, &events).await;
            }
            return Ok(output_limit_sent);
        }
        total = total.saturating_add(count);
        if total > MACHINE_OUTPUT_LIMIT && !output_limit_sent {
            output_limit_sent = true;
            let _ = events.send(RunnerEvent::OutputLimit).await;
        }
        for byte in &chunk[..count] {
            if *byte == b'\n' {
                if !line_overflow {
                    dispatch_machine_line(&line, &events).await;
                }
                line.clear();
                line_overflow = false;
            } else if line.len() < MACHINE_LINE_LIMIT {
                line.push(*byte);
            } else {
                line_overflow = true;
            }
        }
    }
}

async fn dispatch_machine_line(line: &[u8], events: &mpsc::Sender<RunnerEvent>) {
    let Ok(line) = std::str::from_utf8(line) else {
        return;
    };
    let line = line.trim();
    if line.starts_with(POSTPROCESS_PREFIX) {
        let _ = events.send(RunnerEvent::Postprocessing).await;
    } else if let Some(progress) = parse_progress(line) {
        let _ = events.send(RunnerEvent::Progress(progress)).await;
    }
}

fn parse_progress(line: &str) -> Option<DownloadProgress> {
    let fields = line.strip_prefix(PROGRESS_PREFIX)?;
    let mut fields = fields.split('|');
    let downloaded_bytes = parse_i64(fields.next())?.max(0);
    let exact_total = parse_i64(fields.next());
    let estimated_total = parse_i64(fields.next());
    let total = exact_total.or(estimated_total);
    let speed = parse_f64(fields.next());
    let eta = parse_i64(fields.next());
    let printed_percent = fields
        .next()
        .and_then(|value| parse_f64(Some(value.trim().trim_end_matches('%'))));
    let percent = total
        .filter(|total| *total > 0)
        .map(|total| (downloaded_bytes as f64 / total as f64 * 100.0).clamp(0.0, 100.0))
        .or(printed_percent.map(|percent| percent.clamp(0.0, 100.0)));
    Some(DownloadProgress {
        percent,
        downloaded_bytes,
        total_bytes: total.filter(|total| *total >= 0),
        speed_bytes_per_second: speed.filter(|speed| *speed >= 0.0),
        eta_seconds: eta.filter(|eta| *eta >= 0),
    })
}

fn parse_i64(value: Option<&str>) -> Option<i64> {
    value
        .map(str::trim)
        .filter(|value| !matches!(*value, "" | "NA" | "None" | "null"))
        .and_then(|value| value.parse().ok())
}

fn parse_f64(value: Option<&str>) -> Option<f64> {
    value
        .map(str::trim)
        .filter(|value| !matches!(*value, "" | "NA" | "None" | "null"))
        .and_then(|value| value.parse().ok())
}

#[derive(Debug, Deserialize)]
struct RawInspection {
    id: String,
    title: String,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    uploader: String,
    duration: Option<f64>,
    #[serde(default)]
    is_live: bool,
    live_status: Option<String>,
    #[serde(default)]
    formats: Vec<RawFormat>,
    #[serde(default)]
    entries: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawFormat {
    ext: Option<String>,
    height: Option<f64>,
    vcodec: Option<String>,
    acodec: Option<String>,
}

fn normalize_inspection(
    source_url: &str,
    raw: RawInspection,
) -> Result<YoutubeInspection, RunFailure> {
    if !raw.entries.is_empty() || raw.id.is_empty() || raw.title.trim().is_empty() {
        return Err(RunFailure::Failed);
    }
    let is_live = raw.is_live
        || raw
            .live_status
            .as_deref()
            .is_some_and(|status| status != "not_live");
    let mut heights = BTreeSet::new();
    let mut has_video = false;
    let mut has_audio = false;
    let mut has_mp4_video = false;
    let mut has_webm_video = false;
    let mut has_m4a_audio = false;
    let mut has_opus_audio = false;
    for format in raw.formats {
        let video = format
            .vcodec
            .as_deref()
            .is_some_and(|codec| codec != "none");
        let audio = format
            .acodec
            .as_deref()
            .is_some_and(|codec| codec != "none");
        has_video |= video;
        has_audio |= audio;
        has_mp4_video |= video && format.ext.as_deref() == Some("mp4");
        has_webm_video |= video && format.ext.as_deref() == Some("webm");
        has_m4a_audio |= audio && format.ext.as_deref() == Some("m4a");
        has_opus_audio |= audio
            && (format.ext.as_deref() == Some("webm")
                || format
                    .acodec
                    .as_deref()
                    .is_some_and(|codec| codec.contains("opus")));
        if video && let Some(height) = format.height {
            let height = height.round() as i64;
            if (144..=8_640).contains(&height) {
                heights.insert(height);
            }
        }
    }
    let channel_name = if raw.channel.trim().is_empty() {
        raw.uploader
    } else {
        raw.channel
    };
    Ok(YoutubeInspection {
        source_url: source_url.to_owned(),
        video_id: raw.id.chars().take(32).collect(),
        title: raw.title.chars().take(500).collect(),
        channel_name: channel_name.chars().take(300).collect(),
        duration_seconds: raw
            .duration
            .map(|duration| duration.max(0.0).round() as i64),
        is_live,
        available_heights: heights.into_iter().rev().collect(),
        video_formats: [
            (has_mp4_video && has_m4a_audio).then_some("mp4"),
            (has_video && has_audio).then_some("mkv"),
            (has_webm_video && has_opus_audio).then_some("webm"),
        ]
        .into_iter()
        .flatten()
        .map(str::to_owned)
        .collect(),
        audio_formats: [
            has_m4a_audio.then_some("m4a"),
            has_audio.then_some("mp3"),
            has_opus_audio.then_some("opus"),
        ]
        .into_iter()
        .flatten()
        .map(str::to_owned)
        .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_prefers_real_totals_and_bounds_percentages() {
        let progress = parse_progress("PANDAN_PROGRESS:500|1000|1200|25.5|20|50.0%").unwrap();
        assert_eq!(progress.downloaded_bytes, 500);
        assert_eq!(progress.total_bytes, Some(1000));
        assert_eq!(progress.percent, Some(50.0));
        assert_eq!(progress.eta_seconds, Some(20));
    }

    #[test]
    fn profile_arguments_never_include_titles_or_browser_options() {
        let job = YoutubeDownloadJob {
            id: "job".into(),
            user_id: "user".into(),
            source_url: "https://www.youtube.com/watch?v=abcdefghijk".into(),
            youtube_video_id: "abcdefghijk".into(),
            title: "--exec dangerous".into(),
            channel_name: "channel".into(),
            duration_seconds: Some(60),
            media_kind: "video".into(),
            output_format: "mp4".into(),
            max_height: Some(1080),
            status: "queued".into(),
            progress_percent: None,
            downloaded_bytes: 0,
            total_bytes: None,
            speed_bytes_per_second: None,
            eta_seconds: None,
            storage_file_name: String::new(),
            display_file_name: String::new(),
            mime_type: String::new(),
            byte_size: 0,
            attempts: 0,
            error_code: None,
            last_error: None,
            lease_started_at: None,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
            updated_at: String::new(),
        };
        let mut command = Command::new("yt-dlp");
        apply_profile(&mut command, &job);
        let arguments = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            !arguments
                .iter()
                .any(|argument| argument.contains("--exec dangerous"))
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument.contains("height<=1080"))
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument.contains("ext=mp4"))
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument.contains("ext=m4a"))
        );
    }

    #[test]
    fn base_command_disables_dynamic_and_external_behavior() {
        let runner = YtDlpRunner {
            yt_dlp_bin: "yt-dlp".into(),
            ffmpeg_bin: "/usr/bin/ffmpeg".into(),
            deno_bin: "/usr/local/bin/deno".into(),
            proxy_url: "http://127.0.0.1:1234".to_owned(),
            capability: ToolCapability {
                enabled: true,
                available: true,
                yt_dlp_version: Some("test".to_owned()),
                ffmpeg_version: Some("test".to_owned()),
                ffprobe_version: Some("test".to_owned()),
                deno_version: Some("test".to_owned()),
                unavailable_reason: None,
            },
        };
        let command = runner.base_command();
        let arguments = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        for required in [
            "--ignore-config",
            "--no-plugin-dirs",
            "--no-update",
            "--no-playlist",
            "--no-remote-components",
            "--use-extractors",
            "youtube",
            "--downloader",
            "native",
        ] {
            assert!(arguments.iter().any(|argument| argument == required));
        }
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "deno:/usr/local/bin/deno")
        );
    }

    #[test]
    fn inspection_reports_only_compatible_output_containers() {
        let inspection = normalize_inspection(
            "https://www.youtube.com/watch?v=abcdefghijk",
            RawInspection {
                id: "abcdefghijk".to_owned(),
                title: "Video".to_owned(),
                channel: "Channel".to_owned(),
                uploader: String::new(),
                duration: Some(60.0),
                is_live: false,
                live_status: Some("not_live".to_owned()),
                entries: Vec::new(),
                formats: vec![
                    RawFormat {
                        ext: Some("mp4".to_owned()),
                        height: Some(1080.0),
                        vcodec: Some("avc1".to_owned()),
                        acodec: Some("none".to_owned()),
                    },
                    RawFormat {
                        ext: Some("m4a".to_owned()),
                        height: None,
                        vcodec: Some("none".to_owned()),
                        acodec: Some("mp4a".to_owned()),
                    },
                ],
            },
        )
        .expect("inspection normalizes");
        assert_eq!(inspection.available_heights, vec![1080]);
        assert_eq!(inspection.video_formats, vec!["mp4", "mkv"]);
        assert_eq!(inspection.audio_formats, vec!["m4a", "mp3"]);
    }
}
