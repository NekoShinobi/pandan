use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use db::entities::{BookmarkLibraryCategory, BookmarkLibraryItem};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

use crate::{ApiError, AppState, authenticated_account, authenticated_administrator};

const MAX_CATEGORY_NAME_CHARS: usize = 80;
const MAX_TITLE_CHARS: usize = 120;
const MAX_URL_BYTES: usize = 2_048;
const MAX_ICON_BYTES: usize = 256 * 1024;
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BookmarkLibraryCategoryResponse {
    pub id: String,
    pub scope: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub bookmarks: Vec<BookmarkLibraryItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BookmarkLibraryResponse {
    pub global: Vec<BookmarkLibraryCategoryResponse>,
    pub personal: Vec<BookmarkLibraryCategoryResponse>,
}

#[derive(Debug, Deserialize)]
struct CategoryInput {
    name: String,
}

#[derive(Debug, Deserialize)]
struct BookmarkInput {
    category_id: String,
    title: String,
    url: String,
    icon_kind: String,
    #[serde(default)]
    icon_value: Option<String>,
}

pub(crate) fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/bookmark-library", web::get().to(list_library))
        .route(
            "/bookmark-library/categories",
            web::post().to(create_personal_category),
        )
        .route(
            "/bookmark-library/categories/{category_id}",
            web::patch().to(update_personal_category),
        )
        .route(
            "/bookmark-library/categories/{category_id}",
            web::delete().to(delete_personal_category),
        )
        .route(
            "/bookmark-library/bookmarks",
            web::post().to(create_personal_bookmark),
        )
        .route(
            "/bookmark-library/bookmarks/{bookmark_id}",
            web::patch().to(update_personal_bookmark),
        )
        .route(
            "/bookmark-library/bookmarks/{bookmark_id}",
            web::delete().to(delete_personal_bookmark),
        )
        .route(
            "/bookmark-library/bookmarks/{bookmark_id}/icon",
            web::get().to(bookmark_icon),
        )
        .route(
            "/admin/bookmark-library/categories",
            web::post().to(create_global_category),
        )
        .route(
            "/admin/bookmark-library/categories/{category_id}",
            web::patch().to(update_global_category),
        )
        .route(
            "/admin/bookmark-library/categories/{category_id}",
            web::delete().to(delete_global_category),
        )
        .route(
            "/admin/bookmark-library/bookmarks",
            web::post().to(create_global_bookmark),
        )
        .route(
            "/admin/bookmark-library/bookmarks/{bookmark_id}",
            web::patch().to(update_global_bookmark),
        )
        .route(
            "/admin/bookmark-library/bookmarks/{bookmark_id}",
            web::delete().to(delete_global_bookmark),
        );
}

pub(crate) async fn load_library(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<BookmarkLibraryResponse, sqlx::Error> {
    let (global_categories, personal_categories, global_items, personal_items) = tokio::try_join!(
        db::bookmark_library_queries::list_global_categories(pool),
        db::bookmark_library_queries::list_personal_categories(pool, user_id),
        db::bookmark_library_queries::list_global_items(pool),
        db::bookmark_library_queries::list_personal_items(pool, user_id),
    )?;
    Ok(BookmarkLibraryResponse {
        global: group_categories(global_categories, global_items),
        personal: group_categories(personal_categories, personal_items),
    })
}

fn group_categories(
    categories: Vec<BookmarkLibraryCategory>,
    items: Vec<BookmarkLibraryItem>,
) -> Vec<BookmarkLibraryCategoryResponse> {
    let mut items_by_category: HashMap<String, Vec<BookmarkLibraryItem>> = HashMap::new();
    for item in items {
        items_by_category
            .entry(item.category_id.clone())
            .or_default()
            .push(item);
    }
    categories
        .into_iter()
        .map(|category| BookmarkLibraryCategoryResponse {
            bookmarks: items_by_category.remove(&category.id).unwrap_or_default(),
            id: category.id,
            scope: category.scope,
            name: category.name,
            created_at: category.created_at,
            updated_at: category.updated_at,
        })
        .collect()
}

async fn list_library(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<BookmarkLibraryResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(load_library(&state.pool, &account.id).await?))
}

async fn create_personal_category(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CategoryInput>,
) -> Result<(web::Json<BookmarkLibraryCategory>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let name = validate_category_name(&payload.name)?;
    let category =
        db::bookmark_library_queries::create_personal_category(&state.pool, &account.id, &name)
            .await
            .map_err(|error| map_unique(error, "a category with this name already exists"))?;
    Ok((web::Json(category), StatusCode::CREATED))
}

async fn create_global_category(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CategoryInput>,
) -> Result<(web::Json<BookmarkLibraryCategory>, StatusCode), ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    let name = validate_category_name(&payload.name)?;
    let category =
        db::bookmark_library_queries::create_global_category(&state.pool, &administrator.id, &name)
            .await
            .map_err(|error| {
                map_unique(error, "a global category with this name already exists")
            })?;
    Ok((web::Json(category), StatusCode::CREATED))
}

async fn update_personal_category(
    state: web::Data<AppState>,
    request: HttpRequest,
    category_id: web::Path<String>,
    payload: web::Json<CategoryInput>,
) -> Result<web::Json<BookmarkLibraryCategory>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let name = validate_category_name(&payload.name)?;
    db::bookmark_library_queries::update_personal_category(
        &state.pool,
        &account.id,
        &category_id,
        &name,
    )
    .await
    .map_err(|error| map_unique(error, "a category with this name already exists"))?
    .map(web::Json)
    .ok_or(ApiError::NotFound("bookmark category not found"))
}

async fn update_global_category(
    state: web::Data<AppState>,
    request: HttpRequest,
    category_id: web::Path<String>,
    payload: web::Json<CategoryInput>,
) -> Result<web::Json<BookmarkLibraryCategory>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let name = validate_category_name(&payload.name)?;
    db::bookmark_library_queries::update_global_category(&state.pool, &category_id, &name)
        .await
        .map_err(|error| map_unique(error, "a global category with this name already exists"))?
        .map(web::Json)
        .ok_or(ApiError::NotFound("bookmark category not found"))
}

async fn delete_personal_category(
    state: web::Data<AppState>,
    request: HttpRequest,
    category_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::bookmark_library_queries::delete_category(
        &state.pool,
        &category_id,
        "personal",
        Some(&account.id),
    )
    .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("bookmark category not found"))
    }
}

async fn delete_global_category(
    state: web::Data<AppState>,
    request: HttpRequest,
    category_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    if db::bookmark_library_queries::delete_category(&state.pool, &category_id, "global", None)
        .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("bookmark category not found"))
    }
}

async fn create_personal_bookmark(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<BookmarkInput>,
) -> Result<(web::Json<BookmarkLibraryItem>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    create_bookmark_in_scope(&state, &payload, "personal", Some(&account.id))
        .await
        .map(|bookmark| (web::Json(bookmark), StatusCode::CREATED))
}

async fn create_global_bookmark(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<BookmarkInput>,
) -> Result<(web::Json<BookmarkLibraryItem>, StatusCode), ApiError> {
    authenticated_administrator(&state, &request).await?;
    create_bookmark_in_scope(&state, &payload, "global", None)
        .await
        .map(|bookmark| (web::Json(bookmark), StatusCode::CREATED))
}

async fn create_bookmark_in_scope(
    state: &AppState,
    payload: &BookmarkInput,
    scope: &str,
    user_id: Option<&str>,
) -> Result<BookmarkLibraryItem, ApiError> {
    if !db::bookmark_library_queries::category_is_accessible(
        &state.pool,
        &payload.category_id,
        scope,
        user_id,
    )
    .await?
    {
        return Err(ApiError::NotFound("bookmark category not found"));
    }
    let validated = validate_bookmark_input(payload)?;
    let icon = fetch_icon(state, user_id, &validated).await;
    let icon_ref = icon
        .as_ref()
        .map(|(content_type, data)| (content_type.as_str(), data.as_slice()));
    db::bookmark_library_queries::create_item(
        &state.pool,
        &payload.category_id,
        &validated.title,
        &validated.url,
        &validated.icon_kind,
        validated.icon_value.as_deref(),
        icon_ref,
    )
    .await
    .map_err(|error| map_unique(error, "this category already contains that destination"))
}

async fn update_personal_bookmark(
    state: web::Data<AppState>,
    request: HttpRequest,
    bookmark_id: web::Path<String>,
    payload: web::Json<BookmarkInput>,
) -> Result<web::Json<BookmarkLibraryItem>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    update_bookmark_in_scope(
        &state,
        &bookmark_id,
        &payload,
        "personal",
        Some(&account.id),
    )
    .await
    .map(web::Json)
}

async fn update_global_bookmark(
    state: web::Data<AppState>,
    request: HttpRequest,
    bookmark_id: web::Path<String>,
    payload: web::Json<BookmarkInput>,
) -> Result<web::Json<BookmarkLibraryItem>, ApiError> {
    authenticated_administrator(&state, &request).await?;
    update_bookmark_in_scope(&state, &bookmark_id, &payload, "global", None)
        .await
        .map(web::Json)
}

async fn update_bookmark_in_scope(
    state: &AppState,
    bookmark_id: &str,
    payload: &BookmarkInput,
    scope: &str,
    user_id: Option<&str>,
) -> Result<BookmarkLibraryItem, ApiError> {
    if !db::bookmark_library_queries::category_is_accessible(
        &state.pool,
        &payload.category_id,
        scope,
        user_id,
    )
    .await?
    {
        return Err(ApiError::NotFound("bookmark category not found"));
    }
    let validated = validate_bookmark_input(payload)?;
    let icon = fetch_icon(state, user_id, &validated).await;
    let icon_ref = icon
        .as_ref()
        .map(|(content_type, data)| (content_type.as_str(), data.as_slice()));
    db::bookmark_library_queries::update_item(
        &state.pool,
        bookmark_id,
        &payload.category_id,
        &validated.title,
        &validated.url,
        &validated.icon_kind,
        validated.icon_value.as_deref(),
        icon_ref,
        scope,
        user_id,
    )
    .await
    .map_err(|error| map_unique(error, "this category already contains that destination"))?
    .ok_or(ApiError::NotFound("bookmark not found"))
}

async fn delete_personal_bookmark(
    state: web::Data<AppState>,
    request: HttpRequest,
    bookmark_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    delete_bookmark_in_scope(&state.pool, &bookmark_id, "personal", Some(&account.id)).await
}

async fn delete_global_bookmark(
    state: web::Data<AppState>,
    request: HttpRequest,
    bookmark_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    delete_bookmark_in_scope(&state.pool, &bookmark_id, "global", None).await
}

async fn delete_bookmark_in_scope(
    pool: &SqlitePool,
    bookmark_id: &str,
    scope: &str,
    user_id: Option<&str>,
) -> Result<HttpResponse, ApiError> {
    if db::bookmark_library_queries::delete_item(pool, bookmark_id, scope, user_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("bookmark not found"))
    }
}

async fn bookmark_icon(
    state: web::Data<AppState>,
    request: HttpRequest,
    bookmark_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let icon =
        db::bookmark_library_queries::get_visible_icon(&state.pool, &account.id, &bookmark_id)
            .await?
            .ok_or(ApiError::NotFound("bookmark icon not found"))?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "private, no-store"))
        .insert_header(("Content-Type", icon.content_type))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .body(icon.data))
}

struct ValidatedBookmarkInput {
    title: String,
    url: String,
    icon_kind: String,
    icon_value: Option<String>,
    icon_source: Option<String>,
}

fn validate_bookmark_input(payload: &BookmarkInput) -> Result<ValidatedBookmarkInput, ApiError> {
    let title = validate_title(&payload.title)?;
    let url = normalize_destination_url(&payload.url)?;
    let (icon_kind, icon_value, icon_source) =
        validate_icon(&payload.icon_kind, payload.icon_value.as_deref(), &url)?;
    Ok(ValidatedBookmarkInput {
        title,
        url,
        icon_kind,
        icon_value,
        icon_source,
    })
}

fn validate_category_name(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_CATEGORY_NAME_CHARS {
        return Err(ApiError::BadRequest(
            "bookmark category names must be between 1 and 80 characters",
        ));
    }
    Ok(value.to_owned())
}

fn validate_title(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_TITLE_CHARS {
        return Err(ApiError::BadRequest(
            "bookmark names must be between 1 and 120 characters",
        ));
    }
    Ok(value.to_owned())
}

fn normalize_destination_url(value: &str) -> Result<String, ApiError> {
    validate_http_url(value, false, "bookmark URL")
}

fn validate_icon(
    kind: &str,
    value: Option<&str>,
    destination_url: &str,
) -> Result<(String, Option<String>, Option<String>), ApiError> {
    match kind.trim() {
        "favicon" => {
            let mut source = Url::parse(destination_url)
                .map_err(|_| ApiError::BadRequest("bookmark URL is invalid"))?;
            source.set_path("/favicon.ico");
            source.set_query(None);
            source.set_fragment(None);
            Ok(("favicon".to_owned(), None, Some(source.to_string())))
        }
        "lucide" => {
            let name = value.unwrap_or("").trim().to_ascii_lowercase();
            if !SUPPORTED_LUCIDE_ICONS.contains(&name.as_str()) {
                return Err(ApiError::BadRequest("Lucide icon name is unsupported"));
            }
            Ok(("lucide".to_owned(), Some(name), None))
        }
        "custom" => {
            let source = validate_http_url(value.unwrap_or(""), true, "custom icon URL")?;
            Ok(("custom".to_owned(), Some(source.clone()), Some(source)))
        }
        _ => Err(ApiError::BadRequest(
            "bookmark icon kind must be favicon, lucide, or custom",
        )),
    }
}

fn validate_http_url(
    value: &str,
    https_only: bool,
    label: &'static str,
) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_URL_BYTES {
        return Err(ApiError::BadRequest(match label {
            "custom icon URL" => "custom icon URLs must be between 1 and 2048 bytes",
            _ => "bookmark URLs must be between 1 and 2048 bytes",
        }));
    }
    let url = Url::parse(value).map_err(|_| ApiError::BadRequest("URL is invalid"))?;
    let valid_scheme = if https_only {
        url.scheme() == "https"
    } else {
        matches!(url.scheme(), "http" | "https")
    };
    if !valid_scheme || !url.username().is_empty() || url.password().is_some() {
        return Err(ApiError::BadRequest(if https_only {
            "custom icon URLs must be credential-free HTTPS links"
        } else {
            "bookmark URLs must be credential-free HTTP or HTTPS links"
        }));
    }
    if url.host_str().is_none() {
        return Err(ApiError::BadRequest("URL host is missing"));
    }
    Ok(url.to_string())
}

async fn fetch_icon(
    state: &AppState,
    user_id: Option<&str>,
    input: &ValidatedBookmarkInput,
) -> Option<(String, Vec<u8>)> {
    let source = input.icon_source.as_deref()?;
    let fetch_source = if input.icon_kind == "favicon" {
        input.url.as_str()
    } else {
        source
    };
    let origin = Url::parse(fetch_source)
        .ok()
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|| "invalid".to_owned());
    let fetch = async {
        if input.icon_kind == "favicon" {
            state
                .widget_integrations
                .fetch_site_favicon(fetch_source, MAX_ICON_BYTES)
                .await
        } else {
            state
                .widget_integrations
                .fetch_favicon(fetch_source, MAX_ICON_BYTES)
                .await
        }
    };
    match timeout(Duration::from_secs(6), fetch).await {
        Ok(Ok(icon)) => Some(icon),
        Ok(Err(_)) => {
            tracing::warn!(
                user_id = user_id.unwrap_or("global"),
                icon_kind = %input.icon_kind,
                %origin,
                "bookmark library icon fetch failed; saving fallback"
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                user_id = user_id.unwrap_or("global"),
                icon_kind = %input.icon_kind,
                %origin,
                timeout_seconds = 6,
                "bookmark library icon fetch timed out; saving fallback"
            );
            None
        }
    }
}

fn map_unique(error: sqlx::Error, conflict: &'static str) -> ApiError {
    if error
        .as_database_error()
        .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
    {
        ApiError::Conflict(conflict)
    } else {
        ApiError::Database(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_library_urls_and_icons_are_validated() {
        assert_eq!(
            normalize_destination_url(" https://Example.com/docs ")
                .expect("destination normalizes"),
            "https://example.com/docs"
        );
        assert!(normalize_destination_url("javascript:alert(1)").is_err());
        assert!(validate_icon("lucide", Some("book-open"), "https://example.com").is_ok());
        assert!(validate_icon("lucide", Some("not-a-real-icon"), "https://example.com").is_err());
        assert!(
            validate_icon(
                "custom",
                Some("http://example.com/icon.png"),
                "https://example.com"
            )
            .is_err()
        );
    }
}
