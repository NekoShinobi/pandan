use super::{ApiError, AppState, authenticated_account, validate_short_text};
use actix_web::{HttpRequest, HttpResponse, http::header, web};
use db::entities::{LineAuthorFeed, LinePost, LinePostDraft, LineThread};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_LINE_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;
const REACTION_EMOJIS: &[&str] = &["👍", "❤️", "😂", "🎉", "😕", "👀"];

#[derive(Debug, Deserialize)]
struct LineFeedQuery {
    #[serde(default = "default_scope")]
    scope: String,
    #[serde(default)]
    q: String,
    #[serde(default)]
    tag: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct CreateLinePostRequest {
    content: String,
    visibility: String,
    #[serde(default)]
    reply_to_post_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LineAttachmentQuery {
    file_name: String,
}

fn default_scope() -> String {
    "instance".to_owned()
}

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/lines")
            .service(
                web::resource("/posts")
                    .route(web::get().to(list_posts))
                    .route(web::post().to(create_post)),
            )
            .route("/posts/{post_id}", web::get().to(get_post))
            .route("/posts/{post_id}", web::delete().to(delete_post))
            .service(
                web::resource("/posts/{post_id}/attachments")
                    .app_data(web::PayloadConfig::new(MAX_LINE_ATTACHMENT_BYTES))
                    .route(web::post().to(create_attachment)),
            )
            .route(
                "/posts/{post_id}/attachments/{attachment_id}",
                web::get().to(get_attachment),
            )
            .route(
                "/posts/{post_id}/attachments/{attachment_id}",
                web::delete().to(delete_attachment),
            )
            .route(
                "/posts/{post_id}/reactions/{emoji}",
                web::put().to(add_reaction),
            )
            .route(
                "/posts/{post_id}/reactions/{emoji}",
                web::delete().to(remove_reaction),
            )
            .route("/posts/{post_id}/thread", web::get().to(get_thread))
            .route("/authors/{user_id}", web::get().to(get_author_feed))
            .route("/authors/{user_id}/avatar", web::get().to(author_avatar)),
    );
}

async fn list_posts(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<LineFeedQuery>,
) -> Result<web::Json<Vec<LinePost>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !matches!(query.scope.as_str(), "instance" | "mine") {
        return Err(ApiError::BadRequest("Lines feed scope is invalid"));
    }
    let search = query.q.trim();
    if search.chars().count() > 100 {
        return Err(ApiError::BadRequest(
            "Lines search must be 100 characters or fewer",
        ));
    }
    let tag = query.tag.trim().trim_start_matches('#');
    if tag.chars().count() > 64 {
        return Err(ApiError::BadRequest(
            "Lines hashtag must be 64 characters or fewer",
        ));
    }
    Ok(web::Json(
        db::queries::list_line_posts(&state.pool, &account.id, &query.scope, search, tag).await?,
    ))
}

async fn get_thread(
    state: web::Data<AppState>,
    request: HttpRequest,
    post_id: web::Path<String>,
) -> Result<web::Json<LineThread>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let post = db::queries::get_line_post(&state.pool, &account.id, &post_id)
        .await?
        .ok_or(ApiError::NotFound("post not found"))?;
    let parent = match post.reply_to_post_id.as_deref() {
        Some(parent_id) => db::queries::get_line_post(&state.pool, &account.id, parent_id).await?,
        None => None,
    };
    let replies = db::queries::list_line_post_replies(&state.pool, &account.id, &post.id).await?;
    Ok(web::Json(LineThread {
        parent,
        post,
        replies,
    }))
}

async fn get_author_feed(
    state: web::Data<AppState>,
    request: HttpRequest,
    user_id: web::Path<String>,
) -> Result<web::Json<LineAuthorFeed>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let author = db::queries::find_line_author_profile(&state.pool, &account.id, &user_id)
        .await?
        .ok_or(ApiError::NotFound("author not found"))?;
    let posts = db::queries::list_line_posts_by_author(&state.pool, &account.id, &user_id).await?;
    Ok(web::Json(LineAuthorFeed { author, posts }))
}

async fn author_avatar(
    state: web::Data<AppState>,
    request: HttpRequest,
    user_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !db::queries::line_author_is_visible(&state.pool, &account.id, &user_id).await? {
        return Err(ApiError::NotFound("author not found"));
    }
    let avatar = db::queries::find_user_avatar(&state.pool, &user_id)
        .await?
        .ok_or(ApiError::NotFound("avatar image not found"))?;
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, avatar.mime_type))
        .insert_header((header::CACHE_CONTROL, "private, no-cache"))
        .insert_header((header::ETAG, format!("\"{}\"", avatar.updated_at)))
        .body(avatar.image_data))
}

async fn get_post(
    state: web::Data<AppState>,
    request: HttpRequest,
    post_id: web::Path<String>,
) -> Result<web::Json<LinePost>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::queries::get_line_post(&state.pool, &account.id, &post_id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("post not found"))
}

async fn create_post(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateLinePostRequest>,
) -> Result<(web::Json<LinePost>, actix_web::http::StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let mut content = payload.content.trim_end().to_owned();
    if content.trim().is_empty() {
        return Err(ApiError::BadRequest("post content is required"));
    }
    if content.chars().count() > 2000 {
        return Err(ApiError::BadRequest(
            "post content must be 2000 characters or fewer",
        ));
    }
    if !matches!(payload.visibility.as_str(), "private" | "public") {
        return Err(ApiError::BadRequest("post visibility is invalid"));
    }
    let mut visibility = payload.visibility.clone();
    let reply_to_post_id = payload
        .reply_to_post_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned);
    if let Some(parent_id) = &reply_to_post_id {
        let parent = db::queries::get_line_post(&state.pool, &account.id, parent_id)
            .await?
            .ok_or(ApiError::NotFound("post not found"))?;
        if parent.visibility == "private" {
            visibility = "private".to_owned();
        }
    }
    content.shrink_to_fit();
    let draft = LinePostDraft {
        tags: extract_hashtags(&content),
        content,
        visibility,
        reply_to_post_id,
    };
    let post = db::queries::create_line_post(&state.pool, &account.id, &draft).await?;
    Ok((web::Json(post), actix_web::http::StatusCode::CREATED))
}

async fn delete_post(
    state: web::Data<AppState>,
    request: HttpRequest,
    post_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_line_post(
        &state.pool,
        &account.id,
        &post_id,
        account.role == "administrator",
    )
    .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("post not found"))
    }
}

async fn create_attachment(
    state: web::Data<AppState>,
    request: HttpRequest,
    post_id: web::Path<String>,
    query: web::Query<LineAttachmentQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if body.is_empty() || body.len() > MAX_LINE_ATTACHMENT_BYTES {
        return Err(ApiError::BadRequest(
            "attachment must be between 1 byte and 10 MB",
        ));
    }
    let file_name = query
        .file_name
        .split(['/', '\\'])
        .next_back()
        .unwrap_or_default();
    let file_name = validate_short_text(file_name, "attachment name is required", 255)?;
    let mime_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream");
    if mime_type.len() > 120 || mime_type.contains(['\r', '\n']) {
        return Err(ApiError::BadRequest("attachment type is invalid"));
    }
    db::queries::create_line_post_attachment(
        &state.pool,
        &account.id,
        &post_id,
        file_name,
        mime_type,
        &body,
    )
    .await?
    .map(|attachment| HttpResponse::Created().json(attachment))
    .ok_or(ApiError::NotFound("post not found"))
}

async fn get_attachment(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (post_id, attachment_id) = path.into_inner();
    let (file_name, mime_type, data) =
        db::queries::get_line_post_attachment(&state.pool, &account.id, &post_id, &attachment_id)
            .await?
            .ok_or(ApiError::NotFound("attachment not found"))?;
    let safe_name = file_name.replace(['"', '\r', '\n'], "_");
    let disposition = if matches!(
        mime_type.as_str(),
        "image/jpeg" | "image/png" | "image/webp" | "image/avif"
    ) {
        "inline"
    } else {
        "attachment"
    };
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime_type))
        .append_header(("x-content-type-options", "nosniff"))
        .append_header((
            header::CONTENT_DISPOSITION,
            format!("{disposition}; filename=\"{safe_name}\""),
        ))
        .body(data))
}

async fn delete_attachment(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (post_id, attachment_id) = path.into_inner();
    if db::queries::delete_line_post_attachment(&state.pool, &account.id, &post_id, &attachment_id)
        .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("attachment not found"))
    }
}

async fn add_reaction(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<web::Json<LinePost>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (post_id, emoji) = path.into_inner();
    validate_reaction(&emoji)?;
    db::queries::add_line_post_reaction(&state.pool, &account.id, &post_id, &emoji).await?;
    db::queries::get_line_post(&state.pool, &account.id, &post_id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("post not found"))
}

async fn remove_reaction(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<web::Json<LinePost>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (post_id, emoji) = path.into_inner();
    validate_reaction(&emoji)?;
    db::queries::remove_line_post_reaction(&state.pool, &account.id, &post_id, &emoji).await?;
    db::queries::get_line_post(&state.pool, &account.id, &post_id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("post not found"))
}

fn validate_reaction(emoji: &str) -> Result<(), ApiError> {
    if REACTION_EMOJIS.contains(&emoji) {
        Ok(())
    } else {
        Err(ApiError::BadRequest("reaction is invalid"))
    }
}

fn extract_hashtags(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut seen = HashSet::new();
    let mut fenced = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut index = 0;
        let mut inline_code = false;
        while index < chars.len() {
            if chars[index] == '`' {
                inline_code = !inline_code;
                index += 1;
                continue;
            }
            if chars[index] != '#' || inline_code {
                index += 1;
                continue;
            }
            let valid_boundary =
                index == 0 || (!chars[index - 1].is_alphanumeric() && chars[index - 1] != '_');
            if !valid_boundary {
                index += 1;
                continue;
            }
            let start = index + 1;
            let mut end = start;
            while end < chars.len()
                && (chars[end].is_alphanumeric() || matches!(chars[end], '_' | '-'))
            {
                end += 1;
            }
            if end > start {
                let tag: String = chars[start..end].iter().collect::<String>().to_lowercase();
                if tag.chars().count() <= 64 && seen.insert(tag.clone()) {
                    tags.push(tag);
                }
            }
            index = end.max(index + 1);
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::extract_hashtags;

    #[test]
    fn hashtags_ignore_code_and_deduplicate_case() {
        let tags = extract_hashtags("#Rust and #rust `#inline`\n```\n#fenced\n```\n#svelte-5");
        assert_eq!(tags, vec!["rust", "svelte-5"]);
    }
}
