use crate::CodingResponse;
use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use db::entities::{
    CodingProject, ContactAddress, ContactDraft, ContactImportantDate, ContactMethod,
    DashboardWidget,
};
use feed_rs::model::Entry;
use futures_util::{
    future::join_all,
    stream::{self, StreamExt},
};
use primp::{Client as BrowserClient, Impersonate, ImpersonateOS};
use quick_xml::{Reader, de::from_str, events::Event};
use regex::Regex;
use reqwest::{Client, ClientBuilder, Method, StatusCode, Url, header};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::{
    cmp::Reverse,
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, Semaphore};

use crate::network_policy::{NetworkAccessScope, NetworkPolicy};

const CACHE_DURATION: Duration = Duration::from_secs(15 * 60);
const MAX_RESPONSE_BYTES: usize = 2_000_000;
const MAX_RSS_REDIRECTS: usize = 3;
const MAX_FAVICON_DOCUMENT_BYTES: usize = 256 * 1024;
const MAX_FAVICON_CANDIDATES: usize = 8;
const MAX_YAHOO_QUOTE_PAGE_BYTES: usize = 768 * 1024;
const YAHOO_CHART_ENDPOINTS: [&str; 2] = [
    "https://query1.finance.yahoo.com/",
    "https://query2.finance.yahoo.com/",
];
const MAX_YOUTUBE_CHANNEL_METADATA_BYTES: usize = 1_000_000;
const MAX_OWNED_REPOSITORIES: usize = 500;
const MAX_PROVIDER_PAGES: usize = 100;
const PROVIDER_REQUEST_CONCURRENCY: usize = 8;
const REDDIT_LOID_CACHE_DURATION: Duration = Duration::from_secs(6 * 60 * 60);
const REDDIT_LISTING_ORIGINS: [&str; 3] = [
    "https://www.reddit.com",
    "https://api.reddit.com",
    "https://old.reddit.com",
];

#[derive(Clone)]
pub struct WidgetIntegrationService {
    network_policy: NetworkPolicy,
    cipher: Option<XChaCha20Poly1305>,
    invidious_base_url: Option<Url>,
    invidious_allows_private_network: bool,
    reddit_request_gate: Arc<Semaphore>,
    reddit_loid: Arc<RwLock<Option<CachedRedditLoid>>>,
    cache: Arc<RwLock<HashMap<String, CachedData>>>,
    coding_cache: Arc<RwLock<CodingCache>>,
}

struct CachedData {
    stored_at: Instant,
    value: Value,
}

struct CachedRedditLoid {
    stored_at: Instant,
    value: String,
}

#[derive(Default)]
struct CodingCache {
    entries: HashMap<String, CachedCodingData>,
    generations: HashMap<String, u64>,
}

struct CachedCodingData {
    value: CodingResponse,
}

#[derive(Debug, Clone)]
pub struct RssFeedSnapshot {
    pub title: String,
    pub items: Vec<RssFeedEntry>,
}

#[derive(Debug, Clone)]
pub struct RssFeedEntry {
    pub external_id: String,
    pub url: String,
    pub comments_url: String,
    pub title: String,
    pub summary: String,
    pub published_at: String,
}

#[derive(Debug, Deserialize)]
struct RedditListingPayload {
    data: RedditListingData,
}

#[derive(Debug, Deserialize)]
struct RedditListingData {
    #[serde(default)]
    children: Vec<RedditListingChild>,
}

#[derive(Debug, Deserialize)]
struct RedditListingChild {
    data: RedditPost,
}

#[derive(Debug, Deserialize)]
struct RedditPost {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    permalink: String,
    #[serde(default)]
    selftext: String,
    #[serde(default)]
    is_self: bool,
    #[serde(default)]
    created_utc: Option<serde_json::Number>,
    #[serde(default)]
    ups: i64,
    #[serde(default)]
    num_comments: i64,
}

#[derive(Debug, Clone)]
pub struct YoutubeFeedSnapshot {
    pub title: String,
    pub channel_url: String,
    pub thumbnail_urls: Vec<String>,
    pub items: Vec<YoutubeFeedEntry>,
}

#[derive(Debug, Clone)]
pub struct YoutubeFeedEntry {
    pub external_id: String,
    pub url: String,
    pub thumbnail_url: String,
    pub title: String,
    pub published_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRepository {
    pub provider: String,
    pub host: String,
    pub repository: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodingRelease {
    pub project_id: String,
    pub version: String,
    pub url: String,
    pub published_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodingMergeRequest {
    pub id: i64,
    pub reference: String,
    pub title: String,
    pub url: String,
    pub updated_at: String,
    pub draft: bool,
    pub merge_status: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CodingOwnedRepository {
    pub provider: String,
    pub host: String,
    pub repository: String,
    pub url: String,
    pub archived: bool,
    pub open_pull_requests: Option<u64>,
}

#[derive(Debug, Default)]
pub struct CodingOwnedRepositories {
    pub repositories: Vec<CodingOwnedRepository>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodingPipeline {
    pub project_id: String,
    pub id: i64,
    pub status: String,
    pub reference: String,
    pub sha: String,
    pub url: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketQuote {
    pub symbol: String,
    pub name: Option<String>,
    pub price: String,
    pub previous_close: Option<String>,
    pub day_open: Option<String>,
    pub day_high: Option<String>,
    pub day_low: Option<String>,
    pub change_percent: Option<String>,
    pub currency: String,
    pub market_state: Option<String>,
    pub quoted_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NtfyMessage {
    pub id: String,
    pub time: i64,
    #[serde(default)]
    pub event: String,
    pub topic: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub message: String,
    #[serde(default = "default_ntfy_priority")]
    pub priority: i64,
    #[serde(default)]
    pub tags: Vec<String>,
    pub click: Option<String>,
    #[serde(default)]
    pub actions: Vec<NtfyAction>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NtfyAction {
    pub action: String,
    pub label: String,
    pub url: Option<String>,
    pub method: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub value: Option<String>,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Serialize)]
struct NtfyPublishPayload<'a> {
    topic: &'a str,
    title: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    click: Option<&'a str>,
    tags: [&'static str; 2],
}

/// Account-scoped outbound ntfy message prepared by a server-owned worker.
pub(crate) struct NtfyPublishRequest<'a> {
    pub account_id: &'a str,
    pub base_url: &'a str,
    pub topic: &'a str,
    pub title: &'a str,
    pub message: &'a str,
    pub click: Option<&'a str>,
    pub encrypted_token: Option<&'a str>,
}

const fn default_ntfy_priority() -> i64 {
    3
}

impl WidgetIntegrationService {
    /// Builds the provider client and optional credential cipher from environment configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when the encryption key or HTTP client configuration is invalid.
    pub fn from_env(pool: SqlitePool) -> Result<Self, String> {
        let key = std::env::var("PANDAN_SECRET_KEY").ok();
        let invidious_base_url = std::env::var("INVIDIOUS_BASE_URL").ok();
        let invidious_allows_private_network = std::env::var("INVIDIOUS_ALLOW_PRIVATE_NETWORK")
            .is_ok_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            });
        Self::new_with_policy(
            key.as_deref(),
            invidious_base_url.as_deref(),
            invidious_allows_private_network,
            NetworkPolicy::new(pool),
        )
    }

    #[cfg(test)]
    /// Builds an integration service without encrypted credential storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the test HTTP client cannot be initialized.
    pub fn for_tests(pool: SqlitePool) -> Result<Self, String> {
        Self::new_with_policy(None, None, false, NetworkPolicy::new(pool))
    }

    #[cfg(test)]
    fn new(encoded_key: Option<&str>) -> Result<Self, String> {
        Self::new_with_policy(encoded_key, None, false, NetworkPolicy::without_rules())
    }

    #[cfg(test)]
    fn new_with_invidious(
        encoded_key: Option<&str>,
        invidious_base_url: Option<&str>,
        invidious_allows_private_network: bool,
    ) -> Result<Self, String> {
        Self::new_with_policy(
            encoded_key,
            invidious_base_url,
            invidious_allows_private_network,
            NetworkPolicy::without_rules(),
        )
    }

    fn new_with_policy(
        encoded_key: Option<&str>,
        invidious_base_url: Option<&str>,
        invidious_allows_private_network: bool,
        network_policy: NetworkPolicy,
    ) -> Result<Self, String> {
        let cipher = encoded_key
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                let bytes = STANDARD
                    .decode(value)
                    .map_err(|_| "PANDAN_SECRET_KEY must be base64")?;
                let key: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| "PANDAN_SECRET_KEY must decode to exactly 32 bytes")?;
                XChaCha20Poly1305::new_from_slice(&key)
                    .map_err(|_| "PANDAN_SECRET_KEY is invalid".to_owned())
            })
            .transpose()?;
        Ok(Self {
            network_policy,
            cipher,
            invidious_base_url: parse_invidious_base_url(invidious_base_url)?,
            invidious_allows_private_network,
            reddit_request_gate: Arc::new(Semaphore::new(1)),
            reddit_loid: Arc::new(RwLock::new(None)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            coding_cache: Arc::new(RwLock::new(CodingCache::default())),
        })
    }

    async fn client_for(
        &self,
        source: &str,
        scope: NetworkAccessScope,
    ) -> Result<(Client, Url), String> {
        let target = self.network_policy.validate(source, scope).await?;
        tracing::debug!(
            integration = scope.as_str(),
            origin = %target.url().origin().ascii_serialization(),
            resolved_addresses = target.addresses().len(),
            "outbound destination validated"
        );
        let client = target.build_client(widget_client_builder())?;
        Ok((client, target.into_url()))
    }

    async fn reddit_client_for(&self, source: &str) -> Result<(BrowserClient, Url), String> {
        let target = self
            .network_policy
            .validate(source, NetworkAccessScope::Rss)
            .await?;
        tracing::debug!(
            integration = NetworkAccessScope::Rss.as_str(),
            origin = %target.url().origin().ascii_serialization(),
            resolved_addresses = target.addresses().len(),
            browser_profile = "firefox-140-linux",
            "Reddit browser destination validated"
        );
        let host = target
            .url()
            .host_str()
            .ok_or_else(|| "Reddit URL host is missing".to_owned())?;
        let builder = BrowserClient::builder()
            .impersonate(Impersonate::FirefoxV140)
            .impersonate_os(ImpersonateOS::Linux)
            .cookie_store(true)
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(10))
            .redirect(primp::redirect::Policy::none())
            .no_proxy();
        let builder = if host.parse::<IpAddr>().is_ok() {
            builder
        } else {
            builder.resolve_to_addrs(host, target.addresses())
        };
        let client = builder
            .build()
            .map_err(|error| format!("Reddit browser client failed: {error}"))?;
        Ok((client, target.into_url()))
    }

    #[must_use]
    pub fn secrets_enabled(&self) -> bool {
        self.cipher.is_some()
    }

    #[must_use]
    pub fn invidious_enabled(&self) -> bool {
        self.invidious_base_url.is_some()
    }

    /// Reports whether the configured `Invidious` instance is exempt from the private-network guard.
    #[must_use]
    pub fn invidious_allows_private_network(&self) -> bool {
        self.invidious_base_url.is_some() && self.invidious_allows_private_network
    }

    /// Fetches and parses one policy-approved RSS, Atom, or recognized Reddit source.
    ///
    /// The same DNS and response-size protections used by RSS widgets are applied here.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when URL validation, fetching, or feed parsing fails.
    pub async fn fetch_rss_feed(&self, source: &str) -> Result<RssFeedSnapshot, String> {
        let (client, url) = self.client_for(source, NetworkAccessScope::Rss).await?;
        // Stored `.rss` sources and legacy `.json` sources both use Reddit's JSON listing. A
        // Firefox transport plus the short-lived `loid` challenge cookie avoids the anonymous
        // Atom/JSON rate-limit bucket while keeping the stored subscription format compatible.
        if let Some(listing) = reddit_listing_source(&url) {
            drop(client);
            let payload = self.fetch_reddit_listing(&listing).await?;
            return Ok(reddit_listing_snapshot(&listing, &payload));
        }
        let bytes = self.fetch_rss_bytes(client, url).await?;
        parse_rss_feed_snapshot(&bytes)
    }

    /// Fetches an RSS response while revalidating and pinning every redirect destination.
    async fn fetch_rss_bytes(&self, mut client: Client, mut url: Url) -> Result<Vec<u8>, String> {
        for redirect_count in 0..=MAX_RSS_REDIRECTS {
            let response = client
                .get(url.clone())
                .send()
                .await
                .map_err(request_error)?;
            if !response.status().is_redirection() {
                return response_bytes(response).await;
            }
            if redirect_count == MAX_RSS_REDIRECTS {
                return Err("RSS feed redirected too many times".to_owned());
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .ok_or_else(|| "RSS feed redirect was missing a destination".to_owned())?
                .to_str()
                .map_err(|_| "RSS feed redirect destination was invalid".to_owned())?;
            let next = url
                .join(location)
                .map_err(|_| "RSS feed redirect destination was invalid".to_owned())?;
            drop(response);
            (client, url) = self
                .client_for(next.as_str(), NetworkAccessScope::Rss)
                .await?;
        }
        Err("RSS feed redirected too many times".to_owned())
    }

    /// Serializes Reddit's browser challenge and listing requests so one cached `loid` cookie is
    /// shared by background refreshes and interactive requests.
    async fn fetch_reddit_listing(
        &self,
        source: &RedditListingSource,
    ) -> Result<RedditListingPayload, String> {
        let _permit = self
            .reddit_request_gate
            .acquire()
            .await
            .map_err(|_| "Reddit request could not be scheduled".to_owned())?;
        let loid = self.reddit_loid_cookie().await?;
        let mut last_error = "Reddit listing could not be loaded".to_owned();
        for origin in REDDIT_LISTING_ORIGINS {
            let request_url = source.json_url(origin)?;
            let (client, request_url) = match self.reddit_client_for(request_url.as_str()).await {
                Ok(target) => target,
                Err(error) => {
                    last_error = error;
                    continue;
                }
            };
            let mut response = match client
                .get(request_url)
                .header("accept", "application/json")
                .header("cookie", format!("loid={loid}"))
                .send()
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    last_error = reddit_request_error(&error);
                    continue;
                }
            };
            let status = response.status();
            if status.is_success() {
                let body = match reddit_response_bytes(&mut response).await {
                    Ok(body) => body,
                    Err(error) => {
                        last_error = error;
                        continue;
                    }
                };
                if let Ok(payload) = serde_json::from_slice::<RedditListingPayload>(&body) {
                    return Ok(payload);
                }
                last_error = "Reddit returned an invalid listing".to_owned();
                continue;
            }
            last_error = reddit_status_error(status.as_u16());
        }
        Err(last_error)
    }

    async fn reddit_loid_cookie(&self) -> Result<String, String> {
        let cached = self
            .reddit_loid
            .read()
            .await
            .as_ref()
            .filter(|cookie| cookie.stored_at.elapsed() < REDDIT_LOID_CACHE_DURATION)
            .map(|cookie| cookie.value.clone());
        if let Some(cookie) = cached {
            return Ok(cookie);
        }

        let (client, challenge_url) = self.reddit_client_for("https://www.reddit.com/").await?;
        let mut response = client
            .get(challenge_url.clone())
            .header("accept", "text/html,application/xhtml+xml")
            .send()
            .await
            .map_err(|error| reddit_request_error(&error))?;
        if let Some(cookie) = reddit_loid_from_headers(response.headers()) {
            self.store_reddit_loid(cookie.clone()).await;
            return Ok(cookie);
        }
        let challenge_body = reddit_response_bytes(&mut response).await?;
        let challenge_body = String::from_utf8(challenge_body)
            .map_err(|_| "Reddit challenge was not UTF-8".to_owned())?;
        let (challenge, token) = parse_reddit_challenge(&challenge_body)
            .ok_or_else(|| "Reddit browser challenge could not be parsed".to_owned())?;
        let mut solution_url = challenge_url;
        solution_url.query_pairs_mut().clear().extend_pairs([
            ("solution", format!("{challenge}{challenge}")),
            ("js_challenge", "1".to_owned()),
            ("token", token),
        ]);
        let response = client
            .get(solution_url)
            .header("accept", "text/html,application/xhtml+xml")
            .send()
            .await
            .map_err(|error| reddit_request_error(&error))?;
        let cookie = reddit_loid_from_headers(response.headers())
            .ok_or_else(|| "Reddit browser challenge did not issue a cookie".to_owned())?;
        self.store_reddit_loid(cookie.clone()).await;
        Ok(cookie)
    }

    async fn store_reddit_loid(&self, value: String) {
        *self.reddit_loid.write().await = Some(CachedRedditLoid {
            stored_at: Instant::now(),
            value,
        });
    }

    /// Fetches a channel through a configured `Invidious` API, then falls back to `YouTube`.
    ///
    /// The fallback uses the channel's `UULF` uploads playlist so Shorts are excluded.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when the request or Atom parsing fails.
    pub async fn fetch_youtube_channel(
        &self,
        channel_id: &str,
    ) -> Result<YoutubeFeedSnapshot, String> {
        let mut invidious_failed = false;
        if let Some(base_url) = &self.invidious_base_url {
            match self.fetch_invidious_channel(base_url, channel_id).await {
                Ok(snapshot) => return Ok(snapshot),
                Err(error) => {
                    invidious_failed = true;
                    tracing::warn!(%channel_id, %error, "Invidious fetch failed; using YouTube fallback");
                }
            }
        }
        match self.fetch_youtube_atom_channel(channel_id).await {
            Ok(snapshot) => Ok(snapshot),
            Err(error) if invidious_failed => {
                tracing::warn!(%channel_id, %error, "YouTube fallback fetch failed");
                Err("Invidious and YouTube providers were unavailable".to_owned())
            }
            Err(error) => Err(error),
        }
    }

    async fn fetch_invidious_channel(
        &self,
        base_url: &Url,
        channel_id: &str,
    ) -> Result<YoutubeFeedSnapshot, String> {
        let endpoint = base_url
            .join(&format!("api/v1/channels/{channel_id}"))
            .map_err(|_| "Invidious channel URL is invalid".to_owned())?;
        let (client, endpoint) = self.validate_invidious_url(endpoint.as_str()).await?;
        let response = client.get(endpoint).send().await.map_err(request_error)?;
        let text = response_text(response).await?;
        parse_invidious_snapshot(base_url, channel_id, &text)
    }

    /// Validates one URL that belongs to the operator-configured `Invidious` instance.
    ///
    /// A self-hosted instance usually resolves to a private address, which the shared SSRF
    /// policy rejects. `INVIDIOUS_ALLOW_PRIVATE_NETWORK` exempts it, scoped to the configured
    /// host and port: any other destination still goes through the shared network policy,
    /// so a user-supplied URL can never reach the internal network through this path.
    async fn validate_invidious_url(&self, value: &str) -> Result<(Client, Url), String> {
        let parsed = Url::parse(value).map_err(|_| "Invidious URL is invalid".to_owned())?;
        if parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err("only credential-free HTTPS URLs are allowed".to_owned());
        }
        let configured_private_origin = self
            .invidious_base_url
            .as_ref()
            .filter(|_| self.invidious_allows_private_network)
            .is_some_and(|base_url| {
                parsed
                    .host_str()
                    .zip(base_url.host_str())
                    .is_some_and(|(host, instance)| host.eq_ignore_ascii_case(instance))
                    && parsed.port_or_known_default() == base_url.port_or_known_default()
            });
        let target = self
            .network_policy
            .validate_with_operator_override(
                value,
                NetworkAccessScope::Youtube,
                configured_private_origin,
            )
            .await?;
        let client = target.build_client(widget_client_builder())?;
        Ok((client, target.into_url()))
    }

    async fn fetch_youtube_atom_channel(
        &self,
        channel_id: &str,
    ) -> Result<YoutubeFeedSnapshot, String> {
        let uploads_playlist = channel_id.replacen("UC", "UULF", 1);
        let (client, url) = self
            .client_for(
                &format!("https://www.youtube.com/feeds/videos.xml?playlist_id={uploads_playlist}"),
                NetworkAccessScope::Youtube,
            )
            .await?;
        let response = client.get(url).send().await.map_err(request_error)?;
        let text = response_text(response).await?;
        parse_youtube_snapshot(&text)
    }

    /// Fetches portrait candidates from YouTube's public channel metadata.
    ///
    /// This fallback is separate from the Atom refresh so it is only called when the
    /// persistent portrait cache is missing or stale.
    pub async fn fetch_youtube_channel_portrait_urls(
        &self,
        channel_id: &str,
    ) -> Result<Vec<String>, String> {
        let (client, url) = self
            .client_for(
                &format!("https://www.youtube.com/channel/{channel_id}"),
                NetworkAccessScope::Youtube,
            )
            .await?;
        let response = client
            .get(url)
            .header(header::ACCEPT, "text/html")
            .send()
            .await
            .map_err(request_error)?;
        let html = response_prefix_text(response, MAX_YOUTUBE_CHANNEL_METADATA_BYTES).await?;
        let urls = parse_youtube_channel_portrait_urls(&html);
        if urls.is_empty() {
            Err("YouTube channel portrait metadata was unavailable".to_owned())
        } else {
            Ok(urls)
        }
    }

    /// Fetches one policy-approved iCalendar document using the shared SSRF and size guards.
    pub async fn fetch_calendar_file(&self, source: &str) -> Result<Vec<u8>, String> {
        let (client, url) = self
            .client_for(source, NetworkAccessScope::Calendar)
            .await?;
        let response = client.get(url).send().await.map_err(request_error)?;
        response_bytes(response).await
    }

    /// Fetches one bounded channel portrait for the persistent 24-hour cache.
    ///
    /// An `Invidious` instance may serve portraits from its own host, so this path resolves
    /// through `validate_invidious_url` and honours the private-network exemption.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when the URL, media type, or response is invalid.
    pub async fn fetch_public_image(&self, source: &str) -> Result<(String, Vec<u8>), String> {
        let (client, url) = self.validate_invidious_url(source).await?;
        self.fetch_validated_image(&client, url, MAX_RESPONSE_BYTES)
            .await
    }

    /// Fetches one policy-approved image with a caller-supplied response limit.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when the URL, media type, or response is invalid.
    pub async fn fetch_bounded_public_image(
        &self,
        source: &str,
        max_bytes: usize,
    ) -> Result<(String, Vec<u8>), String> {
        let (client, url) = self.client_for(source, NetworkAccessScope::Images).await?;
        self.fetch_validated_image(&client, url, max_bytes).await
    }

    /// Fetches a small favicon while revalidating every redirect through the image policy.
    ///
    /// SVG sources are rendered into a bounded PNG before storage so untrusted active image
    /// content is never served from Pandan's own origin.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when policy, redirect, media type, or size validation fails.
    pub async fn fetch_favicon(
        &self,
        source: &str,
        max_bytes: usize,
    ) -> Result<(String, Vec<u8>), String> {
        const MAX_FAVICON_REDIRECTS: usize = 3;
        let mut current = source.to_owned();
        for redirect_count in 0..=MAX_FAVICON_REDIRECTS {
            let (client, url) = self
                .client_for(&current, NetworkAccessScope::Images)
                .await?;
            let response = client
                .get(url.clone())
                .header(
                    header::ACCEPT,
                    "image/avif,image/webp,image/png,image/*;q=0.8,*/*;q=0.2",
                )
                .send()
                .await
                .map_err(request_error)?;
            if response.status().is_redirection() {
                if redirect_count == MAX_FAVICON_REDIRECTS {
                    return Err("favicon redirected too many times".to_owned());
                }
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| "favicon redirect was missing a destination".to_owned())?;
                current = url
                    .join(location)
                    .map_err(|_| "favicon redirect destination was invalid".to_owned())?
                    .to_string();
                continue;
            }
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| supported_favicon_content_type(value.as_str()))
                .ok_or_else(|| "provider favicon type is unsupported".to_owned())?;
            let bytes = response_bytes_with_limit(response, max_bytes).await?;
            if bytes.is_empty() {
                return Err("provider favicon was empty".to_owned());
            }
            if content_type == "image/svg+xml" {
                let png = tokio::task::spawn_blocking(move || {
                    rasterize_svg_icon(bytes.as_slice(), max_bytes)
                })
                .await
                .map_err(|_| "provider SVG rendering failed".to_owned())??;
                return Ok(("image/png".to_owned(), png));
            }
            return Ok((content_type, bytes));
        }
        Err("favicon request failed".to_owned())
    }

    /// Discovers a site's favicon without trusting the browser to fetch remote bytes directly.
    ///
    /// The conventional origin favicon is tried first. When it is absent, the destination page
    /// is fetched through the image network policy and its declared icon links are tried in order.
    /// Every redirect and icon candidate is independently revalidated by the same policy.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when no supported, policy-approved favicon can be fetched.
    pub async fn fetch_site_favicon(
        &self,
        destination: &str,
        max_bytes: usize,
    ) -> Result<(String, Vec<u8>), String> {
        let destination_url =
            Url::parse(destination).map_err(|_| "favicon destination was invalid".to_owned())?;
        let mut origin_favicon = destination_url.clone();
        origin_favicon.set_path("/favicon.ico");
        origin_favicon.set_query(None);
        origin_favicon.set_fragment(None);
        if let Ok(icon) = self.fetch_favicon(origin_favicon.as_str(), max_bytes).await {
            return Ok(icon);
        }

        let (document_url, document) = self.fetch_favicon_document(destination).await?;
        let mut candidates = parse_declared_favicon_urls(&document_url, &document);
        for path in ["/apple-touch-icon.png", "/favicon.png"] {
            let mut candidate = document_url.clone();
            candidate.set_path(path);
            candidate.set_query(None);
            candidate.set_fragment(None);
            let candidate = candidate.to_string();
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }

        for candidate in candidates.into_iter().take(MAX_FAVICON_CANDIDATES) {
            if let Ok(icon) = self.fetch_favicon(&candidate, max_bytes).await {
                return Ok(icon);
            }
        }
        Err("site did not expose a supported favicon".to_owned())
    }

    async fn fetch_favicon_document(&self, source: &str) -> Result<(Url, String), String> {
        const MAX_FAVICON_REDIRECTS: usize = 3;
        let mut current = source.to_owned();
        for redirect_count in 0..=MAX_FAVICON_REDIRECTS {
            let (client, url) = self
                .client_for(&current, NetworkAccessScope::Images)
                .await?;
            let response = client
                .get(url.clone())
                .header(
                    header::ACCEPT,
                    "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1",
                )
                .send()
                .await
                .map_err(request_error)?;
            if response.status().is_redirection() {
                if redirect_count == MAX_FAVICON_REDIRECTS {
                    return Err("favicon page redirected too many times".to_owned());
                }
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| "favicon page redirect was missing a destination".to_owned())?;
                current = url
                    .join(location)
                    .map_err(|_| "favicon page redirect destination was invalid".to_owned())?
                    .to_string();
                continue;
            }
            let supported_document = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .map(|value| value.trim().to_ascii_lowercase())
                .is_some_and(|value| {
                    matches!(value.as_str(), "text/html" | "application/xhtml+xml")
                });
            if !supported_document {
                return Err("favicon page type was unsupported".to_owned());
            }
            let document = response_prefix_text(response, MAX_FAVICON_DOCUMENT_BYTES).await?;
            return Ok((url, document));
        }
        Err("favicon page request failed".to_owned())
    }

    async fn fetch_validated_image(
        &self,
        client: &Client,
        url: Url,
        max_bytes: usize,
    ) -> Result<(String, Vec<u8>), String> {
        let response = client.get(url).send().await.map_err(request_error)?;
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
            .ok_or_else(|| "provider image type is unsupported".to_owned())?
            .to_owned();
        let bytes = response_bytes_with_limit(response, max_bytes).await?;
        if bytes.is_empty() {
            return Err("provider image was empty".to_owned());
        }
        Ok((content_type, bytes))
    }

    /// Validates one remote integration URL without fetching it.
    pub async fn validate_source(
        &self,
        source: &str,
        scope: NetworkAccessScope,
    ) -> Result<(), String> {
        self.network_policy
            .validate(source, scope)
            .await
            .map(|_| ())
    }

    /// Pulls vCards from one policy-approved CardDAV address-book resource.
    ///
    /// The caller supplies a direct address-book URL. Network access uses the same DNS/IP,
    /// timeout, redirect, and response-size restrictions as other remote Pandan integrations.
    pub async fn fetch_carddav_contacts(
        &self,
        source_id: &str,
        source: &str,
        username: &str,
        encrypted_password: Option<&str>,
    ) -> Result<Vec<ContactDraft>, String> {
        let (client, url) = self
            .client_for(source, NetworkAccessScope::Contacts)
            .await?;
        let password = encrypted_password
            .map(|value| self.decrypt_secret(value))
            .transpose()?;
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<card:addressbook-query xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
  <d:prop><d:getetag/><card:address-data/></d:prop>
</card:addressbook-query>"#;
        let method = Method::from_bytes(b"REPORT")
            .map_err(|_| "CardDAV request method is invalid".to_owned())?;
        let mut request = client
            .request(method, url)
            .header("Depth", "1")
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(body);
        if !username.trim().is_empty() || password.is_some() {
            request = request.basic_auth(username.trim(), password);
        }
        let response = request.send().await.map_err(request_error)?;
        let bytes = response_bytes(response).await?;
        parse_carddav_response(source_id, &bytes)
    }

    /// Encrypts one provider credential for database storage.
    ///
    /// # Errors
    ///
    /// Returns an error when secret storage is disabled or encryption fails.
    pub fn encrypt_secret(&self, plaintext: &str) -> Result<String, String> {
        let cipher = self
            .cipher
            .as_ref()
            .ok_or_else(|| "server secret storage is not configured".to_owned())?;
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| "credential encryption failed".to_owned())?;
        let mut packed = nonce.to_vec();
        packed.extend(ciphertext);
        Ok(STANDARD.encode(packed))
    }

    /// Polls ntfy topics through the shared network policy and response-size guards.
    pub async fn fetch_ntfy_topics(
        &self,
        account_id: &str,
        base_url: &str,
        topics: &[&str],
        since: Option<&str>,
        encrypted_token: Option<&str>,
    ) -> Result<Vec<NtfyMessage>, String> {
        let endpoint = ntfy_poll_endpoint(base_url, topics, since)?;
        let (client, endpoint) = self
            .client_for(endpoint.as_str(), NetworkAccessScope::Notifications)
            .await?;
        let token = encrypted_token
            .map(|value| self.decrypt_secret(value))
            .transpose()?;
        let has_token = token.as_deref().is_some_and(|value| !value.is_empty());
        let context = NtfyRequestLogContext::new(
            account_id,
            "recovery_poll",
            &endpoint,
            topics.len(),
            has_token,
        );
        let mut request = client
            .get(endpoint)
            .header(header::ACCEPT, "application/x-ndjson, application/json");
        if let Some(token) = token.as_deref().filter(|value| !value.is_empty()) {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| ntfy_transport_error(error, &context))?;
        log_ntfy_response(&response, &context);
        let response = response
            .error_for_status()
            .map_err(|error| ntfy_request_error(error, has_token))?;
        let body = response_text(response).await?;
        parse_ntfy_messages(&body)
    }

    /// Publishes one server-owned notification through an account's guarded ntfy connection.
    pub(crate) async fn publish_ntfy_notification(
        &self,
        notification: &NtfyPublishRequest<'_>,
    ) -> Result<(), String> {
        let endpoint = Url::parse(notification.base_url)
            .map_err(|_| "ntfy server URL is invalid".to_owned())?;
        let (client, endpoint) = self
            .client_for(endpoint.as_str(), NetworkAccessScope::Notifications)
            .await?;
        let token = notification
            .encrypted_token
            .map(|value| self.decrypt_secret(value))
            .transpose()?;
        let has_token = token.as_deref().is_some_and(|value| !value.is_empty());
        let context =
            NtfyRequestLogContext::new(notification.account_id, "publish", &endpoint, 1, has_token);
        let payload = NtfyPublishPayload {
            topic: notification.topic,
            title: notification.title,
            message: notification.message,
            click: notification.click,
            tags: ["studio_microphone", "new"],
        };
        let mut request = client.post(endpoint).json(&payload);
        if let Some(token) = token.as_deref().filter(|value| !value.is_empty()) {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| ntfy_transport_error(error, &context))?;
        log_ntfy_response(&response, &context);
        response
            .error_for_status()
            .map_err(|error| ntfy_publish_request_error(error, has_token))?;
        Ok(())
    }

    /// Opens a long-lived ntfy JSON subscription without exposing the stored token to the browser.
    pub async fn open_ntfy_stream(
        &self,
        account_id: &str,
        base_url: &str,
        topics: &[&str],
        since: &str,
        encrypted_token: Option<&str>,
    ) -> Result<reqwest::Response, String> {
        let endpoint = ntfy_stream_endpoint(base_url, topics, since)?;
        let target = self
            .network_policy
            .validate(endpoint.as_str(), NetworkAccessScope::Notifications)
            .await?;
        let client = target.build_client(ntfy_stream_client_builder())?;
        let token = encrypted_token
            .map(|value| self.decrypt_secret(value))
            .transpose()?;
        let has_token = token.as_deref().is_some_and(|value| !value.is_empty());
        let context = NtfyRequestLogContext::new(
            account_id,
            "realtime_stream",
            &endpoint,
            topics.len(),
            has_token,
        );
        let mut request = client
            .get(target.into_url())
            .header(header::ACCEPT, "application/x-ndjson");
        if let Some(token) = token.as_deref().filter(|value| !value.is_empty()) {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| ntfy_transport_error(error, &context))?;
        log_ntfy_response(&response, &context);
        response
            .error_for_status()
            .map_err(|error| ntfy_request_error(error, has_token))
    }

    /// Permanently deletes one ntfy message sequence through the guarded provider client.
    pub async fn delete_ntfy_notification(
        &self,
        account_id: &str,
        base_url: &str,
        topic: &str,
        sequence_id: &str,
        encrypted_token: Option<&str>,
    ) -> Result<(), String> {
        let mut endpoint =
            Url::parse(base_url).map_err(|_| "ntfy server URL is invalid".to_owned())?;
        endpoint
            .path_segments_mut()
            .map_err(|_| "ntfy server URL cannot contain message paths".to_owned())?
            .pop_if_empty()
            .push(topic)
            .push(sequence_id);
        let (client, endpoint) = self
            .client_for(endpoint.as_str(), NetworkAccessScope::Notifications)
            .await?;
        let token = encrypted_token
            .map(|value| self.decrypt_secret(value))
            .transpose()?;
        let has_token = token.as_deref().is_some_and(|value| !value.is_empty());
        let context = NtfyRequestLogContext::new(account_id, "delete", &endpoint, 1, has_token);
        let mut request = client.delete(endpoint);
        if let Some(token) = token.as_deref().filter(|value| !value.is_empty()) {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| ntfy_transport_error(error, &context))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        log_ntfy_response(&response, &context);
        response
            .error_for_status()
            .map_err(|error| ntfy_request_error(error, has_token))?;
        Ok(())
    }

    /// Executes one user-triggered ntfy HTTP action against a policy-approved destination.
    pub async fn execute_ntfy_http_action(&self, action: &NtfyAction) -> Result<u16, String> {
        if action.action != "http" {
            return Err("this ntfy action does not make an HTTP request".to_owned());
        }
        let destination = action
            .url
            .as_deref()
            .ok_or_else(|| "ntfy action URL is missing".to_owned())?;
        let (client, destination) = self
            .client_for(destination, NetworkAccessScope::Notifications)
            .await?;
        let method_name = action
            .method
            .as_deref()
            .unwrap_or("POST")
            .to_ascii_uppercase();
        if !matches!(
            method_name.as_str(),
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
        ) {
            return Err("ntfy action method is unsupported".to_owned());
        }
        let method = Method::from_bytes(method_name.as_bytes())
            .map_err(|_| "ntfy action method is invalid".to_owned())?;
        let mut request = client.request(method, destination);
        for (name, value) in &action.headers {
            let normalized = name.trim().to_ascii_lowercase();
            if matches!(
                normalized.as_str(),
                "connection" | "cookie" | "host" | "content-length" | "transfer-encoding"
            ) {
                continue;
            }
            let name = header::HeaderName::from_bytes(name.trim().as_bytes())
                .map_err(|_| "ntfy action contains an invalid header".to_owned())?;
            let value = header::HeaderValue::from_str(value)
                .map_err(|_| "ntfy action contains an invalid header value".to_owned())?;
            request = request.header(name, value);
        }
        if let Some(body) = action.body.as_deref() {
            if body.len() > 16_384 {
                return Err("ntfy action body is too large".to_owned());
            }
            request = request.body(body.to_owned());
        }
        let response = request.send().await.map_err(request_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("ntfy action returned HTTP {}", status.as_u16()));
        }
        Ok(status.as_u16())
    }

    /// Loads the latest release for one Coding-page project subscription.
    pub async fn fetch_coding_release(
        &self,
        project: &CodingProject,
        encrypted_secret: Option<&str>,
    ) -> Result<CodingRelease, String> {
        let secret = encrypted_secret
            .map(|value| self.decrypt_secret(value))
            .transpose()?;
        let source = if matches!(project.provider.as_str(), "gitea" | "forgejo") {
            format!(
                "{}@{}:{}",
                project.provider, project.host, project.repository
            )
        } else if project.provider == "github" {
            project.repository.clone()
        } else {
            format!("{}:{}", project.provider, project.repository)
        };
        let payload = fetch_release(self, &source, secret.as_deref()).await?;
        Ok(CodingRelease {
            project_id: project.id.clone(),
            version: payload["version"].as_str().unwrap_or("Latest").to_owned(),
            url: payload["url"].as_str().unwrap_or("#").to_owned(),
            published_at: payload["published_at"].as_str().unwrap_or("").to_owned(),
        })
    }

    /// Lists open merge requests created by the authenticated GitLab profile.
    pub async fn fetch_gitlab_merge_requests(
        &self,
        host: &str,
        encrypted_secret: &str,
    ) -> Result<Vec<CodingMergeRequest>, String> {
        let token = self.decrypt_secret(encrypted_secret)?;
        let (client, url) = self.client_for(&format!(
            "https://{host}/api/v4/merge_requests?scope=created_by_me&state=opened&order_by=updated_at&sort=desc&per_page=20"
        ), NetworkAccessScope::Coding)
        .await?;
        let payload: Vec<Value> = client
            .get(url)
            .header("PRIVATE-TOKEN", token)
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        Ok(payload
            .into_iter()
            .filter_map(|item| {
                Some(CodingMergeRequest {
                    id: item["id"].as_i64()?,
                    reference: item["references"]["full"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("!{}", item["iid"].as_i64().unwrap_or(0))),
                    title: item["title"]
                        .as_str()
                        .unwrap_or("Untitled merge request")
                        .to_owned(),
                    url: item["web_url"].as_str().unwrap_or("#").to_owned(),
                    updated_at: item["updated_at"].as_str().unwrap_or("").to_owned(),
                    draft: item["draft"].as_bool().unwrap_or(false),
                    merge_status: item["detailed_merge_status"]
                        .as_str()
                        .unwrap_or("unchecked")
                        .to_owned(),
                })
            })
            .collect())
    }

    /// Lists repositories owned by one authenticated code-host profile and their open PR counts.
    pub async fn fetch_owned_coding_repositories(
        &self,
        provider: &str,
        host: &str,
        encrypted_secret: &str,
    ) -> Result<CodingOwnedRepositories, String> {
        let token = self.decrypt_secret(encrypted_secret)?;
        match provider {
            "github" => fetch_github_owned_repositories(self, host, &token).await,
            "gitlab" => fetch_gitlab_owned_repositories(self, host, &token).await,
            "codeberg" | "gitea" | "forgejo" => {
                fetch_forge_owned_repositories(self, provider, host, &token).await
            }
            _ => Err("code provider is unsupported".to_owned()),
        }
    }

    /// Loads the newest pipeline for one subscribed GitLab project.
    pub async fn fetch_gitlab_pipeline(
        &self,
        project: &CodingProject,
        encrypted_secret: &str,
    ) -> Result<Option<CodingPipeline>, String> {
        if project.provider != "gitlab" {
            return Ok(None);
        }
        let token = self.decrypt_secret(encrypted_secret)?;
        let encoded_repository = project.repository.replace('/', "%2F");
        let (client, url) = self.client_for(&format!(
            "https://{}/api/v4/projects/{encoded_repository}/pipelines?per_page=1&order_by=id&sort=desc",
            project.host
        ), NetworkAccessScope::Coding)
        .await?;
        let payload: Vec<Value> = client
            .get(url)
            .header("PRIVATE-TOKEN", token)
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        Ok(payload.first().and_then(|item| {
            Some(CodingPipeline {
                project_id: project.id.clone(),
                id: item["id"].as_i64()?,
                status: item["status"].as_str().unwrap_or("unknown").to_owned(),
                reference: item["ref"].as_str().unwrap_or("").to_owned(),
                sha: item["sha"].as_str().unwrap_or("").chars().take(8).collect(),
                url: item["web_url"].as_str().unwrap_or("#").to_owned(),
                updated_at: item["updated_at"].as_str().unwrap_or("").to_owned(),
            })
        }))
    }

    pub(crate) fn decrypt_secret(&self, encoded: &str) -> Result<String, String> {
        let cipher = self
            .cipher
            .as_ref()
            .ok_or_else(|| "server secret storage is not configured".to_owned())?;
        let packed = STANDARD
            .decode(encoded)
            .map_err(|_| "stored credential is invalid".to_owned())?;
        if packed.len() <= 24 {
            return Err("stored credential is invalid".to_owned());
        }
        let nonce_bytes: [u8; 24] = packed[..24]
            .try_into()
            .map_err(|_| "stored credential is invalid".to_owned())?;
        let nonce = XNonce::from(nonce_bytes);
        let plaintext = cipher
            .decrypt(&nonce, &packed[24..])
            .map_err(|_| "stored credential could not be decrypted".to_owned())?;
        String::from_utf8(plaintext).map_err(|_| "stored credential is invalid".to_owned())
    }

    pub async fn clear_cache(&self, widget_id: &str) {
        self.cache.write().await.remove(widget_id);
    }

    /// Returns the current generation and latest Coding response for one account.
    ///
    /// The background worker owns freshness. Readers keep seeing the last completed snapshot
    /// while the next provider refresh is in flight.
    pub async fn coding_cache_snapshot(&self, account_id: &str) -> (u64, Option<CodingResponse>) {
        let cache = self.coding_cache.read().await;
        let generation = cache
            .generations
            .get(account_id)
            .copied()
            .unwrap_or_default();
        let response = cache
            .entries
            .get(account_id)
            .map(|cached| cached.value.clone());
        (generation, response)
    }

    /// Invalidates one account's Coding response and returns its new generation.
    pub async fn clear_coding_cache(&self, account_id: &str) -> u64 {
        let mut cache = self.coding_cache.write().await;
        cache.entries.remove(account_id);
        let generation = cache.generations.entry(account_id.to_owned()).or_default();
        *generation = generation.wrapping_add(1);
        *generation
    }

    /// Stores a Coding response unless the account changed while it was being fetched.
    pub async fn cache_coding_if_current(
        &self,
        account_id: &str,
        generation: u64,
        response: &CodingResponse,
    ) {
        let mut cache = self.coding_cache.write().await;
        let current_generation = cache
            .generations
            .get(account_id)
            .copied()
            .unwrap_or_default();
        if current_generation == generation {
            cache.entries.insert(
                account_id.to_owned(),
                CachedCodingData {
                    value: response.clone(),
                },
            );
        }
    }

    /// Loads fresh or cached data for one owned provider widget.
    ///
    /// # Errors
    ///
    /// Returns an error when credentials cannot be decrypted, configuration is incomplete, or
    /// the provider response cannot be fetched or parsed.
    pub async fn fetch(
        &self,
        widget: &DashboardWidget,
        encrypted_secret: Option<&str>,
    ) -> Result<Value, String> {
        let cacheable = widget.kind != "bible-verse";
        if cacheable {
            if let Some(cached) = self.cache.read().await.get(&widget.id)
                && cached.stored_at.elapsed() < CACHE_DURATION
            {
                return Ok(cached.value.clone());
            }
        }

        let secret = encrypted_secret
            .map(|value| self.decrypt_secret(value))
            .transpose()?;
        let value = match widget.kind.as_str() {
            "youtube" => self.fetch_youtube(&widget.config).await?,
            "reddit" => self.fetch_reddit(&widget.config, secret.as_deref()).await?,
            "stocks" => self.fetch_stocks(&widget.config).await?,
            "releases" => {
                self.fetch_releases(&widget.config, secret.as_deref())
                    .await?
            }
            "streams" => {
                self.fetch_streams(&widget.config, secret.as_deref())
                    .await?
            }
            "bible-verse" => crate::bible::daily_verse().await?,
            _ => return Err("this widget does not load remote data".to_owned()),
        };
        if cacheable {
            self.cache.write().await.insert(
                widget.id.clone(),
                CachedData {
                    stored_at: Instant::now(),
                    value: value.clone(),
                },
            );
        }
        Ok(value)
    }

    async fn fetch_youtube(&self, config: &Value) -> Result<Value, String> {
        let include_shorts = config_bool(config, "include_shorts", false);
        let limit = config_limit(config, 12);
        let mut sources = config_strings(config, "channels", 12);
        sources.extend(
            config_strings(config, "playlists", 12)
                .into_iter()
                .map(|id| format!("playlist:{id}")),
        );
        if sources.is_empty() {
            return Err("add at least one YouTube channel or playlist ID".to_owned());
        }
        let requests = sources.into_iter().map(|source| {
            let service = self.clone();
            async move {
                let url = if let Some(playlist) = source.strip_prefix("playlist:") {
                    format!("https://www.youtube.com/feeds/videos.xml?playlist_id={playlist}")
                } else if !include_shorts && source.starts_with("UC") {
                    format!(
                        "https://www.youtube.com/feeds/videos.xml?playlist_id={}",
                        source.replacen("UC", "UULF", 1)
                    )
                } else {
                    format!("https://www.youtube.com/feeds/videos.xml?channel_id={source}")
                };
                let (client, url) = service
                    .client_for(&url, NetworkAccessScope::Youtube)
                    .await?;
                let response = client.get(url).send().await.map_err(request_error)?;
                let text = response_text(response).await?;
                parse_youtube_feed(&text)
            }
        });
        let mut items = Vec::new();
        let mut failures = 0;
        for result in join_all(requests).await {
            match result {
                Ok(mut entries) => items.append(&mut entries),
                Err(_) => failures += 1,
            }
        }
        if items.is_empty() {
            return Err("YouTube returned no videos for the configured sources".to_owned());
        }
        items.sort_by_key(|item| Reverse(value_string(item, "published_at")));
        items.truncate(limit);
        Ok(json!({ "items": items, "partial": failures > 0 }))
    }

    async fn fetch_reddit(&self, config: &Value, secret: Option<&str>) -> Result<Value, String> {
        let subreddit = config_string(config, "subreddit")
            .filter(|value| valid_slug(value))
            .ok_or_else(|| "enter a valid subreddit name".to_owned())?;
        let sort = match config_string(config, "sort").as_deref() {
            Some("new") => "new",
            Some("top") => "top",
            Some("rising") => "rising",
            _ => "hot",
        };
        let source = RedditListingSource::new(
            subreddit.clone(),
            sort,
            u16::try_from(config_limit(config, 15)).unwrap_or(15),
            None,
        );
        let payload =
            if let (Some(client_id), Some(secret)) = (config_string(config, "client_id"), secret) {
                let (client, token_url) = self
                    .client_for(
                        "https://www.reddit.com/api/v1/access_token",
                        NetworkAccessScope::Rss,
                    )
                    .await?;
                let token: Value = client
                    .post(token_url)
                    .basic_auth(client_id, Some(secret))
                    .form(&[("grant_type", "client_credentials")])
                    .send()
                    .await
                    .map_err(request_error)?
                    .error_for_status()
                    .map_err(request_error)?
                    .json()
                    .await
                    .map_err(request_error)?;
                let access = token["access_token"]
                    .as_str()
                    .ok_or_else(|| "Reddit did not issue an access token".to_owned())?;
                let (client, url) = self
                    .client_for(
                        &format!("https://oauth.reddit.com/r/{subreddit}/{sort}"),
                        NetworkAccessScope::Rss,
                    )
                    .await?;
                client
                    .get(url)
                    .bearer_auth(access)
                    .query(&[
                        ("limit", source.limit.to_string()),
                        ("raw_json", "1".to_owned()),
                    ])
                    .send()
                    .await
                    .map_err(request_error)?
                    .error_for_status()
                    .map_err(request_error)?
                    .json::<RedditListingPayload>()
                    .await
                    .map_err(request_error)?
            } else {
                self.fetch_reddit_listing(&source).await?
            };
        let items = payload
            .data
            .children
            .iter()
            .filter_map(|child| {
                let post = &child.data;
                let (url, comments_url) = reddit_post_links(post)?;
                Some(json!({
                    "title": post.title,
                    "url": url,
                    "comments_url": comments_url,
                    "score": post.ups,
                    "comments": post.num_comments,
                    "published_at": post.created_utc.as_ref(),
                    "source": format!("r/{subreddit}")
                }))
            })
            .collect::<Vec<_>>();
        Ok(json!({ "items": items }))
    }

    async fn fetch_stocks(&self, config: &Value) -> Result<Value, String> {
        let symbols = config_strings(config, "symbols", 12);
        if symbols.is_empty() {
            return Err("add at least one market symbol".to_owned());
        }
        let items = self
            .fetch_yahoo_stock_quotes(&symbols)
            .await?
            .into_iter()
            .map(|quote| {
                let title = quote.name.unwrap_or_else(|| quote.symbol.clone());
                let url = format!("https://finance.yahoo.com/quote/{}", quote.symbol);
                json!({
                    "title": title,
                    "symbol": quote.symbol,
                    "value": quote.price,
                    "change": quote.change_percent,
                    "currency": quote.currency,
                    "url": url
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "items": items }))
    }

    /// Fetches recent Yahoo Finance prices through the guarded widget network policy.
    ///
    /// Yahoo's chart endpoint is queried once per symbol with bounded concurrency. The alternate
    /// query host and then the bounded public quote page are used only when the preceding source
    /// fails.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when neither chart host nor any bounded quote-page fallback
    /// produces a usable quote.
    pub async fn fetch_yahoo_stock_quotes(
        &self,
        symbols: &[String],
    ) -> Result<Vec<MarketQuote>, String> {
        if symbols.is_empty() {
            return Ok(Vec::new());
        }
        let requests = symbols.iter().cloned().map(|symbol| {
            let service = self.clone();
            async move { service.fetch_yahoo_chart_or_page_quote(&symbol).await }
        });
        let results = stream::iter(requests)
            .buffer_unordered(2)
            .collect::<Vec<_>>()
            .await;
        let first_error = results
            .iter()
            .find_map(|result| result.as_ref().err().cloned());
        let quotes = results
            .into_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        if quotes.is_empty() {
            Err(first_error.unwrap_or_else(|| "Yahoo Finance returned no quotes".to_owned()))
        } else {
            Ok(quotes)
        }
    }

    async fn fetch_yahoo_chart_or_page_quote(&self, symbol: &str) -> Result<MarketQuote, String> {
        for endpoint in YAHOO_CHART_ENDPOINTS {
            if let Ok(quote) = self.fetch_yahoo_chart_quote(endpoint, symbol).await {
                return Ok(quote);
            }
        }
        self.fetch_yahoo_quote_page(symbol).await
    }

    async fn fetch_yahoo_chart_quote(
        &self,
        base_url: &str,
        symbol: &str,
    ) -> Result<MarketQuote, String> {
        let mut endpoint =
            Url::parse(base_url).map_err(|_| "Yahoo Finance endpoint is invalid".to_owned())?;
        endpoint
            .path_segments_mut()
            .map_err(|()| "Yahoo Finance endpoint is invalid".to_owned())?
            .extend(["v8", "finance", "chart", symbol]);
        let (client, url) = self
            .client_for(endpoint.as_str(), NetworkAccessScope::Widgets)
            .await?;
        let payload: Value = client
            .get(url)
            .query(&[("interval", "1d"), ("range", "1d")])
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        parse_yahoo_chart_quote(symbol, &payload)
            .ok_or_else(|| format!("Yahoo Finance returned no quote for {symbol}"))
    }

    async fn fetch_yahoo_quote_page(&self, symbol: &str) -> Result<MarketQuote, String> {
        let mut endpoint = Url::parse("https://finance.yahoo.com/quote/")
            .map_err(|_| "Yahoo Finance endpoint is invalid".to_owned())?;
        endpoint
            .path_segments_mut()
            .map_err(|()| "Yahoo Finance endpoint is invalid".to_owned())?
            .push(symbol)
            .push("");
        endpoint
            .query_pairs_mut()
            .append_pair("lang", "en-US")
            .append_pair("region", "US");
        let (client, url) = self
            .client_for(endpoint.as_str(), NetworkAccessScope::Widgets)
            .await?;
        let response = client
            .get(url)
            .header(
                header::ACCEPT,
                "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1",
            )
            .send()
            .await
            .map_err(request_error)?;
        let document = response_prefix_text(response, MAX_YAHOO_QUOTE_PAGE_BYTES).await?;
        parse_yahoo_quote_page(symbol, &document)
            .ok_or_else(|| format!("Yahoo Finance returned no quote for {symbol}"))
    }

    /// Fetches the latest Finnhub REST quote for each symbol using an encrypted account key.
    ///
    /// Trading exposes these snapshots as a browser-scoped SSE feed. The provider requests stop
    /// when that page connection closes; there is no background market-data worker.
    ///
    /// # Errors
    ///
    /// Returns a safe provider or credential error when no requested symbol produces a usable
    /// quote.
    pub async fn fetch_finnhub_stock_quotes(
        &self,
        symbols: &[String],
        encrypted_api_key: &str,
    ) -> Result<Vec<MarketQuote>, String> {
        if symbols.is_empty() {
            return Ok(Vec::new());
        }
        let token: Arc<str> = Arc::from(self.decrypt_secret(encrypted_api_key)?);
        let requests = symbols.iter().cloned().map(|symbol| {
            let service = self.clone();
            let token = Arc::clone(&token);
            async move { service.fetch_finnhub_quote(&symbol, &token).await }
        });
        let results = join_all(requests).await;
        let first_error = results
            .iter()
            .find_map(|result| result.as_ref().err().cloned());
        let quotes = results
            .into_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        if quotes.is_empty() {
            Err(first_error.unwrap_or_else(|| "Finnhub returned no quotes".to_owned()))
        } else {
            Ok(quotes)
        }
    }

    async fn fetch_finnhub_quote(&self, symbol: &str, token: &str) -> Result<MarketQuote, String> {
        let (client, url) = self
            .client_for(
                "https://finnhub.io/api/v1/quote",
                NetworkAccessScope::Widgets,
            )
            .await?;
        let payload: Value = client
            .get(url)
            .header("X-Finnhub-Token", token)
            .query(&[("symbol", symbol)])
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        parse_finnhub_quote(symbol, &payload)
            .ok_or_else(|| format!("Finnhub returned no quote for {symbol}"))
    }

    async fn fetch_releases(&self, config: &Value, secret: Option<&str>) -> Result<Value, String> {
        let repositories = config_strings(config, "repositories", 12);
        if repositories.is_empty() {
            return Err("add at least one provider:owner/repository entry".to_owned());
        }
        let requests = repositories.into_iter().map(|repository| {
            let service = self.clone();
            let token = secret.map(str::to_owned);
            async move { fetch_release(&service, &repository, token.as_deref()).await }
        });
        let mut items = join_all(requests)
            .await
            .into_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        items.sort_by_key(|item| Reverse(value_string(item, "published_at")));
        if items.is_empty() {
            return Err("no releases could be loaded".to_owned());
        }
        Ok(json!({ "items": items }))
    }

    async fn fetch_streams(&self, config: &Value, secret: Option<&str>) -> Result<Value, String> {
        let mut twitch_channels = config_strings(config, "twitch_channels", 20);
        let mut kick_channels = config_strings(config, "kick_channels", 20);
        if twitch_channels.is_empty() && kick_channels.is_empty() {
            let platform = config_string(config, "platform").unwrap_or_else(|| "twitch".to_owned());
            let legacy_channels = config_strings(config, "channels", 20);
            if platform == "kick" {
                kick_channels = legacy_channels;
            } else {
                twitch_channels = legacy_channels;
            }
        }
        if twitch_channels.is_empty() && kick_channels.is_empty() {
            return Err("add at least one channel".to_owned());
        }

        let mut items = Vec::new();
        let mut partial = false;
        if !twitch_channels.is_empty() {
            match self
                .fetch_twitch_streams(twitch_channels, config, secret)
                .await
            {
                Ok(value) => items.extend(value["items"].as_array().cloned().unwrap_or_default()),
                Err(error) if kick_channels.is_empty() => return Err(error),
                Err(_) => partial = true,
            }
        }
        if !kick_channels.is_empty() {
            match self.fetch_kick_streams(kick_channels).await {
                Ok(value) => items.extend(value["items"].as_array().cloned().unwrap_or_default()),
                Err(error) if items.is_empty() => return Err(error),
                Err(_) => partial = true,
            }
        }
        items.sort_by(|left, right| {
            right["live"]
                .as_bool()
                .cmp(&left["live"].as_bool())
                .then_with(|| value_string(left, "title").cmp(&value_string(right, "title")))
        });
        Ok(json!({ "items": items, "partial": partial }))
    }

    async fn fetch_kick_streams(&self, channels: Vec<String>) -> Result<Value, String> {
        let requests = channels.into_iter().map(|channel| {
            let service = self.clone();
            async move {
                let (client, url) = service
                    .client_for(
                        &format!("https://kick.com/api/v2/channels/{channel}"),
                        NetworkAccessScope::Widgets,
                    )
                    .await?;
                let payload: Value = client
                    .get(url)
                    .send()
                    .await
                    .map_err(request_error)?
                    .error_for_status()
                    .map_err(request_error)?
                    .json()
                    .await
                    .map_err(request_error)?;
                Ok::<_, String>(json!({
                    "title": payload["user"]["username"].as_str().unwrap_or(&channel),
                    "url": format!("https://kick.com/{channel}"),
                    "provider": "Kick",
                    "live": payload["livestream"].is_object(),
                    "viewers": payload["livestream"]["viewer_count"],
                    "category": payload["livestream"]["categories"][0]["name"],
                    "thumbnail": payload["livestream"]["thumbnail"]["url"]
                }))
            }
        });
        let items = join_all(requests)
            .await
            .into_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        Ok(json!({ "items": items }))
    }

    async fn fetch_twitch_streams(
        &self,
        channels: Vec<String>,
        config: &Value,
        secret: Option<&str>,
    ) -> Result<Value, String> {
        let client_id = config_string(config, "client_id")
            .ok_or_else(|| "Twitch requires a client ID".to_owned())?;
        let secret = secret.ok_or_else(|| "Twitch requires a stored client secret".to_owned())?;
        let (token_client, token_url) = self
            .client_for(
                "https://id.twitch.tv/oauth2/token",
                NetworkAccessScope::Widgets,
            )
            .await?;
        let token: Value = token_client
            .post(token_url)
            .query(&[
                ("client_id", client_id.as_str()),
                ("client_secret", secret),
                ("grant_type", "client_credentials"),
            ])
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        let access = token["access_token"]
            .as_str()
            .ok_or_else(|| "Twitch did not issue an access token".to_owned())?;
        let mut url = Url::parse("https://api.twitch.tv/helix/streams")
            .map_err(|_| "Twitch URL is invalid".to_owned())?;
        for channel in &channels {
            url.query_pairs_mut().append_pair("user_login", channel);
        }
        let (client, url) = self
            .client_for(url.as_str(), NetworkAccessScope::Widgets)
            .await?;
        let payload: Value = client
            .get(url)
            .header("Client-Id", client_id)
            .bearer_auth(access)
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        let live = payload["data"].as_array().cloned().unwrap_or_default();
        let live_by_name = live
            .into_iter()
            .filter_map(|item| {
                let name = item["user_login"].as_str()?.to_owned();
                Some((name, item))
            })
            .collect::<HashMap<_, _>>();
        let items = channels
            .iter()
            .map(|channel| {
                let stream = live_by_name.get(&channel.to_ascii_lowercase());
                json!({
                    "title": stream.and_then(|v| v["user_name"].as_str()).unwrap_or(channel),
                    "url": format!("https://twitch.tv/{channel}"),
                    "provider": "Twitch",
                    "live": stream.is_some(),
                    "viewers": stream.map_or(Value::Null, |v| v["viewer_count"].clone()),
                    "category": stream.map_or(Value::Null, |v| v["game_name"].clone()),
                    "thumbnail": stream.map_or(Value::Null, |v| v["thumbnail_url"].clone())
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "items": items }))
    }
}

fn supported_favicon_content_type(value: &str) -> bool {
    matches!(
        value,
        "image/avif"
            | "image/jpeg"
            | "image/png"
            | "image/webp"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
            | "image/svg+xml"
    )
}

fn rasterize_svg_icon(bytes: &[u8], max_bytes: usize) -> Result<Vec<u8>, String> {
    const ICON_SIZE: u32 = 128;
    let mut options = resvg::usvg::Options::default();
    options.resources_dir = None;
    options.image_href_resolver = resvg::usvg::ImageHrefResolver {
        resolve_data: Box::new(|_, _, _| None),
        resolve_string: Box::new(|_, _| None),
    };
    let tree = resvg::usvg::Tree::from_data(bytes, &options)
        .map_err(|_| "provider SVG icon was invalid".to_owned())?;
    let size = tree.size();
    let canvas = ICON_SIZE as f32;
    let scale = (canvas / size.width()).min(canvas / size.height());
    let translate_x = (canvas - size.width() * scale) / 2.0;
    let translate_y = (canvas - size.height() * scale) / 2.0;
    let transform =
        resvg::tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, translate_x, translate_y);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(ICON_SIZE, ICON_SIZE)
        .ok_or_else(|| "provider SVG icon canvas was invalid".to_owned())?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let png = pixmap
        .encode_png()
        .map_err(|_| "provider SVG icon could not be encoded".to_owned())?;
    if png.len() > max_bytes {
        return Err("provider SVG icon was too large after rendering".to_owned());
    }
    Ok(png)
}

fn parse_declared_favicon_urls(document_url: &Url, document: &str) -> Vec<String> {
    let lowercase = document.to_ascii_lowercase();
    let mut urls = Vec::new();
    let mut offset = 0;
    while urls.len() < MAX_FAVICON_CANDIDATES {
        let Some(relative_start) = lowercase[offset..].find("<link") else {
            break;
        };
        let start = offset + relative_start;
        let Some(relative_end) = lowercase[start..].find('>') else {
            break;
        };
        let end = start + relative_end;
        let tag = &document[start..=end];
        let is_icon = html_attribute_case_insensitive(tag, "rel").is_some_and(|rel| {
            rel.split_ascii_whitespace().any(|value| {
                value.eq_ignore_ascii_case("icon") || value.to_ascii_lowercase().ends_with("-icon")
            })
        });
        if is_icon && let Some(href) = html_attribute_case_insensitive(tag, "href") {
            let href = href.replace("&amp;", "&");
            if let Ok(url) = document_url.join(&href)
                && matches!(url.scheme(), "http" | "https")
                && url.username().is_empty()
                && url.password().is_none()
            {
                let url = url.to_string();
                if !urls.contains(&url) {
                    urls.push(url);
                }
            }
        }
        offset = end + 1;
    }
    urls
}

fn html_attribute_case_insensitive<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lowercase = tag.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(relative_start) = lowercase[offset..].find(name) {
        let start = offset + relative_start;
        let before_is_boundary = start == 0
            || lowercase.as_bytes()[start - 1].is_ascii_whitespace()
            || lowercase.as_bytes()[start - 1] == b'<';
        let mut cursor = start + name.len();
        let after_is_boundary = lowercase
            .as_bytes()
            .get(cursor)
            .is_some_and(|value| value.is_ascii_whitespace() || *value == b'=');
        if !before_is_boundary || !after_is_boundary {
            offset = cursor;
            continue;
        }
        while lowercase
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        if lowercase.as_bytes().get(cursor) != Some(&b'=') {
            offset = cursor;
            continue;
        }
        cursor += 1;
        while lowercase
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        let delimiter = *lowercase.as_bytes().get(cursor)?;
        if matches!(delimiter, b'\'' | b'"') {
            cursor += 1;
            let end = lowercase.as_bytes()[cursor..]
                .iter()
                .position(|value| *value == delimiter)?
                + cursor;
            return Some(&tag[cursor..end]);
        }

        let end = lowercase.as_bytes()[cursor..]
            .iter()
            .position(|value| value.is_ascii_whitespace() || *value == b'>')?
            + cursor;
        if end > cursor {
            return Some(&tag[cursor..end]);
        }
        offset = cursor.saturating_add(1);
    }
    None
}

fn parse_rss_feed_snapshot(bytes: &[u8]) -> Result<RssFeedSnapshot, String> {
    let feed = feed_rs::parser::parse(bytes)
        .map_err(|error| format!("feed could not be parsed: {error}"))?;
    let title = feed
        .title
        .as_ref()
        .map_or_else(|| "Untitled feed".to_owned(), |value| value.content.clone());
    let scanned_comments = scan_rss_comments_urls(bytes);
    let comments_align = scanned_comments.len() == feed.entries.len().min(200);
    let fetched_at = chrono::Utc::now().to_rfc3339();
    let items = feed
        .entries
        .into_iter()
        .take(200)
        .enumerate()
        .map(|(index, entry)| {
            let raw_comments = comments_align
                .then(|| scanned_comments[index].as_deref())
                .flatten();
            let (url, comments_url) = reader_entry_links(&entry, raw_comments);
            let published_at = entry
                .published
                .or(entry.updated)
                .map_or_else(|| fetched_at.clone(), |date| date.to_rfc3339());
            let title = entry
                .title
                .map_or_else(|| "Untitled".to_owned(), |value| value.content);
            let external_id = if entry.id.trim().is_empty() {
                if url.is_empty() {
                    format!("{title}:{published_at}")
                } else {
                    url.clone()
                }
            } else {
                entry.id
            };
            let content = entry
                .content
                .and_then(|value| value.body)
                .or_else(|| entry.summary.map(|value| value.content))
                .unwrap_or_default();
            RssFeedEntry {
                external_id,
                url,
                comments_url,
                title,
                summary: content,
                published_at,
            }
        })
        .collect();
    Ok(RssFeedSnapshot { title, items })
}

fn reader_entry_links(entry: &Entry, raw_comments: Option<&str>) -> (String, String) {
    let comments_link = entry
        .links
        .iter()
        .find(|link| {
            link.rel
                .as_deref()
                .is_some_and(|rel| rel.eq_ignore_ascii_case("replies"))
        })
        .map(|link| link.href.as_str())
        .or(raw_comments);
    let url = entry
        .links
        .iter()
        .find(|link| {
            link.rel
                .as_deref()
                .is_none_or(|rel| rel.is_empty() || rel.eq_ignore_ascii_case("alternate"))
        })
        .or_else(|| {
            entry.links.iter().find(|link| {
                !link
                    .rel
                    .as_deref()
                    .is_some_and(|rel| rel.eq_ignore_ascii_case("replies"))
            })
        })
        .map_or_else(String::new, |link| safe_rss_entry_url(&link.href));
    let mut comments_url = comments_link.map_or_else(String::new, safe_rss_entry_url);
    if comments_url.is_empty() && is_reddit_comments_url(&url) {
        comments_url.clone_from(&url);
    }
    (url, comments_url)
}

fn safe_rss_entry_url(value: &str) -> String {
    let Ok(url) = Url::parse(value.trim()) else {
        return String::new();
    };
    if matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
    {
        url.into()
    } else {
        String::new()
    }
}

fn is_reddit_comments_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.host_str().is_some_and(|host| {
            (host.eq_ignore_ascii_case("reddit.com")
                || host.to_ascii_lowercase().ends_with(".reddit.com"))
                && url.path().contains("/comments/")
        })
    })
}

/// Collects one RSS 2.0 `<comments>` destination per feed item in document order.
///
/// `feed-rs` intentionally omits this RSS-only field, so it is scanned separately and aligned
/// only when the scanner and parser agree on the entry count. Namespaced `slash:comments` values
/// are comment counts and deliberately do not match the exact element name below.
fn scan_rss_comments_urls(bytes: &[u8]) -> Vec<Option<String>> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut comments = Vec::new();
    let mut in_entry = false;
    let mut capturing = false;
    let mut pending = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => match element.name().as_ref() {
                b"item" | b"entry" => {
                    in_entry = true;
                    pending.clear();
                }
                b"comments" if in_entry => capturing = true,
                _ => {}
            },
            Ok(Event::Text(text)) if capturing => {
                if let Ok(decoded) = text.decode()
                    && let Ok(unescaped) = quick_xml::escape::unescape(&decoded)
                {
                    pending.push_str(&unescaped);
                }
            }
            Ok(Event::CData(text)) if capturing => {
                if let Ok(decoded) = text.decode() {
                    pending.push_str(&decoded);
                }
            }
            Ok(Event::GeneralRef(reference)) if capturing => {
                if let Ok(name) = std::str::from_utf8(reference.as_ref()) {
                    let encoded = format!("&{name};");
                    if let Ok(decoded) = quick_xml::escape::unescape(&encoded) {
                        pending.push_str(&decoded);
                    }
                }
            }
            Ok(Event::End(element)) => match element.name().as_ref() {
                b"item" | b"entry" => {
                    let value = pending.trim();
                    comments.push((!value.is_empty()).then(|| value.to_owned()));
                    in_entry = false;
                    capturing = false;
                    if comments.len() >= 200 {
                        break;
                    }
                }
                b"comments" => capturing = false,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }
    comments
}

fn parse_carddav_response(source_id: &str, bytes: &[u8]) -> Result<Vec<ContactDraft>, String> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut in_address_data = false;
    let mut address_data = String::new();
    let mut contacts = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) if element.local_name().as_ref() == b"address-data" => {
                in_address_data = true;
                address_data.clear();
            }
            Ok(Event::Text(text)) if in_address_data => {
                address_data.push_str(
                    &text
                        .xml_content()
                        .map_err(|_| "CardDAV response contains invalid text".to_owned())?,
                );
            }
            Ok(Event::CData(text)) if in_address_data => {
                address_data.push_str(
                    &text
                        .xml_content()
                        .map_err(|_| "CardDAV response contains invalid text".to_owned())?,
                );
            }
            Ok(Event::End(element)) if element.local_name().as_ref() == b"address-data" => {
                in_address_data = false;
                if let Some(contact) = parse_vcard(source_id, &address_data) {
                    contacts.push(contact);
                    if contacts.len() > 5_000 {
                        return Err("CardDAV resource contains more than 5000 contacts".to_owned());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err("CardDAV response is not valid XML".to_owned()),
        }
        buffer.clear();
    }
    Ok(contacts)
}

fn parse_vcard(source_id: &str, input: &str) -> Option<ContactDraft> {
    let lines = unfold_vcard(input);
    if lines
        .iter()
        .any(|line| line.eq_ignore_ascii_case("KIND:group"))
    {
        return None;
    }
    let mut draft = ContactDraft {
        dav_source_id: Some(source_id.to_owned()),
        source_kind: "carddav".to_owned(),
        source_reference: None,
        first_name: String::new(),
        middle_name: String::new(),
        last_name: String::new(),
        nickname: String::new(),
        pronouns: String::new(),
        company: String::new(),
        job_title: String::new(),
        birthday: None,
        emails: Vec::new(),
        phones: Vec::new(),
        addresses: Vec::new(),
        important_dates: Vec::new(),
        tags: Vec::new(),
        relationship_context: String::new(),
        notes: String::new(),
        favorite: false,
        archived: false,
        photo: crate::contacts::parse_vcard_photo(input),
    };
    let mut formatted_name = String::new();
    for line in &lines {
        let Some((property, raw_value)) = line.split_once(':') else {
            continue;
        };
        let name = property
            .split(';')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let value = unescape_vcard(raw_value.trim());
        match name.as_str() {
            "UID" => draft.source_reference = non_empty(value),
            "N" => {
                let parts = raw_value.split(';').map(unescape_vcard).collect::<Vec<_>>();
                draft.last_name = parts.first().cloned().unwrap_or_default();
                draft.first_name = parts.get(1).cloned().unwrap_or_default();
                draft.middle_name = parts.get(2).cloned().unwrap_or_default();
            }
            "FN" => formatted_name = value,
            "NICKNAME" => draft.nickname = value.split(',').next().unwrap_or_default().to_owned(),
            "EMAIL" if !value.is_empty() => draft.emails.push(ContactMethod {
                label: vcard_label(property),
                value,
            }),
            "TEL" if !value.is_empty() => draft.phones.push(ContactMethod {
                label: vcard_label(property),
                value,
            }),
            "ADR" => {
                let parts = raw_value.split(';').map(unescape_vcard).collect::<Vec<_>>();
                draft.addresses.push(ContactAddress {
                    label: vcard_label(property),
                    street: parts.get(2).cloned().unwrap_or_default(),
                    city: parts.get(3).cloned().unwrap_or_default(),
                    region: parts.get(4).cloned().unwrap_or_default(),
                    postal_code: parts.get(5).cloned().unwrap_or_default(),
                    country: parts.get(6).cloned().unwrap_or_default(),
                });
            }
            "BDAY" => draft.birthday = normalized_vcard_birthday(&value),
            "ANNIVERSARY" => {
                if let Some(date) = normalized_vcard_date(&value) {
                    draft.important_dates.push(ContactImportantDate {
                        label: "Anniversary".to_owned(),
                        date,
                        recurring: true,
                    });
                }
            }
            "ORG" => draft.company = value.split(';').next().unwrap_or_default().to_owned(),
            "TITLE" => draft.job_title = value,
            "NOTE" => draft.notes = value,
            "CATEGORIES" => {
                draft.tags.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|tag| !tag.is_empty())
                        .map(str::to_owned),
                );
            }
            _ => {}
        }
    }
    if draft.first_name.is_empty() && draft.last_name.is_empty() && !formatted_name.is_empty() {
        draft.first_name = formatted_name;
    }
    if draft.first_name.is_empty() && draft.last_name.is_empty() && draft.nickname.is_empty() {
        return None;
    }
    let remote_reference = draft
        .source_reference
        .take()
        .unwrap_or_else(|| format!("vcard-{:016x}", stable_hash(input.as_bytes())));
    draft.source_reference = Some(format!("{source_id}:{remote_reference}"));
    Some(draft)
}

fn unfold_vcard(input: &str) -> Vec<String> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = Vec::new();
    for line in normalized.lines() {
        if line.starts_with([' ', '\t']) {
            if let Some(previous) = lines.last_mut() {
                previous.push_str(line.trim_start());
            }
        } else {
            lines.push(line.to_owned());
        }
    }
    lines
}

fn unescape_vcard(value: &str) -> String {
    value
        .replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

fn vcard_label(property: &str) -> String {
    property
        .split(';')
        .skip(1)
        .find_map(|parameter| {
            let (name, value) = parameter.split_once('=')?;
            name.eq_ignore_ascii_case("TYPE").then(|| {
                value
                    .split(',')
                    .next()
                    .unwrap_or("other")
                    .to_ascii_lowercase()
            })
        })
        .unwrap_or_else(|| "other".to_owned())
}

fn normalized_vcard_date(value: &str) -> Option<String> {
    let date = value.trim().replace('/', "-");
    if date.len() == 8 && date.chars().all(|character| character.is_ascii_digit()) {
        return Some(format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..8]));
    }
    (date.len() == 10).then_some(date)
}

fn normalized_vcard_birthday(value: &str) -> Option<String> {
    let birthday = value.trim().replace('/', "-");
    if birthday.starts_with("--") && crate::contacts::birthday_month_day(&birthday).is_some() {
        return Some(birthday);
    }
    normalized_vcard_date(&birthday)
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

/// Validates the public, non-secret configuration for one widget type.
///
/// # Errors
///
/// Returns a user-safe validation message when a field has the wrong shape or exceeds its limit.
pub fn validate_widget_config(kind: &str, config: &Value) -> Result<(), &'static str> {
    if !config.is_object() {
        return Err("widget configuration must be an object");
    }
    let serialized = serde_json::to_vec(config).map_err(|_| "widget configuration is invalid")?;
    if serialized.len() > 32_000 {
        return Err("widget configuration is too large");
    }
    match kind {
        "youtube" => validate_array(config, "channels", 12)
            .and_then(|()| validate_array(config, "playlists", 12)),
        "rss" => validate_array(config, "urls", 8),
        "reddit" => validate_text(config, "subreddit", 80),
        "stocks" => validate_array(config, "symbols", 12),
        "calendar" => validate_array(config, "events", 40),
        "clock" => validate_array(config, "timezones", 8),
        "iframe" => validate_text(config, "url", 2_000),
        "html" => validate_text(config, "source", 20_000),
        "releases" => validate_array(config, "repositories", 12),
        "streams" => validate_array(config, "channels", 20)
            .and_then(|()| validate_array(config, "twitch_channels", 20))
            .and_then(|()| validate_array(config, "kick_channels", 20))
            .and_then(|()| validate_stream_account_count(config)),
        _ => Ok(()),
    }
}

fn validate_stream_account_count(config: &Value) -> Result<(), &'static str> {
    let count = ["twitch_channels", "kick_channels"]
        .iter()
        .filter_map(|key| config.get(*key).and_then(Value::as_array))
        .map(Vec::len)
        .sum::<usize>();
    if count <= 20 {
        Ok(())
    } else {
        Err("a stream tracker can contain at most 20 accounts")
    }
}

fn validate_array(config: &Value, key: &str, max: usize) -> Result<(), &'static str> {
    match config.get(key) {
        None => Ok(()),
        Some(Value::Array(items))
            if items.len() <= max
                && items
                    .iter()
                    .all(|item| item.as_str().is_some_and(|value| value.len() <= 2_000)) =>
        {
            Ok(())
        }
        _ => Err("widget list configuration is invalid"),
    }
}

fn validate_text(config: &Value, key: &str, max: usize) -> Result<(), &'static str> {
    match config.get(key) {
        None => Ok(()),
        Some(Value::String(value)) if value.len() <= max => Ok(()),
        _ => Err("widget text configuration is invalid"),
    }
}

#[derive(Debug, Deserialize)]
struct YoutubeFeed {
    #[serde(default)]
    author: YoutubeAuthor,
    #[serde(rename = "entry", default)]
    entries: Vec<YoutubeEntry>,
}

#[derive(Debug, Default, Deserialize)]
struct YoutubeAuthor {
    #[serde(default)]
    name: String,
    #[serde(default)]
    uri: String,
}

#[derive(Debug, Deserialize)]
struct YoutubeEntry {
    #[serde(rename = "videoId", default)]
    video_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    published: String,
    #[serde(default)]
    link: YoutubeLink,
    #[serde(rename = "group", default)]
    group: YoutubeMediaGroup,
}

fn parse_youtube_snapshot(xml: &str) -> Result<YoutubeFeedSnapshot, String> {
    let feed: YoutubeFeed =
        from_str(xml).map_err(|error| format!("YouTube feed was invalid: {error}"))?;
    if feed.entries.is_empty() {
        return Err("YouTube returned no videos for this channel".to_owned());
    }
    let title = if feed.author.name.trim().is_empty() {
        "Untitled channel".to_owned()
    } else {
        feed.author.name.clone()
    };
    let channel_url = if feed.author.uri.trim().is_empty() {
        String::new()
    } else {
        feed.author.uri.clone()
    };
    let items = feed
        .entries
        .into_iter()
        .take(50)
        .map(|entry| YoutubeFeedEntry {
            external_id: if entry.video_id.trim().is_empty() {
                entry.link.href.clone()
            } else {
                entry.video_id
            },
            url: entry.link.href,
            thumbnail_url: entry.group.thumbnail.url,
            title: entry.title,
            published_at: entry.published,
        })
        .collect();
    Ok(YoutubeFeedSnapshot {
        title,
        channel_url,
        thumbnail_urls: Vec::new(),
        items,
    })
}

#[derive(Debug, Default, Deserialize)]
struct YoutubeLink {
    #[serde(rename = "@href", default)]
    href: String,
}

#[derive(Debug, Default, Deserialize)]
struct YoutubeMediaGroup {
    #[serde(default)]
    thumbnail: YoutubeThumbnail,
}

#[derive(Debug, Default, Deserialize)]
struct YoutubeThumbnail {
    #[serde(rename = "@url", default)]
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvidiousChannel {
    #[serde(default)]
    author: String,
    #[serde(default)]
    author_thumbnails: Vec<InvidiousThumbnail>,
    #[serde(default)]
    latest_videos: Vec<InvidiousVideo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvidiousVideo {
    #[serde(default)]
    video_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    video_thumbnails: Vec<InvidiousThumbnail>,
    #[serde(default)]
    published: i64,
}

#[derive(Debug, Deserialize)]
struct InvidiousThumbnail {
    #[serde(default)]
    url: String,
    #[serde(default)]
    width: u32,
}

fn parse_invidious_snapshot(
    base_url: &Url,
    channel_id: &str,
    json: &str,
) -> Result<YoutubeFeedSnapshot, String> {
    let channel: InvidiousChannel = serde_json::from_str(json)
        .map_err(|_| "Invidious returned an invalid channel response".to_owned())?;
    let items = channel
        .latest_videos
        .into_iter()
        .take(50)
        .filter_map(|video| {
            let video_id = video.video_id.trim();
            let published_at = chrono::DateTime::from_timestamp(video.published, 0)?;
            if video_id.is_empty() {
                return None;
            }
            let thumbnail_url = pick_invidious_thumbnail(base_url, &video.video_thumbnails)
                .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"));
            Some(YoutubeFeedEntry {
                external_id: video_id.to_owned(),
                url: format!("https://www.youtube.com/watch?v={video_id}"),
                thumbnail_url,
                title: video.title,
                published_at: published_at.to_rfc3339(),
            })
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Err("Invidious returned no videos for this channel".to_owned());
    }
    Ok(YoutubeFeedSnapshot {
        title: if channel.author.trim().is_empty() {
            channel_id.to_owned()
        } else {
            channel.author
        },
        channel_url: format!("https://www.youtube.com/channel/{channel_id}"),
        thumbnail_urls: invidious_thumbnail_candidates(base_url, &channel.author_thumbnails),
        items,
    })
}

fn pick_invidious_thumbnail(base_url: &Url, thumbnails: &[InvidiousThumbnail]) -> Option<String> {
    invidious_thumbnail_candidates(base_url, thumbnails)
        .into_iter()
        .next()
}

fn invidious_thumbnail_candidates(
    base_url: &Url,
    thumbnails: &[InvidiousThumbnail],
) -> Vec<String> {
    let mut thumbnails = thumbnails.iter().collect::<Vec<_>>();
    thumbnails.sort_unstable_by_key(|thumbnail| std::cmp::Reverse(thumbnail.width));
    let mut urls = thumbnails
        .into_iter()
        .filter_map(|thumbnail| normalize_invidious_url(base_url, &thumbnail.url))
        .collect::<Vec<_>>();
    urls.dedup();
    urls
}

fn normalize_invidious_url(base_url: &Url, value: &str) -> Option<String> {
    let value = value.trim();
    let url = if value.starts_with("//") {
        Url::parse(&format!("https:{value}")).ok()?
    } else {
        base_url.join(value).ok()?
    };
    (url.scheme() == "https" && url.username().is_empty() && url.password().is_none())
        .then(|| url.to_string())
}

fn parse_youtube_channel_portrait_urls(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut remainder = html;
    while let Some(start) = remainder.find("<meta") {
        remainder = &remainder[start..];
        let Some(end) = remainder.find('>') else {
            break;
        };
        let tag = &remainder[..=end];
        let is_portrait = tag.contains("property=\"og:image\"")
            || tag.contains("property='og:image'")
            || tag.contains("name=\"twitter:image\"")
            || tag.contains("name='twitter:image'");
        if is_portrait {
            if let Some(value) = html_attribute(tag, "content") {
                let value = value.replace("&amp;", "&");
                if Url::parse(&value).is_ok_and(|url| {
                    url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
                }) && !urls.contains(&value)
                {
                    urls.push(value);
                }
            }
        }
        remainder = &remainder[end + 1..];
    }
    urls
}

fn html_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let prefix = format!("{name}={quote}");
        if let Some(start) = tag.find(&prefix) {
            let value = &tag[start + prefix.len()..];
            return value.find(quote).map(|end| &value[..end]);
        }
    }
    None
}

fn parse_youtube_feed(xml: &str) -> Result<Vec<Value>, String> {
    let feed: YoutubeFeed =
        from_str(xml).map_err(|error| format!("YouTube feed was invalid: {error}"))?;
    Ok(feed
        .entries
        .into_iter()
        .map(|entry| {
            json!({
                "title": entry.title,
                "url": entry.link.href,
                "thumbnail": entry.group.thumbnail.url,
                "source": feed.author.name,
                "published_at": entry.published
            })
        })
        .collect())
}

async fn fetch_github_owned_repositories(
    service: &WidgetIntegrationService,
    host: &str,
    token: &str,
) -> Result<CodingOwnedRepositories, String> {
    if host != "github.com" {
        return Err("GitHub host is invalid".to_owned());
    }
    const QUERY: &str = r#"
        query OwnedRepositories($cursor: String) {
          viewer {
            repositories(
              first: 100
              after: $cursor
              affiliations: OWNER
              orderBy: { field: NAME, direction: ASC }
            ) {
              nodes {
                nameWithOwner
                isArchived
                pullRequests(states: OPEN) { totalCount }
              }
              pageInfo { hasNextPage endCursor }
            }
          }
        }
    "#;
    let endpoint = Url::parse("https://api.github.com/graphql")
        .map_err(|_| "GitHub API URL is invalid".to_owned())?;
    let mut repositories = Vec::new();
    let mut cursor: Option<String> = None;

    for _ in 0..MAX_PROVIDER_PAGES {
        let (client, endpoint) = service
            .client_for(endpoint.as_str(), NetworkAccessScope::Coding)
            .await?;
        let response = client
            .post(endpoint)
            .bearer_auth(token)
            .header(header::ACCEPT, "application/vnd.github+json")
            .json(&json!({ "query": QUERY, "variables": { "cursor": cursor } }))
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?;
        let bytes = response_bytes(response).await?;
        let payload: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "GitHub returned invalid repository data".to_owned())?;
        if payload["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty())
        {
            return Err("GitHub rejected the owned repository query".to_owned());
        }
        let connection = &payload["data"]["viewer"]["repositories"];
        let nodes = connection["nodes"]
            .as_array()
            .ok_or_else(|| "GitHub repository data was incomplete".to_owned())?;
        repositories.extend(nodes.iter().filter_map(|node| {
            owned_repository(
                "github",
                host,
                node["nameWithOwner"].as_str()?,
                node["isArchived"].as_bool().unwrap_or(false),
                node["pullRequests"]["totalCount"].as_u64(),
            )
        }));
        repositories.truncate(MAX_OWNED_REPOSITORIES);

        if !connection["pageInfo"]["hasNextPage"]
            .as_bool()
            .unwrap_or(false)
            || repositories.len() >= MAX_OWNED_REPOSITORIES
        {
            break;
        }
        cursor = connection["pageInfo"]["endCursor"]
            .as_str()
            .map(str::to_owned);
        if cursor.is_none() {
            return Err("GitHub repository pagination was incomplete".to_owned());
        }
    }

    Ok(CodingOwnedRepositories {
        repositories,
        errors: Vec::new(),
    })
}

async fn fetch_gitlab_owned_repositories(
    service: &WidgetIntegrationService,
    host: &str,
    token: &str,
) -> Result<CodingOwnedRepositories, String> {
    let mut page = 1_u64;
    let mut projects = Vec::new();
    for _ in 0..MAX_PROVIDER_PAGES {
        let (client, url) = service.client_for(&format!(
            "https://{host}/api/v4/projects?owned=true&simple=true&order_by=path&sort=asc&per_page=100&page={page}"
        ), NetworkAccessScope::Coding)
        .await?;
        let response = client
            .get(url)
            .header("PRIVATE-TOKEN", token)
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?;
        let next_page = response
            .headers()
            .get("x-next-page")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        let bytes = response_bytes(response).await?;
        let payload: Vec<Value> = serde_json::from_slice(&bytes)
            .map_err(|_| "GitLab returned invalid project data".to_owned())?;
        projects.extend(payload.into_iter().filter_map(|project| {
            let id = project["id"].as_i64()?;
            let repository = owned_repository(
                "gitlab",
                host,
                project["path_with_namespace"].as_str()?,
                project["archived"].as_bool().unwrap_or(false),
                None,
            )?;
            Some((id, repository))
        }));
        projects.truncate(MAX_OWNED_REPOSITORIES);
        let Some(next_page) = next_page else {
            break;
        };
        if projects.len() >= MAX_OWNED_REPOSITORIES {
            break;
        }
        page = next_page;
    }

    let results = stream::iter(projects)
        .map(|(project_id, mut repository)| async move {
            let count =
                fetch_gitlab_open_merge_request_count(service, host, token, project_id).await;
            if let Ok(count) = count {
                repository.open_pull_requests = Some(count);
            }
            (repository, count.err())
        })
        .buffer_unordered(PROVIDER_REQUEST_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    let mut snapshot = CodingOwnedRepositories::default();
    for (repository, error) in results {
        if let Some(error) = error {
            snapshot
                .errors
                .push(format!("{} pull requests: {error}", repository.repository));
        }
        snapshot.repositories.push(repository);
    }
    sort_owned_repositories(&mut snapshot.repositories);
    Ok(snapshot)
}

async fn fetch_gitlab_open_merge_request_count(
    service: &WidgetIntegrationService,
    host: &str,
    token: &str,
    project_id: i64,
) -> Result<u64, String> {
    let (client, url) = service.client_for(&format!(
        "https://{host}/api/v4/projects/{project_id}/merge_requests?state=opened&per_page=100&page=1"
    ), NetworkAccessScope::Coding)
    .await?;
    let response = client
        .get(url)
        .header("PRIVATE-TOKEN", token)
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?;
    let total = response
        .headers()
        .get("x-total")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let bytes = response_bytes(response).await?;
    let payload: Vec<Value> = serde_json::from_slice(&bytes)
        .map_err(|_| "GitLab returned invalid merge request data".to_owned())?;
    Ok(total.unwrap_or(payload.len() as u64))
}

async fn fetch_forge_owned_repositories(
    service: &WidgetIntegrationService,
    provider: &str,
    host: &str,
    token: &str,
) -> Result<CodingOwnedRepositories, String> {
    let (client, profile_url) = service
        .client_for(
            &format!("https://{host}/api/v1/user"),
            NetworkAccessScope::Coding,
        )
        .await?;
    let response = client
        .get(profile_url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?;
    let bytes = response_bytes(response).await?;
    let profile: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "code host returned invalid profile data".to_owned())?;
    let login = profile["login"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "code host profile did not include a login".to_owned())?;

    let mut repositories = Vec::new();
    let mut page = 1_u64;
    for _ in 0..MAX_PROVIDER_PAGES {
        let (client, url) = service
            .client_for(
                &format!("https://{host}/api/v1/user/repos?limit=50&page={page}"),
                NetworkAccessScope::Coding,
            )
            .await?;
        let response = client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?;
        let bytes = response_bytes(response).await?;
        let payload: Vec<Value> = serde_json::from_slice(&bytes)
            .map_err(|_| "code host returned invalid repository data".to_owned())?;
        let page_len = payload.len();
        repositories.extend(payload.into_iter().filter_map(|repository| {
            let owner = repository["owner"]["login"].as_str()?;
            if !owner.eq_ignore_ascii_case(login) {
                return None;
            }
            owned_repository(
                provider,
                host,
                repository["full_name"].as_str()?,
                repository["archived"].as_bool().unwrap_or(false),
                repository["open_pr_counter"].as_u64(),
            )
        }));
        repositories.truncate(MAX_OWNED_REPOSITORIES);
        if page_len < 50 || repositories.len() >= MAX_OWNED_REPOSITORIES {
            break;
        }
        page += 1;
    }
    sort_owned_repositories(&mut repositories);
    Ok(CodingOwnedRepositories {
        repositories,
        errors: Vec::new(),
    })
}

fn owned_repository(
    provider: &str,
    host: &str,
    repository: &str,
    archived: bool,
    open_pull_requests: Option<u64>,
) -> Option<CodingOwnedRepository> {
    if !valid_hostname(host)
        || repository.len() > 240
        || repository.split('/').count() < 2
        || repository
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
        || !repository.split('/').all(valid_slug)
    {
        return None;
    }
    Some(CodingOwnedRepository {
        provider: provider.to_owned(),
        host: host.to_owned(),
        repository: repository.to_owned(),
        url: format!("https://{host}/{repository}"),
        archived,
        open_pull_requests,
    })
}

fn sort_owned_repositories(repositories: &mut [CodingOwnedRepository]) {
    repositories.sort_by(|left, right| {
        right
            .open_pull_requests
            .unwrap_or_default()
            .cmp(&left.open_pull_requests.unwrap_or_default())
            .then_with(|| {
                left.repository
                    .to_ascii_lowercase()
                    .cmp(&right.repository.to_ascii_lowercase())
            })
    });
}

async fn fetch_release(
    service: &WidgetIntegrationService,
    repository: &str,
    token: Option<&str>,
) -> Result<Value, String> {
    let parsed = parse_release_repository(repository)?;
    let provider = parsed.provider.as_str();
    let repository = parsed.repository.as_str();
    let custom_host = matches!(provider, "gitea" | "forgejo").then_some(parsed.host.as_str());
    let (client, url) = if let Some(host) = custom_host {
        service
            .client_for(
                &format!("https://{host}/api/v1/repos/{repository}/releases/latest"),
                NetworkAccessScope::Coding,
            )
            .await?
    } else {
        let url = match provider {
            "github" => format!("https://api.github.com/repos/{repository}/releases/latest"),
            "gitlab" => format!(
                "https://gitlab.com/api/v4/projects/{}/releases/permalink/latest",
                repository.replace('/', "%2F")
            ),
            "codeberg" => format!("https://codeberg.org/api/v1/repos/{repository}/releases/latest"),
            _ => return Err("release provider is unsupported".to_owned()),
        };
        service.client_for(&url, NetworkAccessScope::Coding).await?
    };
    let mut request = client.get(url).header(header::ACCEPT, "application/json");
    if let Some(token) = token {
        request = match provider {
            "gitlab" => request.header("PRIVATE-TOKEN", token),
            _ => request.bearer_auth(token),
        };
    }
    let payload: Value = request
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    let release_url = if provider == "gitlab" {
        format!("https://{}/{repository}/-/releases", parsed.host)
    } else {
        payload["html_url"]
            .as_str()
            .or_else(|| payload["url"].as_str())
            .unwrap_or("#")
            .to_owned()
    };
    Ok(json!({
        "title": repository,
        "provider": provider,
        "version": payload["tag_name"].as_str().or_else(|| payload["name"].as_str()).unwrap_or("Latest"),
        "url": release_url,
        "published_at": payload["published_at"].as_str().or_else(|| payload["released_at"].as_str()).unwrap_or("")
    }))
}

/// Parses the Glance-style release repository syntax used by widgets and Coding subscriptions.
pub fn parse_release_repository(value: &str) -> Result<ReleaseRepository, String> {
    let value = value.trim();
    let custom = value
        .strip_prefix("gitea@")
        .map(|rest| ("gitea", rest))
        .or_else(|| value.strip_prefix("forgejo@").map(|rest| ("forgejo", rest)));
    let (provider, host, repository) = if let Some((provider, rest)) = custom {
        let (host, repository) = rest
            .split_once(':')
            .ok_or_else(|| "custom repository must be provider@host:owner/name".to_owned())?;
        if !valid_hostname(host) {
            return Err("custom repository host is invalid".to_owned());
        }
        (provider, host, repository)
    } else {
        let (provider, repository) = value
            .split_once(':')
            .map_or(("github", value), |parts| parts);
        let host = match provider {
            "github" => "github.com",
            "gitlab" => "gitlab.com",
            "codeberg" => "codeberg.org",
            _ => return Err("release provider is unsupported".to_owned()),
        };
        (provider, host, repository)
    };
    if !repository.split('/').all(valid_slug) || repository.matches('/').count() != 1 {
        return Err("repository must be owner/name".to_owned());
    }
    Ok(ReleaseRepository {
        provider: provider.to_owned(),
        host: host.to_owned(),
        repository: repository.to_owned(),
    })
}

fn parse_invidious_base_url(value: Option<&str>) -> Result<Option<Url>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let mut url = Url::parse(value).map_err(|_| "INVIDIOUS_BASE_URL is invalid".to_owned())?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("INVIDIOUS_BASE_URL must be a credential-free HTTPS URL".to_owned());
    }
    if url.host_str().is_none()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("INVIDIOUS_BASE_URL must be an HTTPS instance root".to_owned());
    }
    url.set_path("/");
    Ok(Some(url))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RedditListingSource {
    subreddit: String,
    sort: &'static str,
    limit: u16,
    period: Option<String>,
}

impl RedditListingSource {
    fn new(subreddit: String, sort: &'static str, limit: u16, period: Option<String>) -> Self {
        Self {
            subreddit,
            sort,
            limit: limit.clamp(1, 100),
            period,
        }
    }

    fn json_url(&self, origin: &str) -> Result<Url, String> {
        let mut source = Url::parse(origin).map_err(|_| "Reddit origin is invalid".to_owned())?;
        source.set_path(&format!(
            "/r/{}/{sort}.json",
            self.subreddit,
            sort = self.sort
        ));
        {
            let mut query = source.query_pairs_mut();
            query.append_pair("limit", &self.limit.to_string());
            query.append_pair("raw_json", "1");
            if self.sort == "top"
                && let Some(period) = &self.period
            {
                query.append_pair("t", period);
            }
        }
        Ok(source)
    }
}

fn reddit_listing_source(url: &Url) -> Option<RedditListingSource> {
    if !matches!(
        url.host_str(),
        Some(host)
            if matches!(
                host.to_ascii_lowercase().as_str(),
                "reddit.com" | "www.reddit.com" | "api.reddit.com" | "old.reddit.com"
            )
    ) {
        return None;
    }
    let segments = url.path_segments()?.collect::<Vec<_>>();
    let [root, subreddit, listing] = segments.as_slice() else {
        return None;
    };
    if !root.eq_ignore_ascii_case("r") || !valid_slug(subreddit) {
        return None;
    }
    let listing = listing
        .strip_suffix(".json")
        .or_else(|| listing.strip_suffix(".rss"))?;
    let sort = match listing {
        "hot" => "hot",
        "new" => "new",
        "top" => "top",
        "rising" => "rising",
        _ => return None,
    };
    let limit = url
        .query_pairs()
        .find_map(|(key, value)| (key == "limit").then_some(value.into_owned()))
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|limit| (1..=100).contains(limit))
        .unwrap_or(25);
    let period = url
        .query_pairs()
        .find_map(|(key, value)| (key == "t").then_some(value.into_owned()))
        .filter(|value| {
            matches!(
                value.as_str(),
                "hour" | "day" | "week" | "month" | "year" | "all"
            )
        });
    Some(RedditListingSource::new(
        (*subreddit).to_owned(),
        sort,
        limit,
        period,
    ))
}

fn parse_reddit_challenge(body: &str) -> Option<(String, String)> {
    static CHALLENGE_PATTERN: OnceLock<Regex> = OnceLock::new();
    static TOKEN_PATTERN: OnceLock<Regex> = OnceLock::new();
    let challenge_pattern = CHALLENGE_PATTERN.get_or_init(|| {
        Regex::new(r#"await\(async \w+\s*=>\s*\w+\s*\+\s*\w+\)\(\"([^\"]+)\"\)"#)
            .expect("Reddit challenge regex is valid")
    });
    let token_pattern = TOKEN_PATTERN.get_or_init(|| {
        Regex::new(r#"name=[\"']token[\"'][^>]*value=[\"']([^\"']+)[\"']"#)
            .expect("Reddit token regex is valid")
    });
    let challenge = challenge_pattern
        .captures(body)?
        .get(1)?
        .as_str()
        .to_owned();
    let token = token_pattern.captures(body)?.get(1)?.as_str().to_owned();
    Some((challenge, token))
}

fn reddit_loid_from_headers(headers: &primp::header::HeaderMap) -> Option<String> {
    headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| value.split(';').next())
        .filter_map(|cookie| cookie.split_once('='))
        .find_map(|(name, value)| {
            (name.trim().eq_ignore_ascii_case("loid")
                && !value.is_empty()
                && value.len() <= 512
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_graphic() && byte != b';'))
            .then(|| value.to_owned())
        })
}

fn reddit_listing_snapshot(
    source: &RedditListingSource,
    payload: &RedditListingPayload,
) -> RssFeedSnapshot {
    let items = payload
        .data
        .children
        .iter()
        .filter_map(|child| {
            let post = &child.data;
            let (url, comments_url) = reddit_post_links(post)?;
            let published_at = reddit_published_at(post.created_utc.as_ref()?)?;
            let external_id = if post.name.starts_with("t3_") {
                post.name.clone()
            } else if !post.id.is_empty() {
                format!("t3_{}", post.id)
            } else {
                comments_url.clone()
            };
            let title = quick_xml::escape::unescape(&post.title)
                .map_or_else(|_| post.title.clone(), std::borrow::Cow::into_owned);
            Some(RssFeedEntry {
                external_id,
                url,
                comments_url,
                title,
                summary: post.selftext.clone(),
                published_at,
            })
        })
        .collect();
    RssFeedSnapshot {
        title: format!("r/{}", source.subreddit),
        items,
    }
}

fn reddit_published_at(value: &serde_json::Number) -> Option<String> {
    let seconds = value.as_i64().or_else(|| {
        value
            .to_string()
            .split_once('.')
            .and_then(|(seconds, _)| seconds.parse::<i64>().ok())
    })?;
    if seconds <= 0 {
        return None;
    }
    chrono::DateTime::from_timestamp(seconds, 0).map(|value| value.to_rfc3339())
}

fn reddit_post_links(post: &RedditPost) -> Option<(String, String)> {
    let reddit = Url::parse("https://www.reddit.com/").ok()?;
    let comments_url = reddit.join(&post.permalink).ok()?;
    if comments_url.host_str() != Some("www.reddit.com") {
        return None;
    }
    let comments_url = comments_url.to_string();
    if post.is_self {
        return Some((comments_url.clone(), comments_url));
    }
    let article_url = Url::parse(&post.url)
        .ok()
        .filter(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        })
        .map_or_else(|| comments_url.clone(), |url| url.to_string());
    Some((article_url, comments_url))
}

fn reddit_status_error(status: u16) -> String {
    match status {
        403 => "Reddit refused the browser listing request".to_owned(),
        404 => "Reddit community or listing was not found".to_owned(),
        429 => "Reddit rate limit reached".to_owned(),
        status => format!("Reddit listing request failed with HTTP {status}"),
    }
}

fn ntfy_endpoint(base_url: &str, topics: &[&str]) -> Result<Url, String> {
    if topics.is_empty()
        || topics.len() > 32
        || topics
            .iter()
            .any(|topic| topic.is_empty() || topic.contains(','))
    {
        return Err("ntfy subscriptions require between 1 and 32 valid topics".to_owned());
    }
    let mut endpoint = Url::parse(base_url).map_err(|_| "ntfy server URL is invalid".to_owned())?;
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    let joined_topics = topics.join(",");
    {
        let mut segments = endpoint
            .path_segments_mut()
            .map_err(|_| "ntfy server URL cannot be used as a base".to_owned())?;
        segments.pop_if_empty();
        segments.push(&joined_topics);
        segments.push("json");
    }
    Ok(endpoint)
}

fn ntfy_poll_endpoint(base_url: &str, topics: &[&str], since: Option<&str>) -> Result<Url, String> {
    let mut endpoint = ntfy_endpoint(base_url, topics)?;
    endpoint
        .query_pairs_mut()
        .append_pair("poll", "1")
        .append_pair("since", since.unwrap_or("all"));
    Ok(endpoint)
}

fn ntfy_stream_endpoint(base_url: &str, topics: &[&str], since: &str) -> Result<Url, String> {
    let mut endpoint = ntfy_endpoint(base_url, topics)?;
    endpoint.query_pairs_mut().append_pair("since", since);
    Ok(endpoint)
}

pub(crate) fn parse_ntfy_message_line(line: &str) -> Result<Option<NtfyMessage>, String> {
    if line.trim().is_empty() {
        return Ok(None);
    }
    let message: NtfyMessage = serde_json::from_str(line)
        .map_err(|_| "ntfy returned an invalid message stream".to_owned())?;
    Ok((message.event == "message").then_some(message))
}

fn parse_ntfy_messages(body: &str) -> Result<Vec<NtfyMessage>, String> {
    let mut messages = Vec::new();
    for line in body.lines().filter(|line| !line.trim().is_empty()) {
        if let Some(message) = parse_ntfy_message_line(line)? {
            messages.push(message);
            if messages.len() >= 500 {
                break;
            }
        }
    }
    Ok(messages)
}

async fn response_text(response: reqwest::Response) -> Result<String, String> {
    let bytes = response_bytes(response).await?;
    String::from_utf8(bytes).map_err(|_| "response was not UTF-8".to_owned())
}

async fn response_prefix_text(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<String, String> {
    let mut response = response.error_for_status().map_err(request_error)?;
    let mut bytes = Vec::with_capacity(max_bytes);
    while bytes.len() < max_bytes {
        let Some(chunk) = response.chunk().await.map_err(request_error)? else {
            break;
        };
        let remaining = max_bytes - bytes.len();
        bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn response_bytes(response: reqwest::Response) -> Result<Vec<u8>, String> {
    response_bytes_with_limit(response, MAX_RESPONSE_BYTES).await
}

async fn response_bytes_with_limit(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut response = response.error_for_status().map_err(request_error)?;
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err("provider response was too large".to_owned());
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(max_bytes as u64) as usize,
    );
    while let Some(chunk) = response.chunk().await.map_err(request_error)? {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err("provider response was too large".to_owned());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn reddit_response_bytes(response: &mut primp::Response) -> Result<Vec<u8>, String> {
    let max_response_bytes = u64::try_from(MAX_RESPONSE_BYTES).unwrap_or(u64::MAX);
    if response
        .content_length()
        .is_some_and(|length| length > max_response_bytes)
    {
        return Err("Reddit response was too large".to_owned());
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or_default()
            .min(MAX_RESPONSE_BYTES),
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| reddit_request_error(&error))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("Reddit response was too large".to_owned());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn reddit_request_error(error: &primp::Error) -> String {
    let timed_out = error.is_timeout();
    let connect = error.is_connect();
    let status = error.status().map(|status| status.as_u16());
    let origin = error
        .url()
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|| "unknown".to_owned());
    let error_kind = if timed_out {
        "timeout"
    } else if connect {
        "connect"
    } else if status.is_some() {
        "http_status"
    } else {
        "transport"
    };
    tracing::warn!(
        %origin,
        ?status,
        error_kind,
        "Reddit browser request failed"
    );
    if timed_out {
        "Reddit request timed out".to_owned()
    } else {
        "Reddit request failed".to_owned()
    }
}

fn request_error(error: reqwest::Error) -> String {
    let timed_out = error.is_timeout();
    let connect = error.is_connect();
    let status = error.status().map(|status| status.as_u16());
    let origin = error
        .url()
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|| "unknown".to_owned());
    let error_kind = if timed_out {
        "timeout"
    } else if connect {
        "connect"
    } else if status.is_some() {
        "http_status"
    } else {
        "transport"
    };
    tracing::warn!(
        %origin,
        ?status,
        error_kind,
        "outbound provider request failed"
    );
    if timed_out {
        "provider request timed out".to_owned()
    } else {
        "provider request failed".to_owned()
    }
}

struct NtfyRequestLogContext<'a> {
    account_id: &'a str,
    operation: &'static str,
    origin: String,
    topic_count: usize,
    has_token: bool,
}

impl<'a> NtfyRequestLogContext<'a> {
    fn new(
        account_id: &'a str,
        operation: &'static str,
        endpoint: &Url,
        topic_count: usize,
        has_token: bool,
    ) -> Self {
        Self {
            account_id,
            operation,
            origin: endpoint.origin().ascii_serialization(),
            topic_count,
            has_token,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct NtfyResponseLogMetadata {
    content_type: Option<String>,
    retry_after: Option<String>,
    rate_limit_limit: Option<String>,
    rate_limit_remaining: Option<String>,
    rate_limit_reset: Option<String>,
    server: Option<String>,
    via: Option<String>,
    request_id: Option<String>,
}

fn ntfy_response_log_metadata(headers: &header::HeaderMap) -> NtfyResponseLogMetadata {
    NtfyResponseLogMetadata {
        content_type: safe_header_value(headers, &["content-type"]),
        retry_after: safe_header_value(headers, &["retry-after"]),
        rate_limit_limit: safe_header_value(headers, &["ratelimit-limit", "x-ratelimit-limit"]),
        rate_limit_remaining: safe_header_value(
            headers,
            &["ratelimit-remaining", "x-ratelimit-remaining"],
        ),
        rate_limit_reset: safe_header_value(headers, &["ratelimit-reset", "x-ratelimit-reset"]),
        server: safe_header_value(headers, &["server"]),
        via: safe_header_value(headers, &["via"]),
        request_id: safe_header_value(headers, &["x-request-id", "cf-ray", "traceparent"]),
    }
}

fn safe_header_value(headers: &header::HeaderMap, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        let value = headers.get(*name)?.to_str().ok()?.trim();
        (!value.is_empty() && !value.chars().any(char::is_control))
            .then(|| value.chars().take(160).collect())
    })
}

fn log_ntfy_response(response: &reqwest::Response, context: &NtfyRequestLogContext<'_>) {
    let metadata = ntfy_response_log_metadata(response.headers());
    let status = response.status().as_u16();
    let content_length = response.content_length();
    if response.status().is_success() {
        if context.operation == "realtime_stream" {
            tracing::info!(
                user_id = %context.account_id,
                operation = context.operation,
                origin = %context.origin,
                topic_count = context.topic_count,
                token_present = context.has_token,
                status,
                ?content_length,
                content_type = ?metadata.content_type,
                server = ?metadata.server,
                via = ?metadata.via,
                request_id = ?metadata.request_id,
                "ntfy upstream stream handshake succeeded"
            );
        } else {
            tracing::debug!(
                user_id = %context.account_id,
                operation = context.operation,
                origin = %context.origin,
                topic_count = context.topic_count,
                token_present = context.has_token,
                status,
                ?content_length,
                content_type = ?metadata.content_type,
                "ntfy upstream request succeeded"
            );
        }
        return;
    }
    tracing::warn!(
        user_id = %context.account_id,
        operation = context.operation,
        origin = %context.origin,
        topic_count = context.topic_count,
        token_present = context.has_token,
        status,
        ?content_length,
        content_type = ?metadata.content_type,
        retry_after = ?metadata.retry_after,
        rate_limit_limit = ?metadata.rate_limit_limit,
        rate_limit_remaining = ?metadata.rate_limit_remaining,
        rate_limit_reset = ?metadata.rate_limit_reset,
        server = ?metadata.server,
        via = ?metadata.via,
        request_id = ?metadata.request_id,
        "ntfy upstream request was rejected"
    );
}

fn ntfy_transport_error(error: reqwest::Error, context: &NtfyRequestLogContext<'_>) -> String {
    let timed_out = error.is_timeout();
    let connect = error.is_connect();
    let status = error.status().map(|status| status.as_u16());
    let kind = if timed_out {
        "timeout"
    } else if connect {
        "connect"
    } else {
        "request"
    };
    tracing::warn!(
        user_id = %context.account_id,
        operation = context.operation,
        origin = %context.origin,
        topic_count = context.topic_count,
        token_present = context.has_token,
        error_kind = kind,
        ?status,
        "ntfy upstream transport failed"
    );
    ntfy_request_error(error, context.has_token)
}

fn ntfy_request_error(error: reqwest::Error, has_token: bool) -> String {
    if error.is_timeout() {
        return "ntfy server request timed out; reconnecting automatically".to_owned();
    }
    if error.is_connect() {
        return "ntfy server could not be reached; check its URL and TLS certificate".to_owned();
    }
    let status = error.status();
    drop(error);
    status.map_or_else(
        || "ntfy server request failed; check its URL and TLS certificate".to_owned(),
        |status| ntfy_status_error(status, has_token),
    )
}

fn ntfy_publish_request_error(error: reqwest::Error, has_token: bool) -> String {
    let status = error.status();
    if status == Some(StatusCode::FORBIDDEN) {
        return if has_token {
            "ntfy access token cannot publish to the selected topic".to_owned()
        } else {
            "the selected ntfy topic requires an access token for publishing".to_owned()
        };
    }
    ntfy_request_error(error, has_token)
}

fn ntfy_status_error(status: StatusCode, has_token: bool) -> String {
    match status {
        StatusCode::UNAUTHORIZED if has_token => "ntfy access token was rejected".to_owned(),
        StatusCode::UNAUTHORIZED => "this ntfy server requires an access token".to_owned(),
        StatusCode::FORBIDDEN if has_token => {
            "ntfy access token cannot read one or more subscribed topics".to_owned()
        }
        StatusCode::FORBIDDEN => {
            "one or more subscribed ntfy topics require an access token".to_owned()
        }
        StatusCode::NOT_FOUND => {
            "ntfy subscription endpoint was not found; check the server URL".to_owned()
        }
        StatusCode::TOO_MANY_REQUESTS => {
            "ntfy server rate limit reached; reconnecting automatically".to_owned()
        }
        status if status.is_server_error() => {
            "ntfy server is temporarily unavailable; reconnecting automatically".to_owned()
        }
        status => format!(
            "ntfy server rejected the request (HTTP {})",
            status.as_u16()
        ),
    }
}

fn widget_client_builder() -> ClientBuilder {
    Client::builder()
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Pandan/0.1 widget fetcher")
}

fn ntfy_stream_client_builder() -> ClientBuilder {
    Client::builder()
        .connect_timeout(Duration::from_secs(4))
        // ntfy emits keepalives approximately every 45 seconds. This detects a dead upstream
        // without placing a total lifetime on a healthy subscription.
        .read_timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Pandan/0.1 ntfy subscriber")
}

fn config_strings(config: &Value, key: &str, max: usize) -> Vec<String> {
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

fn config_string(config: &Value, key: &str) -> Option<String> {
    config[key]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn config_bool(config: &Value, key: &str, default: bool) -> bool {
    config[key].as_bool().unwrap_or(default)
}

fn config_limit(config: &Value, default: usize) -> usize {
    config["limit"]
        .as_u64()
        .map_or(default, |limit| limit.clamp(1, 40) as usize)
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn valid_hostname(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
}

fn value_string(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}

fn parse_yahoo_chart_quote(requested_symbol: &str, payload: &Value) -> Option<MarketQuote> {
    let result = payload["chart"]["result"].as_array()?.first()?;
    let meta = &result["meta"];
    let price = decimal_string(&meta["regularMarketPrice"])
        .or_else(|| yahoo_chart_indicator_decimal(result, "close"))?;
    let symbol = meta["symbol"]
        .as_str()
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
        .unwrap_or(requested_symbol)
        .to_ascii_uppercase();
    Some(MarketQuote {
        symbol,
        name: clean_market_name(
            meta["shortName"]
                .as_str()
                .or_else(|| meta["longName"].as_str()),
        ),
        price,
        previous_close: decimal_string(&meta["chartPreviousClose"])
            .or_else(|| decimal_string(&meta["previousClose"])),
        day_open: decimal_string(&meta["regularMarketOpen"])
            .or_else(|| yahoo_chart_indicator_decimal(result, "open")),
        day_high: decimal_string(&meta["regularMarketDayHigh"])
            .or_else(|| yahoo_chart_indicator_decimal(result, "high")),
        day_low: decimal_string(&meta["regularMarketDayLow"])
            .or_else(|| yahoo_chart_indicator_decimal(result, "low")),
        change_percent: decimal_string(&meta["regularMarketChangePercent"]),
        currency: meta["currency"].as_str().unwrap_or_default().to_owned(),
        market_state: meta["marketState"].as_str().map(str::to_owned),
        quoted_at: epoch_rfc3339(&meta["regularMarketTime"])
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
    })
}

fn yahoo_chart_indicator_decimal(result: &Value, field: &str) -> Option<String> {
    result["indicators"]["quote"]
        .as_array()?
        .first()?
        .get(field)?
        .as_array()?
        .iter()
        .find_map(decimal_string)
}

fn parse_yahoo_quote_page(symbol: &str, document: &str) -> Option<MarketQuote> {
    let price = yahoo_html_text_value(document, "data-testid=\"qsp-price\"")
        .and_then(normalize_yahoo_decimal)?;
    let previous_close = yahoo_html_field_value(document, "regularMarketPreviousClose")
        .and_then(|value| normalize_yahoo_decimal(&value));
    let day_open = yahoo_html_field_value(document, "regularMarketOpen")
        .and_then(|value| normalize_yahoo_decimal(&value));
    let (day_low, day_high) = yahoo_html_field_value(document, "regularMarketDayRange")
        .and_then(|range| {
            let (low, high) = range.split_once(" - ")?;
            Some((normalize_yahoo_decimal(low), normalize_yahoo_decimal(high)))
        })
        .unwrap_or_default();
    Some(MarketQuote {
        symbol: symbol.to_ascii_uppercase(),
        name: yahoo_html_quote_name(document, symbol),
        price,
        previous_close,
        day_open,
        day_high,
        day_low,
        change_percent: yahoo_html_text_value(document, "data-testid=\"qsp-price-change-percent\"")
            .and_then(normalize_yahoo_decimal),
        // The semantic page markup does not expose a dependable currency attribute. Trading
        // retains the last batch-provided currency instead of guessing one.
        currency: String::new(),
        market_state: None,
        quoted_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn yahoo_html_text_value<'a>(document: &'a str, marker: &str) -> Option<&'a str> {
    let marker_start = document.find(marker)?;
    let value_start = marker_start + document[marker_start..].find('>')? + 1;
    let value_end = value_start + document[value_start..].find('<')?;
    Some(document[value_start..value_end].trim())
}

fn yahoo_html_field_value(document: &str, field: &str) -> Option<String> {
    let marker = format!("data-field=\"{field}\"");
    let marker_start = document.find(&marker)?;
    let tag_start = document[..marker_start].rfind('<')?;
    let tag_end = marker_start + document[marker_start..].find('>')?;
    html_attribute_case_insensitive(&document[tag_start..=tag_end], "data-value").map(str::to_owned)
}

fn yahoo_html_quote_name(document: &str, symbol: &str) -> Option<String> {
    let mut offset = 0;
    while let Some(relative_start) = document[offset..].find("<h1") {
        let start = offset + relative_start;
        let Some(relative_tag_end) = document[start..].find('>') else {
            break;
        };
        let tag_end = start + relative_tag_end;
        let tag = &document[start..=tag_end];
        if !tag.to_ascii_lowercase().contains("heading") {
            offset = tag_end + 1;
            continue;
        }
        let value_start = tag_end + 1;
        let value_end = value_start + document[value_start..].find("</h1>")?;
        let decoded = quick_xml::escape::unescape(document[value_start..value_end].trim()).ok()?;
        let symbol_suffix = format!(" ({})", symbol.to_ascii_uppercase());
        let name = decoded
            .strip_suffix(&symbol_suffix)
            .unwrap_or(decoded.as_ref())
            .trim();
        return clean_market_name(Some(name));
    }
    None
}

fn normalize_yahoo_decimal(value: &str) -> Option<String> {
    let mut value = value.trim();
    if value.starts_with('(') && value.ends_with(')') {
        value = &value[1..value.len() - 1];
    }
    value = value.strip_suffix('%').unwrap_or(value).trim();
    if value.is_empty() || value.len() > 64 {
        return None;
    }
    let normalized = value.replace(',', "");
    let mut saw_digit = false;
    let mut saw_decimal = false;
    for (index, character) in normalized.chars().enumerate() {
        match character {
            '+' | '-' if index == 0 => {}
            '.' if !saw_decimal => saw_decimal = true,
            '0'..='9' => saw_digit = true,
            _ => return None,
        }
    }
    saw_digit.then_some(normalized)
}

fn parse_finnhub_quote(symbol: &str, quote: &Value) -> Option<MarketQuote> {
    let price = decimal_string(&quote["c"])?;
    if decimal_is_zero(&price) {
        return None;
    }
    Some(MarketQuote {
        symbol: symbol.to_ascii_uppercase(),
        name: None,
        price,
        previous_close: decimal_string(&quote["pc"]),
        day_open: decimal_string(&quote["o"]),
        day_high: decimal_string(&quote["h"]),
        day_low: decimal_string(&quote["l"]),
        change_percent: decimal_string(&quote["dp"]),
        // Finnhub's quote response does not identify the instrument currency. Keep this empty so
        // Trading can retain the last Yahoo-provided currency instead of inventing one.
        currency: String::new(),
        market_state: None,
        quoted_at: epoch_rfc3339(&quote["t"]).unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
    })
}

fn decimal_string(value: &Value) -> Option<String> {
    if let Some(number) = value.as_number() {
        return Some(number.to_string());
    }
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn decimal_is_zero(value: &str) -> bool {
    value
        .chars()
        .all(|character| matches!(character, '0' | '.' | '-' | '+'))
}

fn clean_market_name(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    (!value.is_empty() && value.chars().count() <= 120 && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

fn epoch_rfc3339(value: &Value) -> Option<String> {
    chrono::DateTime::from_timestamp(value.as_i64()?, 0).map(|value| value.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_service() -> WidgetIntegrationService {
        let pool = db::connect("sqlite::memory:")
            .await
            .expect("test database connects");
        WidgetIntegrationService::for_tests(pool).expect("service initializes")
    }

    fn empty_coding_response() -> CodingResponse {
        CodingResponse {
            projects: Vec::new(),
            releases: Vec::new(),
            merge_requests: Vec::new(),
            owned_repositories: Vec::new(),
            pipelines: Vec::new(),
            credentials: Vec::new(),
            secret_storage_enabled: false,
            provider_errors: Vec::new(),
            cached_at: None,
        }
    }

    #[test]
    fn yahoo_chart_parser_preserves_provider_values_and_uses_daily_ohlc() {
        let payload = json!({
            "chart": {
                "result": [{
                    "meta": {
                        "symbol": "aapl",
                        "shortName": "Apple Inc.",
                        "regularMarketPrice": 229.31,
                        "regularMarketChangePercent": 0.68054,
                        "regularMarketDayHigh": 230.45,
                        "regularMarketDayLow": 227.9,
                        "chartPreviousClose": 227.76,
                        "currency": "USD",
                        "regularMarketTime": 1_725_000_000
                    },
                    "indicators": {
                        "quote": [{
                            "open": [228.12],
                            "high": [230.45],
                            "low": [227.9],
                            "close": [229.31]
                        }]
                    }
                }],
                "error": null
            }
        });

        let parsed = parse_yahoo_chart_quote("AAPL", &payload).expect("Yahoo chart quote parses");

        assert_eq!(parsed.symbol, "AAPL");
        assert_eq!(parsed.name.as_deref(), Some("Apple Inc."));
        assert_eq!(parsed.price, "229.31");
        assert_eq!(parsed.previous_close.as_deref(), Some("227.76"));
        assert_eq!(parsed.day_open.as_deref(), Some("228.12"));
        assert_eq!(parsed.day_high.as_deref(), Some("230.45"));
        assert_eq!(parsed.day_low.as_deref(), Some("227.9"));
        assert_eq!(parsed.change_percent.as_deref(), Some("0.68054"));
        assert_eq!(parsed.currency, "USD");
    }

    #[test]
    fn yahoo_chart_parser_rejects_a_response_without_a_price() {
        assert!(parse_yahoo_chart_quote("AAPL", &json!({ "chart": { "result": [] } })).is_none());
        assert!(
            parse_yahoo_chart_quote(
                "AAPL",
                &json!({
                    "chart": {
                        "result": [{
                            "meta": { "symbol": "AAPL" },
                            "indicators": { "quote": [{ "close": [null] }] }
                        }]
                    }
                }),
            )
            .is_none()
        );
    }

    #[test]
    fn yahoo_quote_page_parser_recovers_when_query_endpoints_are_rate_limited() {
        let document = r#"
            <h1 class="heading yf-test">Acme &amp; Co. (ACME)</h1>
            <span data-testid="qsp-price">1,234.50 </span>
            <span data-testid="qsp-price-change-percent">(-1.27%) </span>
            <fin-streamer data-value="1,250.00" data-field="regularMarketPreviousClose">
                1,250.00
            </fin-streamer>
            <fin-streamer data-field="regularMarketOpen" data-value="1,245.25">
                1,245.25
            </fin-streamer>
            <fin-streamer data-value="1,220.10 - 1,260.75" data-field="regularMarketDayRange">
                1,220.10 - 1,260.75
            </fin-streamer>
        "#;

        let parsed = parse_yahoo_quote_page("acme", document).expect("quote page parses");

        assert_eq!(parsed.symbol, "ACME");
        assert_eq!(parsed.name.as_deref(), Some("Acme & Co."));
        assert_eq!(parsed.price, "1234.50");
        assert_eq!(parsed.previous_close.as_deref(), Some("1250.00"));
        assert_eq!(parsed.day_open.as_deref(), Some("1245.25"));
        assert_eq!(parsed.day_low.as_deref(), Some("1220.10"));
        assert_eq!(parsed.day_high.as_deref(), Some("1260.75"));
        assert_eq!(parsed.change_percent.as_deref(), Some("-1.27"));
        assert!(parsed.currency.is_empty());
    }

    #[test]
    fn yahoo_quote_page_parser_rejects_missing_or_non_decimal_prices() {
        assert!(parse_yahoo_quote_page("AAPL", "<h1>Apple Inc. (AAPL)</h1>").is_none());
        assert!(
            parse_yahoo_quote_page(
                "AAPL",
                r#"<span data-testid="qsp-price">not available</span>"#,
            )
            .is_none()
        );
    }

    #[test]
    fn finnhub_quote_parser_rejects_missing_prices_and_does_not_invent_currency() {
        assert!(parse_finnhub_quote("AAPL", &json!({ "c": 0 })).is_none());

        let parsed = parse_finnhub_quote(
            "AAPL",
            &json!({
                "c": 229.31,
                "pc": 227.76,
                "o": 228.12,
                "h": 230.45,
                "l": 227.9,
                "dp": 0.68054,
                "t": 1_725_000_000
            }),
        )
        .expect("Finnhub quote parses");

        assert_eq!(parsed.price, "229.31");
        assert!(parsed.currency.is_empty());
        assert!(parsed.market_state.is_none());
    }

    #[test]
    fn declared_favicons_resolve_relative_urls_and_ignore_unsafe_sources() {
        let document_url = Url::parse("https://example.com/app/").expect("URL parses");
        let document = r#"
            <LINK REL="apple-touch-icon" HREF="icons/touch.png">
            <link rel='shortcut icon' href='/assets/site.ico?version=2'>
            <link rel=icon href=/assets/plain.svg>
            <link rel="icon" href="https://user:secret@example.com/private.png">
            <link rel="stylesheet" href="/assets/site.css">
        "#;

        assert_eq!(
            parse_declared_favicon_urls(&document_url, document),
            vec![
                "https://example.com/app/icons/touch.png",
                "https://example.com/assets/site.ico?version=2",
                "https://example.com/assets/plain.svg",
            ]
        );
    }

    #[test]
    fn svg_icons_are_rasterized_to_bounded_png() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 16">
            <rect width="32" height="16" rx="3" fill="#4caf72"/>
        </svg>"##;

        let png = rasterize_svg_icon(svg, 256 * 1024).expect("SVG rasterizes");

        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(png.len() <= 256 * 1024);
    }

    #[tokio::test]
    async fn coding_cache_is_account_scoped_and_keeps_the_last_completed_snapshot() {
        let service = test_service().await;
        let response = empty_coding_response();
        let (generation, cached) = service.coding_cache_snapshot("account-a").await;
        assert!(cached.is_none());

        service
            .cache_coding_if_current("account-a", generation, &response)
            .await;
        assert!(service.coding_cache_snapshot("account-a").await.1.is_some());
        assert!(service.coding_cache_snapshot("account-b").await.1.is_none());
    }

    #[tokio::test]
    async fn coding_cache_invalidation_rejects_an_in_flight_stale_response() {
        let service = test_service().await;
        let response = empty_coding_response();
        let (stale_generation, _) = service.coding_cache_snapshot("account-a").await;
        let current_generation = service.clear_coding_cache("account-a").await;

        service
            .cache_coding_if_current("account-a", stale_generation, &response)
            .await;
        assert!(service.coding_cache_snapshot("account-a").await.1.is_none());

        service
            .cache_coding_if_current("account-a", current_generation, &response)
            .await;
        assert!(service.coding_cache_snapshot("account-a").await.1.is_some());
    }

    #[test]
    fn widget_secrets_round_trip_without_plaintext_storage() {
        let key = STANDARD.encode([7_u8; 32]);
        let service = WidgetIntegrationService::new(Some(&key)).expect("cipher initializes");
        let ciphertext = service
            .encrypt_secret("private-token")
            .expect("secret encrypts");

        assert!(!ciphertext.contains("private-token"));
        assert_eq!(
            service
                .decrypt_secret(&ciphertext)
                .expect("secret decrypts"),
            "private-token"
        );
    }

    #[test]
    fn blank_widget_secret_key_disables_secret_storage() {
        for key in [None, Some(""), Some(" \t\n ")] {
            let service = WidgetIntegrationService::new(key).expect("blank key is optional");
            assert!(!service.secrets_enabled());
        }
    }

    #[test]
    fn configured_widget_secret_key_remains_strictly_validated() {
        let malformed = WidgetIntegrationService::new(Some("not-base64"))
            .err()
            .expect("malformed key is rejected");
        assert_eq!(malformed, "PANDAN_SECRET_KEY must be base64");

        let short_key = STANDARD.encode([7_u8; 31]);
        let wrong_length = WidgetIntegrationService::new(Some(&short_key))
            .err()
            .expect("short key is rejected");
        assert_eq!(
            wrong_length,
            "PANDAN_SECRET_KEY must decode to exactly 32 bytes"
        );
    }

    #[tokio::test]
    async fn invidious_private_network_exemption_covers_only_the_configured_instance() {
        let service =
            WidgetIntegrationService::new_with_invidious(None, Some("https://127.0.0.1/"), true)
                .expect("service initializes");
        assert!(service.invidious_allows_private_network());

        let allowed = service
            .validate_invidious_url("https://127.0.0.1/api/v1/channels/UCexample")
            .await
            .expect("the configured instance skips the private-network guard");
        assert_eq!(
            allowed.1.as_str(),
            "https://127.0.0.1/api/v1/channels/UCexample"
        );

        for rejected in [
            "http://127.0.0.1/api/v1/channels/UCexample",
            "https://operator:token@127.0.0.1/api/v1/channels/UCexample",
        ] {
            assert_eq!(
                service.validate_invidious_url(rejected).await.unwrap_err(),
                "only credential-free HTTPS URLs are allowed"
            );
        }

        // A different host, or the same host on another port, falls back to the shared policy.
        for external in [
            "https://127.0.0.2/api/v1/channels/UCexample",
            "https://127.0.0.1:8443/api/v1/channels/UCexample",
        ] {
            assert!(service.validate_invidious_url(external).await.is_err());
        }
    }

    #[tokio::test]
    async fn invidious_requests_stay_guarded_without_the_opt_in() {
        let service =
            WidgetIntegrationService::new_with_invidious(None, Some("https://127.0.0.1/"), false)
                .expect("service initializes");
        assert!(service.invidious_enabled());
        assert!(!service.invidious_allows_private_network());
        assert!(
            service
                .validate_invidious_url("https://127.0.0.1/api/v1/channels/UCexample")
                .await
                .is_err()
        );
    }

    #[test]
    fn invidious_instance_requires_a_clean_https_root() {
        let configured = parse_invidious_base_url(Some("https://inv.example/"))
            .expect("HTTPS instance root is accepted")
            .expect("instance is configured");
        assert_eq!(configured.as_str(), "https://inv.example/");
        assert!(parse_invidious_base_url(Some("")).unwrap().is_none());
        assert!(parse_invidious_base_url(Some("http://inv.example")).is_err());
        assert!(parse_invidious_base_url(Some("https://inv.example/api")).is_err());
        assert!(parse_invidious_base_url(Some("https://inv.example/?token=x")).is_err());
    }

    #[test]
    fn invidious_channel_response_supplies_videos_and_portrait() {
        let base_url = Url::parse("https://inv.example/").unwrap();
        let json = r#"{
          "author": "Example channel",
          "authorThumbnails": [
            {"url": "//yt3.ggpht.com/example=s88", "width": 88},
            {"url": "//yt3.ggpht.com/example=s512", "width": 512}
          ],
          "latestVideos": [{
            "videoId": "abc123def45",
            "title": "A new upload",
            "videoThumbnails": [
              {"url": "/vi/abc123def45/mqdefault.jpg", "width": 320}
            ],
            "published": 1786622400
          }]
        }"#;
        let snapshot = parse_invidious_snapshot(&base_url, "UCabcdefghijklmnopqrstuv", json)
            .expect("Invidious response parses");

        assert_eq!(snapshot.title, "Example channel");
        assert_eq!(
            snapshot.thumbnail_urls,
            vec![
                "https://yt3.ggpht.com/example=s512".to_owned(),
                "https://yt3.ggpht.com/example=s88".to_owned(),
            ]
        );
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].external_id, "abc123def45");
        assert_eq!(
            snapshot.items[0].thumbnail_url,
            "https://inv.example/vi/abc123def45/mqdefault.jpg"
        );
        assert_eq!(
            snapshot.items[0].url,
            "https://www.youtube.com/watch?v=abc123def45"
        );
    }

    #[test]
    fn youtube_channel_metadata_supplies_only_safe_portrait_candidates() {
        let html = r#"
          <meta content="https://yt3.googleusercontent.com/channel=s900&amp;v=1" property="og:image">
          <meta name='twitter:image' content='https://yt3.googleusercontent.com/channel=s512'>
          <meta property="og:image" content="http://example.test/insecure.jpg">
          <meta property="og:image" content="https://user@example.test/credential.jpg">
        "#;

        assert_eq!(
            parse_youtube_channel_portrait_urls(html),
            vec![
                "https://yt3.googleusercontent.com/channel=s900&v=1".to_owned(),
                "https://yt3.googleusercontent.com/channel=s512".to_owned(),
            ]
        );
    }

    #[test]
    fn reddit_listing_urls_are_recognized_without_claiming_other_paths() {
        let source =
            Url::parse("https://www.reddit.com/r/selfhosted/top.json?limit=25&raw_json=1&t=week")
                .unwrap();
        assert_eq!(
            reddit_listing_source(&source),
            Some(RedditListingSource::new(
                "selfhosted".to_owned(),
                "top",
                25,
                Some("week".to_owned())
            ))
        );
        assert_eq!(
            reddit_listing_source(
                &Url::parse("https://www.reddit.com/r/selfhosted/new.rss?limit=25").unwrap()
            ),
            Some(RedditListingSource::new(
                "selfhosted".to_owned(),
                "new",
                25,
                None
            ))
        );
        assert!(
            reddit_listing_source(&Url::parse("https://www.reddit.com/search.json").unwrap())
                .is_none()
        );
        assert!(
            reddit_listing_source(
                &Url::parse("https://example.com/r/selfhosted/top.json").unwrap()
            )
            .is_none()
        );
    }

    #[test]
    fn reddit_saved_sources_preserve_listing_options_in_json_requests() {
        let source =
            Url::parse("https://reddit.com/r/selfhosted/top.json?limit=25&raw_json=1&t=week")
                .unwrap();
        let listing = reddit_listing_source(&source).expect("listing is recognized");
        let normalized = listing
            .json_url("https://www.reddit.com")
            .expect("listing URL builds");

        assert_eq!(
            normalized.as_str(),
            "https://www.reddit.com/r/selfhosted/top.json?limit=25&raw_json=1&t=week"
        );
    }

    #[test]
    fn reddit_browser_challenge_and_cookie_are_parsed() {
        let body = r#"
          <script>await(async value => value + value)("challenge-123")</script>
          <input type="hidden" name="token" value="token-456">
        "#;
        assert_eq!(
            parse_reddit_challenge(body),
            Some(("challenge-123".to_owned(), "token-456".to_owned()))
        );

        let mut headers = primp::header::HeaderMap::new();
        headers.append(
            "set-cookie",
            primp::header::HeaderValue::from_static("session=ignored; Path=/"),
        );
        headers.append(
            "set-cookie",
            primp::header::HeaderValue::from_static(
                "loid=0000000000000example.2.1234567890; Domain=reddit.com; Path=/",
            ),
        );
        assert_eq!(
            reddit_loid_from_headers(&headers),
            Some("0000000000000example.2.1234567890".to_owned())
        );
    }

    #[test]
    fn reddit_json_entries_keep_article_and_comments_destinations_separate() {
        let payload: RedditListingPayload = serde_json::from_value(json!({
            "data": {
                "children": [{
                    "data": {
                        "id": "example",
                        "name": "t3_example",
                        "title": "Pandan &amp; Reddit",
                        "url": "https://example.com/article",
                        "permalink": "/r/selfhosted/comments/example/pandan_reddit/",
                        "selftext": "A short summary",
                        "is_self": false,
                        "created_utc": 1788163200.0,
                        "ups": 42,
                        "num_comments": 7
                    }
                }]
            }
        }))
        .expect("Reddit fixture parses");
        let source = RedditListingSource::new("selfhosted".to_owned(), "new", 25, None);
        let snapshot = reddit_listing_snapshot(&source, &payload);

        assert_eq!(snapshot.title, "r/selfhosted");
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].external_id, "t3_example");
        assert_eq!(snapshot.items[0].title, "Pandan & Reddit");
        assert_eq!(snapshot.items[0].url, "https://example.com/article");
        assert_eq!(
            snapshot.items[0].comments_url,
            "https://www.reddit.com/r/selfhosted/comments/example/pandan_reddit/"
        );
    }

    #[tokio::test]
    async fn rss_fetch_revalidates_and_follows_relative_redirects() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let pool = db::connect("sqlite::memory:")
            .await
            .expect("test database connects");
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("test server binds");
        let address = listener.local_addr().expect("test address resolves");
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO network_access_rules (\
             id, action, scheme, host, port, integration, created_at, updated_at\
             ) VALUES ('allow-rss-redirect-test', 'allow', 'http', '127.0.0.1', ?, 'rss', ?, ?)",
        )
        .bind(i64::from(address.port()))
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("RSS test allow rule inserts");

        let server = tokio::spawn(async move {
            for redirected in [false, true] {
                let (mut socket, _) = listener.accept().await.expect("request accepts");
                let mut request = [0_u8; 2048];
                let request_len = socket.read(&mut request).await.expect("request reads");
                let request = String::from_utf8_lossy(&request[..request_len]);
                if redirected {
                    assert!(request.starts_with("GET /rss/ HTTP/1.1"));
                    let body = r#"<?xml version="1.0" encoding="UTF-8"?>
                      <rss version="2.0"><channel><title>Redirected feed</title>
                      <link>http://example.test/</link><description>Example</description>
                      <item><guid>entry-1</guid><title>Redirected entry</title>
                      <link>http://example.test/entry</link>
                      <pubDate>Mon, 31 Aug 2026 08:00:00 GMT</pubDate></item>
                      </channel></rss>"#;
                    let headers = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    socket
                        .write_all(headers.as_bytes())
                        .await
                        .expect("response headers write");
                    socket
                        .write_all(body.as_bytes())
                        .await
                        .expect("response body writes");
                } else {
                    assert!(request.starts_with("GET /rss HTTP/1.1"));
                    socket
                        .write_all(
                            b"HTTP/1.1 301 Moved Permanently\r\nLocation: /rss/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .await
                        .expect("redirect writes");
                }
            }
        });

        let snapshot = WidgetIntegrationService::for_tests(pool)
            .expect("service initializes")
            .fetch_rss_feed(&format!("http://{address}/rss"))
            .await
            .expect("redirected RSS parses");
        server.await.expect("test server completes");

        assert_eq!(snapshot.title, "Redirected feed");
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].title, "Redirected entry");
    }

    #[test]
    fn rss_reader_preserves_rss_comments_destinations() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
          <rss version="2.0">
            <channel>
              <title>Example feed</title>
              <link>https://example.com/</link>
              <description>Example entries</description>
              <item>
                <guid>entry-1</guid>
                <title>Article with discussion</title>
                <link>https://example.com/article</link>
                <comments>https://example.com/article/comments?sort=top&amp;view=all</comments>
                <pubDate>Thu, 20 Aug 2026 05:00:00 +0000</pubDate>
              </item>
            </channel>
          </rss>"#;

        let snapshot = parse_rss_feed_snapshot(xml).expect("RSS feed parses");

        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].url, "https://example.com/article");
        assert_eq!(
            snapshot.items[0].comments_url,
            "https://example.com/article/comments?sort=top&view=all"
        );
    }

    #[test]
    fn rss_reader_uses_atom_replies_links_for_comments() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
          <feed xmlns="http://www.w3.org/2005/Atom">
            <title>Example Atom feed</title>
            <id>https://example.com/feed</id>
            <updated>2026-08-20T05:00:00Z</updated>
            <entry>
              <id>entry-2</id>
              <title>Atom article</title>
              <updated>2026-08-20T05:00:00Z</updated>
              <link rel="replies" href="https://example.com/article/comments" />
              <link rel="alternate" href="https://example.com/article" />
            </entry>
          </feed>"#;

        let snapshot = parse_rss_feed_snapshot(xml).expect("Atom feed parses");

        assert_eq!(snapshot.items[0].url, "https://example.com/article");
        assert_eq!(
            snapshot.items[0].comments_url,
            "https://example.com/article/comments"
        );
    }

    #[test]
    fn reddit_atom_entries_treat_the_thread_as_the_comments_destination() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
          <feed xmlns="http://www.w3.org/2005/Atom">
            <title>r/selfhosted</title>
            <id>https://www.reddit.com/r/selfhosted/.rss</id>
            <updated>2026-08-20T05:00:00Z</updated>
            <entry>
              <id>t3_example</id>
              <title>A self-hosted project</title>
              <updated>2026-08-20T05:00:00Z</updated>
              <link rel="alternate" href="https://www.reddit.com/r/selfhosted/comments/example/a_selfhosted_project/" />
            </entry>
          </feed>"#;

        let snapshot = parse_rss_feed_snapshot(xml).expect("Reddit Atom feed parses");

        assert_eq!(snapshot.items[0].comments_url, snapshot.items[0].url);
    }

    #[test]
    fn youtube_atom_entries_are_parsed() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom" xmlns:media="http://search.yahoo.com/mrss/">
          <author><name>Example channel</name></author>
          <entry>
            <title>A real upload</title>
            <published>2026-08-13T12:00:00+00:00</published>
            <link rel="alternate" href="https://www.youtube.com/watch?v=abc123"/>
            <media:group><media:thumbnail url="https://i.ytimg.com/vi/abc123/hqdefault.jpg"/></media:group>
          </entry>
        </feed>"#;
        let items = parse_youtube_feed(xml).expect("feed parses");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["source"], "Example channel");
        assert_eq!(items[0]["title"], "A real upload");
        assert_eq!(items[0]["url"], "https://www.youtube.com/watch?v=abc123");
    }

    #[test]
    fn carddav_multistatus_imports_vcard_fields() {
        let xml = br#"<?xml version="1.0"?>
        <d:multistatus xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
          <d:response><d:propstat><d:prop><card:address-data>BEGIN:VCARD
VERSION:3.0
UID:remote-42
N:Rivera;Mara;;;
FN:Mara Rivera
EMAIL;TYPE=WORK:mara@example.com
TEL;TYPE=CELL:+1-555-0199
ORG:Northstar Studio
TITLE:Producer
BDAY:1990-04-13
CATEGORIES:friend,film
PHOTO;TYPE=PNG:iVBORw0KGgo=
NOTE:Met through the neighborhood cinema.
END:VCARD</card:address-data></d:prop></d:propstat></d:response>
        </d:multistatus>"#;
        let contacts = parse_carddav_response("source-1", xml).expect("CardDAV response parses");

        assert_eq!(contacts.len(), 1);
        let contact = &contacts[0];
        assert_eq!(
            contact.source_reference.as_deref(),
            Some("source-1:remote-42")
        );
        assert_eq!(contact.first_name, "Mara");
        assert_eq!(contact.last_name, "Rivera");
        assert_eq!(contact.emails[0].value, "mara@example.com");
        assert_eq!(contact.phones[0].label, "cell");
        assert_eq!(contact.company, "Northstar Studio");
        assert_eq!(contact.birthday.as_deref(), Some("1990-04-13"));
        assert_eq!(contact.tags, ["friend", "film"]);
        assert_eq!(
            contact.photo.as_ref().map(|photo| photo.mime_type.as_str()),
            Some("image/png")
        );
    }

    #[test]
    fn carddav_preserves_a_birthday_without_a_year() {
        let contact = parse_vcard(
            "source-1",
            "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:remote-43\r\nFN:Sean Choi\r\nN:Choi;Sean;;;\r\nBDAY:--08-20\r\nEND:VCARD\r\n",
        )
        .expect("yearless birthday vCard parses");

        assert_eq!(contact.birthday.as_deref(), Some("--08-20"));
    }

    #[test]
    fn oversized_widget_configuration_is_rejected() {
        let config = json!({ "source": "x".repeat(33_000) });
        assert!(validate_widget_config("html", &config).is_err());
    }

    #[test]
    fn ntfy_stream_keeps_messages_and_preserves_actions() {
        let body = r#"{"id":"open","time":1,"event":"open","topic":"alerts"}
{"id":"message-1","time":2,"event":"message","topic":"alerts","title":"Door","message":"Opened","priority":4,"tags":["house"],"click":"https://example.com/event","actions":[{"action":"view","label":"Camera","url":"https://example.com/camera","clear":true}]}"#;

        let messages = parse_ntfy_messages(body).expect("ntfy stream parses");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "message-1");
        assert_eq!(messages[0].priority, 4);
        assert_eq!(messages[0].actions[0].action, "view");
        assert!(messages[0].actions[0].clear);
    }

    #[test]
    fn ntfy_poll_endpoint_batches_topics_and_preserves_a_base_path() {
        let endpoint = ntfy_poll_endpoint(
            "https://ntfy.example.com/gateway",
            &["home-alerts", "deploys"],
            Some("message-12"),
        )
        .expect("ntfy endpoint builds");

        assert_eq!(endpoint.path(), "/gateway/home-alerts,deploys/json");
        assert_eq!(
            endpoint
                .query_pairs()
                .collect::<HashMap<_, _>>()
                .get("since")
                .map(|value| value.as_ref()),
            Some("message-12")
        );
    }

    #[test]
    fn ntfy_realtime_endpoint_preserves_path_and_uses_the_requested_replay_window() {
        let endpoint = ntfy_stream_endpoint(
            "https://ntfy.example.com/gateway",
            &["home-alerts", "deploys"],
            "45s",
        )
        .expect("ntfy stream endpoint builds");

        assert_eq!(endpoint.path(), "/gateway/home-alerts,deploys/json");
        let query = endpoint.query_pairs().collect::<HashMap<_, _>>();
        assert_eq!(query.get("since").map(|value| value.as_ref()), Some("45s"));
        assert!(!query.contains_key("poll"));
    }

    #[test]
    fn ntfy_status_errors_distinguish_credentials_and_topic_access() {
        assert_eq!(
            ntfy_status_error(StatusCode::UNAUTHORIZED, true),
            "ntfy access token was rejected"
        );
        assert_eq!(
            ntfy_status_error(StatusCode::UNAUTHORIZED, false),
            "this ntfy server requires an access token"
        );
        assert_eq!(
            ntfy_status_error(StatusCode::FORBIDDEN, true),
            "ntfy access token cannot read one or more subscribed topics"
        );
        assert_eq!(
            ntfy_status_error(StatusCode::TOO_MANY_REQUESTS, true),
            "ntfy server rate limit reached; reconnecting automatically"
        );
    }

    #[test]
    fn ntfy_log_context_excludes_topic_and_base_paths() {
        let endpoint = Url::parse("https://ntfy.example.com:8443/gateway/private-topic/json")
            .expect("ntfy endpoint parses");
        let context =
            NtfyRequestLogContext::new("account-1", "realtime_stream", &endpoint, 1, true);

        assert_eq!(context.origin, "https://ntfy.example.com:8443");
        assert!(!context.origin.contains("gateway"));
        assert!(!context.origin.contains("private-topic"));
    }

    #[test]
    fn ntfy_response_logs_capture_selected_proxy_rate_limit_headers() {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::RETRY_AFTER, header::HeaderValue::from_static("30"));
        headers.insert("x-ratelimit-limit", header::HeaderValue::from_static("20"));
        headers.insert(
            "x-ratelimit-remaining",
            header::HeaderValue::from_static("0"),
        );
        headers.insert(
            "x-ratelimit-reset",
            header::HeaderValue::from_static("1787274000"),
        );
        headers.insert(
            header::SERVER,
            header::HeaderValue::from_static("reverse-proxy"),
        );
        headers.insert(header::VIA, header::HeaderValue::from_static("1.1 gateway"));
        headers.insert("cf-ray", header::HeaderValue::from_static("trace-123"));
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_static("Bearer never-log-this"),
        );

        let metadata = ntfy_response_log_metadata(&headers);

        assert_eq!(metadata.retry_after.as_deref(), Some("30"));
        assert_eq!(metadata.rate_limit_limit.as_deref(), Some("20"));
        assert_eq!(metadata.rate_limit_remaining.as_deref(), Some("0"));
        assert_eq!(metadata.rate_limit_reset.as_deref(), Some("1787274000"));
        assert_eq!(metadata.server.as_deref(), Some("reverse-proxy"));
        assert_eq!(metadata.via.as_deref(), Some("1.1 gateway"));
        assert_eq!(metadata.request_id.as_deref(), Some("trace-123"));
        assert!(!format!("{metadata:?}").contains("never-log-this"));
    }

    #[test]
    fn release_repositories_support_glance_style_providers() {
        let implicit_github = parse_release_repository("glanceapp/glance").expect("GitHub parses");
        assert_eq!(implicit_github.provider, "github");
        assert_eq!(implicit_github.host, "github.com");
        assert_eq!(implicit_github.repository, "glanceapp/glance");

        let gitlab = parse_release_repository("gitlab:gitlab-org/gitlab").expect("GitLab parses");
        assert_eq!(gitlab.provider, "gitlab");
        assert_eq!(gitlab.host, "gitlab.com");

        let forgejo =
            parse_release_repository("forgejo@code.example:team/service").expect("Forgejo parses");
        assert_eq!(forgejo.provider, "forgejo");
        assert_eq!(forgejo.host, "code.example");
        assert_eq!(forgejo.repository, "team/service");

        assert!(parse_release_repository("gitlab:not-a-project").is_err());
        assert!(parse_release_repository("gitea@-invalid.example:team/service").is_err());
    }

    #[test]
    fn owned_repositories_validate_paths_and_sort_open_work_first() {
        assert!(owned_repository("gitlab", "gitlab.com", "../escape", false, Some(2)).is_none());
        assert!(
            owned_repository(
                "gitlab",
                "gitlab.com",
                "group/nested/service",
                false,
                Some(2)
            )
            .is_some()
        );

        let mut repositories = vec![
            owned_repository("github", "github.com", "owner/quiet", false, Some(0)).unwrap(),
            owned_repository("github", "github.com", "owner/busy", false, Some(7)).unwrap(),
            owned_repository("github", "github.com", "owner/unknown", true, None).unwrap(),
        ];
        sort_owned_repositories(&mut repositories);

        assert_eq!(repositories[0].repository, "owner/busy");
        assert_eq!(repositories[1].repository, "owner/quiet");
        assert_eq!(repositories[2].repository, "owner/unknown");
        assert_eq!(repositories[0].url, "https://github.com/owner/busy");
        assert!(repositories[2].archived);
    }
}
