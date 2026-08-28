use crate::network_policy::{NetworkAccessScope, NetworkPolicy};
use futures_util::StreamExt;
use reqwest::{
    Method, Response, StatusCode, Url,
    header::{
        AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, LOCATION,
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 12 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const CLIENT_NAME: &str = "Pandan";
const DEVICE_NAME: &str = "Pandan server";

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum JellyfinClientError {
    #[error("Jellyfin credentials are no longer accepted")]
    Unauthorized,
    #[error("Jellyfin record was not found")]
    NotFound,
    #[error("{0}")]
    Rejected(&'static str),
    #[error("{0}")]
    Unavailable(&'static str),
}

#[derive(Debug, Clone)]
pub struct JellyfinAuth {
    pub token: String,
    pub device_id: String,
}

#[derive(Debug, Clone)]
pub struct JellyfinClient {
    network_policy: NetworkPolicy,
    requests: Arc<Semaphore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PublicSystemInfo {
    pub id: String,
    pub server_name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinUser {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthenticationResult {
    pub user: JellyfinUser,
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnectResult {
    pub authenticated: bool,
    pub secret: String,
    pub code: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinUserData {
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub played: bool,
    #[serde(default)]
    pub playback_position_ticks: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinItem {
    pub id: String,
    #[serde(rename = "Type", default)]
    pub item_type: String,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub collection_type: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub album_id: Option<String>,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default)]
    pub run_time_ticks: Option<i64>,
    #[serde(default)]
    pub index_number: Option<i64>,
    #[serde(default)]
    pub parent_index_number: Option<i64>,
    #[serde(default)]
    pub production_year: Option<i64>,
    #[serde(default)]
    pub image_tags: HashMap<String, String>,
    #[serde(default)]
    pub album_primary_image_tag: Option<String>,
    #[serde(default)]
    pub user_data: JellyfinUserData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinItems {
    #[serde(default)]
    pub items: Vec<JellyfinItem>,
    #[serde(default)]
    pub total_record_count: i64,
    #[serde(default)]
    pub start_index: i64,
}

#[derive(Debug, Clone)]
pub struct ItemQuery {
    pub parent_id: Option<String>,
    pub include_item_types: String,
    pub media_types: Option<String>,
    pub search_term: Option<String>,
    pub start_index: usize,
    pub limit: usize,
    pub recursive: bool,
    pub sort_by: String,
    pub sort_order: String,
    pub ids: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackReport {
    pub item_id: String,
    pub play_session_id: String,
    pub position_ticks: i64,
    pub is_paused: bool,
    pub can_seek: bool,
    pub play_method: &'static str,
}

impl JellyfinClient {
    #[must_use]
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            network_policy: NetworkPolicy::new(pool),
            requests: Arc::new(Semaphore::new(12)),
        }
    }

    pub fn normalize_base_url(value: &str) -> Result<String, JellyfinClientError> {
        let mut url = Url::parse(value.trim())
            .map_err(|_| JellyfinClientError::Rejected("Jellyfin URL is invalid"))?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.host_str().is_none()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(JellyfinClientError::Rejected(
                "Jellyfin URL must be a credential-free HTTP or HTTPS base URL",
            ));
        }
        if !url.path().ends_with('/') {
            let path = format!("{}/", url.path());
            url.set_path(&path);
        }
        Ok(url.to_string())
    }

    pub async fn public_info(
        &self,
        base_url: &str,
        device_id: &str,
    ) -> Result<PublicSystemInfo, JellyfinClientError> {
        let url = endpoint(base_url, &["System", "Info", "Public"])?;
        self.json(Method::GET, url, None, device_id, None).await
    }

    pub async fn authenticate_password(
        &self,
        base_url: &str,
        username: &str,
        password: &str,
        device_id: &str,
    ) -> Result<AuthenticationResult, JellyfinClientError> {
        let url = endpoint(base_url, &["Users", "AuthenticateByName"])?;
        self.json(
            Method::POST,
            url,
            None,
            device_id,
            Some(
                serde_json::to_vec(&serde_json::json!({
                    "Username": username,
                    "Pw": password
                }))
                .map_err(|_| JellyfinClientError::Unavailable("Jellyfin request failed"))?,
            ),
        )
        .await
    }

    pub async fn initiate_quick_connect(
        &self,
        base_url: &str,
        device_id: &str,
    ) -> Result<QuickConnectResult, JellyfinClientError> {
        let url = endpoint(base_url, &["QuickConnect", "Initiate"])?;
        self.json(Method::POST, url, None, device_id, Some(Vec::new()))
            .await
    }

    pub async fn quick_connect_status(
        &self,
        base_url: &str,
        secret: &str,
        device_id: &str,
    ) -> Result<QuickConnectResult, JellyfinClientError> {
        let mut url = endpoint(base_url, &["QuickConnect", "Connect"])?;
        url.query_pairs_mut().append_pair("Secret", secret);
        self.json(Method::GET, url, None, device_id, None).await
    }

    pub async fn authenticate_quick_connect(
        &self,
        base_url: &str,
        secret: &str,
        device_id: &str,
    ) -> Result<AuthenticationResult, JellyfinClientError> {
        let url = endpoint(base_url, &["Users", "AuthenticateWithQuickConnect"])?;
        self.json(
            Method::POST,
            url,
            None,
            device_id,
            Some(
                serde_json::to_vec(&serde_json::json!({ "Secret": secret }))
                    .map_err(|_| JellyfinClientError::Unavailable("Jellyfin request failed"))?,
            ),
        )
        .await
    }

    pub async fn me(
        &self,
        base_url: &str,
        auth: &JellyfinAuth,
    ) -> Result<JellyfinUser, JellyfinClientError> {
        let url = endpoint(base_url, &["Users", "Me"])?;
        self.json(Method::GET, url, Some(auth), &auth.device_id, None)
            .await
    }

    pub async fn user_views(
        &self,
        base_url: &str,
        user_id: &str,
        auth: &JellyfinAuth,
    ) -> Result<JellyfinItems, JellyfinClientError> {
        let mut url = endpoint(base_url, &["UserViews"])?;
        url.query_pairs_mut().append_pair("UserId", user_id);
        self.json(Method::GET, url, Some(auth), &auth.device_id, None)
            .await
    }

    pub async fn items(
        &self,
        base_url: &str,
        user_id: &str,
        auth: &JellyfinAuth,
        query: &ItemQuery,
    ) -> Result<JellyfinItems, JellyfinClientError> {
        let mut url = endpoint(base_url, &["Items"])?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs
                .append_pair("UserId", user_id)
                .append_pair("IncludeItemTypes", &query.include_item_types)
                .append_pair("StartIndex", &query.start_index.to_string())
                .append_pair("Limit", &query.limit.min(200).to_string())
                .append_pair("Recursive", if query.recursive { "true" } else { "false" })
                .append_pair("SortBy", &query.sort_by)
                .append_pair("SortOrder", &query.sort_order)
                .append_pair(
                    "Fields",
                    "MediaType,CollectionType,ParentId,Album,AlbumId,Artists,RunTimeTicks,IndexNumber,ParentIndexNumber,ProductionYear,ImageTags,AlbumPrimaryImageTag",
                );
            if let Some(parent_id) = &query.parent_id {
                pairs.append_pair("ParentId", parent_id);
            }
            if let Some(media_types) = &query.media_types {
                pairs.append_pair("MediaTypes", media_types);
            }
            if let Some(search_term) = &query.search_term {
                pairs.append_pair("SearchTerm", search_term);
            }
            if let Some(ids) = &query.ids {
                pairs.append_pair("Ids", ids);
            }
        }
        self.json(Method::GET, url, Some(auth), &auth.device_id, None)
            .await
    }

    pub async fn item(
        &self,
        base_url: &str,
        user_id: &str,
        item_id: &str,
        auth: &JellyfinAuth,
    ) -> Result<JellyfinItem, JellyfinClientError> {
        let mut url = endpoint(base_url, &["Users", user_id, "Items", item_id])?;
        url.query_pairs_mut().append_pair(
            "Fields",
            "MediaType,CollectionType,ParentId,Album,AlbumId,Artists,RunTimeTicks,IndexNumber,ParentIndexNumber,ProductionYear,ImageTags,AlbumPrimaryImageTag",
        );
        self.json(Method::GET, url, Some(auth), &auth.device_id, None)
            .await
    }

    pub async fn ancestors(
        &self,
        base_url: &str,
        user_id: &str,
        item_id: &str,
        auth: &JellyfinAuth,
    ) -> Result<Vec<JellyfinItem>, JellyfinClientError> {
        let mut url = endpoint(base_url, &["Items", item_id, "Ancestors"])?;
        url.query_pairs_mut().append_pair("UserId", user_id);
        self.json(Method::GET, url, Some(auth), &auth.device_id, None)
            .await
    }

    pub async fn image(
        &self,
        base_url: &str,
        item_id: &str,
        tag: Option<&str>,
        auth: &JellyfinAuth,
    ) -> Result<(HeaderMap, Vec<u8>), JellyfinClientError> {
        let mut url = endpoint(base_url, &["Items", item_id, "Images", "Primary"])?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs
                .append_pair("MaxWidth", "960")
                .append_pair("Quality", "88");
            if let Some(tag) = tag {
                pairs.append_pair("Tag", tag);
            }
        }
        let response = self
            .send(
                Method::GET,
                url,
                Some(auth),
                &auth.device_id,
                None,
                HeaderMap::new(),
                true,
            )
            .await?;
        let headers = response.headers().clone();
        let bytes = read_bounded(response, MAX_IMAGE_BYTES).await?;
        Ok((headers, bytes))
    }

    pub async fn audio(
        &self,
        base_url: &str,
        user_id: &str,
        item_id: &str,
        auth: &JellyfinAuth,
        forwarded_headers: HeaderMap,
    ) -> Result<Response, JellyfinClientError> {
        let url = universal_audio_url(base_url, user_id, item_id, &auth.device_id)?;
        self.send(
            Method::GET,
            url,
            Some(auth),
            &auth.device_id,
            None,
            forwarded_headers,
            false,
        )
        .await
    }

    pub async fn report_playback(
        &self,
        base_url: &str,
        path: &[&str],
        auth: &JellyfinAuth,
        report: &PlaybackReport,
    ) -> Result<(), JellyfinClientError> {
        let url = endpoint(base_url, path)?;
        let body = serde_json::to_vec(report)
            .map_err(|_| JellyfinClientError::Unavailable("Jellyfin playback report failed"))?;
        self.empty(Method::POST, url, Some(auth), &auth.device_id, Some(body))
            .await
    }

    pub async fn logout(
        &self,
        base_url: &str,
        auth: &JellyfinAuth,
    ) -> Result<(), JellyfinClientError> {
        let url = endpoint(base_url, &["Sessions", "Logout"])?;
        self.empty(
            Method::POST,
            url,
            Some(auth),
            &auth.device_id,
            Some(Vec::new()),
        )
        .await
    }

    async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        auth: Option<&JellyfinAuth>,
        device_id: &str,
        body: Option<Vec<u8>>,
    ) -> Result<T, JellyfinClientError> {
        let _permit = self
            .requests
            .acquire()
            .await
            .map_err(|_| JellyfinClientError::Unavailable("Jellyfin is unavailable"))?;
        let response = self
            .send(method, url, auth, device_id, body, HeaderMap::new(), true)
            .await?;
        let bytes = read_bounded(response, MAX_JSON_BYTES).await?;
        serde_json::from_slice(&bytes)
            .map_err(|_| JellyfinClientError::Unavailable("Jellyfin returned invalid data"))
    }

    async fn empty(
        &self,
        method: Method,
        url: Url,
        auth: Option<&JellyfinAuth>,
        device_id: &str,
        body: Option<Vec<u8>>,
    ) -> Result<(), JellyfinClientError> {
        let _permit = self
            .requests
            .acquire()
            .await
            .map_err(|_| JellyfinClientError::Unavailable("Jellyfin is unavailable"))?;
        self.send(method, url, auth, device_id, body, HeaderMap::new(), true)
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn send(
        &self,
        method: Method,
        mut url: Url,
        auth: Option<&JellyfinAuth>,
        device_id: &str,
        body: Option<Vec<u8>>,
        forwarded_headers: HeaderMap,
        metadata_timeout: bool,
    ) -> Result<Response, JellyfinClientError> {
        let initial_origin = origin(&url);
        for redirect_count in 0..=MAX_REDIRECTS {
            let validated = self
                .network_policy
                .validate(url.as_str(), NetworkAccessScope::Jellyfin)
                .await
                .map_err(|_| {
                    JellyfinClientError::Rejected(
                        "Jellyfin destination is blocked by the network policy",
                    )
                })?;
            let mut builder = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none());
            if metadata_timeout {
                builder = builder.timeout(Duration::from_secs(20));
            }
            let client = validated
                .build_client(builder)
                .map_err(|_| JellyfinClientError::Unavailable("Jellyfin is unavailable"))?;
            let mut request = client
                .request(method.clone(), validated.into_url())
                .header(AUTHORIZATION, authorization_header(device_id, auth)?)
                .headers(forwarded_headers.clone());
            if let Some(body) = &body {
                request = request
                    .header(CONTENT_TYPE, "application/json")
                    .body(body.clone());
            }
            let response = request
                .send()
                .await
                .map_err(|_| JellyfinClientError::Unavailable("Jellyfin is unavailable"))?;
            if !response.status().is_redirection() {
                return accepted_response(response);
            }
            if redirect_count == MAX_REDIRECTS {
                return Err(JellyfinClientError::Unavailable(
                    "Jellyfin redirected too many times",
                ));
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(JellyfinClientError::Unavailable(
                    "Jellyfin returned an invalid redirect",
                ))?;
            let next = url.join(location).map_err(|_| {
                JellyfinClientError::Unavailable("Jellyfin returned an invalid redirect")
            })?;
            if origin(&next) != initial_origin {
                return Err(JellyfinClientError::Rejected(
                    "Jellyfin redirected outside its configured origin",
                ));
            }
            url = next;
        }
        Err(JellyfinClientError::Unavailable("Jellyfin is unavailable"))
    }
}

fn universal_audio_url(
    base_url: &str,
    user_id: &str,
    item_id: &str,
    device_id: &str,
) -> Result<Url, JellyfinClientError> {
    let mut url = endpoint(base_url, &["Audio", item_id, "universal"])?;
    url.query_pairs_mut()
        .append_pair("UserId", user_id)
        .append_pair("DeviceId", device_id)
        .append_pair("MaxStreamingBitrate", "192000")
        .append_pair("AudioBitRate", "192000")
        .append_pair("MaxAudioChannels", "2")
        .append_pair("Container", "mp3")
        .append_pair("AudioCodec", "mp3")
        .append_pair("TranscodingContainer", "mp3")
        .append_pair("TranscodingProtocol", "http")
        .append_pair("EnableRedirection", "false");
    Ok(url)
}

fn endpoint(base_url: &str, segments: &[&str]) -> Result<Url, JellyfinClientError> {
    let mut url = Url::parse(base_url)
        .map_err(|_| JellyfinClientError::Rejected("Jellyfin URL is invalid"))?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| JellyfinClientError::Rejected("Jellyfin URL is invalid"))?;
        path.pop_if_empty();
        for segment in segments {
            path.push(segment);
        }
    }
    Ok(url)
}

fn origin(url: &Url) -> (String, String, Option<u16>) {
    (
        url.scheme().to_owned(),
        url.host_str().unwrap_or_default().to_ascii_lowercase(),
        url.port_or_known_default(),
    )
}

fn authorization_header(
    device_id: &str,
    auth: Option<&JellyfinAuth>,
) -> Result<HeaderValue, JellyfinClientError> {
    let safe = |value: &str| value.replace(['"', '\\'], "");
    let mut value = format!(
        "MediaBrowser Client=\"{}\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
        CLIENT_NAME,
        DEVICE_NAME,
        safe(device_id),
        env!("CARGO_PKG_VERSION")
    );
    if let Some(auth) = auth {
        value.push_str(", Token=\"");
        value.push_str(&safe(&auth.token));
        value.push('"');
    }
    HeaderValue::from_str(&value)
        .map_err(|_| JellyfinClientError::Unavailable("Jellyfin authorization failed"))
}

fn accepted_response(response: Response) -> Result<Response, JellyfinClientError> {
    match response.status() {
        status if status.is_success() || status == StatusCode::NOT_MODIFIED => Ok(response),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(JellyfinClientError::Unauthorized),
        StatusCode::NOT_FOUND => Err(JellyfinClientError::NotFound),
        StatusCode::RANGE_NOT_SATISFIABLE => Ok(response),
        _ => Err(JellyfinClientError::Unavailable(
            "Jellyfin could not complete the request",
        )),
    }
}

async fn read_bounded(
    response: Response,
    max_bytes: usize,
) -> Result<Vec<u8>, JellyfinClientError> {
    if response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > max_bytes)
    {
        return Err(JellyfinClientError::Unavailable(
            "Jellyfin response was too large",
        ));
    }
    let mut stream = response.bytes_stream();
    let mut output = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|_| JellyfinClientError::Unavailable("Jellyfin response was interrupted"))?;
        if output.len().saturating_add(chunk.len()) > max_bytes {
            return Err(JellyfinClientError::Unavailable(
                "Jellyfin response was too large",
            ));
        }
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

pub fn safe_forwarded_request_headers(request: &actix_web::HttpRequest) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for name in ["range", "if-range", "if-none-match", "if-modified-since"] {
        if let Some(value) = request.headers().get(name)
            && let Ok(value) = HeaderValue::from_bytes(value.as_bytes())
            && let Ok(name) = HeaderName::from_bytes(name.as_bytes())
        {
            headers.insert(name, value);
        }
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_normalization_preserves_subpath_and_adds_trailing_slash() {
        assert_eq!(
            JellyfinClient::normalize_base_url("https://media.example/jellyfin").unwrap(),
            "https://media.example/jellyfin/"
        );
    }

    #[test]
    fn base_url_rejects_credentials_query_and_fragment() {
        for value in [
            "https://user:secret@example.com",
            "https://example.com?token=secret",
            "https://example.com/#fragment",
        ] {
            assert!(JellyfinClient::normalize_base_url(value).is_err());
        }
    }

    #[test]
    fn endpoint_encodes_untrusted_path_segments() {
        let url = endpoint("https://example.com/jellyfin/", &["Items", "../video"]).unwrap();
        assert_eq!(
            url.as_str(),
            "https://example.com/jellyfin/Items/..%2Fvideo"
        );
    }

    #[test]
    fn universal_audio_requests_one_browser_safe_transcode_target() {
        let url = universal_audio_url(
            "https://example.com/jellyfin/",
            "linked-user",
            "track-id",
            "pandan-device",
        )
        .unwrap();
        let query = url.query_pairs().collect::<HashMap<_, _>>();

        assert_eq!(
            query.get("UserId").map(|value| value.as_ref()),
            Some("linked-user")
        );
        assert_eq!(
            query.get("DeviceId").map(|value| value.as_ref()),
            Some("pandan-device")
        );
        assert_eq!(
            query.get("Container").map(|value| value.as_ref()),
            Some("mp3")
        );
        assert_eq!(
            query.get("AudioCodec").map(|value| value.as_ref()),
            Some("mp3")
        );
        assert_eq!(
            query
                .get("TranscodingContainer")
                .map(|value| value.as_ref()),
            Some("mp3")
        );
        assert_eq!(
            query.get("TranscodingProtocol").map(|value| value.as_ref()),
            Some("http")
        );
        assert_eq!(
            query.get("MaxStreamingBitrate").map(|value| value.as_ref()),
            Some("192000")
        );
        assert_eq!(
            query.get("AudioBitRate").map(|value| value.as_ref()),
            Some("192000")
        );
        assert_eq!(
            query.get("MaxAudioChannels").map(|value| value.as_ref()),
            Some("2")
        );
        assert_eq!(
            query.get("EnableRedirection").map(|value| value.as_ref()),
            Some("false")
        );
    }
}
