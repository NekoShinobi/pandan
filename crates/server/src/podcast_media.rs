//! Podcast media: guarded remote fetching, on-disk episode storage, and the two
//! background workers that keep the catalogue and the cache current.
//!
//! Episode audio is the one Pandan asset that does not live in `SQLite`. Files are written
//! under the media root so playback can be answered with `NamedFile`, which streams and
//! serves HTTP Range requests without materialising a whole episode in memory.
//!
//! The shared `WidgetIntegrationService` client is deliberately not reused here. It is
//! built with `redirect::Policy::none()`, a ten-second total timeout, and a two-megabyte
//! response cap — all correct for small cached widget payloads and all wrong for podcasts,
//! whose enclosures redirect through tracking prefixes and run to hundreds of megabytes.
//! This module owns clients tuned for that, and re-runs the shared SSRF guard on every
//! redirect hop rather than letting reqwest follow them unchecked.

use super::AppState;
use super::network_policy::{NetworkAccessScope, NetworkPolicy};
use actix_web::web;
use chrono::{Duration as ChronoDuration, Utc};
use db::entities::{
    PodcastArtworkDraft, PodcastEpisodeDraft, PodcastFeedPreview, PodcastRefreshTarget,
};
use futures_util::{StreamExt, stream};
use quick_xml::{Reader, events::Event};
use reqwest::{Client, ClientBuilder, Response, StatusCode, header};
use sqlx::SqlitePool;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::io::AsyncWriteExt;
use tokio::time::{Duration, sleep};
use tracing::{info, warn};

/// Feed bodies. Long-running shows with full show notes routinely pass the two megabytes
/// the shared widget client allows.
const MAX_FEED_BYTES: usize = 15 * 1024 * 1024;
const MAX_ARTWORK_BYTES: usize = 4 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const CONNECT_TIMEOUT_SECONDS: u64 = 5;
const FEED_TIMEOUT_SECONDS: u64 = 20;
/// Abort a transfer that has produced no bytes for this long, instead of a total timeout
/// that a large but healthy episode would trip.
const AUDIO_STALL_TIMEOUT_SECONDS: u64 = 30;
/// Hard ceiling on a single transfer, so a trickling server cannot hold the worker open.
const AUDIO_WALL_CLOCK_SECONDS: i64 = 30 * 60;
const PROGRESS_INTERVAL_BYTES: i64 = 2 * 1024 * 1024;

const REFRESH_HOURS: i64 = 4;
const REFRESH_LEASE_MINUTES: i64 = 15;
const REFRESH_BATCH_SIZE: i64 = 25;
const REFRESH_IDLE_SECONDS: u64 = 600;
const MANUAL_REFRESH_COOLDOWN_SECONDS: i64 = 30;
const MANUAL_REFRESH_CONCURRENCY: usize = 4;
const DOWNLOAD_IDLE_SECONDS: u64 = 20;
const DOWNLOAD_LEASE_MINUTES: i64 = 60;
const DOWNLOAD_MAX_ATTEMPTS: i64 = 3;
const NOTIFICATION_IDLE_SECONDS: u64 = 20;
const NOTIFICATION_LEASE_MINUTES: i64 = 5;
const NOTIFICATION_MAX_ATTEMPTS: i64 = 3;
const ARTWORK_CACHE_HOURS: i64 = 24;
const MAX_EPISODES_PER_REFRESH: usize = 500;

const PARTIAL_DIR: &str = ".partial";

/// Remote fetching and on-disk storage for podcast audio and artwork.
#[derive(Debug, Clone)]
pub struct PodcastMedia {
    network_policy: NetworkPolicy,
    root: PathBuf,
}

#[derive(Debug, Clone, Copy)]
enum PodcastClientKind {
    Feed,
    Audio,
}

impl PodcastMedia {
    /// Prepares the media root and the HTTP clients.
    ///
    /// # Errors
    ///
    /// Returns a message when the media root cannot be created or written, or when the
    /// HTTP clients cannot be built.
    pub fn from_env(pool: SqlitePool) -> Result<Self, String> {
        let root = std::env::var("PANDAN_MEDIA_DIR")
            .map_or_else(|_| PathBuf::from("data/podcasts"), PathBuf::from);
        Self::with_root_and_policy(root, NetworkPolicy::new(pool))
    }

    /// Prepares the media root and the HTTP clients at an explicit location.
    ///
    /// # Errors
    ///
    /// Returns a message when the media root cannot be created or written, or when the
    /// HTTP clients cannot be built.
    pub fn with_root_and_pool(root: PathBuf, pool: SqlitePool) -> Result<Self, String> {
        Self::with_root_and_policy(root, NetworkPolicy::new(pool))
    }

    #[cfg(test)]
    pub fn with_root(root: PathBuf) -> Result<Self, String> {
        Self::with_root_and_policy(root, NetworkPolicy::without_rules())
    }

    fn with_root_and_policy(root: PathBuf, network_policy: NetworkPolicy) -> Result<Self, String> {
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("podcast media directory could not be created: {error}"))?;
        std::fs::create_dir_all(root.join(PARTIAL_DIR))
            .map_err(|error| format!("podcast partial directory could not be created: {error}"))?;
        let probe = root.join(PARTIAL_DIR).join(".writable");
        std::fs::write(&probe, b"")
            .map_err(|error| format!("podcast media directory is not writable: {error}"))?;
        let _ = std::fs::remove_file(&probe);

        podcast_client_builder(PodcastClientKind::Feed)
            .build()
            .map_err(|error| format!("podcast feed client could not be built: {error}"))?;
        podcast_client_builder(PodcastClientKind::Audio)
            .build()
            .map_err(|error| format!("podcast audio client could not be built: {error}"))?;

        Ok(Self {
            network_policy,
            root,
        })
    }

    /// The directory cached episodes are written to.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves one stored file name inside the media root.
    ///
    /// File names are generated by this module from an episode identifier and an
    /// allowlisted extension, never from a remote URL or header. This check rejects
    /// anything that acquired a separator anyway, so a corrupted row cannot escape the
    /// media root.
    ///
    /// # Errors
    ///
    /// Returns a message when the file name is not a plain name inside the media root.
    pub fn resolve(&self, file_name: &str) -> Result<PathBuf, String> {
        if file_name.is_empty()
            || file_name.contains('/')
            || file_name.contains('\\')
            || file_name.contains("..")
        {
            return Err("cached file name is invalid".to_owned());
        }
        Ok(self.root.join(file_name))
    }

    /// Removes one cached file, tolerating a file that is already gone.
    pub async fn remove(&self, file_name: &str) {
        let Ok(path) = self.resolve(file_name) else {
            warn!(%file_name, "refusing to remove an invalid cached file name");
            return;
        };
        if let Err(error) = tokio::fs::remove_file(&path).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            warn!(%file_name, %error, "cached episode could not be removed");
        }
    }

    /// Fetches and parses one podcast feed.
    ///
    /// # Errors
    ///
    /// Returns a safe message when the URL, transfer, or feed body is unusable.
    pub async fn fetch_feed(
        &self,
        source: &str,
    ) -> Result<(PodcastFeedPreview, Vec<PodcastEpisodeDraft>), String> {
        let response = self
            .get_following_redirects(source, PodcastClientKind::Feed)
            .await?;
        let bytes = read_bounded(response, MAX_FEED_BYTES).await?;
        let feed = feed_rs::parser::parse(&bytes[..])
            .map_err(|error| format!("podcast feed could not be parsed: {error}"))?;

        let title = feed
            .title
            .as_ref()
            .map(|value| value.content.trim().to_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "podcast feed has no title".to_owned())?;
        let preview = PodcastFeedPreview {
            title: truncate(&title, 300),
            description: truncate(
                &feed
                    .description
                    .as_ref()
                    .map_or_else(String::new, |value| value.content.trim().to_owned()),
                2000,
            ),
            author: truncate(
                &feed
                    .authors
                    .first()
                    .map_or_else(String::new, |person| person.name.trim().to_owned()),
                200,
            ),
            site_url: feed
                .links
                .iter()
                .find(|link| link.rel.as_deref() != Some("self"))
                .map_or_else(String::new, |link| truncate(&link.href, 2048)),
            language: truncate(feed.language.as_deref().unwrap_or_default(), 32),
            artwork_url: feed
                .logo
                .as_ref()
                .or(feed.icon.as_ref())
                .map_or_else(String::new, |image| truncate(&image.uri, 2048)),
        };

        // `feed-rs` only understands the HH:MM:SS and plain-seconds forms of
        // itunes:duration; it reads the common MM:SS form as its leading number, turning
        // a 36:12 episode into 36 seconds. Recover the real value from the same bytes.
        let durations = scan_itunes_durations(&bytes);
        let aligned = durations.len() == feed.entries.len();
        let episodes = feed
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let mut draft = episode_draft(entry)?;
                if aligned && let Some(Some(seconds)) = durations.get(index).copied() {
                    draft.duration_seconds = Some(seconds);
                }
                Some(draft)
            })
            .take(MAX_EPISODES_PER_REFRESH)
            .collect::<Vec<_>>();

        Ok((preview, episodes))
    }

    /// Fetches one bounded artwork image for the shared cache.
    ///
    /// # Errors
    ///
    /// Returns a safe message when the URL, media type, or response is unusable. A failed
    /// fetch is never written, so the cache cannot be poisoned by a bad response.
    pub async fn fetch_artwork(&self, source: &str) -> Result<PodcastArtworkDraft, String> {
        let response = self
            .get_following_redirects(source, PodcastClientKind::Feed)
            .await?;
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .filter(|value| {
                matches!(
                    *value,
                    "image/jpeg" | "image/png" | "image/webp" | "image/avif"
                )
            })
            .ok_or_else(|| "podcast artwork type is unsupported".to_owned())?
            .to_owned();
        let data = read_bounded(response, MAX_ARTWORK_BYTES).await?;
        if data.is_empty() {
            return Err("podcast artwork was empty".to_owned());
        }
        Ok(PodcastArtworkDraft {
            source_url: source.to_owned(),
            content_type,
            data,
        })
    }

    /// Downloads one episode to the media root, reporting progress as it streams.
    ///
    /// The transfer is written to a temporary file inside the media root and renamed into
    /// place only once it is complete and flushed, so an interrupted download can never be
    /// mistaken for a playable episode. The partial directory lives inside the media root
    /// deliberately: the production container mounts only that volume writable, and a
    /// rename across filesystems would not be atomic.
    ///
    /// # Errors
    ///
    /// Returns a safe message when the URL, media type, size, or transfer is unusable.
    pub async fn download_episode(
        &self,
        pool: &sqlx::SqlitePool,
        episode_id: &str,
        source: &str,
        declared_type: &str,
        max_bytes: i64,
    ) -> Result<(String, String, i64), String> {
        let response = self
            .get_following_redirects(source, PodcastClientKind::Audio)
            .await?;

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| audio_extension(value).is_some())
            .or_else(|| {
                let declared = declared_type.trim().to_ascii_lowercase();
                audio_extension(&declared).map(|_| declared)
            })
            .ok_or_else(|| "episode media type is unsupported".to_owned())?;
        let extension = audio_extension(&content_type)
            .ok_or_else(|| "episode media type is unsupported".to_owned())?;

        // Reject on the declared length before a single byte is written...
        if let Some(declared_length) = response.content_length()
            && i64::try_from(declared_length).unwrap_or(i64::MAX) > max_bytes
        {
            return Err("episode is larger than the configured limit".to_owned());
        }

        let file_name = format!("{episode_id}.{extension}");
        let partial_path = self
            .root
            .join(PARTIAL_DIR)
            .join(format!("{episode_id}.part"));
        let final_path = self.resolve(&file_name)?;
        let expected = response
            .content_length()
            .and_then(|length| i64::try_from(length).ok())
            .unwrap_or(0);

        let outcome = self
            .stream_to_partial(
                pool,
                episode_id,
                response,
                &partial_path,
                max_bytes,
                expected,
            )
            .await;
        let written = match outcome {
            Ok(written) => written,
            Err(error) => {
                let _ = tokio::fs::remove_file(&partial_path).await;
                return Err(error);
            }
        };

        tokio::fs::rename(&partial_path, &final_path)
            .await
            .map_err(|error| format!("episode could not be stored: {error}"))?;
        Ok((file_name, content_type, written))
    }

    /// Streams one response body into the partial file, enforcing the size ceiling as it
    /// goes rather than trusting the declared length.
    async fn stream_to_partial(
        &self,
        pool: &sqlx::SqlitePool,
        episode_id: &str,
        mut response: Response,
        partial_path: &Path,
        max_bytes: i64,
        expected: i64,
    ) -> Result<i64, String> {
        let mut file = tokio::fs::File::create(partial_path)
            .await
            .map_err(|error| format!("episode file could not be opened: {error}"))?;
        let deadline = Utc::now() + ChronoDuration::seconds(AUDIO_WALL_CLOCK_SECONDS);
        let mut written: i64 = 0;
        let mut reported: i64 = 0;

        loop {
            let chunk = response
                .chunk()
                .await
                .map_err(|error| format!("episode transfer failed: {error}"))?;
            let Some(chunk) = chunk else { break };
            written += i64::try_from(chunk.len()).unwrap_or(i64::MAX);
            // ...and again while streaming, so a lying Content-Length cannot fill the disk.
            if written > max_bytes {
                return Err("episode is larger than the configured limit".to_owned());
            }
            if Utc::now() > deadline {
                return Err("episode transfer exceeded the time limit".to_owned());
            }
            file.write_all(&chunk)
                .await
                .map_err(|error| format!("episode could not be written: {error}"))?;
            if written - reported >= PROGRESS_INTERVAL_BYTES {
                reported = written;
                let _ = db::queries::update_podcast_download_progress(
                    pool,
                    episode_id,
                    written,
                    expected.max(written),
                )
                .await;
            }
        }

        file.flush()
            .await
            .map_err(|error| format!("episode could not be flushed: {error}"))?;
        file.sync_all()
            .await
            .map_err(|error| format!("episode could not be flushed: {error}"))?;
        drop(file);

        if written == 0 {
            return Err("episode transfer was empty".to_owned());
        }
        Ok(written)
    }

    /// Issues a GET, revalidating every redirect hop against the shared SSRF policy.
    ///
    /// reqwest's own redirect following is disabled on purpose. A tracking prefix that
    /// 302s to a private address must be refused, and only a manual loop can re-run the
    /// DNS and address checks on the destination.
    async fn get_following_redirects(
        &self,
        source: &str,
        kind: PodcastClientKind,
    ) -> Result<Response, String> {
        let mut current = source.to_owned();
        let request_kind = match kind {
            PodcastClientKind::Feed => "feed",
            PodcastClientKind::Audio => "audio",
        };
        let started = Instant::now();
        for redirect_count in 0..MAX_REDIRECTS {
            let target = self
                .network_policy
                .validate(&current, NetworkAccessScope::Podcasts)
                .await?;
            let client = target.build_client(podcast_client_builder(kind))?;
            let current_url = target.into_url();
            let origin = current_url.origin().ascii_serialization();
            let response = client
                .get(current_url.clone())
                .send()
                .await
                .map_err(|error| {
                    warn!(
                        %origin,
                        request_kind,
                        redirect_count,
                        elapsed_ms = started.elapsed().as_millis(),
                        timed_out = error.is_timeout(),
                        connect_error = error.is_connect(),
                        "podcast upstream transport failed"
                    );
                    request_message(&error)
                })?;
            let status = response.status();
            if !status.is_redirection() {
                if status.is_success() {
                    tracing::debug!(
                        %origin,
                        request_kind,
                        status = status.as_u16(),
                        redirect_count,
                        elapsed_ms = started.elapsed().as_millis(),
                        "podcast upstream request completed"
                    );
                    return Ok(response);
                }
                warn!(
                    %origin,
                    request_kind,
                    status = status.as_u16(),
                    redirect_count,
                    elapsed_ms = started.elapsed().as_millis(),
                    "podcast upstream request was rejected"
                );
                return response
                    .error_for_status()
                    .map_err(|error| request_message(&error));
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "redirect did not name a destination".to_owned())?;
            let next = current_url
                .join(location)
                .map_err(|_| "redirect destination is invalid".to_owned())?;
            tracing::debug!(
                %origin,
                request_kind,
                status = status.as_u16(),
                redirect_count,
                "following podcast upstream redirect"
            );
            current = next.into();
        }
        warn!(
            request_kind,
            redirects = MAX_REDIRECTS,
            "podcast upstream exceeded the redirect limit"
        );
        Err("too many redirects".to_owned())
    }
}

fn podcast_client_builder(kind: PodcastClientKind) -> ClientBuilder {
    let builder = Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
        .redirect(reqwest::redirect::Policy::none());
    match kind {
        PodcastClientKind::Feed => builder.timeout(Duration::from_secs(FEED_TIMEOUT_SECONDS)),
        PodcastClientKind::Audio => {
            builder.read_timeout(Duration::from_secs(AUDIO_STALL_TIMEOUT_SECONDS))
        }
    }
}

/// Collects one `itunes:duration` per feed item, in document order.
///
/// Returned positionally so the caller can align it with the parsed entries; the caller
/// discards the whole scan if the counts disagree rather than risk misattributing a
/// duration to the wrong episode.
fn scan_itunes_durations(bytes: &[u8]) -> Vec<Option<i64>> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut durations = Vec::new();
    let mut in_item = false;
    let mut capturing = false;
    let mut pending = None;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => match element.name().as_ref() {
                b"item" | b"entry" => {
                    in_item = true;
                    pending = None;
                }
                b"itunes:duration" if in_item => capturing = true,
                _ => {}
            },
            Ok(Event::Text(text)) if capturing => {
                pending = text
                    .decode()
                    .ok()
                    .and_then(|value| parse_itunes_duration(value.as_ref()));
                capturing = false;
            }
            Ok(Event::CData(text)) if capturing => {
                pending = std::str::from_utf8(&text)
                    .ok()
                    .and_then(parse_itunes_duration);
                capturing = false;
            }
            Ok(Event::End(element)) => match element.name().as_ref() {
                b"item" | b"entry" => {
                    durations.push(pending.take());
                    in_item = false;
                }
                b"itunes:duration" => capturing = false,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }
    durations
}

/// Parses `SS`, `MM:SS`, or `HH:MM:SS` into seconds.
fn parse_itunes_duration(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let mut total: i64 = 0;
    let mut parts = 0;
    for part in value.split(':') {
        let component: i64 = part.trim().parse().ok()?;
        if component < 0 {
            return None;
        }
        total = total.checked_mul(60)?.checked_add(component)?;
        parts += 1;
        if parts > 3 {
            return None;
        }
    }
    Some(total)
}

/// Maps one parsed feed entry to an indexable episode, skipping entries with no audio.
fn episode_draft(entry: &feed_rs::model::Entry) -> Option<PodcastEpisodeDraft> {
    let (object, content) = entry.media.iter().find_map(|object| {
        object
            .content
            .iter()
            .find(|content| {
                content.url.is_some()
                    && content.content_type.as_ref().is_some_and(|media_type| {
                        media_type.ty().as_str().eq_ignore_ascii_case("audio")
                    })
            })
            .map(|content| (object, content))
    })?;
    let enclosure_url = content.url.as_ref()?.to_string();
    let title = entry
        .title
        .as_ref()
        .map(|value| value.content.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Untitled episode".to_owned());
    // Some feeds omit a guid. Falling back to the enclosure URL keeps re-indexing
    // idempotent instead of duplicating every episode on each refresh.
    let guid = if entry.id.trim().is_empty() {
        enclosure_url.clone()
    } else {
        entry.id.trim().to_owned()
    };
    let description = entry
        .summary
        .as_ref()
        .map(|value| value.content.clone())
        .or_else(|| entry.content.as_ref().and_then(|value| value.body.clone()))
        .unwrap_or_default();
    Some(PodcastEpisodeDraft {
        guid: truncate(&guid, 2048),
        title: truncate(&title, 500),
        description: truncate(description.trim(), 5000),
        episode_url: entry
            .links
            .first()
            .map_or_else(String::new, |link| truncate(&link.href, 2048)),
        enclosure_url: truncate(&enclosure_url, 2048),
        enclosure_type: content
            .content_type
            .as_ref()
            .map_or_else(String::new, |media_type| truncate(media_type.as_ref(), 120)),
        enclosure_bytes: content.size.and_then(|size| i64::try_from(size).ok()),
        duration_seconds: content
            .duration
            .or(object.duration)
            .and_then(|duration| i64::try_from(duration.as_secs()).ok()),
        published_at: entry
            .published
            .or(entry.updated)
            .unwrap_or_else(Utc::now)
            .to_rfc3339(),
    })
}

/// Maps a media type to the extension cached files are stored under.
///
/// This allowlist is the only source of a stored file's extension. Nothing from a remote
/// URL or `Content-Disposition` header reaches the filesystem.
fn audio_extension(content_type: &str) -> Option<&'static str> {
    match content_type {
        "audio/mpeg" | "audio/mp3" | "audio/x-mp3" | "audio/mpeg3" => Some("mp3"),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => Some("m4a"),
        "audio/aac" | "audio/aacp" => Some("aac"),
        "audio/ogg" | "audio/opus" | "application/ogg" => Some("ogg"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        _ => None,
    }
}

/// Reads a bounded response body without trusting the declared length.
async fn read_bounded(mut response: Response, max_bytes: usize) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err("provider response was too large".to_owned());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| request_message(&error))?
    {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err("provider response was too large".to_owned());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Renders a transfer failure without leaking an upstream URL or response body.
fn request_message(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        return "provider request timed out".to_owned();
    }
    if error.is_connect() {
        return "provider could not be reached".to_owned();
    }
    error.status().map_or_else(
        || "provider request failed".to_owned(),
        |status: StatusCode| format!("provider responded with status {}", status.as_u16()),
    )
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

// ---------------------------------------------------------------------------
// Startup reconciliation
// ---------------------------------------------------------------------------

/// Reconciles the media root against the database on startup.
///
/// Three things can drift while the process is not running: a transfer interrupted
/// mid-flight, a file removed from the volume behind the application's back, and a
/// leftover partial. This is what makes a crash during a download survivable.
pub async fn reconcile_media(state: &AppState) {
    match db::queries::reset_interrupted_podcast_downloads(&state.pool).await {
        Ok(0) => {}
        Ok(count) => info!(count, "requeued podcast downloads interrupted by shutdown"),
        Err(error) => warn!(%error, "interrupted podcast downloads could not be requeued"),
    }

    let partial_dir = state.podcast_media.root().join(PARTIAL_DIR);
    if let Ok(mut entries) = tokio::fs::read_dir(&partial_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Err(error) = tokio::fs::remove_file(entry.path()).await {
                warn!(%error, "leftover podcast partial could not be removed");
            }
        }
    }

    let expected = match db::queries::list_podcast_cached_file_names(&state.pool).await {
        Ok(names) => names,
        Err(error) => {
            warn!(%error, "cached podcast files could not be listed");
            return;
        }
    };

    // Rows whose file has vanished are marked failed so they can be fetched again.
    for file_name in &expected {
        let Ok(path) = state.podcast_media.resolve(file_name) else {
            continue;
        };
        if !tokio::fs::try_exists(&path).await.unwrap_or(false)
            && let Err(error) =
                db::queries::invalidate_missing_podcast_download(&state.pool, file_name).await
        {
            warn!(%error, "missing cached podcast file could not be invalidated");
        }
    }

    // Files nothing points at are removed so the volume cannot grow without bound.
    let known = expected
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    if let Ok(mut entries) = tokio::fs::read_dir(state.podcast_media.root()).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == PARTIAL_DIR || known.contains(&name) {
                continue;
            }
            if entry.path().is_dir() {
                continue;
            }
            info!(%name, "removing orphaned podcast file");
            if let Err(error) = tokio::fs::remove_file(entry.path()).await {
                warn!(%name, %error, "orphaned podcast file could not be removed");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Workers
// ---------------------------------------------------------------------------

/// Starts the feed refresh, episode download, and notification workers.
///
/// Reconciliation runs to completion before either worker starts. The orphan sweep
/// removes every file no `ready` row points at, so a download finishing while the sweep
/// was mid-flight would have its freshly renamed file deleted.
pub fn spawn_podcast_workers(state: web::Data<AppState>) {
    tokio::spawn(async move {
        reconcile_media(&state).await;

        let download_state = state.clone();
        tokio::spawn(async move {
            loop {
                match run_next_download(&download_state).await {
                    Ok(true) => {}
                    Ok(false) => sleep(Duration::from_secs(DOWNLOAD_IDLE_SECONDS)).await,
                    Err(error) => {
                        warn!(%error, "podcast download worker failed");
                        sleep(Duration::from_secs(DOWNLOAD_IDLE_SECONDS)).await;
                    }
                }
            }
        });

        let notification_state = state.clone();
        tokio::spawn(async move {
            loop {
                match run_next_notification(&notification_state).await {
                    Ok(true) => sleep(Duration::from_secs(1)).await,
                    Ok(false) => sleep(Duration::from_secs(NOTIFICATION_IDLE_SECONDS)).await,
                    Err(error) => {
                        warn!(%error, "podcast notification worker failed");
                        sleep(Duration::from_secs(NOTIFICATION_IDLE_SECONDS)).await;
                    }
                }
            }
        });

        loop {
            let refreshed = run_refresh_batch(&state).await;
            if refreshed == 0 {
                sleep(Duration::from_secs(REFRESH_IDLE_SECONDS)).await;
            }
        }
    });
}

/// Publishes at most one due new-episode notification through the listener's ntfy route.
async fn run_next_notification(state: &AppState) -> Result<bool, String> {
    let abandoned_before =
        (Utc::now() - ChronoDuration::minutes(NOTIFICATION_LEASE_MINUTES)).to_rfc3339();
    let Some(job) = db::queries::claim_podcast_notification(
        &state.pool,
        &abandoned_before,
        NOTIFICATION_MAX_ATTEMPTS,
    )
    .await
    .map_err(|_| "podcast notification could not be claimed".to_owned())?
    else {
        return Ok(false);
    };

    let title = format!("New episode · {}", job.podcast_title);
    let click = url::Url::parse(&job.episode_url).ok().and_then(|url| {
        (matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none())
        .then_some(job.episode_url.as_str())
    });
    match state
        .widget_integrations
        .publish_ntfy_notification(&crate::widget_integrations::NtfyPublishRequest {
            account_id: &job.user_id,
            base_url: &job.base_url,
            topic: &job.topic,
            title: &title,
            message: &job.episode_title,
            click,
            encrypted_token: job.token_ciphertext.as_deref(),
        })
        .await
    {
        Ok(()) => {
            db::queries::mark_podcast_notification_delivered(
                &state.pool,
                &job.user_id,
                &job.episode_id,
            )
            .await
            .map_err(|_| "podcast notification delivery could not be recorded".to_owned())?;
            info!(episode = %job.episode_id, "published podcast ntfy notification");
        }
        Err(error) => {
            let retry_at =
                (Utc::now() + ChronoDuration::minutes(job.attempts.clamp(1, 10))).to_rfc3339();
            db::queries::mark_podcast_notification_failed(
                &state.pool,
                &job.user_id,
                &job.episode_id,
                &error,
                &retry_at,
            )
            .await
            .map_err(|_| "podcast notification failure could not be recorded".to_owned())?;
            warn!(episode = %job.episode_id, attempt = job.attempts, "podcast ntfy publish failed");
        }
    }
    Ok(true)
}

/// Refreshes one batch of due podcasts, reporting how many were processed.
async fn run_refresh_batch(state: &AppState) -> usize {
    let due_before = (Utc::now() - ChronoDuration::hours(REFRESH_HOURS)).to_rfc3339();
    let abandoned_before =
        (Utc::now() - ChronoDuration::minutes(REFRESH_LEASE_MINUTES)).to_rfc3339();
    let targets = match db::queries::list_due_podcasts(
        &state.pool,
        &due_before,
        &abandoned_before,
        REFRESH_BATCH_SIZE,
    )
    .await
    {
        Ok(targets) => targets,
        Err(error) => {
            warn!(%error, "due podcasts could not be loaded");
            return 0;
        }
    };

    let mut processed = 0;
    for target in targets {
        match db::queries::claim_podcast_refresh(
            &state.pool,
            &target.id,
            &due_before,
            &abandoned_before,
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => {
                warn!(%error, "podcast refresh could not be claimed");
                continue;
            }
        }
        processed += 1;
        // Per-podcast failure isolation: one unreachable feed must not stall the batch.
        let outcome = refresh_podcast(state, &target).await;
        let error = outcome.err();
        if let Some(message) = error.as_deref() {
            warn!(podcast = %target.id, %message, "podcast refresh failed");
        }
        if let Err(error) =
            db::queries::finish_podcast_refresh(&state.pool, &target.id, error.as_deref()).await
        {
            warn!(%error, "podcast refresh state could not be recorded");
        }
        sleep(Duration::from_secs(1)).await;
    }
    processed
}

/// Immediately refreshes the feeds the authenticated listener is subscribed to.
///
/// The normal worker retains its four-hour cadence. This path bypasses that cadence for
/// an explicit user action, while the database still enforces subscription ownership,
/// the cross-process lease, and a short repeat-request cooldown.
///
/// Upstream failures remain isolated per show and are exposed through the existing
/// `last_error` field. Database failures are returned to the request handler.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when targets, claims, or refresh state cannot be
/// read or stored.
pub async fn refresh_subscribed_podcasts(
    state: &AppState,
    user_id: &str,
) -> Result<usize, sqlx::Error> {
    let targets = db::queries::list_subscribed_podcasts_for_refresh(&state.pool, user_id).await?;
    let refreshed_before =
        (Utc::now() - ChronoDuration::seconds(MANUAL_REFRESH_COOLDOWN_SECONDS)).to_rfc3339();
    let abandoned_before =
        (Utc::now() - ChronoDuration::minutes(REFRESH_LEASE_MINUTES)).to_rfc3339();

    let outcomes = stream::iter(targets)
        .map(|target| {
            let refreshed_before = refreshed_before.clone();
            let abandoned_before = abandoned_before.clone();
            async move {
                if !db::queries::claim_subscribed_podcast_refresh(
                    &state.pool,
                    user_id,
                    &target.id,
                    &refreshed_before,
                    &abandoned_before,
                )
                .await?
                {
                    return Ok(false);
                }

                let outcome = refresh_podcast(state, &target).await;
                let error = outcome.err();
                if let Some(message) = error.as_deref() {
                    warn!(podcast = %target.id, %message, "manual podcast refresh failed");
                }
                db::queries::finish_podcast_refresh(&state.pool, &target.id, error.as_deref())
                    .await?;
                Ok(true)
            }
        })
        .buffer_unordered(MANUAL_REFRESH_CONCURRENCY)
        .collect::<Vec<Result<bool, sqlx::Error>>>()
        .await;

    let mut processed = 0;
    for outcome in outcomes {
        if outcome? {
            processed += 1;
        }
    }
    Ok(processed)
}

/// Re-indexes one podcast, then applies its retention and auto-download settings.
async fn refresh_podcast(state: &AppState, target: &PodcastRefreshTarget) -> Result<(), String> {
    let (preview, episodes) = state.podcast_media.fetch_feed(&target.feed_url).await?;
    db::queries::update_podcast_metadata(&state.pool, &target.id, &preview)
        .await
        .map_err(|_| "podcast metadata could not be stored".to_owned())?;

    let discovered = db::queries::upsert_podcast_episodes(&state.pool, &target.id, &episodes)
        .await
        .map_err(|_| "podcast episodes could not be indexed".to_owned())?;
    if !discovered.is_empty() {
        info!(podcast = %target.id, count = discovered.len(), "indexed new episodes");
    }

    refresh_artwork(state, &target.id, &preview.artwork_url).await;

    let Some(podcast) = db::queries::get_podcast(&state.pool, &target.id)
        .await
        .map_err(|_| "podcast could not be reloaded".to_owned())?
    else {
        return Ok(());
    };

    let orphaned =
        db::queries::trim_podcast_episodes(&state.pool, &target.id, podcast.max_retained_episodes)
            .await
            .map_err(|_| "podcast retention could not be applied".to_owned())?;
    for file_name in &orphaned {
        state.podcast_media.remove(file_name).await;
    }

    // Keep the newest episodes warm, since those are what people press play on.
    if podcast.auto_download_count > 0 {
        let newest = db::queries::list_newest_episode_ids(
            &state.pool,
            &target.id,
            podcast.auto_download_count,
        )
        .await
        .map_err(|_| "podcast auto-download set could not be loaded".to_owned())?;
        for episode_id in newest {
            if let Err(error) =
                db::queries::enqueue_podcast_download(&state.pool, &episode_id, None).await
            {
                warn!(%error, "podcast auto-download could not be queued");
            }
        }
    }
    Ok(())
}

/// Refetches artwork at most once a day, never caching a failed response.
pub(crate) async fn refresh_artwork(state: &AppState, podcast_id: &str, artwork_url: &str) {
    if artwork_url.is_empty() {
        return;
    }
    let stale_before = (Utc::now() - ChronoDuration::hours(ARTWORK_CACHE_HOURS)).to_rfc3339();
    match db::queries::podcast_artwork_is_stale(&state.pool, podcast_id, &stale_before).await {
        Ok(true) => {}
        Ok(false) => return,
        Err(error) => {
            warn!(%error, "podcast artwork staleness could not be checked");
            return;
        }
    }
    match state.podcast_media.fetch_artwork(artwork_url).await {
        Ok(draft) => {
            if let Err(error) =
                db::queries::store_podcast_artwork(&state.pool, podcast_id, &draft).await
            {
                warn!(%error, "podcast artwork could not be stored");
            }
        }
        Err(message) => warn!(podcast = %podcast_id, %message, "podcast artwork fetch failed"),
    }
}

/// Runs at most one queued download, reporting whether work was available.
async fn run_next_download(state: &AppState) -> Result<bool, String> {
    let abandoned_before =
        (Utc::now() - ChronoDuration::minutes(DOWNLOAD_LEASE_MINUTES)).to_rfc3339();
    let Some(job) =
        db::queries::claim_podcast_download(&state.pool, &abandoned_before, DOWNLOAD_MAX_ATTEMPTS)
            .await
            .map_err(|_| "podcast download could not be claimed".to_owned())?
    else {
        return Ok(false);
    };

    let settings = db::queries::get_podcast_settings(&state.pool)
        .await
        .map_err(|_| "podcast policy could not be loaded".to_owned())?;

    if let Err(message) = admit_download(state, &settings).await {
        // A storage problem is not the episode's fault, so the attempt is returned.
        if let Err(error) =
            db::queries::requeue_podcast_download(&state.pool, &job.episode_id, &message).await
        {
            warn!(%error, "podcast download could not be requeued");
        }
        warn!(%message, "podcast download deferred");
        sleep(Duration::from_secs(DOWNLOAD_IDLE_SECONDS)).await;
        return Ok(true);
    }

    match state
        .podcast_media
        .download_episode(
            &state.pool,
            &job.episode_id,
            &job.enclosure_url,
            &job.enclosure_type,
            settings.max_episode_bytes,
        )
        .await
    {
        Ok((file_name, content_type, byte_size)) => {
            db::queries::mark_podcast_download_ready(
                &state.pool,
                &job.episode_id,
                &file_name,
                &content_type,
                byte_size,
            )
            .await
            .map_err(|_| "podcast download could not be published".to_owned())?;
            info!(episode = %job.episode_id, byte_size, "cached podcast episode");
        }
        Err(message) => {
            warn!(episode = %job.episode_id, %message, "podcast download failed");
            db::queries::mark_podcast_download_failed(&state.pool, &job.episode_id, &message)
                .await
                .map_err(|_| "podcast download failure could not be recorded".to_owned())?;
        }
    }
    Ok(true)
}

/// Makes room for one more episode, evicting least recently used files if needed.
///
/// Eviction never touches a pinned file or one sitting in someone's play queue. When it
/// cannot free enough, the download is deferred rather than allowed to fill the volume.
async fn admit_download(
    state: &AppState,
    settings: &db::entities::PodcastSettings,
) -> Result<(), String> {
    let headroom = settings.max_episode_bytes;
    let mut used = db::queries::podcast_storage_used_bytes(&state.pool)
        .await
        .map_err(|_| "podcast storage usage could not be measured".to_owned())?;
    if used + headroom <= settings.storage_budget_bytes {
        return Ok(());
    }

    let candidates = db::queries::list_podcast_eviction_candidates(&state.pool)
        .await
        .map_err(|_| "podcast eviction candidates could not be loaded".to_owned())?;
    for candidate in candidates {
        if used + headroom <= settings.storage_budget_bytes {
            return Ok(());
        }
        state.podcast_media.remove(&candidate.file_name).await;
        if let Err(error) =
            db::queries::delete_podcast_download(&state.pool, &candidate.episode_id).await
        {
            warn!(%error, "evicted podcast download could not be forgotten");
            continue;
        }
        used -= candidate.byte_size;
        info!(episode = %candidate.episode_id, "evicted cached podcast episode");
    }

    if used + headroom <= settings.storage_budget_bytes {
        Ok(())
    } else {
        Err("podcast storage budget is full and nothing else can be evicted".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn itunes_durations_parse_every_documented_form() {
        assert_eq!(parse_itunes_duration("165"), Some(165));
        assert_eq!(parse_itunes_duration("02:45"), Some(165));
        assert_eq!(parse_itunes_duration("36:12"), Some(2172));
        assert_eq!(parse_itunes_duration("01:06:41"), Some(4001));
        assert_eq!(parse_itunes_duration("1:36:35"), Some(5795));
        assert_eq!(parse_itunes_duration(""), None);
        assert_eq!(parse_itunes_duration("about an hour"), None);
        assert_eq!(parse_itunes_duration("1:2:3:4"), None);
    }

    #[test]
    fn item_durations_are_scanned_in_document_order() {
        let feed = br#"<?xml version="1.0"?>
            <rss xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
              <channel>
                <itunes:duration>99:99</itunes:duration>
                <item><title>One</title><itunes:duration>02:45</itunes:duration></item>
                <item><title>Two</title></item>
                <item><title>Three</title><itunes:duration>01:06:41</itunes:duration></item>
              </channel>
            </rss>"#;
        assert_eq!(
            scan_itunes_durations(feed),
            vec![Some(165), None, Some(4001)],
            "a channel-level duration must not be attributed to an item"
        );
    }

    #[test]
    fn audio_extensions_come_only_from_the_allowlist() {
        assert_eq!(audio_extension("audio/mpeg"), Some("mp3"));
        assert_eq!(audio_extension("audio/x-m4a"), Some("m4a"));
        assert_eq!(audio_extension("application/ogg"), Some("ogg"));
        assert_eq!(audio_extension("text/html"), None);
        assert_eq!(audio_extension("application/octet-stream"), None);
    }

    #[test]
    fn cached_file_names_cannot_escape_the_media_root() {
        let root = std::env::temp_dir().join(format!("pandan-media-{}", uuid::Uuid::new_v4()));
        let media = PodcastMedia::with_root(root.clone()).expect("media root prepares");

        assert!(media.resolve("abc.mp3").is_ok());
        for hostile in [
            "",
            "../escape.mp3",
            "nested/abc.mp3",
            "..\\escape.mp3",
            "a/../../b.mp3",
        ] {
            assert!(
                media.resolve(hostile).is_err(),
                "{hostile} must be rejected"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
