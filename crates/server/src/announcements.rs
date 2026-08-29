use super::{
    ApiError, AppState, authenticated_account, authenticated_administrator, validate_image_upload,
    validate_short_text,
};
use actix_web::{HttpRequest, HttpResponse, http::header, web};
use db::entities::{Announcement, AnnouncementDraft};
use serde::Deserialize;

const MAX_ANNOUNCEMENT_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_ANNOUNCEMENT_CONTENT_CHARS: usize = 50_000;
const REACTION_EMOJIS: &[&str] = &["👍", "❤️", "😂", "🎉", "😕", "👀"];

#[derive(Debug, Deserialize)]
struct AnnouncementPayload {
    title: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnnouncementImageQuery {
    file_name: String,
}

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/announcements")
            .service(
                web::resource("")
                    .route(web::get().to(list_announcements))
                    .route(web::post().to(create_announcement)),
            )
            .service(
                web::resource("/{announcement_id}")
                    .route(web::patch().to(update_announcement))
                    .route(web::delete().to(delete_announcement)),
            )
            .service(
                web::resource("/{announcement_id}/images")
                    .app_data(web::PayloadConfig::new(MAX_ANNOUNCEMENT_IMAGE_BYTES))
                    .route(web::post().to(create_image)),
            )
            .service(
                web::resource("/{announcement_id}/images/{image_id}")
                    .route(web::get().to(get_image))
                    .route(web::delete().to(delete_image)),
            )
            .route(
                "/{announcement_id}/author-avatar",
                web::get().to(author_avatar),
            )
            .route(
                "/{announcement_id}/reactions/{emoji}",
                web::put().to(add_reaction),
            )
            .route(
                "/{announcement_id}/reactions/{emoji}",
                web::delete().to(remove_reaction),
            ),
    );
}

async fn list_announcements(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<Vec<Announcement>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(
        db::announcement_queries::list_announcements(&state.pool, &account.id).await?,
    ))
}

async fn create_announcement(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<AnnouncementPayload>,
) -> Result<(web::Json<Announcement>, actix_web::http::StatusCode), ApiError> {
    let account = authenticated_administrator(&state, &request).await?;
    let draft = announcement_draft(&payload)?;
    let announcement =
        db::announcement_queries::create_announcement(&state.pool, &account.id, &draft).await?;
    Ok((
        web::Json(announcement),
        actix_web::http::StatusCode::CREATED,
    ))
}

async fn update_announcement(
    state: web::Data<AppState>,
    request: HttpRequest,
    announcement_id: web::Path<String>,
    payload: web::Json<AnnouncementPayload>,
) -> Result<web::Json<Announcement>, ApiError> {
    let account = authenticated_administrator(&state, &request).await?;
    let draft = announcement_draft(&payload)?;
    db::announcement_queries::update_announcement(
        &state.pool,
        &account.id,
        &announcement_id,
        &draft,
    )
    .await?
    .map(web::Json)
    .ok_or(ApiError::NotFound("announcement not found"))
}

async fn delete_announcement(
    state: web::Data<AppState>,
    request: HttpRequest,
    announcement_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    if db::announcement_queries::delete_announcement(&state.pool, &announcement_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("announcement not found"))
    }
}

async fn create_image(
    state: web::Data<AppState>,
    request: HttpRequest,
    announcement_id: web::Path<String>,
    query: web::Query<AnnouncementImageQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    if body.is_empty() || body.len() > MAX_ANNOUNCEMENT_IMAGE_BYTES {
        return Err(ApiError::BadRequest(
            "announcement image must be between 1 byte and 10 MB",
        ));
    }
    let file_name = query
        .file_name
        .split(['/', '\\'])
        .next_back()
        .unwrap_or_default();
    let file_name = validate_short_text(file_name, "image name is required", 255)?;
    let mime_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .ok_or(ApiError::BadRequest("announcement image type is required"))?;
    validate_image_upload(mime_type, &body, "announcement image")?;
    db::announcement_queries::create_announcement_image(
        &state.pool,
        &announcement_id,
        file_name,
        mime_type,
        &body,
    )
    .await?
    .map(|image| HttpResponse::Created().json(image))
    .ok_or(ApiError::NotFound("announcement not found"))
}

async fn get_image(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    authenticated_account(&state, &request).await?;
    let (announcement_id, image_id) = path.into_inner();
    let (file_name, mime_type, data) =
        db::announcement_queries::get_announcement_image(&state.pool, &announcement_id, &image_id)
            .await?
            .ok_or(ApiError::NotFound("announcement image not found"))?;
    let safe_name = file_name.replace(['"', '\r', '\n'], "_");
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime_type))
        .append_header((header::CACHE_CONTROL, "private, no-cache"))
        .append_header(("x-content-type-options", "nosniff"))
        .append_header((
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{safe_name}\""),
        ))
        .body(data))
}

async fn author_avatar(
    state: web::Data<AppState>,
    request: HttpRequest,
    announcement_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    authenticated_account(&state, &request).await?;
    let avatar =
        db::announcement_queries::get_announcement_author_avatar(&state.pool, &announcement_id)
            .await?
            .ok_or(ApiError::NotFound("announcement author avatar not found"))?;
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, avatar.mime_type))
        .insert_header((header::CACHE_CONTROL, "private, no-cache"))
        .insert_header(("x-content-type-options", "nosniff"))
        .insert_header((header::ETAG, format!("\"{}\"", avatar.updated_at)))
        .body(avatar.image_data))
}

async fn delete_image(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    authenticated_administrator(&state, &request).await?;
    let (announcement_id, image_id) = path.into_inner();
    if db::announcement_queries::delete_announcement_image(&state.pool, &announcement_id, &image_id)
        .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("announcement image not found"))
    }
}

async fn add_reaction(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<web::Json<Announcement>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (announcement_id, emoji) = path.into_inner();
    validate_reaction(&emoji)?;
    db::announcement_queries::add_announcement_reaction(
        &state.pool,
        &account.id,
        &announcement_id,
        &emoji,
    )
    .await?;
    db::announcement_queries::get_announcement(&state.pool, &account.id, &announcement_id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("announcement not found"))
}

async fn remove_reaction(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<web::Json<Announcement>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (announcement_id, emoji) = path.into_inner();
    validate_reaction(&emoji)?;
    db::announcement_queries::remove_announcement_reaction(
        &state.pool,
        &account.id,
        &announcement_id,
        &emoji,
    )
    .await?;
    db::announcement_queries::get_announcement(&state.pool, &account.id, &announcement_id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("announcement not found"))
}

fn announcement_draft(payload: &AnnouncementPayload) -> Result<AnnouncementDraft, ApiError> {
    let title = validate_short_text(&payload.title, "announcement title is required", 160)?;
    let content = payload.content.trim_end();
    if content.trim().is_empty() {
        return Err(ApiError::BadRequest("announcement content is required"));
    }
    if content.chars().count() > MAX_ANNOUNCEMENT_CONTENT_CHARS {
        return Err(ApiError::BadRequest(
            "announcement content must be 50000 characters or fewer",
        ));
    }
    Ok(AnnouncementDraft {
        title: title.to_owned(),
        content: content.to_owned(),
    })
}

fn validate_reaction(emoji: &str) -> Result<(), ApiError> {
    if REACTION_EMOJIS.contains(&emoji) {
        Ok(())
    } else {
        Err(ApiError::BadRequest("reaction is invalid"))
    }
}

#[cfg(test)]
mod tests {
    use super::{AnnouncementPayload, announcement_draft, validate_reaction};

    #[test]
    fn announcement_payload_requires_title_and_content() {
        assert!(
            announcement_draft(&AnnouncementPayload {
                title: "Maintenance window".to_owned(),
                content: "**Sunday:** services restart at 03:00 UTC.".to_owned(),
            })
            .is_ok()
        );
        assert!(
            announcement_draft(&AnnouncementPayload {
                title: " ".to_owned(),
                content: "Body".to_owned(),
            })
            .is_err()
        );
    }

    #[test]
    fn reactions_use_the_fixed_picker_catalogue() {
        assert!(validate_reaction("🎉").is_ok());
        assert!(validate_reaction("🚀").is_err());
    }
}
