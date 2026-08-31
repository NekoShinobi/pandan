use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use db::entities::{CodingCategory, CodingProjectCategoryAssignment};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::{ApiError, AppState, authenticated_account};

const MAX_CODING_CATEGORIES: usize = 32;
const MAX_CODING_CATEGORY_NAME_CHARS: usize = 48;

#[derive(Debug, Deserialize)]
struct CategoryInput {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ProjectCategoriesInput {
    category_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CodingCategoryState {
    categories: Vec<CodingCategory>,
    assignments: Vec<CodingProjectCategoryAssignment>,
}

#[derive(Debug, Serialize)]
struct CodingProjectCategoryUpdate {
    project_id: String,
    category_ids: Vec<String>,
}

pub(crate) fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/coding/categories", web::get().to(list_categories))
        .route("/coding/categories", web::post().to(create_category))
        .route(
            "/coding/categories/{category_id}",
            web::patch().to(update_category),
        )
        .route(
            "/coding/categories/{category_id}",
            web::delete().to(delete_category),
        )
        .route(
            "/coding/projects/{project_id}/categories",
            web::put().to(update_project_categories),
        );
}

async fn list_categories(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<CodingCategoryState>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (categories, assignments) = tokio::try_join!(
        db::queries::list_coding_categories(&state.pool, &account.id),
        db::queries::list_coding_project_category_assignments(&state.pool, &account.id),
    )?;
    Ok(web::Json(CodingCategoryState {
        categories,
        assignments,
    }))
}

async fn create_category(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CategoryInput>,
) -> Result<(web::Json<CodingCategory>, StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::list_coding_categories(&state.pool, &account.id)
        .await?
        .len()
        >= MAX_CODING_CATEGORIES
    {
        return Err(ApiError::BadRequest(
            "a Coding workspace can contain at most 32 categories",
        ));
    }
    let name = validate_category_name(&payload.name)?;
    let category = db::queries::create_coding_category(&state.pool, &account.id, &name)
        .await
        .map_err(map_unique_category)?;
    Ok((web::Json(category), StatusCode::CREATED))
}

async fn update_category(
    state: web::Data<AppState>,
    request: HttpRequest,
    category_id: web::Path<String>,
    payload: web::Json<CategoryInput>,
) -> Result<web::Json<CodingCategory>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let name = validate_category_name(&payload.name)?;
    db::queries::update_coding_category(&state.pool, &account.id, &category_id, &name)
        .await
        .map_err(map_unique_category)?
        .map(web::Json)
        .ok_or(ApiError::NotFound("Coding category not found"))
}

async fn delete_category(
    state: web::Data<AppState>,
    request: HttpRequest,
    category_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::delete_coding_category(&state.pool, &account.id, &category_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Coding category not found"))
    }
}

async fn update_project_categories(
    state: web::Data<AppState>,
    request: HttpRequest,
    project_id: web::Path<String>,
    payload: web::Json<ProjectCategoriesInput>,
) -> Result<web::Json<CodingProjectCategoryUpdate>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if payload.category_ids.len() > MAX_CODING_CATEGORIES {
        return Err(ApiError::BadRequest(
            "a project can use at most 32 categories",
        ));
    }
    let mut seen = HashSet::new();
    let mut category_ids = Vec::with_capacity(payload.category_ids.len());
    for category_id in &payload.category_ids {
        let category_id = category_id.trim();
        if category_id.is_empty() {
            return Err(ApiError::BadRequest("category identifiers cannot be empty"));
        }
        if seen.insert(category_id.to_owned()) {
            category_ids.push(category_id.to_owned());
        }
    }
    if !db::queries::replace_coding_project_categories(
        &state.pool,
        &account.id,
        &project_id,
        &category_ids,
    )
    .await?
    {
        return Err(ApiError::NotFound("Coding project or category not found"));
    }
    Ok(web::Json(CodingProjectCategoryUpdate {
        project_id: project_id.into_inner(),
        category_ids,
    }))
}

fn validate_category_name(value: &str) -> Result<String, ApiError> {
    let name = value.trim();
    if name.is_empty()
        || name.chars().count() > MAX_CODING_CATEGORY_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(ApiError::BadRequest(
            "category name must contain 1 to 48 visible characters",
        ));
    }
    Ok(name.to_owned())
}

fn map_unique_category(error: sqlx::Error) -> ApiError {
    if error
        .as_database_error()
        .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
    {
        ApiError::Conflict("a Coding category with this name already exists")
    } else {
        ApiError::Database(error)
    }
}
