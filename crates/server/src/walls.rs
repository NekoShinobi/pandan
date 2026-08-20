use super::{
    ApiError, AppState, MAX_WALLPAPER_BYTES, SessionAccount, authenticated_account,
    authenticated_administrator, validate_image_upload, validate_short_text,
};
use actix_web::{HttpRequest, HttpResponse, http::header, web};
use db::entities::{Wall, WallDraft};
use image::ImageReader;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

/// Longest edge of a generated gallery thumbnail, in pixels.
const THUMBNAIL_EDGE: u32 = 640;

/// JPEG quality for generated thumbnails.
const THUMBNAIL_QUALITY: u8 = 80;

/// Ceiling on a decoded submission, in bytes of pixel buffer.
///
/// The 30 MB upload limit bounds the *compressed* image only. A small file can legitimately
/// decode to many gigabytes of RGBA, so decoding is capped independently.
const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;

/// Ceiling on either edge of a decoded submission, in pixels.
const MAX_DECODED_EDGE: u32 = 16_384;

/// The most tags one submission may carry.
const MAX_WALL_TAGS: usize = 8;

#[derive(Debug, Deserialize)]
struct WallListQuery {
    #[serde(default)]
    scope: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    q: String,
    #[serde(default)]
    tag: String,
}

#[derive(Debug, Deserialize)]
struct SubmitWallQuery {
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tags: String,
}

#[derive(Debug, Deserialize)]
struct UpdateWallPayload {
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DecisionPayload {
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize)]
struct ApplyWallPayload {
    slot: String,
}

#[derive(Debug, Serialize)]
struct WallSelections {
    welcome: Option<String>,
    login: Option<String>,
}

/// A decoded submission, ready to store.
struct PreparedImage {
    width: i64,
    height: i64,
    thumbnail: Vec<u8>,
}

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/walls")
            .service(
                web::resource("")
                    .app_data(web::PayloadConfig::new(MAX_WALLPAPER_BYTES))
                    .route(web::get().to(list_walls))
                    .route(web::post().to(submit_wall)),
            )
            .route("/selections", web::get().to(get_selections))
            .service(
                web::resource("/{wall_id}")
                    .route(web::get().to(get_wall))
                    .route(web::patch().to(update_wall))
                    .route(web::delete().to(delete_wall)),
            )
            .route("/{wall_id}/image", web::get().to(get_wall_image))
            .route("/{wall_id}/thumbnail", web::get().to(get_wall_thumbnail))
            .route("/{wall_id}/approve", web::post().to(approve_wall))
            .route("/{wall_id}/reject", web::post().to(reject_wall))
            .route("/{wall_id}/apply", web::put().to(apply_wall)),
    );
}

async fn list_walls(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<WallListQuery>,
) -> Result<web::Json<Vec<Wall>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let scope = if query.scope.is_empty() {
        "collection"
    } else {
        query.scope.as_str()
    };
    if !matches!(scope, "collection" | "mine" | "review") {
        return Err(ApiError::BadRequest("wall scope is invalid"));
    }

    // The filter bar is page-level rather than per-view, so every scope validates and
    // applies the same search and tag, including the submitter's own list.
    let search = query.q.trim();
    if search.chars().count() > 100 {
        return Err(ApiError::BadRequest(
            "wall search must be 100 characters or fewer",
        ));
    }
    let tag = query.tag.trim().trim_start_matches('#');
    if tag.chars().count() > 32 {
        return Err(ApiError::BadRequest(
            "wall tag must be 32 characters or fewer",
        ));
    }

    if scope == "mine" {
        return Ok(web::Json(
            db::wall_queries::list_walls_by_submitter(&state.pool, &account.id, search, tag)
                .await?,
        ));
    }

    let is_administrator = account.role == "administrator";
    if scope == "review" && !is_administrator {
        return Err(ApiError::Forbidden);
    }
    let status = query.status.trim();
    if !matches!(status, "" | "pending" | "approved" | "rejected") {
        return Err(ApiError::BadRequest("wall status is invalid"));
    }
    // The review queue is the pending backlog; the collection is the approved set unless
    // the caller asked for something narrower.
    let status = match (scope, status) {
        ("review", "") => "pending",
        ("collection", "") => "approved",
        (_, value) => value,
    };
    Ok(web::Json(
        db::wall_queries::list_walls(
            &state.pool,
            &account.id,
            is_administrator,
            status,
            search,
            tag,
        )
        .await?,
    ))
}

async fn get_wall(
    state: web::Data<AppState>,
    request: HttpRequest,
    wall_id: web::Path<String>,
) -> Result<web::Json<Wall>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let wall = visible_wall(&state, &account, &wall_id).await?;
    Ok(web::Json(wall))
}

async fn submit_wall(
    state: web::Data<AppState>,
    request: HttpRequest,
    query: web::Query<SubmitWallQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if body.is_empty() || body.len() > MAX_WALLPAPER_BYTES {
        return Err(ApiError::BadRequest(
            "wall image must be between 1 byte and 30 MB",
        ));
    }
    let title = validate_short_text(&query.title, "wall title is required", 120)?.to_owned();
    let description = query.description.trim();
    if description.chars().count() > 500 {
        return Err(ApiError::BadRequest("wall description is too long"));
    }
    let tags = parse_tags(&query.tags)?;
    let mime_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .ok_or(ApiError::BadRequest("wall image type is required"))?;
    validate_image_upload(mime_type, &body, "wall")?;

    let prepared = prepare_image(body.to_vec()).await?;
    let wall = db::wall_queries::create_wall(
        &state.pool,
        &WallDraft {
            user_id: account.id,
            title,
            description: description.to_owned(),
            tags,
            mime_type: mime_type.to_owned(),
            width: prepared.width,
            height: prepared.height,
            image_data: body.to_vec(),
            thumbnail_data: prepared.thumbnail,
        },
    )
    .await?;
    Ok(HttpResponse::Created().json(wall))
}

async fn update_wall(
    state: web::Data<AppState>,
    request: HttpRequest,
    wall_id: web::Path<String>,
    payload: web::Json<UpdateWallPayload>,
) -> Result<web::Json<Wall>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let wall = visible_wall(&state, &account, &wall_id).await?;
    // The submitter and administrators may correct a wall's description at any status.
    // Review state is untouched by this: only `decide` moves a wall between statuses.
    if account.role != "administrator" && wall.user_id.as_deref() != Some(account.id.as_str()) {
        return Err(ApiError::Forbidden);
    }
    let title = validate_short_text(&payload.title, "wall title is required", 120)?;
    let description = payload.description.trim();
    if description.chars().count() > 500 {
        return Err(ApiError::BadRequest("wall description is too long"));
    }
    let tags = normalize_tags(payload.tags.iter().map(String::as_str))?;

    if !db::wall_queries::update_wall_details(&state.pool, &wall.id, title, description, &tags)
        .await?
    {
        return Err(ApiError::NotFound("wall not found"));
    }
    db::wall_queries::get_wall(&state.pool, &wall.id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("wall not found"))
}

async fn approve_wall(
    state: web::Data<AppState>,
    request: HttpRequest,
    wall_id: web::Path<String>,
    payload: web::Json<DecisionPayload>,
) -> Result<web::Json<Wall>, ApiError> {
    decide(
        state,
        request,
        wall_id.into_inner(),
        "approved",
        &payload.note,
    )
    .await
}

async fn reject_wall(
    state: web::Data<AppState>,
    request: HttpRequest,
    wall_id: web::Path<String>,
    payload: web::Json<DecisionPayload>,
) -> Result<web::Json<Wall>, ApiError> {
    decide(
        state,
        request,
        wall_id.into_inner(),
        "rejected",
        &payload.note,
    )
    .await
}

async fn decide(
    state: web::Data<AppState>,
    request: HttpRequest,
    wall_id: String,
    status: &str,
    note: &str,
) -> Result<web::Json<Wall>, ApiError> {
    let administrator = authenticated_administrator(&state, &request).await?;
    let note = note.trim();
    if note.chars().count() > 500 {
        return Err(ApiError::BadRequest("decision note is too long"));
    }
    let wall = db::wall_queries::get_wall(&state.pool, &wall_id)
        .await?
        .ok_or(ApiError::NotFound("wall not found"))?;
    if wall.status != "pending" {
        return Err(ApiError::Conflict("this wall has already been reviewed"));
    }
    if !db::wall_queries::decide_wall(&state.pool, &wall_id, status, &administrator.id, note)
        .await?
    {
        return Err(ApiError::Conflict("this wall has already been reviewed"));
    }
    db::wall_queries::get_wall(&state.pool, &wall_id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("wall not found"))
}

async fn delete_wall(
    state: web::Data<AppState>,
    request: HttpRequest,
    wall_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let wall = visible_wall(&state, &account, &wall_id).await?;
    if account.role != "administrator" && wall.user_id.as_deref() != Some(account.id.as_str()) {
        return Err(ApiError::Forbidden);
    }
    if db::wall_queries::delete_wall(&state.pool, &wall.id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("wall not found"))
    }
}

async fn get_wall_image(
    state: web::Data<AppState>,
    request: HttpRequest,
    wall_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let wall = visible_wall(&state, &account, &wall_id).await?;
    let image = db::wall_queries::find_wall_image(&state.pool, &wall.id)
        .await?
        .ok_or(ApiError::NotFound("wall image not found"))?;
    Ok(image_response(
        &image.mime_type,
        &image.updated_at,
        image.image_data,
    ))
}

async fn get_wall_thumbnail(
    state: web::Data<AppState>,
    request: HttpRequest,
    wall_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let wall = visible_wall(&state, &account, &wall_id).await?;
    let image = db::wall_queries::find_wall_thumbnail(&state.pool, &wall.id)
        .await?
        .ok_or(ApiError::NotFound("wall image not found"))?;
    Ok(image_response(
        &image.mime_type,
        &image.updated_at,
        image.image_data,
    ))
}

async fn apply_wall(
    state: web::Data<AppState>,
    request: HttpRequest,
    wall_id: web::Path<String>,
    payload: web::Json<ApplyWallPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !matches!(payload.slot.as_str(), "welcome" | "login") {
        return Err(ApiError::BadRequest("wallpaper slot is invalid"));
    }
    if payload.slot == "login" && account.role != "administrator" {
        return Err(ApiError::Forbidden);
    }
    let wall = db::wall_queries::get_wall(&state.pool, &wall_id)
        .await?
        .ok_or(ApiError::NotFound("wall not found"))?;
    // Only the approved collection can be applied, including by the submitter and by
    // administrators: a pending image has not passed review yet.
    if wall.status != "approved" {
        return Err(ApiError::Conflict(
            "only an approved wall can be used as a wallpaper",
        ));
    }

    if payload.slot == "login" {
        db::wall_queries::apply_wall_to_login(&state.pool, &account.id, &wall.id).await?;
    } else {
        db::wall_queries::apply_wall_to_slot(&state.pool, &account.id, &payload.slot, &wall.id)
            .await?;
    }
    Ok(HttpResponse::NoContent().finish())
}

async fn get_selections(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<WallSelections>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let selections = db::wall_queries::list_wall_selections(&state.pool, &account.id).await?;
    Ok(web::Json(WallSelections {
        welcome: selections
            .iter()
            .find(|(slot, _)| slot == "welcome")
            .map(|(_, wall_id)| wall_id.clone()),
        login: db::wall_queries::find_login_wall_selection(&state.pool).await?,
    }))
}

/// Loads a wall only when the caller is allowed to see it.
///
/// Approved walls are visible to every authenticated account. Pending and rejected walls
/// are visible to their submitter and to administrators, and are reported as missing to
/// everyone else so review state never leaks.
async fn visible_wall(
    state: &AppState,
    account: &SessionAccount,
    wall_id: &str,
) -> Result<Wall, ApiError> {
    let wall = db::wall_queries::get_wall(&state.pool, wall_id)
        .await?
        .ok_or(ApiError::NotFound("wall not found"))?;
    if wall.status == "approved"
        || account.role == "administrator"
        || wall.user_id.as_deref() == Some(account.id.as_str())
    {
        Ok(wall)
    } else {
        Err(ApiError::NotFound("wall not found"))
    }
}

fn image_response(mime_type: &str, updated_at: &str, data: Vec<u8>) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, mime_type.to_owned()))
        .insert_header((header::CACHE_CONTROL, "private, no-cache"))
        .insert_header((header::ETAG, format!("\"{updated_at}\"")))
        .body(data)
}

fn parse_tags(raw: &str) -> Result<Vec<String>, ApiError> {
    normalize_tags(raw.split(','))
}

fn normalize_tags<'a>(values: impl Iterator<Item = &'a str>) -> Result<Vec<String>, ApiError> {
    let mut tags: Vec<String> = Vec::new();
    for value in values {
        let tag = value.trim().trim_start_matches('#').trim();
        if tag.is_empty() {
            continue;
        }
        if tag.chars().count() > 32 {
            return Err(ApiError::BadRequest(
                "wall tag must be 32 characters or fewer",
            ));
        }
        if !tags
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(tag))
        {
            tags.push(tag.to_owned());
        }
    }
    if tags.len() > MAX_WALL_TAGS {
        return Err(ApiError::BadRequest("a wall may carry at most eight tags"));
    }
    Ok(tags)
}

/// Decodes a submission and renders its gallery thumbnail.
///
/// Decoding and resizing are CPU-bound and run on the blocking pool: a large AVIF or PNG
/// would otherwise stall an Actix worker for the duration.
async fn prepare_image(image_data: Vec<u8>) -> Result<PreparedImage, ApiError> {
    web::block(move || {
        let mut reader = ImageReader::new(Cursor::new(&image_data))
            .with_guessed_format()
            .map_err(|_| ApiError::BadRequest("wall image could not be read"))?;

        // Bound the decode itself. Without this a small, valid file can expand into
        // gigabytes of pixel buffer before any of our own checks run.
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(MAX_DECODED_BYTES);
        limits.max_image_width = Some(MAX_DECODED_EDGE);
        limits.max_image_height = Some(MAX_DECODED_EDGE);
        reader.limits(limits);

        let decoded = reader
            .decode()
            .map_err(|_| ApiError::BadRequest("wall image could not be decoded"))?;
        let width = i64::from(decoded.width());
        let height = i64::from(decoded.height());

        // `thumbnail` scales to fit in both directions, which means it also scales *up*.
        // A submission already smaller than the target is kept at its own size rather than
        // being inflated into a thumbnail larger than the original.
        let thumbnail = if decoded.width() > THUMBNAIL_EDGE || decoded.height() > THUMBNAIL_EDGE {
            decoded.thumbnail(THUMBNAIL_EDGE, THUMBNAIL_EDGE).to_rgb8()
        } else {
            decoded.to_rgb8()
        };
        let mut buffer = Cursor::new(Vec::new());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, THUMBNAIL_QUALITY)
            .encode_image(&thumbnail)
            .map_err(|_| ApiError::Internal("wall thumbnail could not be encoded"))?;

        Ok(PreparedImage {
            width,
            height,
            thumbnail: buffer.into_inner(),
        })
    })
    .await
    .map_err(|_| ApiError::Internal("wall image could not be processed"))?
}
