use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use db::entities::Bookmark;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;

use crate::{ApiError, AppState, authenticated_account};

const MAX_BOOKMARKS: i64 = 32;
const MAX_BOOKMARK_TITLE_CHARS: usize = 120;
const MAX_BOOKMARK_URL_BYTES: usize = 2_048;
const MAX_FAVICON_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateBookmarkRequest {
    pub title: String,
    pub url: String,
}

pub(crate) fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/bookmarks", web::get().to(list_bookmarks))
        .route("/bookmarks", web::post().to(create_bookmark))
        .route(
            "/bookmarks/{bookmark_id}/favicon",
            web::get().to(bookmark_favicon),
        )
        .route(
            "/bookmarks/{bookmark_id}",
            web::delete().to(delete_bookmark),
        );
}

async fn list_bookmarks(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<Vec<Bookmark>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(
        db::queries::list_bookmarks(&state.pool, &account.id).await?,
    ))
}

async fn create_bookmark(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateBookmarkRequest>,
) -> Result<(web::Json<Bookmark>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let title = validate_title(&payload.title)?;
    let url = normalize_bookmark_url(&payload.url)?;
    if db::queries::find_bookmark_by_url(&state.pool, &account.id, &url)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict("this bookmark is already saved"));
    }
    if db::queries::count_bookmarks(&state.pool, &account.id).await? >= MAX_BOOKMARKS {
        return Err(ApiError::Conflict(
            "a bookmark list can contain at most 32 links",
        ));
    }

    let favicon_url = favicon_url(&url)?;
    let favicon_origin = favicon_url.origin().ascii_serialization();
    let favicon = match timeout(
        Duration::from_secs(4),
        state
            .widget_integrations
            .fetch_favicon(favicon_url.as_str(), MAX_FAVICON_BYTES),
    )
    .await
    {
        Ok(Ok(favicon)) => {
            tracing::debug!(
                user_id = %account.id,
                origin = %favicon_origin,
                "bookmark favicon fetched"
            );
            Some(favicon)
        }
        Ok(Err(message)) => {
            tracing::warn!(
                user_id = %account.id,
                origin = %favicon_origin,
                %message,
                "bookmark favicon fetch failed; saving bookmark without it"
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                user_id = %account.id,
                origin = %favicon_origin,
                timeout_seconds = 4,
                "bookmark favicon fetch timed out; saving bookmark without it"
            );
            None
        }
    };
    let favicon_ref = favicon
        .as_ref()
        .map(|(content_type, data)| (content_type.as_str(), data.as_slice()));
    let bookmark =
        db::queries::create_bookmark(&state.pool, &account.id, &title, &url, favicon_ref)
            .await
            .map_err(|error| {
                if error
                    .as_database_error()
                    .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
                {
                    ApiError::Conflict("this bookmark is already saved")
                } else {
                    ApiError::Database(error)
                }
            })?
            .ok_or(ApiError::Conflict(
                "a bookmark list can contain at most 32 links",
            ))?;
    Ok((web::Json(bookmark), StatusCode::CREATED))
}

async fn bookmark_favicon(
    state: web::Data<AppState>,
    request: HttpRequest,
    bookmark_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let favicon = db::queries::get_bookmark_favicon(&state.pool, &account.id, &bookmark_id)
        .await?
        .ok_or(ApiError::NotFound("bookmark favicon not found"))?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "private, no-store"))
        .insert_header(("Content-Type", favicon.content_type))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .body(favicon.data))
}

async fn delete_bookmark(
    state: web::Data<AppState>,
    request: HttpRequest,
    bookmark_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_bookmark(&state.pool, &account.id, &bookmark_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("bookmark not found"))
    }
}

fn validate_title(value: &str) -> Result<String, ApiError> {
    let title = value.trim();
    if title.is_empty() || title.chars().count() > MAX_BOOKMARK_TITLE_CHARS {
        return Err(ApiError::BadRequest(
            "bookmark titles must be between 1 and 120 characters",
        ));
    }
    Ok(title.to_owned())
}

fn normalize_bookmark_url(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_BOOKMARK_URL_BYTES {
        return Err(ApiError::BadRequest(
            "bookmark URLs must be between 1 and 2048 bytes",
        ));
    }
    let url = Url::parse(value).map_err(|_| ApiError::BadRequest("bookmark URL is invalid"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ApiError::BadRequest(
            "bookmark URLs must be credential-free HTTP or HTTPS links",
        ));
    }
    if url.host_str().is_none() {
        return Err(ApiError::BadRequest("bookmark URL host is missing"));
    }
    Ok(url.to_string())
}

fn favicon_url(value: &str) -> Result<Url, ApiError> {
    let mut url = Url::parse(value).map_err(|_| ApiError::BadRequest("bookmark URL is invalid"))?;
    url.set_path("/favicon.ico");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_urls_are_normalized_and_favicons_use_the_origin_root() {
        let normalized = normalize_bookmark_url(" https://Example.com/docs?q=1#start ")
            .expect("bookmark URL normalizes");
        assert_eq!(normalized, "https://example.com/docs?q=1#start");
        assert_eq!(
            favicon_url(&normalized)
                .expect("favicon URL builds")
                .as_str(),
            "https://example.com/favicon.ico"
        );
    }

    #[test]
    fn bookmark_urls_reject_credentials_and_non_http_schemes() {
        assert!(normalize_bookmark_url("https://user:secret@example.com").is_err());
        assert!(normalize_bookmark_url("javascript:alert(1)").is_err());
    }
}
