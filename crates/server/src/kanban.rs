use super::{ApiError, AppState, authenticated_account, validate_short_text};
use actix_web::{HttpRequest, HttpResponse, http::header, web};
use chrono::NaiveDate;
use db::entities::{KanbanBoard, KanbanBoardSummary, KanbanCard, KanbanCardDraft};
use serde::Deserialize;

const MAX_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;
const PERMISSIONS: &[&str] = &[
    "workspace:view",
    "workspace:edit",
    "workspace:delete",
    "workspace:manage",
    "board:view",
    "board:create",
    "board:edit",
    "board:delete",
    "list:view",
    "list:create",
    "list:edit",
    "list:delete",
    "card:view",
    "card:create",
    "card:edit",
    "card:delete",
    "comment:view",
    "comment:create",
    "comment:edit",
    "comment:delete",
    "member:view",
    "member:invite",
    "member:edit",
    "member:remove",
];

#[derive(Deserialize)]
struct WorkspacePayload {
    name: String,
    #[serde(default)]
    description: String,
}
#[derive(Deserialize)]
struct BoardPayload {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_visibility")]
    visibility: String,
    #[serde(default)]
    archived: bool,
}
#[derive(Deserialize)]
struct ArchivedQuery {
    #[serde(default)]
    archived: bool,
}
#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}
#[derive(Deserialize)]
struct InvitePayload {
    user_id: String,
    role: String,
}
#[derive(Deserialize)]
struct InvitationPayload {
    accept: bool,
}
#[derive(Deserialize)]
struct RolePayload {
    role: String,
}
#[derive(Deserialize)]
struct GrantPayload {
    granted: bool,
}
#[derive(Deserialize)]
struct FavoritePayload {
    favorite: bool,
}
#[derive(Deserialize)]
struct NamePayload {
    name: String,
}
#[derive(Deserialize)]
struct ColumnPayload {
    name: Option<String>,
    position: Option<i64>,
}
#[derive(Deserialize)]
struct CardPayload {
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    due_date: Option<String>,
    #[serde(default)]
    assignee_ids: Vec<String>,
    #[serde(default)]
    label_ids: Vec<String>,
}
#[derive(Deserialize)]
struct MoveCardPayload {
    column_id: String,
    position: i64,
}
#[derive(Deserialize)]
struct LabelPayload {
    name: String,
    color: String,
}
#[derive(Deserialize)]
struct ContentPayload {
    content: String,
}
#[derive(Deserialize)]
struct ChecklistItemPayload {
    title: String,
    #[serde(default)]
    completed: bool,
}
#[derive(Deserialize)]
struct AttachmentQuery {
    file_name: String,
}

fn default_visibility() -> String {
    "private".to_owned()
}

fn valid_label_color(value: &str) -> bool {
    matches!(
        value,
        "accent" | "blue" | "amber" | "red" | "violet" | "gray"
    ) || value
        .strip_prefix('#')
        .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/kanban")
            .route("", web::get().to(overview))
            .route("/workspaces", web::post().to(create_workspace))
            .route(
                "/workspaces/{workspace_id}",
                web::put().to(update_workspace),
            )
            .route(
                "/workspaces/{workspace_id}",
                web::delete().to(delete_workspace),
            )
            .route(
                "/workspaces/{workspace_id}/settings",
                web::get().to(workspace_settings),
            )
            .route(
                "/workspaces/{workspace_id}/directory",
                web::get().to(search_directory),
            )
            .route(
                "/workspaces/{workspace_id}/members",
                web::post().to(invite_member),
            )
            .route(
                "/workspaces/{workspace_id}/members/{user_id}/avatar",
                web::get().to(member_avatar),
            )
            .route(
                "/workspaces/{workspace_id}/invitations",
                web::put().to(respond_invitation),
            )
            .route(
                "/workspaces/{workspace_id}/members/{user_id}",
                web::put().to(update_member),
            )
            .route(
                "/workspaces/{workspace_id}/members/{user_id}",
                web::delete().to(remove_member),
            )
            .route(
                "/workspaces/{workspace_id}/roles/{role}/permissions/{permission}",
                web::put().to(set_role_permission),
            )
            .route(
                "/workspaces/{workspace_id}/members/{user_id}/permissions/{permission}",
                web::put().to(set_member_permission),
            )
            .route(
                "/workspaces/{workspace_id}/members/{user_id}/permissions",
                web::delete().to(reset_member_permissions),
            )
            .route(
                "/workspaces/{workspace_id}/boards",
                web::get().to(list_boards),
            )
            .route(
                "/workspaces/{workspace_id}/boards",
                web::post().to(create_board),
            )
            .route("/boards/{board_id}", web::get().to(get_board))
            .route("/boards/{board_id}", web::put().to(update_board))
            .route("/boards/{board_id}", web::delete().to(delete_board))
            .route(
                "/boards/{board_id}/favorite",
                web::put().to(set_board_favorite),
            )
            .route("/boards/{board_id}/columns", web::post().to(create_column))
            .route("/columns/{column_id}", web::put().to(update_column))
            .route("/columns/{column_id}", web::delete().to(delete_column))
            .route("/columns/{column_id}/cards", web::post().to(create_card))
            .route("/cards/{card_id}", web::get().to(get_card))
            .route("/cards/{card_id}", web::put().to(update_card))
            .route("/cards/{card_id}", web::delete().to(archive_card))
            .route("/cards/{card_id}/move", web::put().to(move_card))
            .route("/boards/{board_id}/labels", web::post().to(create_label))
            .route(
                "/boards/{board_id}/labels/{label_id}",
                web::delete().to(delete_label),
            )
            .route("/cards/{card_id}/comments", web::post().to(create_comment))
            .route("/comments/{comment_id}", web::put().to(update_comment))
            .route("/comments/{comment_id}", web::delete().to(delete_comment))
            .route(
                "/cards/{card_id}/checklists",
                web::post().to(create_checklist),
            )
            .route(
                "/checklists/{checklist_id}",
                web::delete().to(delete_checklist),
            )
            .route(
                "/checklists/{checklist_id}/items",
                web::post().to(create_checklist_item),
            )
            .route(
                "/checklists/{checklist_id}/items/{item_id}",
                web::put().to(update_checklist_item),
            )
            .service(
                web::resource("/cards/{card_id}/attachments")
                    .app_data(web::PayloadConfig::new(MAX_ATTACHMENT_BYTES))
                    .route(web::post().to(create_attachment)),
            )
            .route(
                "/attachments/{attachment_id}",
                web::get().to(get_attachment),
            )
            .route(
                "/attachments/{attachment_id}",
                web::delete().to(delete_attachment),
            ),
    );
}

async fn require_permission(
    state: &AppState,
    user_id: &str,
    workspace_id: &str,
    permission: &str,
) -> Result<String, ApiError> {
    let role = db::queries::kanban_workspace_role(&state.pool, workspace_id, user_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban workspace not found"))?;
    if !db::queries::kanban_has_permission(&state.pool, workspace_id, user_id, permission).await? {
        return Err(ApiError::AccessDenied(
            "this Kanban action is not permitted",
        ));
    }
    Ok(role)
}

async fn board_workspace(state: &AppState, board_id: &str) -> Result<String, ApiError> {
    db::queries::kanban_board_workspace_id(&state.pool, board_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban board not found"))
}
async fn column_workspace(state: &AppState, column_id: &str) -> Result<String, ApiError> {
    db::queries::kanban_column_workspace_id(&state.pool, column_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban column not found"))
}
async fn card_workspace(state: &AppState, card_id: &str) -> Result<String, ApiError> {
    db::queries::kanban_card_workspace_id(&state.pool, card_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban card not found"))
}

fn valid_role(role: &str) -> bool {
    matches!(role, "admin" | "member" | "guest")
}
fn role_level(role: &str) -> u8 {
    match role {
        "admin" => 100,
        "member" => 50,
        "guest" => 10,
        _ => 0,
    }
}
fn valid_permission(permission: &str) -> bool {
    PERMISSIONS.contains(&permission)
}
fn text(value: &str, required: &'static str, max: usize) -> Result<String, ApiError> {
    if value.trim().is_empty() && required.is_empty() {
        return Ok(String::new());
    }
    if required.is_empty() {
        if value.chars().count() > max {
            return Err(ApiError::BadRequest("text is too long"));
        }
        return Ok(value.trim().to_owned());
    }
    Ok(validate_short_text(value, required, max)?.to_owned())
}
fn card_draft(payload: &CardPayload) -> Result<KanbanCardDraft, ApiError> {
    let title = text(&payload.title, "card title is required", 240)?;
    if payload.description.chars().count() > 100_000 {
        return Err(ApiError::BadRequest(
            "card description must be 100000 characters or fewer",
        ));
    }
    let due_date = payload
        .due_date
        .as_deref()
        .map(str::trim)
        .filter(|date| !date.is_empty())
        .map(|date| {
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map(|_| date.to_owned())
                .map_err(|_| ApiError::BadRequest("due date must use YYYY-MM-DD"))
        })
        .transpose()?;
    if payload.assignee_ids.len() > 50 || payload.label_ids.len() > 50 {
        return Err(ApiError::BadRequest(
            "a card can have at most 50 assignees and labels",
        ));
    }
    Ok(KanbanCardDraft {
        title,
        description: payload.description.trim().to_owned(),
        due_date,
        assignee_ids: payload.assignee_ids.clone(),
        label_ids: payload.label_ids.clone(),
    })
}

async fn overview(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<db::entities::KanbanOverview>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    Ok(web::Json(
        db::queries::kanban_overview(&state.pool, &account.id).await?,
    ))
}
async fn create_workspace(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<WorkspacePayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let name = text(&payload.name, "workspace name is required", 80)?;
    let description = text(&payload.description, "", 1000)?;
    Ok(HttpResponse::Created().json(
        db::queries::create_kanban_workspace(&state.pool, &account.id, &name, &description).await?,
    ))
}
async fn update_workspace(
    state: web::Data<AppState>,
    request: HttpRequest,
    workspace_id: web::Path<String>,
    payload: web::Json<WorkspacePayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    require_permission(&state, &account.id, &workspace_id, "workspace:edit").await?;
    let name = text(&payload.name, "workspace name is required", 80)?;
    let description = text(&payload.description, "", 1000)?;
    if db::queries::update_kanban_workspace(&state.pool, &workspace_id, &name, &description).await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Kanban workspace not found"))
    }
}
async fn delete_workspace(
    state: web::Data<AppState>,
    request: HttpRequest,
    workspace_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    require_permission(&state, &account.id, &workspace_id, "workspace:delete").await?;
    if db::queries::delete_kanban_workspace(&state.pool, &workspace_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Kanban workspace not found"))
    }
}
async fn workspace_settings(
    state: web::Data<AppState>,
    request: HttpRequest,
    workspace_id: web::Path<String>,
) -> Result<web::Json<db::entities::KanbanWorkspaceSettings>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    require_permission(&state, &account.id, &workspace_id, "member:view").await?;
    db::queries::get_kanban_workspace_settings(&state.pool, &workspace_id, &account.id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("Kanban workspace not found"))
}

async fn member_avatar(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, user_id) = path.into_inner();
    require_permission(&state, &account.id, &workspace_id, "member:view").await?;
    if !db::queries::is_kanban_workspace_member(&state.pool, &workspace_id, &user_id).await? {
        return Err(ApiError::NotFound("workspace member not found"));
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

async fn search_directory(
    state: web::Data<AppState>,
    request: HttpRequest,
    workspace_id: web::Path<String>,
    query: web::Query<SearchQuery>,
) -> Result<web::Json<Vec<db::entities::KanbanDirectoryUser>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    require_permission(&state, &account.id, &workspace_id, "member:invite").await?;
    if query.q.chars().count() > 120 {
        return Err(ApiError::BadRequest(
            "member search must be 120 characters or fewer",
        ));
    }
    Ok(web::Json(
        db::queries::search_kanban_directory(&state.pool, &workspace_id, query.q.trim()).await?,
    ))
}
async fn invite_member(
    state: web::Data<AppState>,
    request: HttpRequest,
    workspace_id: web::Path<String>,
    payload: web::Json<InvitePayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let actor_role =
        require_permission(&state, &account.id, &workspace_id, "member:invite").await?;
    if !valid_role(&payload.role) || (payload.role == "admin" && actor_role != "admin") {
        return Err(ApiError::BadRequest("workspace role is invalid"));
    }
    if db::queries::invite_kanban_member(
        &state.pool,
        &workspace_id,
        &payload.user_id,
        &payload.role,
        &account.id,
    )
    .await?
    {
        Ok(HttpResponse::Created().finish())
    } else {
        Err(ApiError::Conflict(
            "user is unavailable or already belongs to this workspace",
        ))
    }
}
async fn respond_invitation(
    state: web::Data<AppState>,
    request: HttpRequest,
    workspace_id: web::Path<String>,
    payload: web::Json<InvitationPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::queries::respond_to_kanban_invitation(
        &state.pool,
        &workspace_id,
        &account.id,
        payload.accept,
    )
    .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Kanban invitation not found"))
    }
}
async fn update_member(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
    payload: web::Json<RolePayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, user_id) = path.into_inner();
    let actor_role = require_permission(&state, &account.id, &workspace_id, "member:edit").await?;
    let target_role = db::queries::kanban_workspace_role(&state.pool, &workspace_id, &user_id)
        .await?
        .ok_or(ApiError::NotFound("workspace member not found"))?;
    if !valid_role(&payload.role)
        || (actor_role != "admin"
            && (role_level(&actor_role) <= role_level(&target_role)
                || role_level(&actor_role) <= role_level(&payload.role)))
    {
        return Err(ApiError::AccessDenied(
            "you cannot assign or change this workspace role",
        ));
    }
    match db::queries::update_kanban_member_role(
        &state.pool,
        &workspace_id,
        &user_id,
        &payload.role,
    )
    .await?
    {
        Some(true) => Ok(HttpResponse::NoContent().finish()),
        Some(false) => Err(ApiError::Conflict(
            "the final workspace administrator cannot be demoted",
        )),
        None => Err(ApiError::NotFound("workspace member not found")),
    }
}
async fn remove_member(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, user_id) = path.into_inner();
    let actor_role =
        require_permission(&state, &account.id, &workspace_id, "member:remove").await?;
    let target_role = db::queries::kanban_workspace_role(&state.pool, &workspace_id, &user_id)
        .await?
        .ok_or(ApiError::NotFound("workspace member not found"))?;
    if actor_role != "admin" && role_level(&actor_role) <= role_level(&target_role) {
        return Err(ApiError::AccessDenied(
            "you cannot remove this workspace member",
        ));
    }
    match db::queries::remove_kanban_member(&state.pool, &workspace_id, &user_id).await? {
        Some(true) => Ok(HttpResponse::NoContent().finish()),
        Some(false) => Err(ApiError::Conflict(
            "the final workspace administrator cannot be removed",
        )),
        None => Err(ApiError::NotFound("workspace member not found")),
    }
}
async fn set_role_permission(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String, String)>,
    payload: web::Json<GrantPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, role, permission) = path.into_inner();
    require_permission(&state, &account.id, &workspace_id, "workspace:manage").await?;
    if !matches!(role.as_str(), "member" | "guest")
        || !valid_permission(&permission)
        || matches!(permission.as_str(), "workspace:manage" | "workspace:delete")
    {
        return Err(ApiError::BadRequest(
            "role permission is immutable or invalid",
        ));
    }
    if db::queries::set_kanban_role_permission(
        &state.pool,
        &workspace_id,
        &role,
        &permission,
        payload.granted,
    )
    .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("role permission not found"))
    }
}
async fn set_member_permission(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String, String)>,
    payload: web::Json<GrantPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, user_id, permission) = path.into_inner();
    require_permission(&state, &account.id, &workspace_id, "workspace:manage").await?;
    if !valid_permission(&permission)
        || matches!(permission.as_str(), "workspace:manage" | "workspace:delete")
    {
        return Err(ApiError::BadRequest(
            "member permission is immutable or invalid",
        ));
    }
    if db::queries::kanban_workspace_role(&state.pool, &workspace_id, &user_id)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound("workspace member not found"));
    }
    db::queries::set_kanban_member_permission(
        &state.pool,
        &workspace_id,
        &user_id,
        &permission,
        payload.granted,
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}
async fn reset_member_permissions(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, user_id) = path.into_inner();
    require_permission(&state, &account.id, &workspace_id, "workspace:manage").await?;
    db::queries::reset_kanban_member_permissions(&state.pool, &workspace_id, &user_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn list_boards(
    state: web::Data<AppState>,
    request: HttpRequest,
    workspace_id: web::Path<String>,
    query: web::Query<ArchivedQuery>,
) -> Result<web::Json<Vec<KanbanBoardSummary>>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    require_permission(&state, &account.id, &workspace_id, "board:view").await?;
    Ok(web::Json(
        db::queries::list_kanban_boards(&state.pool, &workspace_id, &account.id, query.archived)
            .await?,
    ))
}
async fn create_board(
    state: web::Data<AppState>,
    request: HttpRequest,
    workspace_id: web::Path<String>,
    payload: web::Json<BoardPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    require_permission(&state, &account.id, &workspace_id, "board:create").await?;
    let name = text(&payload.name, "board name is required", 120)?;
    let description = text(&payload.description, "", 2000)?;
    if !matches!(payload.visibility.as_str(), "private" | "public") {
        return Err(ApiError::BadRequest("board visibility is invalid"));
    }
    let id = db::queries::create_kanban_board(
        &state.pool,
        &workspace_id,
        &account.id,
        &name,
        &description,
        &payload.visibility,
    )
    .await?;
    let board = db::queries::get_kanban_board(&state.pool, &id, &account.id)
        .await?
        .ok_or(ApiError::NotFound("Kanban board not found"))?;
    Ok(HttpResponse::Created().json(board))
}
async fn get_board(
    state: web::Data<AppState>,
    request: HttpRequest,
    board_id: web::Path<String>,
) -> Result<web::Json<KanbanBoard>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = board_workspace(&state, &board_id).await?;
    require_permission(&state, &account.id, &workspace_id, "board:view").await?;
    db::queries::get_kanban_board(&state.pool, &board_id, &account.id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("Kanban board not found"))
}
async fn update_board(
    state: web::Data<AppState>,
    request: HttpRequest,
    board_id: web::Path<String>,
    payload: web::Json<BoardPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = board_workspace(&state, &board_id).await?;
    require_permission(&state, &account.id, &workspace_id, "board:edit").await?;
    let name = text(&payload.name, "board name is required", 120)?;
    let description = text(&payload.description, "", 2000)?;
    if !matches!(payload.visibility.as_str(), "private" | "public") {
        return Err(ApiError::BadRequest("board visibility is invalid"));
    }
    if db::queries::update_kanban_board(
        &state.pool,
        &board_id,
        &name,
        &description,
        &payload.visibility,
        payload.archived,
    )
    .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Kanban board not found"))
    }
}
async fn delete_board(
    state: web::Data<AppState>,
    request: HttpRequest,
    board_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = board_workspace(&state, &board_id).await?;
    require_permission(&state, &account.id, &workspace_id, "board:delete").await?;
    if db::queries::delete_kanban_board(&state.pool, &board_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Kanban board not found"))
    }
}
async fn set_board_favorite(
    state: web::Data<AppState>,
    request: HttpRequest,
    board_id: web::Path<String>,
    payload: web::Json<FavoritePayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = board_workspace(&state, &board_id).await?;
    require_permission(&state, &account.id, &workspace_id, "board:view").await?;
    db::queries::set_kanban_board_favorite(&state.pool, &board_id, &account.id, payload.favorite)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn create_column(
    state: web::Data<AppState>,
    request: HttpRequest,
    board_id: web::Path<String>,
    payload: web::Json<NamePayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = board_workspace(&state, &board_id).await?;
    require_permission(&state, &account.id, &workspace_id, "list:create").await?;
    let name = text(&payload.name, "column name is required", 80)?;
    let id = db::queries::create_kanban_column(&state.pool, &board_id, &name).await?;
    Ok(HttpResponse::Created().json(serde_json::json!({ "id": id })))
}
async fn update_column(
    state: web::Data<AppState>,
    request: HttpRequest,
    column_id: web::Path<String>,
    payload: web::Json<ColumnPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = column_workspace(&state, &column_id).await?;
    require_permission(&state, &account.id, &workspace_id, "list:edit").await?;
    if payload.name.is_none() && payload.position.is_none() {
        return Err(ApiError::BadRequest("column name or position is required"));
    }
    if let Some(name) = &payload.name {
        db::queries::rename_kanban_column(
            &state.pool,
            &column_id,
            &text(name, "column name is required", 80)?,
        )
        .await?;
    }
    if let Some(position) = payload.position {
        db::queries::reorder_kanban_column(&state.pool, &column_id, position).await?;
    }
    Ok(HttpResponse::NoContent().finish())
}
async fn delete_column(
    state: web::Data<AppState>,
    request: HttpRequest,
    column_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = column_workspace(&state, &column_id).await?;
    require_permission(&state, &account.id, &workspace_id, "list:delete").await?;
    if db::queries::delete_kanban_column(&state.pool, &column_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::Conflict(
            "move or archive every card before deleting this column",
        ))
    }
}

async fn create_card(
    state: web::Data<AppState>,
    request: HttpRequest,
    column_id: web::Path<String>,
    payload: web::Json<CardPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = column_workspace(&state, &column_id).await?;
    require_permission(&state, &account.id, &workspace_id, "card:create").await?;
    Ok(HttpResponse::Created().json(
        db::queries::create_kanban_card(
            &state.pool,
            &column_id,
            &account.id,
            &card_draft(&payload)?,
        )
        .await?,
    ))
}
async fn get_card(
    state: web::Data<AppState>,
    request: HttpRequest,
    card_id: web::Path<String>,
) -> Result<web::Json<KanbanCard>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = card_workspace(&state, &card_id).await?;
    require_permission(&state, &account.id, &workspace_id, "card:view").await?;
    db::queries::get_kanban_card(&state.pool, &card_id)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("Kanban card not found"))
}
async fn update_card(
    state: web::Data<AppState>,
    request: HttpRequest,
    card_id: web::Path<String>,
    payload: web::Json<CardPayload>,
) -> Result<web::Json<KanbanCard>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = card_workspace(&state, &card_id).await?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    db::queries::update_kanban_card(&state.pool, &card_id, &account.id, &card_draft(&payload)?)
        .await?
        .map(web::Json)
        .ok_or(ApiError::NotFound("Kanban card not found"))
}
async fn move_card(
    state: web::Data<AppState>,
    request: HttpRequest,
    card_id: web::Path<String>,
    payload: web::Json<MoveCardPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = card_workspace(&state, &card_id).await?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    if db::queries::move_kanban_card(
        &state.pool,
        &card_id,
        &payload.column_id,
        payload.position,
        &account.id,
    )
    .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::BadRequest(
            "card must move within its current board",
        ))
    }
}
async fn archive_card(
    state: web::Data<AppState>,
    request: HttpRequest,
    card_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = card_workspace(&state, &card_id).await?;
    require_permission(&state, &account.id, &workspace_id, "card:delete").await?;
    if db::queries::archive_kanban_card(&state.pool, &card_id, &account.id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Kanban card not found"))
    }
}

async fn create_label(
    state: web::Data<AppState>,
    request: HttpRequest,
    board_id: web::Path<String>,
    payload: web::Json<LabelPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = board_workspace(&state, &board_id).await?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    let name = text(&payload.name, "label name is required", 40)?;
    let color = payload.color.trim();
    if !valid_label_color(color) {
        return Err(ApiError::BadRequest("label color is invalid"));
    }
    let color = if color.starts_with('#') {
        color.to_ascii_uppercase()
    } else {
        color.to_owned()
    };
    Ok(HttpResponse::Created()
        .json(db::queries::create_kanban_label(&state.pool, &board_id, &name, &color).await?))
}
async fn delete_label(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (board_id, label_id) = path.into_inner();
    let workspace_id = board_workspace(&state, &board_id).await?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    if db::queries::delete_kanban_label(&state.pool, &board_id, &label_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Kanban label not found"))
    }
}

async fn create_comment(
    state: web::Data<AppState>,
    request: HttpRequest,
    card_id: web::Path<String>,
    payload: web::Json<ContentPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = card_workspace(&state, &card_id).await?;
    require_permission(&state, &account.id, &workspace_id, "comment:create").await?;
    let content = text(&payload.content, "comment is required", 10_000)?;
    let id =
        db::queries::create_kanban_comment(&state.pool, &card_id, &account.id, &content).await?;
    Ok(HttpResponse::Created().json(serde_json::json!({ "id": id })))
}
async fn update_comment(
    state: web::Data<AppState>,
    request: HttpRequest,
    comment_id: web::Path<String>,
    payload: web::Json<ContentPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, author_id) = db::queries::kanban_comment_context(&state.pool, &comment_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban comment not found"))?;
    if author_id.as_deref() != Some(&account.id) {
        require_permission(&state, &account.id, &workspace_id, "comment:edit").await?;
    } else {
        require_permission(&state, &account.id, &workspace_id, "comment:view").await?;
    }
    let content = text(&payload.content, "comment is required", 10_000)?;
    db::queries::update_kanban_comment(&state.pool, &comment_id, &content).await?;
    Ok(HttpResponse::NoContent().finish())
}
async fn delete_comment(
    state: web::Data<AppState>,
    request: HttpRequest,
    comment_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, author_id) = db::queries::kanban_comment_context(&state.pool, &comment_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban comment not found"))?;
    if author_id.as_deref() != Some(&account.id) {
        require_permission(&state, &account.id, &workspace_id, "comment:delete").await?;
    } else {
        require_permission(&state, &account.id, &workspace_id, "comment:view").await?;
    }
    db::queries::delete_kanban_comment(&state.pool, &comment_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn create_checklist(
    state: web::Data<AppState>,
    request: HttpRequest,
    card_id: web::Path<String>,
    payload: web::Json<NamePayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = card_workspace(&state, &card_id).await?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    let name = text(&payload.name, "checklist name is required", 120)?;
    let id =
        db::queries::create_kanban_checklist(&state.pool, &card_id, &name, &account.id).await?;
    Ok(HttpResponse::Created().json(serde_json::json!({ "id": id })))
}
async fn delete_checklist(
    state: web::Data<AppState>,
    request: HttpRequest,
    checklist_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, _) = db::queries::kanban_checklist_context(&state.pool, &checklist_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban checklist not found"))?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    db::queries::delete_kanban_checklist(&state.pool, &checklist_id).await?;
    Ok(HttpResponse::NoContent().finish())
}
async fn create_checklist_item(
    state: web::Data<AppState>,
    request: HttpRequest,
    checklist_id: web::Path<String>,
    payload: web::Json<ChecklistItemPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, _) = db::queries::kanban_checklist_context(&state.pool, &checklist_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban checklist not found"))?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    let title = text(&payload.title, "checklist item title is required", 240)?;
    let id = db::queries::create_kanban_checklist_item(&state.pool, &checklist_id, &title).await?;
    Ok(HttpResponse::Created().json(serde_json::json!({ "id": id })))
}
async fn update_checklist_item(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(String, String)>,
    payload: web::Json<ChecklistItemPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (checklist_id, item_id) = path.into_inner();
    let (workspace_id, _) = db::queries::kanban_checklist_context(&state.pool, &checklist_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban checklist not found"))?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    let title = text(&payload.title, "checklist item title is required", 240)?;
    if db::queries::update_kanban_checklist_item(
        &state.pool,
        &checklist_id,
        &item_id,
        &title,
        payload.completed,
    )
    .await?
    {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("Kanban checklist item not found"))
    }
}

async fn create_attachment(
    state: web::Data<AppState>,
    request: HttpRequest,
    card_id: web::Path<String>,
    query: web::Query<AttachmentQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let workspace_id = card_workspace(&state, &card_id).await?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    if body.is_empty() || body.len() > MAX_ATTACHMENT_BYTES {
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
    Ok(HttpResponse::Created().json(
        db::queries::create_kanban_attachment(
            &state.pool,
            &card_id,
            &account.id,
            file_name,
            mime_type,
            &body,
        )
        .await?,
    ))
}
async fn get_attachment(
    state: web::Data<AppState>,
    request: HttpRequest,
    attachment_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, file_name, mime_type, data) =
        db::queries::get_kanban_attachment(&state.pool, &attachment_id)
            .await?
            .ok_or(ApiError::NotFound("Kanban attachment not found"))?;
    require_permission(&state, &account.id, &workspace_id, "card:view").await?;
    let safe_name = file_name.replace(['"', '\r', '\n'], "_");
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime_type))
        .append_header(("x-content-type-options", "nosniff"))
        .append_header((
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{safe_name}\""),
        ))
        .body(data))
}
async fn delete_attachment(
    state: web::Data<AppState>,
    request: HttpRequest,
    attachment_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (workspace_id, _, _, _) = db::queries::get_kanban_attachment(&state.pool, &attachment_id)
        .await?
        .ok_or(ApiError::NotFound("Kanban attachment not found"))?;
    require_permission(&state, &account.id, &workspace_id, "card:edit").await?;
    db::queries::delete_kanban_attachment(&state.pool, &attachment_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::valid_label_color;

    #[test]
    fn label_colors_accept_hex_and_legacy_values() {
        assert!(valid_label_color("#2DD4BF"));
        assert!(valid_label_color("#a3e635"));
        assert!(valid_label_color("accent"));
        assert!(valid_label_color("gray"));
    }

    #[test]
    fn label_colors_reject_malformed_values() {
        assert!(!valid_label_color("2DD4BF"));
        assert!(!valid_label_color("#2DD4B"));
        assert!(!valid_label_color("#2DD4BFG"));
        assert!(!valid_label_color("var(--accent)"));
    }
}
