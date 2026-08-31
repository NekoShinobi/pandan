use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use db::entities::{EmbeddedPage, EmbeddedPageIconCache};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::time::timeout;
use url::Url;

use super::{ApiError, AppState, authenticated_account, authenticated_administrator};

const MAX_EMBEDDED_PAGES_PER_SCOPE: i64 = 32;
const MAX_TITLE_CHARACTERS: usize = 80;
const MAX_DESCRIPTION_CHARACTERS: usize = 280;
const MAX_URL_CHARACTERS: usize = 2_000;
const MAX_ICON_URL_CHARACTERS: usize = 2_000;
const MAX_ICON_BYTES: usize = 256 * 1024;
const DEFAULT_IFRAME_HEIGHT: i64 = 720;
const MIN_IFRAME_HEIGHT: i64 = 320;
const MAX_IFRAME_HEIGHT: i64 = 2_400;
const SUPPORTED_LUCIDE_ICONS: &[&str] = &[
    "bell",
    "book-open",
    "bookmark",
    "briefcase",
    "calendar-days",
    "cloud",
    "code",
    "database",
    "folder",
    "gamepad-2",
    "git-branch",
    "globe",
    "heart",
    "house",
    "image",
    "link",
    "lock",
    "mail",
    "music",
    "podcast",
    "rocket",
    "rss",
    "shopping-bag",
    "star",
    "terminal",
    "video",
    "wrench",
];

const fn default_iframe_height() -> i64 {
    DEFAULT_IFRAME_HEIGHT
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddedPagesResponse {
    pub global: Vec<EmbeddedPage>,
    pub personal: Vec<EmbeddedPage>,
}

#[derive(Debug, Deserialize)]
struct EmbeddedPageInput {
    title: String,
    #[serde(default)]
    description: String,
    url: String,
    #[serde(default)]
    icon_kind: Option<String>,
    #[serde(default)]
    icon_value: Option<String>,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    allow_scripts: bool,
    #[serde(default)]
    allow_same_origin: bool,
    #[serde(default = "default_iframe_height")]
    iframe_height: i64,
}

#[derive(Debug, Deserialize)]
struct EmbeddedPageOrderInput {
    page_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EmbeddedPageScopeInput {
    scope: String,
}

struct ValidatedEmbeddedPageInput {
    title: String,
    description: String,
    url: String,
    icon_kind: String,
    icon_value: Option<String>,
    iframe_height: i64,
}

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/embedded-pages", web::get().to(list_pages))
        .route("/embedded-pages", web::post().to(create_personal_page))
        .route(
            "/embedded-pages/order",
            web::put().to(reorder_personal_pages),
        )
        .route(
            "/embedded-pages/{page_id}/icon",
            web::get().to(embedded_page_icon),
        )
        .route(
            "/embedded-pages/{page_id}",
            web::patch().to(update_personal_page),
        )
        .route(
            "/embedded-pages/{page_id}",
            web::delete().to(delete_personal_page),
        )
        .route("/admin/embedded-pages", web::post().to(create_global_page))
        .route(
            "/admin/embedded-pages/order",
            web::put().to(reorder_global_pages),
        )
        .route(
            "/admin/embedded-pages/{page_id}",
            web::patch().to(update_global_page),
        )
        .route(
            "/admin/embedded-pages/{page_id}",
            web::delete().to(delete_global_page),
        )
        .route(
            "/admin/embedded-pages/{page_id}/scope",
            web::patch().to(move_page_scope),
        );
}

pub async fn load_visible_pages(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<EmbeddedPagesResponse, sqlx::Error> {
    let (global, personal) = tokio::try_join!(
        db::queries::list_global_embedded_pages(pool),
        db::queries::list_personal_embedded_pages(pool, user_id),
    )?;
    Ok(EmbeddedPagesResponse { global, personal })
}

async fn list_pages(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<EmbeddedPagesResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(
        load_visible_pages(&state.pool, &account.id).await?,
    ))
}

async fn embedded_page_icon(
    state: web::Data<AppState>,
    request: HttpRequest,
    page_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let cache =
        db::queries::get_visible_embedded_page_icon_cache(&state.pool, &account.id, &page_id)
            .await?
            .ok_or(ApiError::NotFound("embedded page icon not found"))?;
    if let Some(response) = cached_icon_response(&cache) {
        return Ok(response);
    }
    if cache.icon_kind == "lucide" || cache.fetched_at.is_some() {
        return Err(ApiError::NotFound("embedded page icon not found"));
    }

    let icon = fetch_embedded_page_icon(&state, &account.id, &cache).await;
    let icon_ref = icon
        .as_ref()
        .map(|(content_type, data)| (content_type.as_str(), data.as_slice()));
    db::queries::store_embedded_page_icon_attempt(
        &state.pool,
        &page_id,
        &cache.page_url,
        &cache.icon_kind,
        cache.icon_value.as_deref(),
        icon_ref,
    )
    .await?;

    let refreshed =
        db::queries::get_visible_embedded_page_icon_cache(&state.pool, &account.id, &page_id)
            .await?
            .ok_or(ApiError::NotFound("embedded page icon not found"))?;
    cached_icon_response(&refreshed).ok_or(ApiError::NotFound("embedded page icon not found"))
}

fn cached_icon_response(cache: &EmbeddedPageIconCache) -> Option<HttpResponse> {
    let (Some(content_type), Some(data)) = (&cache.content_type, &cache.data) else {
        return None;
    };
    Some(
        HttpResponse::Ok()
            .insert_header(("Cache-Control", "private, max-age=31536000, immutable"))
            .insert_header(("Content-Type", content_type.as_str()))
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .body(data.clone()),
    )
}

async fn fetch_embedded_page_icon(
    state: &AppState,
    user_id: &str,
    cache: &EmbeddedPageIconCache,
) -> Option<(String, Vec<u8>)> {
    let source = match cache.icon_kind.as_str() {
        "favicon" => cache.page_url.as_str(),
        "custom" => cache.icon_value.as_deref()?,
        _ => return None,
    };
    let origin = Url::parse(source)
        .ok()
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|| "invalid".to_owned());
    let fetch = async {
        if cache.icon_kind == "favicon" {
            state
                .widget_integrations
                .fetch_site_favicon(source, MAX_ICON_BYTES)
                .await
        } else {
            state
                .widget_integrations
                .fetch_favicon(source, MAX_ICON_BYTES)
                .await
        }
    };
    match timeout(Duration::from_secs(6), fetch).await {
        Ok(Ok(icon)) => Some(icon),
        Ok(Err(_)) => {
            tracing::warn!(
                %user_id,
                icon_kind = %cache.icon_kind,
                %origin,
                "embedded page icon fetch failed; keeping fallback"
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                %user_id,
                icon_kind = %cache.icon_kind,
                %origin,
                timeout_seconds = 6,
                "embedded page icon fetch timed out; keeping fallback"
            );
            None
        }
    }
}

async fn create_personal_page(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<EmbeddedPageInput>,
) -> Result<(web::Json<EmbeddedPage>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    enforce_personal_page_limit(&state.pool, &account.id).await?;
    let input = validate_input(&payload)?;
    let page = db::queries::create_personal_embedded_page(
        &state.pool,
        &account.id,
        &input.title,
        &input.description,
        &input.url,
        &input.icon_kind,
        input.icon_value.as_deref(),
        payload.allow_scripts,
        payload.allow_same_origin,
        input.iframe_height,
    )
    .await?;
    Ok((web::Json(page), StatusCode::CREATED))
}

async fn create_global_page(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<EmbeddedPageInput>,
) -> Result<(web::Json<EmbeddedPage>, StatusCode), ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    enforce_global_page_limit(&state.pool).await?;
    let input = validate_input(&payload)?;
    let page = db::queries::create_global_embedded_page(
        &state.pool,
        &administrator.id,
        &input.title,
        &input.description,
        &input.url,
        &input.icon_kind,
        input.icon_value.as_deref(),
        payload.allow_scripts,
        payload.allow_same_origin,
        input.iframe_height,
    )
    .await?;
    Ok((web::Json(page), StatusCode::CREATED))
}

async fn update_personal_page(
    state: web::Data<AppState>,
    request: HttpRequest,
    page_id: web::Path<String>,
    payload: web::Json<EmbeddedPageInput>,
) -> Result<web::Json<EmbeddedPage>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let input = validate_input(&payload)?;
    db::queries::update_personal_embedded_page(
        &state.pool,
        &account.id,
        &page_id,
        &input.title,
        &input.description,
        &input.url,
        &input.icon_kind,
        input.icon_value.as_deref(),
        payload.allow_scripts,
        payload.allow_same_origin,
        input.iframe_height,
    )
    .await?
    .map(web::Json)
    .ok_or(ApiError::NotFound("embedded page not found"))
}

async fn update_global_page(
    state: web::Data<AppState>,
    request: HttpRequest,
    page_id: web::Path<String>,
    payload: web::Json<EmbeddedPageInput>,
) -> Result<web::Json<EmbeddedPage>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let input = validate_input(&payload)?;
    db::queries::update_global_embedded_page(
        &state.pool,
        &page_id,
        &input.title,
        &input.description,
        &input.url,
        &input.icon_kind,
        input.icon_value.as_deref(),
        payload.allow_scripts,
        payload.allow_same_origin,
        input.iframe_height,
    )
    .await?
    .map(web::Json)
    .ok_or(ApiError::NotFound("embedded page not found"))
}

async fn delete_personal_page(
    state: web::Data<AppState>,
    request: HttpRequest,
    page_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_personal_embedded_page(&state.pool, &account.id, &page_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("embedded page not found"))
    }
}

async fn delete_global_page(
    state: web::Data<AppState>,
    request: HttpRequest,
    page_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    if db::queries::delete_global_embedded_page(&state.pool, &page_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("embedded page not found"))
    }
}

async fn move_page_scope(
    state: web::Data<AppState>,
    request: HttpRequest,
    page_id: web::Path<String>,
    payload: web::Json<EmbeddedPageScopeInput>,
) -> Result<web::Json<EmbeddedPage>, ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    let outcome = match payload.scope.trim() {
        "user" => {
            db::queries::move_global_embedded_page_to_personal(
                &state.pool,
                &administrator.id,
                &page_id,
                MAX_EMBEDDED_PAGES_PER_SCOPE,
            )
            .await?
        }
        "global" => {
            db::queries::move_personal_embedded_page_to_global(
                &state.pool,
                &administrator.id,
                &page_id,
                MAX_EMBEDDED_PAGES_PER_SCOPE,
            )
            .await?
        }
        _ => {
            return Err(ApiError::BadRequest(
                "embedded page scope must be global or user",
            ));
        }
    };
    match outcome {
        db::queries::EmbeddedPageMoveOutcome::Moved(page) => Ok(web::Json(*page)),
        db::queries::EmbeddedPageMoveOutcome::NotFound => {
            Err(ApiError::NotFound("embedded page not found"))
        }
        db::queries::EmbeddedPageMoveOutcome::TargetFull => Err(ApiError::Conflict(
            "the destination can contain at most 32 embedded pages",
        )),
    }
}

async fn reorder_personal_pages(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<EmbeddedPageOrderInput>,
) -> Result<web::Json<Vec<EmbeddedPage>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    validate_order(&payload.page_ids)?;
    db::queries::reorder_personal_embedded_pages(&state.pool, &account.id, &payload.page_ids)
        .await?
        .map(web::Json)
        .ok_or(ApiError::Conflict(
            "embedded page order does not match the personal page list",
        ))
}

async fn reorder_global_pages(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<EmbeddedPageOrderInput>,
) -> Result<web::Json<Vec<EmbeddedPage>>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    validate_order(&payload.page_ids)?;
    db::queries::reorder_global_embedded_pages(&state.pool, &payload.page_ids)
        .await?
        .map(web::Json)
        .ok_or(ApiError::Conflict(
            "embedded page order does not match the global page list",
        ))
}

fn validate_input(payload: &EmbeddedPageInput) -> Result<ValidatedEmbeddedPageInput, ApiError> {
    let title = payload.title.trim();
    if title.is_empty() {
        return Err(ApiError::BadRequest("embedded page title is required"));
    }
    if title.chars().count() > MAX_TITLE_CHARACTERS {
        return Err(ApiError::BadRequest(
            "embedded page title must be 80 characters or fewer",
        ));
    }
    let description = payload.description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARACTERS {
        return Err(ApiError::BadRequest(
            "embedded page description must be 280 characters or fewer",
        ));
    }
    let url = payload.url.trim();
    if url.chars().count() > MAX_URL_CHARACTERS {
        return Err(ApiError::BadRequest(
            "embedded page URL must be 2000 characters or fewer",
        ));
    }
    let parsed = Url::parse(url)
        .map_err(|_| ApiError::BadRequest("embedded page URL must be a valid HTTPS URL"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(ApiError::BadRequest(
            "embedded page URL must be credential-free HTTPS",
        ));
    }
    if !(MIN_IFRAME_HEIGHT..=MAX_IFRAME_HEIGHT).contains(&payload.iframe_height) {
        return Err(ApiError::BadRequest(
            "embedded page height must be between 320 and 2400 pixels",
        ));
    }
    let (icon_kind, icon_value) = validate_icon(payload)?;

    Ok(ValidatedEmbeddedPageInput {
        title: title.to_owned(),
        description: description.to_owned(),
        url: parsed.to_string(),
        icon_kind,
        icon_value,
        iframe_height: payload.iframe_height,
    })
}

fn validate_icon(payload: &EmbeddedPageInput) -> Result<(String, Option<String>), ApiError> {
    let explicit_kind = payload
        .icon_kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let legacy_icon_url = payload
        .icon_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let kind = explicit_kind.unwrap_or(if legacy_icon_url.is_some() {
        "custom"
    } else {
        "favicon"
    });
    let value = if explicit_kind.is_some() {
        payload.icon_value.as_deref()
    } else {
        legacy_icon_url
    };

    match kind {
        "favicon" => Ok(("favicon".to_owned(), None)),
        "lucide" => {
            let name = value.unwrap_or("").trim().to_ascii_lowercase();
            if !SUPPORTED_LUCIDE_ICONS.contains(&name.as_str()) {
                return Err(ApiError::BadRequest("Lucide icon name is unsupported"));
            }
            Ok(("lucide".to_owned(), Some(name)))
        }
        "custom" => {
            let url = validate_icon_url(value)?.ok_or(ApiError::BadRequest(
                "custom embedded page icons require an HTTPS URL",
            ))?;
            Ok(("custom".to_owned(), Some(url)))
        }
        _ => Err(ApiError::BadRequest(
            "embedded page icon kind must be favicon, lucide, or custom",
        )),
    }
}

fn validate_icon_url(icon_url: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(icon_url) = icon_url.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if icon_url.chars().count() > MAX_ICON_URL_CHARACTERS {
        return Err(ApiError::BadRequest(
            "embedded page icon URL must be 2000 characters or fewer",
        ));
    }
    let parsed = Url::parse(icon_url)
        .map_err(|_| ApiError::BadRequest("embedded page icon URL must be a valid HTTPS URL"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(ApiError::BadRequest(
            "embedded page icon URL must be credential-free HTTPS",
        ));
    }
    Ok(Some(parsed.to_string()))
}

fn validate_order(page_ids: &[String]) -> Result<(), ApiError> {
    if i64::try_from(page_ids.len()).unwrap_or(i64::MAX) > MAX_EMBEDDED_PAGES_PER_SCOPE {
        return Err(ApiError::BadRequest(
            "embedded page order contains too many pages",
        ));
    }
    if page_ids
        .iter()
        .enumerate()
        .any(|(index, id)| id.is_empty() || page_ids[..index].contains(id))
    {
        return Err(ApiError::BadRequest(
            "embedded page order contains duplicate or empty identifiers",
        ));
    }
    Ok(())
}

async fn enforce_personal_page_limit(pool: &SqlitePool, user_id: &str) -> Result<(), ApiError> {
    if db::queries::count_personal_embedded_pages(pool, user_id).await?
        >= MAX_EMBEDDED_PAGES_PER_SCOPE
    {
        return Err(ApiError::Conflict(
            "an account can contain at most 32 personal embedded pages",
        ));
    }
    Ok(())
}

async fn enforce_global_page_limit(pool: &SqlitePool) -> Result<(), ApiError> {
    if db::queries::count_global_embedded_pages(pool).await? >= MAX_EMBEDDED_PAGES_PER_SCOPE {
        return Err(ApiError::Conflict(
            "the instance can contain at most 32 global embedded pages",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_page_urls_are_https_and_credential_free() {
        let valid = EmbeddedPageInput {
            title: "Status".to_owned(),
            description: String::new(),
            url: "https://example.com/embed?view=compact".to_owned(),
            icon_kind: None,
            icon_value: None,
            icon_url: Some("https://cdn.example.com/status.svg".to_owned()),
            allow_scripts: true,
            allow_same_origin: true,
            iframe_height: DEFAULT_IFRAME_HEIGHT,
        };
        assert!(validate_input(&valid).is_ok());

        for url in [
            "http://example.com",
            "javascript:alert(1)",
            "data:text/html,hello",
            "https://user:secret@example.com",
            "not a url",
        ] {
            let invalid = EmbeddedPageInput {
                title: "Status".to_owned(),
                description: String::new(),
                url: url.to_owned(),
                icon_kind: None,
                icon_value: None,
                icon_url: None,
                allow_scripts: false,
                allow_same_origin: false,
                iframe_height: DEFAULT_IFRAME_HEIGHT,
            };
            assert!(validate_input(&invalid).is_err(), "{url} is rejected");
        }
    }

    #[test]
    fn embedded_page_permissions_are_opt_in_for_older_clients() {
        let input: EmbeddedPageInput = serde_json::from_str(
            r#"{"title":"Status","description":"","url":"https://example.com/"}"#,
        )
        .expect("embedded page input deserializes");

        assert!(!input.allow_scripts);
        assert!(!input.allow_same_origin);
        assert_eq!(input.icon_kind, None);
        assert_eq!(input.icon_value, None);
        assert_eq!(input.icon_url, None);
        assert_eq!(input.iframe_height, DEFAULT_IFRAME_HEIGHT);
        assert_eq!(
            validate_icon(&input).expect("older input receives an icon default"),
            ("favicon".to_owned(), None)
        );
    }

    #[test]
    fn embedded_page_icon_sources_are_validated() {
        assert_eq!(
            validate_icon_url(None).expect("missing icon is valid"),
            None
        );
        assert_eq!(
            validate_icon_url(Some("  ")).expect("blank icon is removed"),
            None
        );
        assert_eq!(
            validate_icon_url(Some("https://cdn.example.com/icon.svg"))
                .expect("HTTPS icon is valid")
                .as_deref(),
            Some("https://cdn.example.com/icon.svg")
        );

        for icon_url in [
            "http://cdn.example.com/icon.svg",
            "data:image/svg+xml,hello",
            "https://user:secret@cdn.example.com/icon.svg",
            "not a url",
        ] {
            assert!(
                validate_icon_url(Some(icon_url)).is_err(),
                "{icon_url} is rejected"
            );
        }

        for (kind, value, valid) in [
            ("favicon", None, true),
            ("lucide", Some("terminal"), true),
            ("lucide", Some("not-a-real-icon"), false),
            ("custom", Some("https://cdn.example.com/icon.png"), true),
            ("custom", None, false),
        ] {
            let input = EmbeddedPageInput {
                title: "Status".to_owned(),
                description: String::new(),
                url: "https://example.com/".to_owned(),
                icon_kind: Some(kind.to_owned()),
                icon_value: value.map(str::to_owned),
                icon_url: None,
                allow_scripts: false,
                allow_same_origin: false,
                iframe_height: DEFAULT_IFRAME_HEIGHT,
            };
            assert_eq!(validate_icon(&input).is_ok(), valid, "{kind} is checked");
        }
    }

    #[test]
    fn embedded_page_height_is_bounded() {
        for iframe_height in [MIN_IFRAME_HEIGHT - 1, MAX_IFRAME_HEIGHT + 1] {
            let input = EmbeddedPageInput {
                title: "Status".to_owned(),
                description: String::new(),
                url: "https://example.com/".to_owned(),
                icon_kind: None,
                icon_value: None,
                icon_url: None,
                allow_scripts: false,
                allow_same_origin: false,
                iframe_height,
            };

            assert!(validate_input(&input).is_err());
        }
    }
}
