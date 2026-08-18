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
use quick_xml::{Reader, de::from_str, events::Event};
use reqwest::{Client, Method, Url, header};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{net::lookup_host, sync::RwLock};

const CACHE_DURATION: Duration = Duration::from_secs(15 * 60);
const MAX_RESPONSE_BYTES: usize = 2_000_000;
const MAX_YOUTUBE_CHANNEL_METADATA_BYTES: usize = 1_000_000;
const MAX_OWNED_REPOSITORIES: usize = 500;
const MAX_PROVIDER_PAGES: usize = 100;
const PROVIDER_REQUEST_CONCURRENCY: usize = 8;

#[derive(Clone)]
pub struct WidgetIntegrationService {
    client: Client,
    cipher: Option<XChaCha20Poly1305>,
    invidious_base_url: Option<Url>,
    cache: Arc<RwLock<HashMap<String, CachedData>>>,
}

struct CachedData {
    stored_at: Instant,
    value: Value,
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
    pub title: String,
    pub summary: String,
    pub published_at: String,
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

impl WidgetIntegrationService {
    /// Builds the provider client and optional credential cipher from environment configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when the encryption key or HTTP client configuration is invalid.
    pub fn from_env() -> Result<Self, String> {
        let key = std::env::var("PANDAN_SECRET_KEY").ok();
        let invidious_base_url = std::env::var("INVIDIOUS_BASE_URL").ok();
        Self::new_with_invidious(key.as_deref(), invidious_base_url.as_deref())
    }

    #[cfg(test)]
    /// Builds an integration service without encrypted credential storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the test HTTP client cannot be initialized.
    pub fn for_tests() -> Result<Self, String> {
        Self::new(None)
    }

    fn new(encoded_key: Option<&str>) -> Result<Self, String> {
        Self::new_with_invidious(encoded_key, None)
    }

    fn new_with_invidious(
        encoded_key: Option<&str>,
        invidious_base_url: Option<&str>,
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
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Pandan/0.1 widget fetcher")
            .build()
            .map_err(|error| format!("widget HTTP client failed: {error}"))?;
        Ok(Self {
            client,
            cipher,
            invidious_base_url: parse_invidious_base_url(invidious_base_url)?,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    #[must_use]
    pub fn secrets_enabled(&self) -> bool {
        self.cipher.is_some()
    }

    #[must_use]
    pub fn invidious_enabled(&self) -> bool {
        self.invidious_base_url.is_some()
    }

    /// Fetches and parses one public HTTPS RSS, Atom, or recognized Reddit source for the reader.
    ///
    /// The same DNS and response-size protections used by RSS widgets are applied here.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when URL validation, fetching, or feed parsing fails.
    pub async fn fetch_rss_feed(&self, source: &str) -> Result<RssFeedSnapshot, String> {
        let url = validate_public_https_url(source).await?;
        if let Some((subreddit, sort)) = reddit_listing_source(&url) {
            let response = self
                .client
                .get(url.clone())
                .send()
                .await
                .map_err(request_error)?;
            let bytes = response_bytes(response).await?;
            let payload: Value = serde_json::from_slice(&bytes)
                .map_err(|_| "Reddit returned an invalid listing".to_owned())?;
            return parse_reddit_feed_snapshot(&url, &subreddit, sort, &payload);
        }
        let response = self.client.get(url).send().await.map_err(request_error)?;
        let bytes = response_bytes(response).await?;
        let feed = feed_rs::parser::parse(&bytes[..])
            .map_err(|error| format!("feed could not be parsed: {error}"))?;
        let title = feed
            .title
            .as_ref()
            .map_or_else(|| "Untitled feed".to_owned(), |value| value.content.clone());
        let fetched_at = chrono::Utc::now().to_rfc3339();
        let items = feed
            .entries
            .into_iter()
            .take(200)
            .map(|entry| {
                let url = entry
                    .links
                    .first()
                    .map_or_else(String::new, |link| link.href.clone());
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
                    title,
                    summary: content,
                    published_at,
                }
            })
            .collect();
        Ok(RssFeedSnapshot { title, items })
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
        let endpoint = validate_public_https_url(endpoint.as_str()).await?;
        let response = self
            .client
            .get(endpoint)
            .send()
            .await
            .map_err(request_error)?;
        let text = response_text(response).await?;
        parse_invidious_snapshot(base_url, channel_id, &text)
    }

    async fn fetch_youtube_atom_channel(
        &self,
        channel_id: &str,
    ) -> Result<YoutubeFeedSnapshot, String> {
        let uploads_playlist = channel_id.replacen("UC", "UULF", 1);
        let response = self
            .client
            .get(format!(
                "https://www.youtube.com/feeds/videos.xml?playlist_id={uploads_playlist}"
            ))
            .send()
            .await
            .map_err(request_error)?;
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
        let response = self
            .client
            .get(format!("https://www.youtube.com/channel/{channel_id}"))
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

    /// Fetches one public HTTPS iCalendar document using the shared SSRF and size guards.
    pub async fn fetch_calendar_file(&self, source: &str) -> Result<Vec<u8>, String> {
        let url = validate_public_https_url(source).await?;
        let response = self.client.get(url).send().await.map_err(request_error)?;
        response_bytes(response).await
    }

    /// Fetches one bounded public channel portrait for the persistent 24-hour cache.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when the URL, media type, or response is invalid.
    pub async fn fetch_public_image(&self, source: &str) -> Result<(String, Vec<u8>), String> {
        self.fetch_bounded_public_image(source, MAX_RESPONSE_BYTES)
            .await
    }

    /// Fetches one public HTTPS image with a caller-supplied response limit.
    ///
    /// # Errors
    ///
    /// Returns a safe provider error when the URL, media type, or response is invalid.
    pub async fn fetch_bounded_public_image(
        &self,
        source: &str,
        max_bytes: usize,
    ) -> Result<(String, Vec<u8>), String> {
        let url = validate_public_https_url(source).await?;
        let response = self.client.get(url).send().await.map_err(request_error)?;
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
    pub async fn validate_public_https_source(&self, source: &str) -> Result<(), String> {
        validate_public_https_url(source).await.map(|_| ())
    }

    /// Pulls vCards from one public HTTPS CardDAV address-book resource.
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
        let url = validate_public_https_url(source).await?;
        let password = encrypted_password
            .map(|value| self.decrypt_secret(value))
            .transpose()?;
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<card:addressbook-query xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
  <d:prop><d:getetag/><card:address-data/></d:prop>
</card:addressbook-query>"#;
        let method = Method::from_bytes(b"REPORT")
            .map_err(|_| "CardDAV request method is invalid".to_owned())?;
        let mut request = self
            .client
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
        let payload = fetch_release(&self.client, &source, secret.as_deref()).await?;
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
        let url = validate_public_https_url(&format!(
            "https://{host}/api/v4/merge_requests?scope=created_by_me&state=opened&order_by=updated_at&sort=desc&per_page=20"
        ))
        .await?;
        let payload: Vec<Value> = self
            .client
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
            "github" => fetch_github_owned_repositories(&self.client, host, &token).await,
            "gitlab" => fetch_gitlab_owned_repositories(&self.client, host, &token).await,
            "codeberg" | "gitea" | "forgejo" => {
                fetch_forge_owned_repositories(&self.client, provider, host, &token).await
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
        let url = validate_public_https_url(&format!(
            "https://{}/api/v4/projects/{encoded_repository}/pipelines?per_page=1&order_by=id&sort=desc",
            project.host
        ))
        .await?;
        let payload: Vec<Value> = self
            .client
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

    fn decrypt_secret(&self, encoded: &str) -> Result<String, String> {
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
            "rss" => self.fetch_rss(&widget.config).await?,
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
            let client = self.client.clone();
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

    async fn fetch_rss(&self, config: &Value) -> Result<Value, String> {
        let urls = config_strings(config, "urls", 8);
        if urls.is_empty() {
            return Err("add at least one RSS or Atom feed URL".to_owned());
        }
        let requests = urls.into_iter().map(|source| {
            let client = self.client.clone();
            async move {
                let url = validate_public_https_url(&source).await?;
                let response = client.get(url).send().await.map_err(request_error)?;
                let bytes = response_bytes(response).await?;
                let feed = feed_rs::parser::parse(&bytes[..])
                    .map_err(|error| format!("feed could not be parsed: {error}"))?;
                let source_title = feed
                    .title
                    .as_ref()
                    .map_or_else(|| "Feed".to_owned(), |value| value.content.clone());
                Ok::<_, String>(
                    feed.entries
                        .into_iter()
                        .map(|entry| rss_entry(&source_title, entry))
                        .collect::<Vec<_>>(),
                )
            }
        });
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        for mut entries in join_all(requests).await.into_iter().flatten() {
            entries.retain(|entry| {
                entry["url"]
                    .as_str()
                    .is_some_and(|url| seen.insert(url.to_owned()))
            });
            items.append(&mut entries);
        }
        items.sort_by_key(|item| Reverse(value_string(item, "published_at")));
        items.truncate(config_limit(config, 20));
        if items.is_empty() {
            return Err("the configured feeds returned no readable items".to_owned());
        }
        Ok(json!({ "items": items }))
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
        let limit = config_limit(config, 15);
        let mut request =
            if let (Some(client_id), Some(secret)) = (config_string(config, "client_id"), secret) {
                let token: Value = self
                    .client
                    .post("https://www.reddit.com/api/v1/access_token")
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
                self.client
                    .get(format!("https://oauth.reddit.com/r/{subreddit}/{sort}"))
                    .bearer_auth(access)
            } else {
                self.client
                    .get(format!("https://www.reddit.com/r/{subreddit}/{sort}.json"))
            };
        request = request.query(&[("limit", limit.to_string()), ("raw_json", "1".to_owned())]);
        let payload: Value = request
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        let items = payload["data"]["children"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|child| child.get("data"))
            .map(|post| {
                json!({
                    "title": post["title"],
                    "url": post["url"],
                    "comments_url": format!("https://www.reddit.com{}", post["permalink"].as_str().unwrap_or("")),
                    "score": post["ups"],
                    "comments": post["num_comments"],
                    "published_at": post["created_utc"],
                    "source": format!("r/{subreddit}")
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "items": items }))
    }

    async fn fetch_stocks(&self, config: &Value) -> Result<Value, String> {
        let symbols = config_strings(config, "symbols", 12);
        if symbols.is_empty() {
            return Err("add at least one market symbol".to_owned());
        }
        let payload: Value = self
            .client
            .get("https://query1.finance.yahoo.com/v7/finance/quote")
            .query(&[("symbols", symbols.join(","))])
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        let items = payload["quoteResponse"]["result"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|quote| {
                json!({
                    "title": quote["shortName"].as_str().unwrap_or_else(|| quote["symbol"].as_str().unwrap_or("Symbol")),
                    "symbol": quote["symbol"],
                    "value": quote["regularMarketPrice"],
                    "change": quote["regularMarketChangePercent"],
                    "currency": quote["currency"],
                    "url": format!("https://finance.yahoo.com/quote/{}", quote["symbol"].as_str().unwrap_or(""))
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "items": items }))
    }

    async fn fetch_releases(&self, config: &Value, secret: Option<&str>) -> Result<Value, String> {
        let repositories = config_strings(config, "repositories", 12);
        if repositories.is_empty() {
            return Err("add at least one provider:owner/repository entry".to_owned());
        }
        let requests = repositories.into_iter().map(|repository| {
            let client = self.client.clone();
            let token = secret.map(str::to_owned);
            async move { fetch_release(&client, &repository, token.as_deref()).await }
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
        let platform = config_string(config, "platform").unwrap_or_else(|| "twitch".to_owned());
        let channels = config_strings(config, "channels", 20);
        if channels.is_empty() {
            return Err("add at least one channel".to_owned());
        }
        if platform == "kick" {
            return self.fetch_kick_streams(channels).await;
        }
        self.fetch_twitch_streams(channels, config, secret).await
    }

    async fn fetch_kick_streams(&self, channels: Vec<String>) -> Result<Value, String> {
        let requests = channels.into_iter().map(|channel| {
            let client = self.client.clone();
            async move {
                let payload: Value = client
                    .get(format!("https://kick.com/api/v2/channels/{channel}"))
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
        let token: Value = self
            .client
            .post("https://id.twitch.tv/oauth2/token")
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
        let payload: Value = self
            .client
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
        "streams" => validate_array(config, "channels", 20),
        _ => Ok(()),
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

fn rss_entry(source: &str, entry: Entry) -> Value {
    let url = entry.links.first().map_or("", |link| link.href.as_str());
    let published = entry
        .published
        .or(entry.updated)
        .map(|date| date.to_rfc3339())
        .unwrap_or_default();
    json!({
        "title": entry.title.map_or_else(|| "Untitled".to_owned(), |title| title.content),
        "url": url,
        "source": source,
        "summary": entry.summary.map(|summary| summary.content),
        "published_at": published
    })
}

async fn fetch_github_owned_repositories(
    client: &Client,
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
        let response = client
            .post(endpoint.clone())
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
    client: &Client,
    host: &str,
    token: &str,
) -> Result<CodingOwnedRepositories, String> {
    let mut page = 1_u64;
    let mut projects = Vec::new();
    for _ in 0..MAX_PROVIDER_PAGES {
        let url = validate_public_https_url(&format!(
            "https://{host}/api/v4/projects?owned=true&simple=true&order_by=path&sort=asc&per_page=100&page={page}"
        ))
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
                fetch_gitlab_open_merge_request_count(client, host, token, project_id).await;
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
    client: &Client,
    host: &str,
    token: &str,
    project_id: i64,
) -> Result<u64, String> {
    let url = validate_public_https_url(&format!(
        "https://{host}/api/v4/projects/{project_id}/merge_requests?state=opened&per_page=100&page=1"
    ))
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
    client: &Client,
    provider: &str,
    host: &str,
    token: &str,
) -> Result<CodingOwnedRepositories, String> {
    let profile_url = validate_public_https_url(&format!("https://{host}/api/v1/user")).await?;
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
        let url = validate_public_https_url(&format!(
            "https://{host}/api/v1/user/repos?limit=50&page={page}"
        ))
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
    client: &Client,
    repository: &str,
    token: Option<&str>,
) -> Result<Value, String> {
    let parsed = parse_release_repository(repository)?;
    let provider = parsed.provider.as_str();
    let repository = parsed.repository.as_str();
    let custom_host = matches!(provider, "gitea" | "forgejo").then_some(parsed.host.as_str());
    let url = if let Some(host) = custom_host {
        validate_public_https_url(&format!(
            "https://{host}/api/v1/repos/{repository}/releases/latest"
        ))
        .await?
    } else {
        Url::parse(&match provider {
            "github" => format!("https://api.github.com/repos/{repository}/releases/latest"),
            "gitlab" => format!(
                "https://gitlab.com/api/v4/projects/{}/releases/permalink/latest",
                repository.replace('/', "%2F")
            ),
            "codeberg" => format!("https://codeberg.org/api/v1/repos/{repository}/releases/latest"),
            _ => return Err("release provider is unsupported".to_owned()),
        })
        .map_err(|_| "release provider URL is invalid".to_owned())?
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

fn reddit_listing_source(url: &Url) -> Option<(String, &'static str)> {
    if !matches!(url.host_str(), Some(host) if host.eq_ignore_ascii_case("reddit.com") || host.eq_ignore_ascii_case("www.reddit.com"))
    {
        return None;
    }
    let segments = url.path_segments()?.collect::<Vec<_>>();
    let [root, subreddit, listing] = segments.as_slice() else {
        return None;
    };
    if !root.eq_ignore_ascii_case("r") || !valid_slug(subreddit) {
        return None;
    }
    let sort = match listing.strip_suffix(".json")? {
        "hot" => "hot",
        "new" => "new",
        "top" => "top",
        "rising" => "rising",
        _ => return None,
    };
    Some(((*subreddit).to_owned(), sort))
}

fn parse_reddit_feed_snapshot(
    source: &Url,
    subreddit: &str,
    sort: &str,
    payload: &Value,
) -> Result<RssFeedSnapshot, String> {
    let fetched_at = chrono::Utc::now().to_rfc3339();
    let children = payload["data"]["children"]
        .as_array()
        .ok_or_else(|| "Reddit returned an invalid listing".to_owned())?;
    let items = children
        .iter()
        .filter_map(|child| child.get("data"))
        .filter(|post| {
            !post["stickied"].as_bool().unwrap_or(false)
                && !post["pinned"].as_bool().unwrap_or(false)
        })
        .take(200)
        .map(|post| {
            let title = post["title"].as_str().unwrap_or("Untitled").to_owned();
            let permalink = post["permalink"].as_str().unwrap_or_default();
            let comments_url = format!("https://www.reddit.com{permalink}");
            let url = post["url"]
                .as_str()
                .filter(|value| {
                    Url::parse(value).is_ok_and(|url| {
                        matches!(url.scheme(), "http" | "https")
                            && url.username().is_empty()
                            && url.password().is_none()
                    })
                })
                .unwrap_or(&comments_url)
                .to_owned();
            let published_at = post["created_utc"]
                .as_f64()
                .and_then(|timestamp| {
                    let seconds = timestamp.trunc() as i64;
                    let nanos = (timestamp.fract().abs() * 1_000_000_000.0) as u32;
                    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, nanos)
                })
                .map_or_else(|| fetched_at.clone(), |date| date.to_rfc3339());
            let external_id = post["id"]
                .as_str()
                .filter(|value| !value.is_empty())
                .map_or_else(
                    || format!("{title}:{published_at}"),
                    |value| value.to_owned(),
                );
            RssFeedEntry {
                external_id,
                url,
                title,
                summary: post["selftext"].as_str().unwrap_or_default().to_owned(),
                published_at,
            }
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Err("Reddit returned no readable posts".to_owned());
    }
    let period = source
        .query_pairs()
        .find_map(|(key, value)| (key == "t").then_some(value.into_owned()))
        .filter(|value| {
            matches!(
                value.as_str(),
                "hour" | "day" | "week" | "month" | "year" | "all"
            )
        });
    let sort_label = match (sort, period.as_deref()) {
        ("top", Some(period)) => format!("Top · {period}"),
        ("new", _) => "New".to_owned(),
        ("rising", _) => "Rising".to_owned(),
        ("top", _) => "Top".to_owned(),
        _ => "Hot".to_owned(),
    };
    Ok(RssFeedSnapshot {
        title: format!("r/{subreddit} · {sort_label}"),
        items,
    })
}

async fn validate_public_https_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "URL is invalid".to_owned())?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("only credential-free HTTPS URLs are allowed".to_owned());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "URL host is missing".to_owned())?;
    if host.eq_ignore_ascii_case("localhost")
        || host
            .rsplit('.')
            .next()
            .is_some_and(|suffix| suffix.eq_ignore_ascii_case("local"))
    {
        return Err("local network URLs are not allowed".to_owned());
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = lookup_host((host, port))
        .await
        .map_err(|_| "URL host could not be resolved".to_owned())?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| !public_ip(address.ip())) {
        return Err("private or reserved network URLs are not allowed".to_owned());
    }
    Ok(url)
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, ..] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || a == 0
                || a >= 224
                || (a == 100 && (64..=127).contains(&b)))
        }
        IpAddr::V6(ip) => {
            let first = ip.segments()[0];
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80)
        }
    }
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

fn request_error(error: reqwest::Error) -> String {
    let timed_out = error.is_timeout();
    drop(error);
    if timed_out {
        "provider request timed out".to_owned()
    } else {
        "provider request failed".to_owned()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
            Some(("selfhosted".to_owned(), "top"))
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
    fn reddit_listings_become_reader_items_and_skip_pinned_posts() {
        let source = Url::parse("https://www.reddit.com/r/selfhosted/new.json?limit=25").unwrap();
        let payload = json!({
            "data": { "children": [
                { "data": {
                    "id": "post-1",
                    "title": "A useful project",
                    "url": "https://example.com/project",
                    "permalink": "/r/selfhosted/comments/post-1/a_useful_project/",
                    "created_utc": 1_786_622_400.0,
                    "selftext": "Project notes",
                    "stickied": false,
                    "pinned": false
                } },
                { "data": {
                    "id": "rules",
                    "title": "Community rules",
                    "permalink": "/r/selfhosted/comments/rules/community_rules/",
                    "created_utc": 1_786_622_300.0,
                    "stickied": true
                } }
            ] }
        });

        let snapshot = parse_reddit_feed_snapshot(&source, "selfhosted", "new", &payload)
            .expect("listing parses");

        assert_eq!(snapshot.title, "r/selfhosted · New");
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].external_id, "post-1");
        assert_eq!(snapshot.items[0].url, "https://example.com/project");
        assert_eq!(snapshot.items[0].summary, "Project notes");
        assert_eq!(snapshot.items[0].published_at, "2026-08-13T12:00:00+00:00");
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
        assert!(owned_repository("gitlab", "gitlab.com", "../escape", Some(2)).is_none());
        assert!(
            owned_repository("gitlab", "gitlab.com", "group/nested/service", Some(2)).is_some()
        );

        let mut repositories = vec![
            owned_repository("github", "github.com", "owner/quiet", Some(0)).unwrap(),
            owned_repository("github", "github.com", "owner/busy", Some(7)).unwrap(),
            owned_repository("github", "github.com", "owner/unknown", None).unwrap(),
        ];
        sort_owned_repositories(&mut repositories);

        assert_eq!(repositories[0].repository, "owner/busy");
        assert_eq!(repositories[1].repository, "owner/quiet");
        assert_eq!(repositories[2].repository, "owner/unknown");
        assert_eq!(repositories[0].url, "https://github.com/owner/busy");
    }
}
