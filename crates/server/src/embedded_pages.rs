use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use db::entities::EmbeddedPage;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use url::Url;

use super::{ApiError, AppState, authenticated_account, authenticated_administrator};

const MAX_EMBEDDED_PAGES_PER_SCOPE: i64 = 32;
const MAX_TITLE_CHARACTERS: usize = 80;
const MAX_DESCRIPTION_CHARACTERS: usize = 280;
const MAX_URL_CHARACTERS: usize = 2_000;
const DEFAULT_IFRAME_HEIGHT: i64 = 720;
const MIN_IFRAME_HEIGHT: i64 = 320;
const MAX_IFRAME_HEIGHT: i64 = 2_400;

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
    allow_same_origin: bool,
    #[serde(default = "default_iframe_height")]
    iframe_height: i64,
}

#[derive(Debug, Deserialize)]
struct EmbeddedPageOrderInput {
    page_ids: Vec<String>,
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

async fn create_personal_page(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<EmbeddedPageInput>,
) -> Result<(web::Json<EmbeddedPage>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    enforce_personal_page_limit(&state.pool, &account.id).await?;
    let (title, description, url, iframe_height) = validate_input(&payload)?;
    let page = db::queries::create_personal_embedded_page(
        &state.pool,
        &account.id,
        &title,
        &description,
        &url,
        payload.allow_same_origin,
        iframe_height,
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
    let (title, description, url, iframe_height) = validate_input(&payload)?;
    let page = db::queries::create_global_embedded_page(
        &state.pool,
        &administrator.id,
        &title,
        &description,
        &url,
        payload.allow_same_origin,
        iframe_height,
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
    let (title, description, url, iframe_height) = validate_input(&payload)?;
    db::queries::update_personal_embedded_page(
        &state.pool,
        &account.id,
        &page_id,
        &title,
        &description,
        &url,
        payload.allow_same_origin,
        iframe_height,
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
    let (title, description, url, iframe_height) = validate_input(&payload)?;
    db::queries::update_global_embedded_page(
        &state.pool,
        &page_id,
        &title,
        &description,
        &url,
        payload.allow_same_origin,
        iframe_height,
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

fn validate_input(payload: &EmbeddedPageInput) -> Result<(String, String, String, i64), ApiError> {
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

    Ok((
        title.to_owned(),
        description.to_owned(),
        parsed.to_string(),
        payload.iframe_height,
    ))
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
                allow_same_origin: false,
                iframe_height: DEFAULT_IFRAME_HEIGHT,
            };
            assert!(validate_input(&invalid).is_err(), "{url} is rejected");
        }
    }

    #[test]
    fn embedded_page_trust_is_opt_in_for_older_clients() {
        let input: EmbeddedPageInput = serde_json::from_str(
            r#"{"title":"Status","description":"","url":"https://example.com/"}"#,
        )
        .expect("embedded page input deserializes");

        assert!(!input.allow_same_origin);
        assert_eq!(input.iframe_height, DEFAULT_IFRAME_HEIGHT);
    }

    #[test]
    fn embedded_page_height_is_bounded() {
        for iframe_height in [MIN_IFRAME_HEIGHT - 1, MAX_IFRAME_HEIGHT + 1] {
            let input = EmbeddedPageInput {
                title: "Status".to_owned(),
                description: String::new(),
                url: "https://example.com/".to_owned(),
                allow_same_origin: false,
                iframe_height,
            };

            assert!(validate_input(&input).is_err());
        }
    }
}
