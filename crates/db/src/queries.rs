use crate::entities::{
    AccountSession, AppMetadata, AuthenticationSettings, Bookmark, BookmarkFavicon, CalendarEvent,
    CalendarEventDraft, CalendarSubscription, CodingCredential, CodingProject, DashboardWidget,
    EmbeddedPage, FeedItem, JournalNode, KanbanActivity, KanbanAttachment, KanbanBoard,
    KanbanBoardSummary, KanbanCard, KanbanCardDraft, KanbanChecklist, KanbanChecklistItem,
    KanbanColumn, KanbanComment, KanbanDirectoryUser, KanbanInvitation, KanbanLabel, KanbanMember,
    KanbanMemberPermission, KanbanOverview, KanbanRolePermission, KanbanWorkspace,
    KanbanWorkspaceSettings, LineAuthorProfile, LinePost, LinePostAttachment, LinePostDraft,
    LinePostReaction, LoggingSettings, LoginAppearance, ManagedUser, NetworkAccessRule,
    OidcAuthorization, PaymentSubscription, RssItem, RssItemDraft, RssRefreshTarget,
    RssSubscription, RssSubscriptionDraft, SessionAccount, Task, TaskAttachment, TaskDraft,
    TaskSubtask, User, UserAppearance, UserAvatar, UserBackground, UserCredentials, UserSettings,
    Workspace,
};
pub use crate::podcast_queries::*;
pub use crate::youtube_queries::*;
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool, Transaction};

const DEFAULT_WIDGETS: &[(&str, i64, i64, &str, i64, i64, i64, i64)] = &[
    ("weather", 0, 0, "wide", 0, 0, 8, 5),
    ("task-summary", 0, 1, "compact", 8, 0, 4, 5),
    ("focus", 0, 2, "standard", 0, 5, 6, 4),
    ("task-list", 0, 3, "wide", 0, 9, 8, 6),
    ("feed-list", 0, 4, "wide", 0, 15, 8, 6),
    ("feed-sources", 0, 5, "compact", 8, 9, 4, 6),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserMutationOutcome {
    Updated(ManagedUser),
    Deleted,
    NotFound,
    SelfAction,
    LastAdministrator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceDeleteOutcome {
    Deleted,
    NotFound,
    LastWorkspace,
}

#[derive(Debug, Clone, FromRow)]
struct TaskRecord {
    id: String,
    title: String,
    description: String,
    completed: bool,
    priority: String,
    due_date: Option<String>,
    repeat_rule: String,
    repeat_interval: i64,
    repeat_unit: String,
    reschedule_from: String,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct LinePostRecord {
    id: String,
    user_id: String,
    author_name: String,
    content: String,
    visibility: String,
    reply_to_post_id: Option<String>,
    reply_to_author_name: Option<String>,
    reply_to_content: Option<String>,
    reply_count: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct LinePostReactionRecord {
    emoji: String,
    count: i64,
    reacted_by_viewer: bool,
}

#[derive(Debug, Clone, FromRow)]
struct UserSettingsRecord {
    user_id: String,
    display_name: String,
    location: String,
    timezone: String,
    sidebar_timezones_json: String,
    calendar_week_start: String,
    temperature_unit: String,
    lines_default_visibility: String,
    podcast_playback_rate: f64,
    updated_at: String,
}

impl UserSettingsRecord {
    fn into_settings(self) -> UserSettings {
        let fallback_timezone = self.timezone.clone();
        let sidebar_timezones = serde_json::from_str::<Vec<String>>(&self.sidebar_timezones_json)
            .ok()
            .filter(|timezones| !timezones.is_empty())
            .unwrap_or_else(|| vec![fallback_timezone]);
        UserSettings {
            user_id: self.user_id,
            display_name: self.display_name,
            location: self.location,
            timezone: self.timezone,
            sidebar_timezones,
            calendar_week_start: self.calendar_week_start,
            temperature_unit: self.temperature_unit,
            lines_default_visibility: self.lines_default_visibility,
            podcast_playback_rate: self.podcast_playback_rate,
            updated_at: self.updated_at,
        }
    }
}

/// Lists the authenticated user's workspaces in navigation order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_workspaces(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<Workspace>, sqlx::Error> {
    sqlx::query_as::<_, Workspace>(
        "SELECT user_workspaces.workspace, user_workspaces.name, user_workspaces.position, \
                EXISTS(SELECT 1 FROM user_backgrounds \
                       WHERE user_backgrounds.user_id = user_workspaces.user_id \
                         AND user_backgrounds.workspace = user_workspaces.workspace) \
                    AS has_custom_background, \
                user_workspaces.created_at, user_workspaces.updated_at \
         FROM user_workspaces \
         WHERE user_workspaces.user_id = ? \
         ORDER BY user_workspaces.position ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Reports whether a workspace belongs to the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn workspace_exists(
    pool: &SqlitePool,
    user_id: &str,
    workspace: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_workspaces WHERE user_id = ? AND workspace = ?)",
    )
    .bind(user_id)
    .bind(workspace)
    .fetch_one(pool)
    .await
}

/// Creates a workspace using the first available stable identifier.
///
/// Returns `None` when the user already has eight workspaces.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn create_workspace(
    pool: &SqlitePool,
    user_id: &str,
    name: &str,
) -> Result<Option<Workspace>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let identifiers = sqlx::query_scalar::<_, i64>(
        "SELECT workspace FROM user_workspaces WHERE user_id = ? ORDER BY workspace",
    )
    .bind(user_id)
    .fetch_all(&mut *transaction)
    .await?;
    if identifiers.len() >= 8 {
        transaction.rollback().await?;
        return Ok(None);
    }
    let workspace = (0..=31)
        .find(|candidate| !identifiers.contains(candidate))
        .expect("fewer than eight workspaces always leaves an identifier");
    let position = identifiers.len() as i64;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO user_workspaces \
         (user_id, workspace, name, position, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(workspace)
    .bind(name)
    .bind(position)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(Some(Workspace {
        workspace,
        name: name.to_owned(),
        position,
        has_custom_background: false,
        created_at: now.clone(),
        updated_at: now,
    }))
}

/// Renames one workspace owned by the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_workspace(
    pool: &SqlitePool,
    user_id: &str,
    workspace: i64,
    name: &str,
) -> Result<Option<Workspace>, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE user_workspaces SET name = ?, updated_at = ? \
         WHERE user_id = ? AND workspace = ?",
    )
    .bind(name)
    .bind(&updated_at)
    .bind(user_id)
    .bind(workspace)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(None);
    }
    Ok(list_workspaces(pool, user_id)
        .await?
        .into_iter()
        .find(|candidate| candidate.workspace == workspace))
}

/// Deletes a workspace and all of its widgets and background data atomically.
///
/// The final workspace cannot be removed.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn delete_workspace(
    pool: &SqlitePool,
    user_id: &str,
    workspace: i64,
) -> Result<WorkspaceDeleteOutcome, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let position = sqlx::query_scalar::<_, i64>(
        "SELECT position FROM user_workspaces WHERE user_id = ? AND workspace = ?",
    )
    .bind(user_id)
    .bind(workspace)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(position) = position else {
        transaction.rollback().await?;
        return Ok(WorkspaceDeleteOutcome::NotFound);
    };
    let deleted = sqlx::query(
        "DELETE FROM user_workspaces \
         WHERE user_id = ? AND workspace = ? \
           AND (SELECT COUNT(*) FROM user_workspaces WHERE user_id = ?) > 1",
    )
    .bind(user_id)
    .bind(workspace)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    if deleted.rows_affected() != 1 {
        transaction.rollback().await?;
        return Ok(WorkspaceDeleteOutcome::LastWorkspace);
    }
    sqlx::query(
        "UPDATE user_workspaces SET position = position + 16 \
         WHERE user_id = ? AND position > ?",
    )
    .bind(user_id)
    .bind(position)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE user_workspaces SET position = position - 17 \
         WHERE user_id = ? AND position >= ?",
    )
    .bind(user_id)
    .bind(position + 17)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(WorkspaceDeleteOutcome::Deleted)
}

#[derive(Debug, FromRow)]
struct DashboardWidgetRow {
    id: String,
    kind: String,
    workspace: i64,
    position: i64,
    size: String,
    grid_x: i64,
    grid_y: i64,
    grid_w: i64,
    grid_h: i64,
    config_json: String,
    has_secret: bool,
    created_at: String,
    updated_at: String,
}

impl From<DashboardWidgetRow> for DashboardWidget {
    fn from(row: DashboardWidgetRow) -> Self {
        Self {
            id: row.id,
            kind: row.kind,
            workspace: row.workspace,
            position: row.position,
            size: row.size,
            grid_x: row.grid_x,
            grid_y: row.grid_y,
            grid_w: row.grid_w,
            grid_h: row.grid_h,
            config: serde_json::from_str(&row.config_json)
                .unwrap_or_else(|_| serde_json::json!({})),
            has_secret: row.has_secret,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Fetches one application metadata value by key.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn fetch_metadata(
    pool: &SqlitePool,
    key: &str,
) -> Result<Option<AppMetadata>, sqlx::Error> {
    sqlx::query_as::<_, AppMetadata>(
        "SELECT key, value, updated_at FROM app_metadata WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(pool)
    .await
}

/// Inserts or replaces one application metadata value.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the write cannot be completed.
pub async fn upsert_metadata(
    pool: &SqlitePool,
    key: &str,
    value: &str,
) -> Result<AppMetadata, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO app_metadata (key, value, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .bind(&updated_at)
    .execute(pool)
    .await?;

    Ok(AppMetadata {
        key: key.to_owned(),
        value: value.to_owned(),
        updated_at,
    })
}

/// Loads tasks in active-first, creation order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_tasks(pool: &SqlitePool, user_id: &str) -> Result<Vec<Task>, sqlx::Error> {
    let records = sqlx::query_as::<_, TaskRecord>(
        "SELECT id, title, description, completed, priority, due_date, repeat_rule, \
         repeat_interval, repeat_unit, reschedule_from, completed_at, created_at, updated_at \
         FROM tasks WHERE user_id = ? AND archived_at IS NULL \
         ORDER BY completed ASC, created_at ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut tasks = Vec::with_capacity(records.len());
    for record in records {
        tasks.push(hydrate_task(pool, record).await?);
    }
    Ok(tasks)
}

/// Counts archived tasks for one user without hydrating their related records.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn count_archived_tasks(pool: &SqlitePool, user_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE user_id = ? AND archived_at IS NOT NULL")
        .bind(user_id)
        .fetch_one(pool)
        .await
}

/// Loads archived tasks for one user, newest archive first.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_archived_tasks(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<Task>, sqlx::Error> {
    let records = sqlx::query_as::<_, TaskRecord>(
        "SELECT id, title, description, completed, priority, due_date, repeat_rule, \
         repeat_interval, repeat_unit, reschedule_from, completed_at, created_at, updated_at \
         FROM tasks WHERE user_id = ? AND archived_at IS NOT NULL \
         ORDER BY archived_at DESC, created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut tasks = Vec::with_capacity(records.len());
    for record in records {
        tasks.push(hydrate_task(pool, record).await?);
    }
    Ok(tasks)
}

async fn hydrate_task(pool: &SqlitePool, record: TaskRecord) -> Result<Task, sqlx::Error> {
    let labels = sqlx::query_scalar::<_, String>(
        "SELECT label FROM task_labels WHERE task_id = ? ORDER BY position ASC",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let subtasks = sqlx::query_as::<_, TaskSubtask>(
        "SELECT id, title, completed, position, created_at, updated_at \
         FROM task_subtasks WHERE task_id = ? ORDER BY position ASC",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let attachments = sqlx::query_as::<_, TaskAttachment>(
        "SELECT id, file_name, mime_type, byte_size, created_at \
         FROM task_attachments WHERE task_id = ? ORDER BY created_at ASC",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;

    Ok(Task {
        id: record.id,
        title: record.title,
        description: record.description,
        completed: record.completed,
        priority: record.priority,
        due_date: record.due_date,
        repeat_rule: record.repeat_rule,
        repeat_interval: record.repeat_interval,
        repeat_unit: record.repeat_unit,
        reschedule_from: record.reschedule_from,
        completed_at: record.completed_at,
        labels,
        subtasks,
        attachments,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

/// Loads one user-owned task with its labels, subtasks, and attachment metadata.
///
/// # Errors
///
/// Returns the underlying SQLx error when the task or its child records cannot be loaded.
pub async fn get_task(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<Option<Task>, sqlx::Error> {
    let record = sqlx::query_as::<_, TaskRecord>(
        "SELECT id, title, description, completed, priority, due_date, repeat_rule, \
         repeat_interval, repeat_unit, reschedule_from, completed_at, created_at, updated_at \
         FROM tasks WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    match record {
        Some(record) => Ok(Some(hydrate_task(pool, record).await?)),
        None => Ok(None),
    }
}

async fn replace_task_children(
    transaction: &mut Transaction<'_, Sqlite>,
    task_id: &str,
    draft: &TaskDraft,
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM task_labels WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut **transaction)
        .await?;
    for (position, label) in draft.labels.iter().enumerate() {
        sqlx::query("INSERT INTO task_labels (task_id, label, position) VALUES (?, ?, ?)")
            .bind(task_id)
            .bind(label)
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .execute(&mut **transaction)
            .await?;
    }

    sqlx::query("DELETE FROM task_subtasks WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut **transaction)
        .await?;
    for (position, subtask) in draft.subtasks.iter().enumerate() {
        sqlx::query(
            "INSERT INTO task_subtasks \
             (id, task_id, title, completed, position, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(
            subtask
                .id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        )
        .bind(task_id)
        .bind(&subtask.title)
        .bind(subtask.completed)
        .bind(i64::try_from(position).unwrap_or(i64::MAX))
        .bind(now)
        .bind(now)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

/// Creates a task with a generated identifier.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn create_task(
    pool: &SqlitePool,
    user_id: &str,
    draft: &TaskDraft,
) -> Result<Task, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;

    sqlx::query(
        "INSERT INTO tasks \
         (id, user_id, title, description, completed, priority, due_date, repeat_rule, \
          repeat_interval, repeat_unit, reschedule_from, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&draft.title)
    .bind(&draft.description)
    .bind(&draft.priority)
    .bind(&draft.due_date)
    .bind(&draft.repeat_rule)
    .bind(draft.repeat_interval)
    .bind(&draft.repeat_unit)
    .bind(&draft.reschedule_from)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    replace_task_children(&mut transaction, &id, draft, &now).await?;
    transaction.commit().await?;

    get_task(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Replaces one task's editable fields and child records atomically.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_task(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    draft: &TaskDraft,
    completed: bool,
    completed_at: Option<&str>,
) -> Result<Option<Task>, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE tasks SET title = ?, description = ?, completed = ?, priority = ?, \
         due_date = ?, repeat_rule = ?, repeat_interval = ?, repeat_unit = ?, \
         reschedule_from = ?, completed_at = ?, updated_at = ? \
         WHERE id = ? AND user_id = ?",
    )
    .bind(&draft.title)
    .bind(&draft.description)
    .bind(completed)
    .bind(&draft.priority)
    .bind(&draft.due_date)
    .bind(&draft.repeat_rule)
    .bind(draft.repeat_interval)
    .bind(&draft.repeat_unit)
    .bind(&draft.reschedule_from)
    .bind(completed_at)
    .bind(&updated_at)
    .bind(id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;

    if result.rows_affected() == 0 {
        transaction.rollback().await?;
        return Ok(None);
    }
    replace_task_children(&mut transaction, id, draft, &updated_at).await?;
    transaction.commit().await?;

    get_task(pool, user_id, id).await
}

/// Deletes one user-owned task and its cascading child records.
///
/// # Errors
///
/// Returns the underlying SQLx error when the delete cannot be completed.
pub async fn delete_task(pool: &SqlitePool, user_id: &str, id: &str) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM tasks WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Archives one user-owned task without deleting its child records.
///
/// # Errors
///
/// Returns the underlying SQLx error when the update cannot be completed.
pub async fn archive_task(pool: &SqlitePool, user_id: &str, id: &str) -> Result<bool, sqlx::Error> {
    let archived_at = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE tasks SET archived_at = ?, updated_at = ? \
         WHERE id = ? AND user_id = ? AND archived_at IS NULL",
    )
    .bind(&archived_at)
    .bind(&archived_at)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Restores one archived, user-owned task to the active list.
///
/// # Errors
///
/// Returns the underlying SQLx error when the update cannot be completed.
pub async fn restore_task(pool: &SqlitePool, user_id: &str, id: &str) -> Result<bool, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE tasks SET archived_at = NULL, updated_at = ? \
         WHERE id = ? AND user_id = ? AND archived_at IS NOT NULL",
    )
    .bind(&updated_at)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Stores attachment bytes for a user-owned task.
///
/// # Errors
///
/// Returns the underlying SQLx error when ownership cannot be checked or the insert fails.
pub async fn create_task_attachment(
    pool: &SqlitePool,
    user_id: &str,
    task_id: &str,
    file_name: &str,
    mime_type: &str,
    data: &[u8],
) -> Result<Option<TaskAttachment>, sqlx::Error> {
    if get_task(pool, user_id, task_id).await?.is_none() {
        return Ok(None);
    }
    let attachment = TaskAttachment {
        id: uuid::Uuid::new_v4().to_string(),
        file_name: file_name.to_owned(),
        mime_type: mime_type.to_owned(),
        byte_size: i64::try_from(data.len()).unwrap_or(i64::MAX),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    sqlx::query(
        "INSERT INTO task_attachments \
         (id, task_id, file_name, mime_type, byte_size, file_data, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&attachment.id)
    .bind(task_id)
    .bind(&attachment.file_name)
    .bind(&attachment.mime_type)
    .bind(attachment.byte_size)
    .bind(data)
    .bind(&attachment.created_at)
    .execute(pool)
    .await?;
    Ok(Some(attachment))
}

/// Loads attachment bytes after verifying ownership through the parent task.
///
/// # Errors
///
/// Returns the underlying SQLx error when the attachment cannot be queried.
pub async fn get_task_attachment(
    pool: &SqlitePool,
    user_id: &str,
    task_id: &str,
    attachment_id: &str,
) -> Result<Option<(String, String, Vec<u8>)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.file_name, a.mime_type, a.file_data \
         FROM task_attachments a \
         JOIN tasks t ON t.id = a.task_id \
         WHERE a.id = ? AND a.task_id = ? AND t.user_id = ?",
    )
    .bind(attachment_id)
    .bind(task_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Deletes an attachment after verifying ownership through the parent task.
///
/// # Errors
///
/// Returns the underlying SQLx error when the delete cannot be completed.
pub async fn delete_task_attachment(
    pool: &SqlitePool,
    user_id: &str,
    task_id: &str,
    attachment_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM task_attachments WHERE id = ? AND task_id = ? \
         AND EXISTS (SELECT 1 FROM tasks WHERE id = ? AND user_id = ?)",
    )
    .bind(attachment_id)
    .bind(task_id)
    .bind(task_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Deletes all completed tasks and returns the number removed.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn clear_completed_tasks(pool: &SqlitePool, user_id: &str) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM tasks \
             WHERE completed = 1 AND user_id = ? AND archived_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected())
}

async fn hydrate_line_post(
    pool: &SqlitePool,
    viewer_id: &str,
    record: LinePostRecord,
) -> Result<LinePost, sqlx::Error> {
    let tags = sqlx::query_scalar::<_, String>(
        "SELECT tag FROM line_post_tags WHERE post_id = ? ORDER BY tag COLLATE NOCASE ASC",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let attachments = sqlx::query_as::<_, LinePostAttachment>(
        "SELECT id, file_name, mime_type, byte_size, created_at \
         FROM line_post_attachments WHERE post_id = ? ORDER BY created_at ASC, id ASC",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let reactions = sqlx::query_as::<_, LinePostReactionRecord>(
        "SELECT emoji, COUNT(*) AS count, \
                CAST(MAX(CASE WHEN user_id = ? THEN 1 ELSE 0 END) AS BOOLEAN) \
                    AS reacted_by_viewer \
         FROM line_post_reactions WHERE post_id = ? \
         GROUP BY emoji ORDER BY MIN(created_at) ASC, emoji ASC",
    )
    .bind(viewer_id)
    .bind(&record.id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|reaction| LinePostReaction {
        emoji: reaction.emoji,
        count: reaction.count,
        reacted_by_viewer: reaction.reacted_by_viewer,
    })
    .collect();

    Ok(LinePost {
        id: record.id,
        user_id: record.user_id,
        author_name: record.author_name,
        content: record.content,
        visibility: record.visibility,
        reply_to_post_id: record.reply_to_post_id,
        reply_to_author_name: record.reply_to_author_name,
        reply_to_content: record.reply_to_content,
        tags,
        attachments,
        reactions,
        reply_count: record.reply_count,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

/// Builds a Lines feed statement from the shared projection; the first bind is the viewer id.
macro_rules! line_post_select {
    ($tail:literal) => {
        concat!(
            "SELECT p.id, p.user_id, author.display_name AS author_name, p.content, p.visibility, \
             p.reply_to_post_id, reply_author.display_name AS reply_to_author_name, \
             parent.content AS reply_to_content, \
             (SELECT COUNT(*) FROM line_posts replies \
              WHERE replies.reply_to_post_id = p.id \
                AND (replies.visibility = 'public' OR replies.user_id = ?)) AS reply_count, \
             p.created_at, p.updated_at \
             FROM line_posts p \
             JOIN user_settings author ON author.user_id = p.user_id \
             LEFT JOIN line_posts parent ON parent.id = p.reply_to_post_id \
             LEFT JOIN user_settings reply_author ON reply_author.user_id = parent.user_id ",
            $tail
        )
    };
}

/// Lists Lines posts visible to an authenticated viewer.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the feed cannot be loaded.
pub async fn list_line_posts(
    pool: &SqlitePool,
    viewer_id: &str,
    scope: &str,
    query: &str,
    tag: &str,
) -> Result<Vec<LinePost>, sqlx::Error> {
    let search_pattern = format!("%{}%", query.to_lowercase());
    let records = sqlx::query_as::<_, LinePostRecord>(line_post_select!(
        "WHERE (p.visibility = 'public' OR p.user_id = ?) \
         AND (? = 'instance' OR p.user_id = ?) \
         AND (? = '' OR LOWER(p.content) LIKE ?) \
         AND (? = '' OR EXISTS( \
             SELECT 1 FROM line_post_tags filter_tags \
             WHERE filter_tags.post_id = p.id AND filter_tags.tag = ? COLLATE NOCASE \
         )) \
         ORDER BY p.created_at DESC, p.id DESC LIMIT 100"
    ))
    .bind(viewer_id)
    .bind(viewer_id)
    .bind(scope)
    .bind(viewer_id)
    .bind(query)
    .bind(&search_pattern)
    .bind(tag)
    .bind(tag)
    .fetch_all(pool)
    .await?;

    hydrate_line_posts(pool, viewer_id, records).await
}

async fn hydrate_line_posts(
    pool: &SqlitePool,
    viewer_id: &str,
    records: Vec<LinePostRecord>,
) -> Result<Vec<LinePost>, sqlx::Error> {
    let mut posts = Vec::with_capacity(records.len());
    for record in records {
        posts.push(hydrate_line_post(pool, viewer_id, record).await?);
    }
    Ok(posts)
}

/// Loads one Lines post only when it is visible to the authenticated viewer.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the post cannot be queried.
pub async fn get_line_post(
    pool: &SqlitePool,
    viewer_id: &str,
    post_id: &str,
) -> Result<Option<LinePost>, sqlx::Error> {
    let record = sqlx::query_as::<_, LinePostRecord>(line_post_select!(
        "WHERE p.id = ? AND (p.visibility = 'public' OR p.user_id = ?)"
    ))
    .bind(viewer_id)
    .bind(post_id)
    .bind(viewer_id)
    .fetch_optional(pool)
    .await?;

    match record {
        Some(record) => Ok(Some(hydrate_line_post(pool, viewer_id, record).await?)),
        None => Ok(None),
    }
}

/// Lists one author's Lines posts that are visible to the viewer, newest first.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the author feed cannot be loaded.
pub async fn list_line_posts_by_author(
    pool: &SqlitePool,
    viewer_id: &str,
    author_id: &str,
) -> Result<Vec<LinePost>, sqlx::Error> {
    let records = sqlx::query_as::<_, LinePostRecord>(line_post_select!(
        "WHERE p.user_id = ? AND (p.visibility = 'public' OR p.user_id = ?) \
         ORDER BY p.created_at DESC, p.id DESC LIMIT 100"
    ))
    .bind(viewer_id)
    .bind(author_id)
    .bind(viewer_id)
    .fetch_all(pool)
    .await?;

    hydrate_line_posts(pool, viewer_id, records).await
}

/// Lists the direct replies to one Lines post in thread reading order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the replies cannot be loaded.
pub async fn list_line_post_replies(
    pool: &SqlitePool,
    viewer_id: &str,
    post_id: &str,
) -> Result<Vec<LinePost>, sqlx::Error> {
    let records = sqlx::query_as::<_, LinePostRecord>(line_post_select!(
        "WHERE p.reply_to_post_id = ? AND (p.visibility = 'public' OR p.user_id = ?) \
         ORDER BY p.created_at ASC, p.id ASC LIMIT 100"
    ))
    .bind(viewer_id)
    .bind(post_id)
    .bind(viewer_id)
    .fetch_all(pool)
    .await?;

    hydrate_line_posts(pool, viewer_id, records).await
}

/// Loads the Lines profile of an author who has posts the viewer can see.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the profile cannot be loaded.
pub async fn find_line_author_profile(
    pool: &SqlitePool,
    viewer_id: &str,
    author_id: &str,
) -> Result<Option<LineAuthorProfile>, sqlx::Error> {
    sqlx::query_as::<_, LineAuthorProfile>(
        "SELECT settings.user_id, settings.display_name, \
                (SELECT COUNT(*) FROM line_posts p \
                 WHERE p.user_id = settings.user_id \
                   AND (p.visibility = 'public' OR p.user_id = ?)) AS post_count, \
                (SELECT MIN(p.created_at) FROM line_posts p \
                 WHERE p.user_id = settings.user_id \
                   AND (p.visibility = 'public' OR p.user_id = ?)) AS first_post_at \
         FROM user_settings settings \
         WHERE settings.user_id = ? \
           AND EXISTS( \
               SELECT 1 FROM line_posts p \
               WHERE p.user_id = settings.user_id \
                 AND (p.visibility = 'public' OR p.user_id = ?) \
           )",
    )
    .bind(viewer_id)
    .bind(viewer_id)
    .bind(author_id)
    .bind(viewer_id)
    .fetch_optional(pool)
    .await
}

/// Reports whether an author has at least one Lines post visible to the viewer.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the lookup cannot be completed.
pub async fn line_author_is_visible(
    pool: &SqlitePool,
    viewer_id: &str,
    author_id: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS( \
             SELECT 1 FROM line_posts p \
             WHERE p.user_id = ? AND (p.visibility = 'public' OR p.user_id = ?) \
         )",
    )
    .bind(author_id)
    .bind(viewer_id)
    .fetch_one(pool)
    .await
}

/// Creates one Lines post and its normalized hashtag index atomically.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the post cannot be stored.
pub async fn create_line_post(
    pool: &SqlitePool,
    user_id: &str,
    draft: &LinePostDraft,
) -> Result<LinePost, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO line_posts \
         (id, user_id, content, visibility, reply_to_post_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&draft.content)
    .bind(&draft.visibility)
    .bind(&draft.reply_to_post_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    for tag in &draft.tags {
        sqlx::query("INSERT INTO line_post_tags (post_id, tag) VALUES (?, ?)")
            .bind(&id)
            .bind(tag)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    get_line_post(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Deletes a post owned by the actor, or a public post when the actor is an administrator.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_line_post(
    pool: &SqlitePool,
    actor_id: &str,
    post_id: &str,
    administrator: bool,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM line_posts WHERE id = ? \
         AND (user_id = ? OR (? AND visibility = 'public'))",
    )
    .bind(post_id)
    .bind(actor_id)
    .bind(administrator)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Stores an attachment on a post owned by the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when ownership cannot be checked or storage fails.
pub async fn create_line_post_attachment(
    pool: &SqlitePool,
    user_id: &str,
    post_id: &str,
    file_name: &str,
    mime_type: &str,
    data: &[u8],
) -> Result<Option<LinePostAttachment>, sqlx::Error> {
    let owns_post: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM line_posts WHERE id = ? AND user_id = ?)")
            .bind(post_id)
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    if !owns_post {
        return Ok(None);
    }
    let attachment = LinePostAttachment {
        id: uuid::Uuid::new_v4().to_string(),
        file_name: file_name.to_owned(),
        mime_type: mime_type.to_owned(),
        byte_size: i64::try_from(data.len()).unwrap_or(i64::MAX),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    sqlx::query(
        "INSERT INTO line_post_attachments \
         (id, post_id, file_name, mime_type, byte_size, file_data, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&attachment.id)
    .bind(post_id)
    .bind(&attachment.file_name)
    .bind(&attachment.mime_type)
    .bind(attachment.byte_size)
    .bind(data)
    .bind(&attachment.created_at)
    .execute(pool)
    .await?;
    Ok(Some(attachment))
}

/// Loads attachment bytes only when the parent post is visible to the viewer.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the attachment cannot be queried.
pub async fn get_line_post_attachment(
    pool: &SqlitePool,
    viewer_id: &str,
    post_id: &str,
    attachment_id: &str,
) -> Result<Option<(String, String, Vec<u8>)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.file_name, a.mime_type, a.file_data \
         FROM line_post_attachments a \
         JOIN line_posts p ON p.id = a.post_id \
         WHERE a.id = ? AND a.post_id = ? \
           AND (p.visibility = 'public' OR p.user_id = ?)",
    )
    .bind(attachment_id)
    .bind(post_id)
    .bind(viewer_id)
    .fetch_optional(pool)
    .await
}

/// Deletes an attachment from a post owned by the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_line_post_attachment(
    pool: &SqlitePool,
    user_id: &str,
    post_id: &str,
    attachment_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM line_post_attachments WHERE id = ? AND post_id = ? \
         AND EXISTS(SELECT 1 FROM line_posts WHERE id = ? AND user_id = ?)",
    )
    .bind(attachment_id)
    .bind(post_id)
    .bind(post_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Adds one reaction when the target post is visible to the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the reaction cannot be stored.
pub async fn add_line_post_reaction(
    pool: &SqlitePool,
    user_id: &str,
    post_id: &str,
    emoji: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "INSERT OR IGNORE INTO line_post_reactions (post_id, user_id, emoji, created_at) \
         SELECT id, ?, ?, ? FROM line_posts \
         WHERE id = ? AND (visibility = 'public' OR user_id = ?)",
    )
    .bind(user_id)
    .bind(emoji)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(post_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Removes one reaction created by the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the reaction cannot be removed.
pub async fn remove_line_post_reaction(
    pool: &SqlitePool,
    user_id: &str,
    post_id: &str,
    emoji: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM line_post_reactions WHERE post_id = ? AND user_id = ? AND emoji = ?",
    )
    .bind(post_id)
    .bind(user_id)
    .bind(emoji)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Deletes one complete category of content owned by an authenticated account.
///
/// Shared YouTube channel and video metadata is retained because other accounts may use it.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn delete_user_content(
    pool: &SqlitePool,
    user_id: &str,
    scope: &str,
) -> Result<u64, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let deleted = match scope {
        "contacts" => {
            let contacts = sqlx::query("DELETE FROM contacts WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            let sources = sqlx::query("DELETE FROM contact_dav_sources WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            contacts + sources
        }
        "tasks" => sqlx::query("DELETE FROM tasks WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected(),
        "lines" => sqlx::query("DELETE FROM line_posts WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected(),
        "calendar" => sqlx::query("DELETE FROM calendar_subscriptions WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected(),
        "rss" => sqlx::query("DELETE FROM rss_subscriptions WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected(),
        "journal" => sqlx::query("DELETE FROM journal_nodes WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected(),
        "youtube" => {
            let watch_later = sqlx::query("DELETE FROM youtube_watch_later WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            let groups = sqlx::query("DELETE FROM youtube_groups WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            let subscriptions = sqlx::query("DELETE FROM youtube_subscriptions WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            let settings = sqlx::query("DELETE FROM youtube_settings WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            watch_later + groups + subscriptions + settings
        }
        "coding" => {
            let projects = sqlx::query("DELETE FROM coding_projects WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            let credentials = sqlx::query("DELETE FROM coding_credentials WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            projects + credentials
        }
        // The catalogue and cached audio are shared instance resources: clearing one
        // listener's data must never delete an episode another is midway through.
        "podcasts" => {
            let mut removed = 0;
            for statement in [
                "DELETE FROM podcast_queue WHERE user_id = ?",
                "DELETE FROM podcast_saved_episodes WHERE user_id = ?",
                "DELETE FROM podcast_episode_progress WHERE user_id = ?",
                "DELETE FROM podcast_subscriptions WHERE user_id = ?",
                "DELETE FROM podcast_requests WHERE user_id = ?",
            ] {
                removed += sqlx::query(statement)
                    .bind(user_id)
                    .execute(&mut *transaction)
                    .await?
                    .rows_affected();
            }
            removed
        }
        "downloads" => sqlx::query("DELETE FROM youtube_download_jobs WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected(),
        "subscriptions" => sqlx::query("DELETE FROM payment_subscriptions WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected(),
        _ => return Err(sqlx::Error::Protocol("content scope is invalid".to_owned())),
    };
    transaction.commit().await?;
    Ok(deleted)
}

/// Loads one user's subscribed calendars in name order.
pub async fn list_calendar_subscriptions(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<CalendarSubscription>, sqlx::Error> {
    sqlx::query_as::<_, CalendarSubscription>(
        "SELECT id, url, name, color_value AS color, last_fetched_at, last_error, created_at, updated_at \
         FROM calendar_subscriptions WHERE user_id = ? ORDER BY name COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Loads calendar events owned through one user's subscriptions.
pub async fn list_calendar_events(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<CalendarEvent>, sqlx::Error> {
    sqlx::query_as::<_, CalendarEvent>(
        "SELECT e.id, e.subscription_id, s.name AS calendar_name, s.color_value AS calendar_color, \
         e.title, e.description, e.location, e.url, e.start_at, e.end_at, e.all_day \
         FROM calendar_events e JOIN calendar_subscriptions s ON s.id = e.subscription_id \
         WHERE s.user_id = ? ORDER BY e.start_at ASC, e.title COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Loads one user-owned calendar subscription.
pub async fn get_calendar_subscription(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<Option<CalendarSubscription>, sqlx::Error> {
    sqlx::query_as::<_, CalendarSubscription>(
        "SELECT id, url, name, color_value AS color, last_fetched_at, last_error, created_at, updated_at \
         FROM calendar_subscriptions WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

async fn replace_calendar_events(
    transaction: &mut Transaction<'_, Sqlite>,
    subscription_id: &str,
    events: &[CalendarEventDraft],
    fetched_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM calendar_events WHERE subscription_id = ?")
        .bind(subscription_id)
        .execute(&mut **transaction)
        .await?;
    for event in events {
        sqlx::query(
            "INSERT INTO calendar_events \
             (id, subscription_id, external_id, title, description, location, url, start_at, \
              end_at, all_day, fetched_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(subscription_id)
        .bind(&event.external_id)
        .bind(&event.title)
        .bind(&event.description)
        .bind(&event.location)
        .bind(&event.url)
        .bind(&event.start_at)
        .bind(&event.end_at)
        .bind(event.all_day)
        .bind(fetched_at)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

/// Creates a calendar subscription and its initial bounded event snapshot atomically.
pub async fn create_calendar_subscription(
    pool: &SqlitePool,
    user_id: &str,
    url: &str,
    name: &str,
    color: &str,
    events: &[CalendarEventDraft],
) -> Result<CalendarSubscription, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO calendar_subscriptions \
         (id, user_id, url, name, color_value, last_fetched_at, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(url)
    .bind(name)
    .bind(color)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    replace_calendar_events(&mut transaction, &id, events, &now).await?;
    transaction.commit().await?;
    get_calendar_subscription(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Replaces one calendar's fetched event snapshot.
pub async fn refresh_calendar_subscription(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    name: &str,
    events: &[CalendarEventDraft],
) -> Result<Option<CalendarSubscription>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE calendar_subscriptions SET name = ?, last_fetched_at = ?, last_error = NULL, \
         updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(name)
    .bind(&now)
    .bind(&now)
    .bind(id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() == 0 {
        transaction.rollback().await?;
        return Ok(None);
    }
    replace_calendar_events(&mut transaction, id, events, &now).await?;
    transaction.commit().await?;
    get_calendar_subscription(pool, user_id, id).await
}

/// Records the last calendar fetch failure without discarding cached events.
pub async fn set_calendar_refresh_error(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    message: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE calendar_subscriptions SET last_error = ?, updated_at = ? \
         WHERE id = ? AND user_id = ?",
    )
    .bind(message)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes one user-owned calendar and its events.
pub async fn delete_calendar_subscription(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM calendar_subscriptions WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Lists one user's recurring payment subscriptions.
pub async fn list_payment_subscriptions(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<PaymentSubscription>, sqlx::Error> {
    sqlx::query_as::<_, PaymentSubscription>(
        "SELECT id, service, description, frequency, amount_micros, currency, first_paid_on, created_at, updated_at \
         FROM payment_subscriptions WHERE user_id = ? \
         ORDER BY service COLLATE NOCASE ASC, first_paid_on ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Creates one user-owned recurring payment subscription.
pub async fn create_payment_subscription(
    pool: &SqlitePool,
    user_id: &str,
    service: &str,
    description: &str,
    frequency: &str,
    amount_micros: i64,
    currency: &str,
    first_paid_on: &str,
) -> Result<PaymentSubscription, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO payment_subscriptions \
         (id, user_id, service, description, frequency, amount_micros, currency, first_paid_on, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(service)
    .bind(description)
    .bind(frequency)
    .bind(amount_micros)
    .bind(currency)
    .bind(first_paid_on)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, PaymentSubscription>(
        "SELECT id, service, description, frequency, amount_micros, currency, first_paid_on, created_at, updated_at \
         FROM payment_subscriptions WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Updates one user-owned recurring payment subscription.
pub async fn update_payment_subscription(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    service: &str,
    description: &str,
    frequency: &str,
    amount_micros: i64,
    currency: &str,
    first_paid_on: &str,
) -> Result<Option<PaymentSubscription>, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE payment_subscriptions SET service = ?, description = ?, frequency = ?, \
         amount_micros = ?, currency = ?, first_paid_on = ?, updated_at = ? \
         WHERE id = ? AND user_id = ?",
    )
    .bind(service)
    .bind(description)
    .bind(frequency)
    .bind(amount_micros)
    .bind(currency)
    .bind(first_paid_on)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    sqlx::query_as::<_, PaymentSubscription>(
        "SELECT id, service, description, frequency, amount_micros, currency, first_paid_on, created_at, updated_at \
         FROM payment_subscriptions WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Deletes one user-owned recurring payment subscription.
pub async fn delete_payment_subscription(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM payment_subscriptions WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Lists one user's subscribed software projects.
pub async fn list_coding_projects(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<CodingProject>, sqlx::Error> {
    sqlx::query_as::<_, CodingProject>(
        "SELECT p.id, p.provider, p.host, p.repository, \
         EXISTS(SELECT 1 FROM coding_credentials c WHERE c.user_id = p.user_id \
         AND c.provider = p.provider AND c.host = p.host) AS has_credential, \
         p.created_at, p.updated_at FROM coding_projects p WHERE p.user_id = ? \
         ORDER BY p.provider ASC, p.repository COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Creates one user-owned software project subscription.
pub async fn create_coding_project(
    pool: &SqlitePool,
    user_id: &str,
    provider: &str,
    host: &str,
    repository: &str,
) -> Result<CodingProject, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO coding_projects \
         (id, user_id, provider, host, repository, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(provider)
    .bind(host)
    .bind(repository)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, CodingProject>(
        "SELECT p.id, p.provider, p.host, p.repository, \
         EXISTS(SELECT 1 FROM coding_credentials c WHERE c.user_id = p.user_id \
         AND c.provider = p.provider AND c.host = p.host) AS has_credential, \
         p.created_at, p.updated_at FROM coding_projects p WHERE p.id = ? AND p.user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Deletes one user-owned software project subscription.
pub async fn delete_coding_project(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM coding_projects WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Lists encrypted code-host credentials without exposing them through API models.
pub async fn list_coding_credentials(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<CodingCredential>, sqlx::Error> {
    sqlx::query_as::<_, CodingCredential>(
        "SELECT provider, host, ciphertext, updated_at FROM coding_credentials \
         WHERE user_id = ? ORDER BY provider ASC, host ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Inserts or replaces an encrypted credential for one code host.
pub async fn upsert_coding_credential(
    pool: &SqlitePool,
    user_id: &str,
    provider: &str,
    host: &str,
    ciphertext: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO coding_credentials (user_id, provider, host, ciphertext, updated_at) \
         VALUES (?, ?, ?, ?, ?) ON CONFLICT(user_id, provider, host) DO UPDATE SET \
         ciphertext = excluded.ciphertext, updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(provider)
    .bind(host)
    .bind(ciphertext)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

/// Removes one encrypted code-host credential.
pub async fn delete_coding_credential(
    pool: &SqlitePool,
    user_id: &str,
    provider: &str,
    host: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM coding_credentials WHERE user_id = ? AND provider = ? AND host = ?",
    )
    .bind(user_id)
    .bind(provider)
    .bind(host)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Loads the dashboard's curated feed entries in newest-first order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_feed_items(pool: &SqlitePool) -> Result<Vec<FeedItem>, sqlx::Error> {
    sqlx::query_as::<_, FeedItem>(
        "SELECT id, category, source, title, summary, reading_minutes, published_at \
         FROM feed_items ORDER BY published_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Loads one user's RSS subscriptions in source-name order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when subscriptions cannot be loaded.
pub async fn list_rss_subscriptions(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<RssSubscription>, sqlx::Error> {
    sqlx::query_as::<_, RssSubscription>(
        "SELECT id, url, base_url, title, category, auto_delete_days, auto_delete_mode, \
         last_fetched_at, last_error, refresh_generation, created_at, updated_at \
         FROM rss_subscriptions WHERE user_id = ? ORDER BY title COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Loads RSS items owned through one user's subscriptions, newest first.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when entries cannot be loaded.
pub async fn list_rss_items(pool: &SqlitePool, user_id: &str) -> Result<Vec<RssItem>, sqlx::Error> {
    sqlx::query_as::<_, RssItem>(
        "SELECT i.id, i.subscription_id, s.title AS source, s.category, s.base_url, i.url, \
         i.comments_url, i.title, i.summary, i.published_at, i.fetched_at, i.read_at, rl.saved_at, \
         CASE WHEN s.refresh_generation > 0 AND i.last_seen_generation = s.refresh_generation \
              THEN 1 ELSE 0 END AS is_current \
         FROM rss_items i JOIN rss_subscriptions s ON s.id = i.subscription_id \
         LEFT JOIN rss_read_later rl ON rl.item_id = i.id AND rl.user_id = ? \
         WHERE s.user_id = ? ORDER BY datetime(i.published_at) DESC, i.fetched_at DESC",
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Loads the latest successful RSS snapshot for selected user-owned subscriptions.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when current entries cannot be loaded.
pub async fn list_current_rss_items(
    pool: &SqlitePool,
    user_id: &str,
    subscription_ids: &[String],
    limit: usize,
) -> Result<Vec<RssItem>, sqlx::Error> {
    if subscription_ids.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT i.id, i.subscription_id, s.title AS source, s.category, s.base_url, i.url, \
         i.comments_url, i.title, i.summary, i.published_at, i.fetched_at, i.read_at, rl.saved_at, \
         1 AS is_current FROM rss_items i \
         JOIN rss_subscriptions s ON s.id = i.subscription_id \
         LEFT JOIN rss_read_later rl ON rl.item_id = i.id AND rl.user_id = ",
    );
    query.push_bind(user_id);
    query.push(" WHERE s.user_id = ");
    query.push_bind(user_id);
    query.push(
        " AND s.refresh_generation > 0 \
         AND i.last_seen_generation = s.refresh_generation AND s.id IN (",
    );
    {
        let mut separated = query.separated(", ");
        for subscription_id in subscription_ids {
            separated.push_bind(subscription_id);
        }
    }
    query.push(") ORDER BY datetime(i.published_at) DESC, i.fetched_at DESC LIMIT ");
    query.push_bind(i64::try_from(limit).unwrap_or(i64::MAX));
    query.build_query_as::<RssItem>().fetch_all(pool).await
}

/// Loads one user-owned RSS subscription.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the subscription cannot be loaded.
pub async fn get_rss_subscription(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<Option<RssSubscription>, sqlx::Error> {
    sqlx::query_as::<_, RssSubscription>(
        "SELECT id, url, base_url, title, category, auto_delete_days, auto_delete_mode, \
         last_fetched_at, last_error, refresh_generation, created_at, updated_at \
         FROM rss_subscriptions WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Lists subscriptions whose refresh window has elapsed, least recently attempted first.
///
/// Scheduling uses the last attempt rather than the last success so a failing source backs off
/// for a full window instead of being retried on every sweep.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when due subscriptions cannot be loaded.
pub async fn list_due_rss_subscriptions(
    pool: &SqlitePool,
    due_before: &str,
    limit: usize,
) -> Result<Vec<RssRefreshTarget>, sqlx::Error> {
    sqlx::query_as::<_, RssRefreshTarget>(
        "SELECT id, user_id, url FROM rss_subscriptions \
         WHERE datetime(COALESCE(last_attempted_at, last_fetched_at, created_at)) \
               <= datetime(?) \
         ORDER BY datetime(COALESCE(last_attempted_at, last_fetched_at, created_at)) ASC \
         LIMIT ?",
    )
    .bind(due_before)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await
}

/// Claims one due subscription refresh by stamping its attempt timestamp.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the atomic update cannot be completed.
pub async fn claim_rss_subscription_refresh(
    pool: &SqlitePool,
    id: &str,
    due_before: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE rss_subscriptions SET last_attempted_at = ? WHERE id = ? \
         AND datetime(COALESCE(last_attempted_at, last_fetched_at, created_at)) <= datetime(?)",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .bind(due_before)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

async fn upsert_rss_items(
    transaction: &mut Transaction<'_, Sqlite>,
    subscription_id: &str,
    items: &[RssItemDraft],
    fetched_at: &str,
    generation: i64,
) -> Result<(), sqlx::Error> {
    for item in items {
        sqlx::query(
            "INSERT INTO rss_items \
             (id, subscription_id, external_id, url, comments_url, title, summary, published_at, \
              fetched_at, last_seen_generation) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(subscription_id, external_id) DO UPDATE SET \
             url = excluded.url, comments_url = excluded.comments_url, \
             title = excluded.title, summary = excluded.summary, \
             published_at = excluded.published_at, fetched_at = excluded.fetched_at, \
             last_seen_generation = excluded.last_seen_generation",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(subscription_id)
        .bind(&item.external_id)
        .bind(&item.url)
        .bind(&item.comments_url)
        .bind(&item.title)
        .bind(&item.summary)
        .bind(&item.published_at)
        .bind(fetched_at)
        .bind(generation)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

/// Creates a user-owned RSS subscription and stores its first fetched entries atomically.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be committed.
pub async fn create_rss_subscription(
    pool: &SqlitePool,
    user_id: &str,
    draft: &RssSubscriptionDraft,
    items: &[RssItemDraft],
) -> Result<RssSubscription, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO rss_subscriptions \
         (id, user_id, url, base_url, title, category, auto_delete_days, auto_delete_mode, \
          last_fetched_at, last_attempted_at, refresh_generation, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&draft.url)
    .bind(&draft.base_url)
    .bind(&draft.title)
    .bind(&draft.category)
    .bind(draft.auto_delete_days)
    .bind(&draft.auto_delete_mode)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    upsert_rss_items(&mut transaction, &id, items, &now, 1).await?;
    transaction.commit().await?;
    get_rss_subscription(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Updates category and retention settings for one user-owned subscription.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_rss_subscription(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    category: &str,
    auto_delete_days: Option<i64>,
    auto_delete_mode: &str,
) -> Result<Option<RssSubscription>, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE rss_subscriptions SET category = ?, auto_delete_days = ?, \
         auto_delete_mode = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(category)
    .bind(auto_delete_days)
    .bind(auto_delete_mode)
    .bind(updated_at)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    get_rss_subscription(pool, user_id, id).await
}

/// Stores a refresh result without replacing existing read state.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the refresh transaction cannot be committed.
pub async fn refresh_rss_subscription(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    title: &str,
    items: &[RssItemDraft],
) -> Result<Option<RssSubscription>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE rss_subscriptions SET title = ?, last_fetched_at = ?, last_attempted_at = ?, \
         last_error = NULL, refresh_generation = refresh_generation + 1, updated_at = ? \
         WHERE id = ? AND user_id = ?",
    )
    .bind(title)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() == 0 {
        transaction.rollback().await?;
        return Ok(None);
    }
    let generation: i64 = sqlx::query_scalar(
        "SELECT refresh_generation FROM rss_subscriptions WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&mut *transaction)
    .await?;
    upsert_rss_items(&mut transaction, id, items, &now, generation).await?;
    transaction.commit().await?;
    get_rss_subscription(pool, user_id, id).await
}

/// Records the latest refresh error for a user-owned subscription.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the status cannot be stored.
pub async fn set_rss_refresh_error(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    message: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE rss_subscriptions SET last_error = ?, last_attempted_at = ?, updated_at = ? \
         WHERE id = ? AND user_id = ?",
    )
    .bind(message)
    .bind(&now)
    .bind(&now)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes one user-owned subscription and its cascading entries.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_rss_subscription(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM rss_subscriptions WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Sets or clears the read timestamp for one user-owned RSS entry.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the entry cannot be updated or loaded.
pub async fn set_rss_item_read(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    read: bool,
) -> Result<Option<RssItem>, sqlx::Error> {
    let read_at = read.then(|| chrono::Utc::now().to_rfc3339());
    let result = sqlx::query(
        "UPDATE rss_items SET read_at = ? WHERE id = ? AND EXISTS (\
         SELECT 1 FROM rss_subscriptions s WHERE s.id = rss_items.subscription_id AND s.user_id = ?)",
    )
    .bind(read_at)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    sqlx::query_as::<_, RssItem>(
        "SELECT i.id, i.subscription_id, s.title AS source, s.category, s.base_url, i.url, \
         i.comments_url, i.title, i.summary, i.published_at, i.fetched_at, i.read_at, rl.saved_at, \
         CASE WHEN s.refresh_generation > 0 AND i.last_seen_generation = s.refresh_generation \
              THEN 1 ELSE 0 END AS is_current \
         FROM rss_items i JOIN rss_subscriptions s ON s.id = i.subscription_id \
         LEFT JOIN rss_read_later rl ON rl.item_id = i.id AND rl.user_id = ? \
         WHERE i.id = ? AND s.user_id = ?",
    )
    .bind(user_id)
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Saves or removes one user-owned RSS entry from Read Later.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when ownership cannot be checked or the row cannot be stored.
pub async fn set_rss_item_saved(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    saved: bool,
) -> Result<Option<RssItem>, sqlx::Error> {
    let owned: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM rss_items i JOIN rss_subscriptions s \
         ON s.id = i.subscription_id WHERE i.id = ? AND s.user_id = ?)",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if owned == 0 {
        return Ok(None);
    }
    if saved {
        sqlx::query(
            "INSERT INTO rss_read_later (user_id, item_id, saved_at) VALUES (?, ?, ?) \
             ON CONFLICT(user_id, item_id) DO UPDATE SET saved_at = excluded.saved_at",
        )
        .bind(user_id)
        .bind(id)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(pool)
        .await?;
    } else {
        sqlx::query("DELETE FROM rss_read_later WHERE user_id = ? AND item_id = ?")
            .bind(user_id)
            .bind(id)
            .execute(pool)
            .await?;
    }
    sqlx::query_as::<_, RssItem>(
        "SELECT i.id, i.subscription_id, s.title AS source, s.category, s.base_url, i.url, \
         i.comments_url, i.title, i.summary, i.published_at, i.fetched_at, i.read_at, rl.saved_at, \
         CASE WHEN s.refresh_generation > 0 AND i.last_seen_generation = s.refresh_generation \
              THEN 1 ELSE 0 END AS is_current \
         FROM rss_items i JOIN rss_subscriptions s ON s.id = i.subscription_id \
         LEFT JOIN rss_read_later rl ON rl.item_id = i.id AND rl.user_id = ? \
         WHERE i.id = ? AND s.user_id = ?",
    )
    .bind(user_id)
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Applies each subscription's automatic retention rule for one user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when expired entries cannot be removed.
pub async fn apply_rss_retention(pool: &SqlitePool, user_id: &str) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM rss_items WHERE NOT EXISTS (\
         SELECT 1 FROM rss_read_later rl WHERE rl.item_id = rss_items.id AND rl.user_id = ?) \
         AND NOT EXISTS (SELECT 1 FROM rss_subscriptions current \
         WHERE current.id = rss_items.subscription_id AND current.refresh_generation > 0 \
         AND rss_items.last_seen_generation = current.refresh_generation) \
         AND EXISTS (\
         SELECT 1 FROM rss_subscriptions s WHERE s.id = rss_items.subscription_id \
         AND s.user_id = ? AND s.auto_delete_days IS NOT NULL \
         AND datetime(rss_items.published_at) < datetime('now', '-' || s.auto_delete_days || ' days') \
         AND (s.auto_delete_mode = 'all' OR rss_items.read_at IS NOT NULL))",
    )
    .bind(user_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected())
}

/// Prunes entries older than the requested age across all of one user's subscriptions.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when matching entries cannot be removed.
pub async fn prune_rss_items(
    pool: &SqlitePool,
    user_id: &str,
    days: i64,
    mode: &str,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM rss_items WHERE NOT EXISTS (\
         SELECT 1 FROM rss_read_later rl WHERE rl.item_id = rss_items.id AND rl.user_id = ?) \
         AND NOT EXISTS (SELECT 1 FROM rss_subscriptions current \
         WHERE current.id = rss_items.subscription_id AND current.refresh_generation > 0 \
         AND rss_items.last_seen_generation = current.refresh_generation) \
         AND EXISTS (\
         SELECT 1 FROM rss_subscriptions s WHERE s.id = rss_items.subscription_id AND s.user_id = ?) \
         AND datetime(published_at) < datetime('now', '-' || ? || ' days') \
         AND (? = 'all' OR read_at IS NOT NULL)",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(days)
    .bind(mode)
    .execute(pool)
    .await?
    .rows_affected())
}

/// Loads every journal node owned by one user in stable tree order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when journal nodes cannot be loaded.
pub async fn list_journal_nodes(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<JournalNode>, sqlx::Error> {
    sqlx::query_as::<_, JournalNode>(
        "SELECT id, parent_id, name, content, position, created_at, updated_at \
         FROM journal_nodes WHERE user_id = ? \
         ORDER BY parent_id, position ASC, name COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Loads one user-owned journal node.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the node cannot be loaded.
pub async fn get_journal_node(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<Option<JournalNode>, sqlx::Error> {
    sqlx::query_as::<_, JournalNode>(
        "SELECT id, parent_id, name, content, position, created_at, updated_at \
         FROM journal_nodes WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Reports whether a sibling already uses a case-insensitive journal name.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the sibling lookup fails.
pub async fn journal_sibling_name_exists(
    pool: &SqlitePool,
    user_id: &str,
    parent_id: Option<&str>,
    name: &str,
    excluding_id: Option<&str>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM journal_nodes WHERE user_id = ? \
         AND ((parent_id IS NULL AND ? IS NULL) OR parent_id = ?) \
         AND lower(name) = lower(?) AND (? IS NULL OR id != ?))",
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(parent_id)
    .bind(name)
    .bind(excluding_id)
    .bind(excluding_id)
    .fetch_one(pool)
    .await
}

/// Creates a journal document beneath an optional parent document.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the node cannot be inserted.
pub async fn create_journal_node(
    pool: &SqlitePool,
    user_id: &str,
    parent_id: Option<&str>,
    name: &str,
    content: &str,
) -> Result<JournalNode, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM journal_nodes WHERE user_id = ? \
         AND ((parent_id IS NULL AND ? IS NULL) OR parent_id = ?)",
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(parent_id)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "INSERT INTO journal_nodes \
         (id, user_id, parent_id, name, content, position, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(parent_id)
    .bind(name)
    .bind(content)
    .bind(position)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_journal_node(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Updates a journal node and atomically normalizes user-defined sibling order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the node cannot be updated.
pub async fn update_journal_node(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    parent_id: Option<&str>,
    name: &str,
    content: &str,
    requested_position: Option<i64>,
) -> Result<Option<JournalNode>, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let current = sqlx::query_as::<_, JournalNode>(
        "SELECT id, parent_id, name, content, position, created_at, updated_at \
         FROM journal_nodes WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(current) = current else {
        transaction.rollback().await?;
        return Ok(None);
    };

    let parent_changed = current.parent_id.as_deref() != parent_id;
    let source_order = sqlx::query_scalar::<_, String>(
        "SELECT id FROM journal_nodes WHERE user_id = ? \
         AND ((parent_id IS NULL AND ? IS NULL) OR parent_id = ?) \
         ORDER BY position ASC, name COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .bind(current.parent_id.as_deref())
    .bind(current.parent_id.as_deref())
    .fetch_all(&mut *transaction)
    .await?;
    let mut destination_order = if parent_changed {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM journal_nodes WHERE user_id = ? AND id != ? \
             AND ((parent_id IS NULL AND ? IS NULL) OR parent_id = ?) \
             ORDER BY position ASC, name COLLATE NOCASE ASC",
        )
        .bind(user_id)
        .bind(id)
        .bind(parent_id)
        .bind(parent_id)
        .fetch_all(&mut *transaction)
        .await?
    } else {
        source_order
            .iter()
            .filter(|sibling_id| sibling_id.as_str() != id)
            .cloned()
            .collect()
    };

    let position = requested_position
        .and_then(|position| usize::try_from(position).ok())
        .unwrap_or_else(|| {
            if parent_changed {
                destination_order.len()
            } else {
                source_order
                    .iter()
                    .position(|sibling_id| sibling_id == id)
                    .unwrap_or(destination_order.len())
            }
        })
        .min(destination_order.len());
    destination_order.insert(position, id.to_owned());

    let result = sqlx::query(
        "UPDATE journal_nodes SET parent_id = ?, name = ?, content = ?, position = ?, updated_at = ? \
         WHERE id = ? AND user_id = ?",
    )
    .bind(parent_id)
    .bind(name)
    .bind(content)
    .bind(i64::try_from(position).unwrap_or(i64::MAX))
    .bind(&updated_at)
    .bind(id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() == 0 {
        transaction.rollback().await?;
        return Ok(None);
    }

    for (position, sibling_id) in destination_order.iter().enumerate() {
        sqlx::query("UPDATE journal_nodes SET position = ? WHERE id = ? AND user_id = ?")
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .bind(sibling_id)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
    }
    if parent_changed {
        for (position, sibling_id) in source_order
            .iter()
            .filter(|sibling_id| sibling_id.as_str() != id)
            .enumerate()
        {
            sqlx::query("UPDATE journal_nodes SET position = ? WHERE id = ? AND user_id = ?")
                .bind(i64::try_from(position).unwrap_or(i64::MAX))
                .bind(sibling_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
        }
    }
    transaction.commit().await?;
    get_journal_node(pool, user_id, id).await
}

/// Reports whether moving a document beneath a candidate parent would form a cycle.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when descendants cannot be inspected.
pub async fn journal_move_would_cycle(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
    parent_id: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "WITH RECURSIVE descendants(id) AS (\
         SELECT id FROM journal_nodes WHERE parent_id = ? AND user_id = ? \
         UNION ALL SELECT child.id FROM journal_nodes child \
         JOIN descendants d ON child.parent_id = d.id WHERE child.user_id = ?) \
         SELECT EXISTS(SELECT 1 FROM descendants WHERE id = ?)",
    )
    .bind(id)
    .bind(user_id)
    .bind(user_id)
    .bind(parent_id)
    .fetch_one(pool)
    .await
}

/// Deletes a user-owned journal node and its descendants.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the node cannot be deleted.
pub async fn delete_journal_node(
    pool: &SqlitePool,
    user_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM journal_nodes WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Loads one user's widgets in workspace and layout order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_dashboard_widgets(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<DashboardWidget>, sqlx::Error> {
    sqlx::query_as::<_, DashboardWidgetRow>(
        "SELECT w.id, w.kind, w.workspace, w.position, w.size, w.grid_x, w.grid_y, w.grid_w, w.grid_h, w.config_json, EXISTS(SELECT 1 FROM widget_secrets s WHERE s.widget_id = w.id) AS has_secret, w.created_at, w.updated_at FROM dashboard_widgets w WHERE w.user_id = ? ORDER BY w.workspace ASC, w.grid_y ASC, w.grid_x ASC, w.created_at ASC",
    )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map(|rows| rows.into_iter().map(DashboardWidget::from).collect())
}

/// Loads one widget when it belongs to the requested user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_dashboard_widget(
    pool: &SqlitePool,
    user_id: &str,
    widget_id: &str,
) -> Result<Option<DashboardWidget>, sqlx::Error> {
    sqlx::query_as::<_, DashboardWidgetRow>(
        "SELECT w.id, w.kind, w.workspace, w.position, w.size, w.grid_x, w.grid_y, w.grid_w, w.grid_h, w.config_json, EXISTS(SELECT 1 FROM widget_secrets s WHERE s.widget_id = w.id) AS has_secret, w.created_at, w.updated_at FROM dashboard_widgets w WHERE w.user_id = ? AND w.id = ?",
    )
        .bind(user_id)
        .bind(widget_id)
        .fetch_optional(pool)
        .await
        .map(|row| row.map(DashboardWidget::from))
}

/// Adds one widget at the end of a user's selected workspace.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn create_dashboard_widget(
    pool: &SqlitePool,
    user_id: &str,
    kind: &str,
    workspace: i64,
    size: &str,
) -> Result<DashboardWidget, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM dashboard_widgets WHERE user_id = ? AND workspace = ?",
    )
    .bind(user_id)
    .bind(workspace)
    .fetch_one(pool)
    .await?;
    let grid_y: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(grid_y + grid_h), 0) FROM dashboard_widgets WHERE user_id = ? AND workspace = ?",
    )
    .bind(user_id)
    .bind(workspace)
    .fetch_one(pool)
    .await?;
    let (grid_w, grid_h) = match size {
        "compact" => (4, 4),
        "standard" => (6, 4),
        "wide" => (8, 5),
        _ => (12, 6),
    };

    sqlx::query(
        "INSERT INTO dashboard_widgets (id, user_id, kind, workspace, position, size, grid_x, grid_y, grid_w, grid_h, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(kind)
    .bind(workspace)
    .bind(position)
    .bind(size)
    .bind(grid_y)
    .bind(grid_w)
    .bind(grid_h)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(DashboardWidget {
        id,
        kind: kind.to_owned(),
        workspace,
        position,
        size: size.to_owned(),
        grid_x: 0,
        grid_y,
        grid_w,
        grid_h,
        config: serde_json::json!({}),
        has_secret: false,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Replaces one owned widget's public configuration.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_dashboard_widget_config(
    pool: &SqlitePool,
    user_id: &str,
    widget_id: &str,
    config_json: &str,
) -> Result<Option<DashboardWidget>, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE dashboard_widgets SET config_json = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(config_json)
    .bind(updated_at)
    .bind(widget_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(None);
    }
    get_dashboard_widget(pool, user_id, widget_id).await
}

/// Atomically replaces public configuration and optionally sets or clears its credential.
///
/// The secret value retains, clears, or replaces the current encrypted credential.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn update_dashboard_widget_integration(
    pool: &SqlitePool,
    user_id: &str,
    widget_id: &str,
    config_json: &str,
    secret: Option<Option<&str>>,
) -> Result<Option<DashboardWidget>, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE dashboard_widgets SET config_json = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(config_json)
    .bind(&updated_at)
    .bind(widget_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() != 1 {
        transaction.rollback().await?;
        return Ok(None);
    }
    match secret {
        Some(Some(ciphertext)) => {
            sqlx::query(
                "INSERT INTO widget_secrets (widget_id, user_id, ciphertext, updated_at) VALUES (?, ?, ?, ?) ON CONFLICT(widget_id) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = excluded.updated_at",
            )
            .bind(widget_id)
            .bind(user_id)
            .bind(ciphertext)
            .bind(&updated_at)
            .execute(&mut *transaction)
            .await?;
        }
        Some(None) => {
            sqlx::query("DELETE FROM widget_secrets WHERE widget_id = ? AND user_id = ?")
                .bind(widget_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
        }
        None => {}
    }
    transaction.commit().await?;
    get_dashboard_widget(pool, user_id, widget_id).await
}

/// Inserts or replaces one owned widget's encrypted credential.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the write cannot be completed.
pub async fn upsert_widget_secret(
    pool: &SqlitePool,
    user_id: &str,
    widget_id: &str,
    ciphertext: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO widget_secrets (widget_id, user_id, ciphertext, updated_at) SELECT id, user_id, ?, ? FROM dashboard_widgets WHERE id = ? AND user_id = ? ON CONFLICT(widget_id) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = excluded.updated_at",
    )
    .bind(ciphertext)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(widget_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Loads one encrypted widget credential without exposing it through widget serialization.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_widget_secret(
    pool: &SqlitePool,
    user_id: &str,
    widget_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT ciphertext FROM widget_secrets WHERE widget_id = ? AND user_id = ?")
        .bind(widget_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

/// Removes an owned widget's stored credential.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_widget_secret(
    pool: &SqlitePool,
    user_id: &str,
    widget_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM widget_secrets WHERE widget_id = ? AND user_id = ?")
            .bind(widget_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}

/// Atomically replaces the authenticated user's complete widget layout.
///
/// Returns None if any identifier does not belong to the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn update_dashboard_widget_layout(
    pool: &SqlitePool,
    user_id: &str,
    widgets: &[(String, i64, i64, String, i64, i64, i64, i64)],
) -> Result<Option<Vec<DashboardWidget>>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;

    for (id, workspace, position, size, grid_x, grid_y, grid_w, grid_h) in widgets {
        let result = sqlx::query(
            "UPDATE dashboard_widgets SET workspace = ?, position = ?, size = ?, grid_x = ?, grid_y = ?, grid_w = ?, grid_h = ?, updated_at = ? WHERE id = ? AND user_id = ?",
        )
        .bind(workspace)
        .bind(position)
        .bind(size)
        .bind(grid_x)
        .bind(grid_y)
        .bind(grid_w)
        .bind(grid_h)
        .bind(&now)
        .bind(id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok(None);
        }
    }

    transaction.commit().await?;
    Ok(Some(list_dashboard_widgets(pool, user_id).await?))
}

/// Deletes one widget owned by the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_dashboard_widget(
    pool: &SqlitePool,
    user_id: &str,
    widget_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM dashboard_widgets WHERE id = ? AND user_id = ?")
            .bind(widget_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}

/// Lists the authenticated account's bookmarks in title order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_bookmarks(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<Bookmark>, sqlx::Error> {
    sqlx::query_as::<_, Bookmark>(
        "SELECT id, title, url, favicon_data IS NOT NULL AS has_favicon, created_at, updated_at \
         FROM bookmarks WHERE user_id = ? \
         ORDER BY title COLLATE NOCASE ASC, created_at ASC, id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Counts bookmarks owned by one account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn count_bookmarks(pool: &SqlitePool, user_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM bookmarks WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
}

/// Finds an account-owned bookmark by its normalized destination.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_bookmark_by_url(
    pool: &SqlitePool,
    user_id: &str,
    url: &str,
) -> Result<Option<Bookmark>, sqlx::Error> {
    sqlx::query_as::<_, Bookmark>(
        "SELECT id, title, url, favicon_data IS NOT NULL AS has_favicon, created_at, updated_at \
         FROM bookmarks WHERE user_id = ? AND url = ?",
    )
    .bind(user_id)
    .bind(url)
    .fetch_optional(pool)
    .await
}

/// Creates one account-owned bookmark with an optional cached favicon.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert or follow-up query fails.
pub async fn create_bookmark(
    pool: &SqlitePool,
    user_id: &str,
    title: &str,
    url: &str,
    favicon: Option<(&str, &[u8])>,
) -> Result<Option<Bookmark>, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let (favicon_content_type, favicon_data, favicon_fetched_at) = favicon
        .map_or((None, None, None), |(content_type, data)| {
            (Some(content_type), Some(data), Some(now.as_str()))
        });
    let result = sqlx::query(
        "INSERT INTO bookmarks \
         (id, user_id, title, url, favicon_content_type, favicon_data, favicon_fetched_at, \
          created_at, updated_at) \
         SELECT ?, ?, ?, ?, ?, ?, ?, ?, ? \
         WHERE (SELECT COUNT(*) FROM bookmarks WHERE user_id = ?) < 32",
    )
    .bind(&id)
    .bind(user_id)
    .bind(title)
    .bind(url)
    .bind(favicon_content_type)
    .bind(favicon_data)
    .bind(favicon_fetched_at)
    .bind(&now)
    .bind(&now)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    Ok(Some(sqlx::query_as::<_, Bookmark>(
        "SELECT id, title, url, favicon_data IS NOT NULL AS has_favicon, created_at, updated_at \
         FROM bookmarks WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await?))
}

/// Loads cached favicon bytes only when the bookmark belongs to the account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_bookmark_favicon(
    pool: &SqlitePool,
    user_id: &str,
    bookmark_id: &str,
) -> Result<Option<BookmarkFavicon>, sqlx::Error> {
    sqlx::query_as::<_, BookmarkFavicon>(
        "SELECT favicon_content_type AS content_type, favicon_data AS data \
         FROM bookmarks WHERE id = ? AND user_id = ? AND favicon_data IS NOT NULL",
    )
    .bind(bookmark_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Deletes one bookmark owned by the authenticated account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_bookmark(
    pool: &SqlitePool,
    user_id: &str,
    bookmark_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM bookmarks WHERE id = ? AND user_id = ?")
            .bind(bookmark_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}

async fn insert_default_widgets(
    transaction: &mut Transaction<'_, Sqlite>,
    user_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    for (kind, workspace, position, size, grid_x, grid_y, grid_w, grid_h) in DEFAULT_WIDGETS {
        sqlx::query(
            "INSERT INTO dashboard_widgets (id, user_id, kind, workspace, position, size, grid_x, grid_y, grid_w, grid_h, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(kind)
        .bind(workspace)
        .bind(position)
        .bind(size)
        .bind(grid_x)
        .bind(grid_y)
        .bind(grid_w)
        .bind(grid_h)
        .bind(now)
        .bind(now)
        .execute(&mut **transaction)
        .await?;
    }
    sqlx::query(
        "INSERT INTO dashboard_widgets (id, user_id, kind, workspace, position, size, config_json, grid_x, grid_y, grid_w, grid_h, created_at, updated_at) VALUES (?, ?, 'streams', 0, ?, 'standard', ?, 0, 0, 4, 4, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(i64::try_from(DEFAULT_WIDGETS.len()).expect("default widget count fits i64"))
    .bind(r#"{"placement":"utility_rail","twitch_channels":[],"kick_channels":[]}"#)
    .bind(now)
    .bind(now)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_default_workspaces(
    transaction: &mut Transaction<'_, Sqlite>,
    user_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_workspaces \
         (user_id, workspace, name, position, created_at, updated_at) \
         VALUES (?, 0, 'Dashboard', 0, ?, ?)",
    )
    .bind(user_id)
    .bind(now)
    .bind(now)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Lists the instance-wide embedded pages in their administrator-defined order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_global_embedded_pages(
    pool: &SqlitePool,
) -> Result<Vec<EmbeddedPage>, sqlx::Error> {
    sqlx::query_as::<_, EmbeddedPage>(
        "SELECT id, scope, owner_user_id, created_by_user_id, title, description, url, icon_url, \
         allow_scripts, allow_same_origin, iframe_height, position, created_at, updated_at \
         FROM embedded_pages \
         WHERE scope = 'global' AND owner_user_id IS NULL \
         ORDER BY position ASC, created_at ASC, id ASC",
    )
    .fetch_all(pool)
    .await
}

/// Lists one account's private embedded pages in their chosen order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_personal_embedded_pages(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<EmbeddedPage>, sqlx::Error> {
    sqlx::query_as::<_, EmbeddedPage>(
        "SELECT id, scope, owner_user_id, created_by_user_id, title, description, url, icon_url, \
         allow_scripts, allow_same_origin, iframe_height, position, created_at, updated_at \
         FROM embedded_pages \
         WHERE scope = 'user' AND owner_user_id = ? \
         ORDER BY position ASC, created_at ASC, id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Counts the instance-wide embedded pages.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn count_global_embedded_pages(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM embedded_pages \
         WHERE scope = 'global' AND owner_user_id IS NULL",
    )
    .fetch_one(pool)
    .await
}

/// Counts one account's private embedded pages.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn count_personal_embedded_pages(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM embedded_pages \
         WHERE scope = 'user' AND owner_user_id = ?",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Creates a private embedded page at the end of one account's tier.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn create_personal_embedded_page(
    pool: &SqlitePool,
    user_id: &str,
    title: &str,
    description: &str,
    url: &str,
    icon_url: Option<&str>,
    allow_scripts: bool,
    allow_same_origin: bool,
    iframe_height: i64,
) -> Result<EmbeddedPage, sqlx::Error> {
    create_embedded_page(
        pool,
        "user",
        Some(user_id),
        user_id,
        title,
        description,
        url,
        icon_url,
        allow_scripts,
        allow_same_origin,
        iframe_height,
    )
    .await
}

/// Creates an instance-wide embedded page at the end of the global tier.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn create_global_embedded_page(
    pool: &SqlitePool,
    administrator_id: &str,
    title: &str,
    description: &str,
    url: &str,
    icon_url: Option<&str>,
    allow_scripts: bool,
    allow_same_origin: bool,
    iframe_height: i64,
) -> Result<EmbeddedPage, sqlx::Error> {
    create_embedded_page(
        pool,
        "global",
        None,
        administrator_id,
        title,
        description,
        url,
        icon_url,
        allow_scripts,
        allow_same_origin,
        iframe_height,
    )
    .await
}

async fn create_embedded_page(
    pool: &SqlitePool,
    scope: &str,
    owner_user_id: Option<&str>,
    created_by_user_id: &str,
    title: &str,
    description: &str,
    url: &str,
    icon_url: Option<&str>,
    allow_scripts: bool,
    allow_same_origin: bool,
    iframe_height: i64,
) -> Result<EmbeddedPage, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let position: i64 = if let Some(owner_user_id) = owner_user_id {
        sqlx::query_scalar(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM embedded_pages \
             WHERE scope = 'user' AND owner_user_id = ?",
        )
        .bind(owner_user_id)
        .fetch_one(&mut *transaction)
        .await?
    } else {
        sqlx::query_scalar(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM embedded_pages \
             WHERE scope = 'global' AND owner_user_id IS NULL",
        )
        .fetch_one(&mut *transaction)
        .await?
    };

    sqlx::query(
        "INSERT INTO embedded_pages \
         (id, scope, owner_user_id, created_by_user_id, title, description, url, icon_url, \
          allow_scripts, allow_same_origin, iframe_height, position, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(scope)
    .bind(owner_user_id)
    .bind(created_by_user_id)
    .bind(title)
    .bind(description)
    .bind(url)
    .bind(icon_url)
    .bind(allow_scripts)
    .bind(allow_same_origin)
    .bind(iframe_height)
    .bind(position)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(EmbeddedPage {
        id,
        scope: scope.to_owned(),
        owner_user_id: owner_user_id.map(str::to_owned),
        created_by_user_id: Some(created_by_user_id.to_owned()),
        title: title.to_owned(),
        description: description.to_owned(),
        url: url.to_owned(),
        icon_url: icon_url.map(str::to_owned),
        allow_scripts,
        allow_same_origin,
        iframe_height,
        position,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Updates a private embedded page owned by one account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_personal_embedded_page(
    pool: &SqlitePool,
    user_id: &str,
    page_id: &str,
    title: &str,
    description: &str,
    url: &str,
    icon_url: Option<&str>,
    allow_scripts: bool,
    allow_same_origin: bool,
    iframe_height: i64,
) -> Result<Option<EmbeddedPage>, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE embedded_pages SET title = ?, description = ?, url = ?, icon_url = ?, \
         allow_scripts = ?, allow_same_origin = ?, iframe_height = ?, updated_at = ? \
         WHERE id = ? AND scope = 'user' AND owner_user_id = ?",
    )
    .bind(title)
    .bind(description)
    .bind(url)
    .bind(icon_url)
    .bind(allow_scripts)
    .bind(allow_same_origin)
    .bind(iframe_height)
    .bind(&updated_at)
    .bind(page_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(None);
    }

    sqlx::query_as::<_, EmbeddedPage>(
        "SELECT id, scope, owner_user_id, created_by_user_id, title, description, url, icon_url, \
         allow_scripts, allow_same_origin, iframe_height, position, created_at, updated_at \
         FROM embedded_pages WHERE id = ?",
    )
    .bind(page_id)
    .fetch_optional(pool)
    .await
}

/// Updates one instance-wide embedded page.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_global_embedded_page(
    pool: &SqlitePool,
    page_id: &str,
    title: &str,
    description: &str,
    url: &str,
    icon_url: Option<&str>,
    allow_scripts: bool,
    allow_same_origin: bool,
    iframe_height: i64,
) -> Result<Option<EmbeddedPage>, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE embedded_pages SET title = ?, description = ?, url = ?, icon_url = ?, \
         allow_scripts = ?, allow_same_origin = ?, iframe_height = ?, updated_at = ? \
         WHERE id = ? AND scope = 'global' AND owner_user_id IS NULL",
    )
    .bind(title)
    .bind(description)
    .bind(url)
    .bind(icon_url)
    .bind(allow_scripts)
    .bind(allow_same_origin)
    .bind(iframe_height)
    .bind(&updated_at)
    .bind(page_id)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(None);
    }

    sqlx::query_as::<_, EmbeddedPage>(
        "SELECT id, scope, owner_user_id, created_by_user_id, title, description, url, icon_url, \
         allow_scripts, allow_same_origin, iframe_height, position, created_at, updated_at \
         FROM embedded_pages WHERE id = ?",
    )
    .bind(page_id)
    .fetch_optional(pool)
    .await
}

/// Deletes a private embedded page owned by one account and closes its ordering gap.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn delete_personal_embedded_page(
    pool: &SqlitePool,
    user_id: &str,
    page_id: &str,
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let position = sqlx::query_scalar::<_, i64>(
        "SELECT position FROM embedded_pages \
         WHERE id = ? AND scope = 'user' AND owner_user_id = ?",
    )
    .bind(page_id)
    .bind(user_id)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(position) = position else {
        transaction.rollback().await?;
        return Ok(false);
    };
    sqlx::query(
        "DELETE FROM embedded_pages \
         WHERE id = ? AND scope = 'user' AND owner_user_id = ?",
    )
    .bind(page_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE embedded_pages SET position = position - 1 \
         WHERE scope = 'user' AND owner_user_id = ? AND position > ?",
    )
    .bind(user_id)
    .bind(position)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(true)
}

/// Deletes an instance-wide embedded page and closes its ordering gap.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn delete_global_embedded_page(
    pool: &SqlitePool,
    page_id: &str,
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let position = sqlx::query_scalar::<_, i64>(
        "SELECT position FROM embedded_pages \
         WHERE id = ? AND scope = 'global' AND owner_user_id IS NULL",
    )
    .bind(page_id)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(position) = position else {
        transaction.rollback().await?;
        return Ok(false);
    };
    sqlx::query(
        "DELETE FROM embedded_pages \
         WHERE id = ? AND scope = 'global' AND owner_user_id IS NULL",
    )
    .bind(page_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE embedded_pages SET position = position - 1 \
         WHERE scope = 'global' AND owner_user_id IS NULL AND position > ?",
    )
    .bind(position)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(true)
}

/// Replaces one account's complete private embedded-page order atomically.
///
/// Returns `None` when the identifiers are incomplete, duplicated, or not owned by the account.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn reorder_personal_embedded_pages(
    pool: &SqlitePool,
    user_id: &str,
    page_ids: &[String],
) -> Result<Option<Vec<EmbeddedPage>>, sqlx::Error> {
    reorder_embedded_pages(pool, Some(user_id), page_ids).await?;
    let pages = list_personal_embedded_pages(pool, user_id).await?;
    if pages.len() == page_ids.len()
        && pages
            .iter()
            .zip(page_ids)
            .all(|(page, expected_id)| page.id == *expected_id)
    {
        Ok(Some(pages))
    } else {
        Ok(None)
    }
}

/// Replaces the complete instance-wide embedded-page order atomically.
///
/// Returns `None` when the identifiers are incomplete, duplicated, or not global pages.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be completed.
pub async fn reorder_global_embedded_pages(
    pool: &SqlitePool,
    page_ids: &[String],
) -> Result<Option<Vec<EmbeddedPage>>, sqlx::Error> {
    reorder_embedded_pages(pool, None, page_ids).await?;
    let pages = list_global_embedded_pages(pool).await?;
    if pages.len() == page_ids.len()
        && pages
            .iter()
            .zip(page_ids)
            .all(|(page, expected_id)| page.id == *expected_id)
    {
        Ok(Some(pages))
    } else {
        Ok(None)
    }
}

async fn reorder_embedded_pages(
    pool: &SqlitePool,
    owner_user_id: Option<&str>,
    page_ids: &[String],
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let existing_ids = if let Some(owner_user_id) = owner_user_id {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM embedded_pages \
             WHERE scope = 'user' AND owner_user_id = ? ORDER BY position ASC, id ASC",
        )
        .bind(owner_user_id)
        .fetch_all(&mut *transaction)
        .await?
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM embedded_pages \
             WHERE scope = 'global' AND owner_user_id IS NULL ORDER BY position ASC, id ASC",
        )
        .fetch_all(&mut *transaction)
        .await?
    };
    let valid = existing_ids.len() == page_ids.len()
        && page_ids
            .iter()
            .enumerate()
            .all(|(index, id)| !page_ids[..index].contains(id) && existing_ids.contains(id));
    if !valid {
        transaction.rollback().await?;
        return Ok(());
    }

    let updated_at = chrono::Utc::now().to_rfc3339();
    for (position, page_id) in page_ids.iter().enumerate() {
        let result = if let Some(owner_user_id) = owner_user_id {
            sqlx::query(
                "UPDATE embedded_pages SET position = ?, updated_at = ? \
                 WHERE id = ? AND scope = 'user' AND owner_user_id = ?",
            )
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .bind(&updated_at)
            .bind(page_id)
            .bind(owner_user_id)
            .execute(&mut *transaction)
            .await?
        } else {
            sqlx::query(
                "UPDATE embedded_pages SET position = ?, updated_at = ? \
                 WHERE id = ? AND scope = 'global' AND owner_user_id IS NULL",
            )
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .bind(&updated_at)
            .bind(page_id)
            .execute(&mut *transaction)
            .await?
        };
        if result.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok(());
        }
    }
    transaction.commit().await?;
    Ok(())
}

/// Creates an account, its settings, and a small starter task set atomically.
///
/// # Errors
///
/// Returns the underlying `SQLx` error if any account setup step fails.
pub async fn create_account(
    pool: &SqlitePool,
    email: &str,
    password_hash: &str,
    display_name: &str,
) -> Result<(User, UserSettings), sqlx::Error> {
    let user_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, role, created_at) \
         VALUES (?, ?, ?, 'member', ?)",
    )
    .bind(&user_id)
    .bind(email)
    .bind(password_hash)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        "INSERT INTO user_settings \
         (user_id, display_name, location, timezone, temperature_unit, updated_at) \
         VALUES (?, ?, 'London', 'UTC', 'celsius', ?)",
    )
    .bind(&user_id)
    .bind(display_name)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;

    for (title, completed, priority) in [
        ("Review today’s notes", true, "none"),
        ("Plan the next focus block", false, "p1"),
        ("Organize saved references", false, "none"),
    ] {
        sqlx::query(
            "INSERT INTO tasks \
             (id, title, completed, priority, created_at, updated_at, user_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(title)
        .bind(completed)
        .bind(priority)
        .bind(&now)
        .bind(&now)
        .bind(&user_id)
        .execute(&mut *transaction)
        .await?;
    }

    insert_default_workspaces(&mut transaction, &user_id, &now).await?;
    insert_default_widgets(&mut transaction, &user_id, &now).await?;

    transaction.commit().await?;

    Ok((
        User {
            id: user_id.clone(),
            email: email.to_owned(),
            role: "member".to_owned(),
            created_at: now.clone(),
        },
        UserSettings {
            user_id,
            display_name: display_name.to_owned(),
            location: "London".to_owned(),
            timezone: "UTC".to_owned(),
            sidebar_timezones: vec!["UTC".to_owned()],
            calendar_week_start: "sunday".to_owned(),
            temperature_unit: "celsius".to_owned(),
            lines_default_visibility: "private".to_owned(),
            podcast_playback_rate: 1.0,
            updated_at: now,
        },
    ))
}

/// Loads the private password credential record for a normalized email address.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_user_credentials(
    pool: &SqlitePool,
    email: &str,
) -> Result<Option<UserCredentials>, sqlx::Error> {
    sqlx::query_as::<_, UserCredentials>(
        "SELECT id, email, password_hash, role, created_at FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

/// Persists an opaque session token.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn create_session(
    pool: &SqlitePool,
    token: &str,
    user_id: &str,
    user_agent: &str,
    ip_address: &str,
    expires_at: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO sessions (token, id, user_id, user_agent, ip_address, expires_at, created_at, last_seen_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(token)
    .bind(id)
    .bind(user_id)
    .bind(user_agent)
    .bind(ip_address)
    .bind(expires_at)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
        .bind(&now)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

/// Refreshes the last observed metadata for one session.
///
/// Repeated requests from an unchanged client update the timestamp at most once every five
/// minutes, while a changed user agent or address is recorded immediately.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn touch_session(
    pool: &SqlitePool,
    token: &str,
    user_agent: &str,
    ip_address: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now();
    let refresh_before = (now - chrono::Duration::minutes(5)).to_rfc3339();
    let now = now.to_rfc3339();
    sqlx::query(
        "UPDATE sessions \
         SET user_agent = ?, ip_address = ?, last_seen_at = ? \
         WHERE token = ? AND (user_agent != ? OR ip_address != ? OR last_seen_at < ?)",
    )
    .bind(user_agent)
    .bind(ip_address)
    .bind(now)
    .bind(token)
    .bind(user_agent)
    .bind(ip_address)
    .bind(refresh_before)
    .execute(pool)
    .await?;
    Ok(())
}

/// Lists the unexpired sessions owned by one account, newest activity first.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_account_sessions(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<AccountSession>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query_as::<_, AccountSession>(
        "SELECT id, token, user_agent, ip_address \
         FROM sessions \
         WHERE user_id = ? AND expires_at > ? \
         ORDER BY last_seen_at DESC, created_at DESC",
    )
    .bind(user_id)
    .bind(now)
    .fetch_all(pool)
    .await
}

/// Deletes one account-owned session by its public identifier and returns its private token.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_account_session(
    pool: &SqlitePool,
    user_id: &str,
    session_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("DELETE FROM sessions WHERE id = ? AND user_id = ? RETURNING token")
        .bind(session_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

/// Resolves an unexpired session to the owning account and its settings.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_session_account(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<SessionAccount>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query_as::<_, SessionAccount>(
        "SELECT users.id, users.email, users.role, users.created_at, \
                user_settings.display_name, user_settings.location, user_settings.timezone, \
                user_settings.sidebar_timezones_json, user_settings.calendar_week_start, \
                user_settings.temperature_unit, \
                user_settings.lines_default_visibility, \
                user_settings.podcast_playback_rate, \
                user_settings.updated_at AS settings_updated_at \
         FROM sessions \
         JOIN users ON users.id = sessions.user_id \
         JOIN user_settings ON user_settings.user_id = users.id \
         WHERE sessions.token = ? AND sessions.expires_at > ?",
    )
    .bind(token)
    .bind(now)
    .fetch_optional(pool)
    .await
}

/// Deletes one session token.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_session(pool: &SqlitePool, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Updates only the authenticated user's settings.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update or reload cannot be completed.
pub async fn update_user_settings(
    pool: &SqlitePool,
    user_id: &str,
    display_name: &str,
    location: &str,
    timezone: &str,
    sidebar_timezones_json: &str,
    calendar_week_start: &str,
    temperature_unit: &str,
    lines_default_visibility: &str,
    podcast_playback_rate: f64,
) -> Result<UserSettings, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE user_settings SET display_name = ?, location = ?, timezone = ?, \
         sidebar_timezones_json = ?, calendar_week_start = ?, temperature_unit = ?, \
         lines_default_visibility = ?, \
         podcast_playback_rate = ?, updated_at = ? WHERE user_id = ?",
    )
    .bind(display_name)
    .bind(location)
    .bind(timezone)
    .bind(sidebar_timezones_json)
    .bind(calendar_week_start)
    .bind(temperature_unit)
    .bind(lines_default_visibility)
    .bind(podcast_playback_rate)
    .bind(&updated_at)
    .bind(user_id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, UserSettingsRecord>(
        "SELECT user_id, display_name, location, timezone, sidebar_timezones_json, \
                calendar_week_start, temperature_unit, lines_default_visibility, \
                podcast_playback_rate, updated_at \
         FROM user_settings WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map(UserSettingsRecord::into_settings)
}

/// Stores one background image for one of the authenticated user's workspaces.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the upsert cannot be completed.
pub async fn upsert_user_background(
    pool: &SqlitePool,
    user_id: &str,
    workspace: i64,
    mime_type: &str,
    image_data: &[u8],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_backgrounds (user_id, workspace, mime_type, image_data, updated_at) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(user_id, workspace) DO UPDATE SET \
         mime_type = excluded.mime_type, image_data = excluded.image_data, \
         updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(workspace)
    .bind(mime_type)
    .bind(image_data)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

/// Loads one workspace background owned by the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_user_background(
    pool: &SqlitePool,
    user_id: &str,
    workspace: i64,
) -> Result<Option<UserBackground>, sqlx::Error> {
    sqlx::query_as::<_, UserBackground>(
        "SELECT mime_type, image_data, updated_at FROM user_backgrounds \
         WHERE user_id = ? AND workspace = ?",
    )
    .bind(user_id)
    .bind(workspace)
    .fetch_optional(pool)
    .await
}

/// Removes one workspace background owned by the authenticated user.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_user_background(
    pool: &SqlitePool,
    user_id: &str,
    workspace: i64,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM user_backgrounds WHERE user_id = ? AND workspace = ?")
            .bind(user_id)
            .bind(workspace)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Stores one user-owned wallpaper slot.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the upsert cannot be completed.
pub async fn upsert_user_wallpaper(
    pool: &SqlitePool,
    user_id: &str,
    slot: &str,
    mime_type: &str,
    image_data: &[u8],
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    // An upload replaces any wall applied to the same slot, so the two sources of a
    // wallpaper never disagree about which image the slot resolves to.
    sqlx::query("DELETE FROM user_wallpaper_selections WHERE user_id = ? AND slot = ?")
        .bind(user_id)
        .bind(slot)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO user_wallpapers (user_id, slot, mime_type, image_data, updated_at) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(user_id, slot) DO UPDATE SET \
         mime_type = excluded.mime_type, image_data = excluded.image_data, \
         updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(slot)
    .bind(mime_type)
    .bind(image_data)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await
}

/// Replaces the singleton administrator-controlled login wallpaper.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the transaction cannot be committed.
pub async fn replace_login_wallpaper(
    pool: &SqlitePool,
    user_id: &str,
    mime_type: &str,
    image_data: &[u8],
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query("DELETE FROM user_wallpapers WHERE slot = 'login'")
        .execute(&mut *transaction)
        .await?;
    // The login screen is a singleton across every administrator, so an uploaded image
    // clears applied walls too rather than competing with them on `updated_at`.
    sqlx::query("DELETE FROM user_wallpaper_selections WHERE slot = 'login'")
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO user_wallpapers (user_id, slot, mime_type, image_data, updated_at) \
         VALUES (?, 'login', ?, ?, ?)",
    )
    .bind(user_id)
    .bind(mime_type)
    .bind(image_data)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await
}

/// Loads one user-owned wallpaper slot.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_user_wallpaper(
    pool: &SqlitePool,
    user_id: &str,
    slot: &str,
) -> Result<Option<UserBackground>, sqlx::Error> {
    // A slot resolves an applied wall first, then the owner's uploaded image. The status
    // guard means a wall rejected after it was applied stops being served immediately,
    // without a cleanup pass over the selections.
    if let Some(wall) = sqlx::query_as::<_, UserBackground>(
        "SELECT walls.mime_type, walls.image_data, user_wallpaper_selections.updated_at \
         FROM user_wallpaper_selections \
         JOIN walls ON walls.id = user_wallpaper_selections.wall_id \
         WHERE user_wallpaper_selections.user_id = ? \
           AND user_wallpaper_selections.slot = ? \
           AND walls.status = 'approved'",
    )
    .bind(user_id)
    .bind(slot)
    .fetch_optional(pool)
    .await?
    {
        return Ok(Some(wall));
    }

    sqlx::query_as::<_, UserBackground>(
        "SELECT mime_type, image_data, updated_at FROM user_wallpapers \
         WHERE user_id = ? AND slot = ?",
    )
    .bind(user_id)
    .bind(slot)
    .fetch_optional(pool)
    .await
}

/// Loads the singleton administrator login wallpaper.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_login_wallpaper(
    pool: &SqlitePool,
) -> Result<Option<UserBackground>, sqlx::Error> {
    // Both writers keep the login slot a singleton, so at most one of these two sources
    // holds a row. The applied wall is tried first for symmetry with `find_user_wallpaper`.
    if let Some(wall) = sqlx::query_as::<_, UserBackground>(
        "SELECT walls.mime_type, walls.image_data, user_wallpaper_selections.updated_at \
         FROM user_wallpaper_selections \
         JOIN walls ON walls.id = user_wallpaper_selections.wall_id \
         JOIN users ON users.id = user_wallpaper_selections.user_id \
         WHERE user_wallpaper_selections.slot = 'login' \
           AND walls.status = 'approved' AND users.role = 'administrator' \
         ORDER BY user_wallpaper_selections.updated_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok(Some(wall));
    }

    sqlx::query_as::<_, UserBackground>(
        "SELECT user_wallpapers.mime_type, user_wallpapers.image_data, \
                user_wallpapers.updated_at \
         FROM user_wallpapers \
         JOIN users ON users.id = user_wallpapers.user_id \
         WHERE user_wallpapers.slot = 'login' AND users.role = 'administrator' \
         ORDER BY user_wallpapers.updated_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
}

/// Removes one user-owned wallpaper slot.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_user_wallpaper(
    pool: &SqlitePool,
    user_id: &str,
    slot: &str,
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    // Resetting a slot has to clear both sources, or the slot falls back to a previously
    // applied wall instead of the packaged default.
    let uploaded = sqlx::query("DELETE FROM user_wallpapers WHERE user_id = ? AND slot = ?")
        .bind(user_id)
        .bind(slot)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    let selected =
        sqlx::query("DELETE FROM user_wallpaper_selections WHERE user_id = ? AND slot = ?")
            .bind(user_id)
            .bind(slot)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
    transaction.commit().await?;
    Ok(uploaded + selected > 0)
}

/// Removes the singleton login wallpaper.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_login_wallpaper(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let uploaded = sqlx::query("DELETE FROM user_wallpapers WHERE slot = 'login'")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    let selected = sqlx::query("DELETE FROM user_wallpaper_selections WHERE slot = 'login'")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    transaction.commit().await?;
    Ok(uploaded + selected > 0)
}

/// Stores the authenticated user's avatar image.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the upsert cannot be completed.
pub async fn upsert_user_avatar(
    pool: &SqlitePool,
    user_id: &str,
    mime_type: &str,
    image_data: &[u8],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_avatars (user_id, mime_type, image_data, updated_at) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT(user_id) DO UPDATE SET \
         mime_type = excluded.mime_type, image_data = excluded.image_data, \
         updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(mime_type)
    .bind(image_data)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

/// Reports whether the authenticated user already has an avatar image.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the lookup cannot be completed.
pub async fn has_user_avatar(pool: &SqlitePool, user_id: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_avatars WHERE user_id = ?)")
        .bind(user_id)
        .fetch_one(pool)
        .await
}

/// Stores an imported avatar only when the user does not already have one.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the insert cannot be completed.
pub async fn insert_user_avatar_if_absent(
    pool: &SqlitePool,
    user_id: &str,
    mime_type: &str,
    image_data: &[u8],
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "INSERT INTO user_avatars (user_id, mime_type, image_data, updated_at) \
         VALUES (?, ?, ?, ?) ON CONFLICT(user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(mime_type)
    .bind(image_data)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Loads the authenticated user's avatar image.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_user_avatar(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Option<UserAvatar>, sqlx::Error> {
    sqlx::query_as::<_, UserAvatar>(
        "SELECT mime_type, image_data, updated_at FROM user_avatars WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Removes the authenticated user's avatar image.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_user_avatar(pool: &SqlitePool, user_id: &str) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM user_avatars WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

/// Loads the authenticated user's dashboard appearance controls.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn find_user_appearance(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<UserAppearance, sqlx::Error> {
    sqlx::query_as::<_, UserAppearance>(
        "SELECT user_id, EXISTS(SELECT 1 FROM user_wallpapers \
                WHERE user_wallpapers.user_id = user_appearance.user_id \
                  AND user_wallpapers.slot = 'dashboard') \
                OR EXISTS(SELECT 1 FROM user_wallpaper_selections \
                JOIN walls ON walls.id = user_wallpaper_selections.wall_id \
                WHERE user_wallpaper_selections.user_id = user_appearance.user_id \
                  AND user_wallpaper_selections.slot = 'dashboard' \
                  AND walls.status = 'approved') AS has_dashboard_wallpaper, \
                EXISTS(SELECT 1 FROM user_wallpapers \
                WHERE user_wallpapers.user_id = user_appearance.user_id \
                  AND user_wallpapers.slot = 'welcome') \
                OR EXISTS(SELECT 1 FROM user_wallpaper_selections \
                JOIN walls ON walls.id = user_wallpaper_selections.wall_id \
                WHERE user_wallpaper_selections.user_id = user_appearance.user_id \
                  AND user_wallpaper_selections.slot = 'welcome' \
                  AND walls.status = 'approved') AS has_welcome_wallpaper, \
                EXISTS(SELECT 1 FROM user_wallpapers \
                WHERE user_wallpapers.user_id = user_appearance.user_id \
                  AND user_wallpapers.slot = 'loading') \
                OR EXISTS(SELECT 1 FROM user_wallpaper_selections \
                JOIN walls ON walls.id = user_wallpaper_selections.wall_id \
                WHERE user_wallpaper_selections.user_id = user_appearance.user_id \
                  AND user_wallpaper_selections.slot = 'loading' \
                  AND walls.status = 'approved') AS has_loading_wallpaper, \
                EXISTS(SELECT 1 FROM user_wallpapers \
                JOIN users ON users.id = user_wallpapers.user_id \
                WHERE user_wallpapers.slot = 'login' \
                  AND users.role = 'administrator') \
                OR EXISTS(SELECT 1 FROM user_wallpaper_selections \
                JOIN walls ON walls.id = user_wallpaper_selections.wall_id \
                JOIN users ON users.id = user_wallpaper_selections.user_id \
                WHERE user_wallpaper_selections.slot = 'login' \
                  AND walls.status = 'approved' \
                  AND users.role = 'administrator') AS has_login_wallpaper, \
                background_blur, background_brightness, \
                background_contrast, background_saturation, updated_at \
         FROM user_appearance WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Updates the authenticated user's dashboard appearance controls.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_user_appearance(
    pool: &SqlitePool,
    user_id: &str,
    background_blur: i64,
    background_brightness: i64,
    background_contrast: i64,
    background_saturation: i64,
) -> Result<UserAppearance, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE user_appearance SET background_blur = ?, background_brightness = ?, \
                background_contrast = ?, background_saturation = ?, updated_at = ? \
         WHERE user_id = ?",
    )
    .bind(background_blur)
    .bind(background_brightness)
    .bind(background_contrast)
    .bind(background_saturation)
    .bind(&updated_at)
    .bind(user_id)
    .execute(pool)
    .await?;
    find_user_appearance(pool, user_id).await
}

/// Loads the singleton processing controls for the public login background.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn get_login_appearance(pool: &SqlitePool) -> Result<LoginAppearance, sqlx::Error> {
    sqlx::query_as::<_, LoginAppearance>(
        "SELECT background_blur, background_brightness, background_contrast, \
                background_saturation, updated_at \
         FROM login_appearance WHERE id = 1",
    )
    .fetch_one(pool)
    .await
}

/// Replaces the singleton processing controls for the public login background.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update cannot be completed.
pub async fn update_login_appearance(
    pool: &SqlitePool,
    background_blur: i64,
    background_brightness: i64,
    background_contrast: i64,
    background_saturation: i64,
) -> Result<LoginAppearance, sqlx::Error> {
    sqlx::query(
        "UPDATE login_appearance \
         SET background_blur = ?, background_brightness = ?, background_contrast = ?, \
             background_saturation = ?, updated_at = ? \
         WHERE id = 1",
    )
    .bind(background_blur)
    .bind(background_brightness)
    .bind(background_contrast)
    .bind(background_saturation)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    get_login_appearance(pool).await
}

/// Lists all accounts for the administrator directory.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_managed_users(pool: &SqlitePool) -> Result<Vec<ManagedUser>, sqlx::Error> {
    sqlx::query_as::<_, ManagedUser>(
        "SELECT users.id, users.email, user_settings.display_name, users.role, users.created_at, \
                users.last_login_at \
         FROM users \
         JOIN user_settings ON user_settings.user_id = users.id \
         ORDER BY users.role = 'administrator' DESC, users.created_at ASC",
    )
    .fetch_all(pool)
    .await
}

/// Loads the singleton administrator-controlled authentication policy.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the settings cannot be loaded.
pub async fn get_authentication_settings(
    pool: &SqlitePool,
) -> Result<AuthenticationSettings, sqlx::Error> {
    sqlx::query_as::<_, AuthenticationSettings>(
        "SELECT password_login_enabled, password_registration_enabled, \
                oidc_registration_enabled, updated_at \
         FROM authentication_settings WHERE id = 1",
    )
    .fetch_one(pool)
    .await
}

/// Replaces the singleton authentication policy.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the settings cannot be updated or reloaded.
pub async fn update_authentication_settings(
    pool: &SqlitePool,
    password_login_enabled: bool,
    password_registration_enabled: bool,
    oidc_registration_enabled: bool,
) -> Result<AuthenticationSettings, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE authentication_settings \
         SET password_login_enabled = ?, password_registration_enabled = ?, \
             oidc_registration_enabled = ?, updated_at = ? \
         WHERE id = 1",
    )
    .bind(password_login_enabled)
    .bind(password_registration_enabled)
    .bind(oidc_registration_enabled)
    .bind(updated_at)
    .execute(pool)
    .await?;
    get_authentication_settings(pool).await
}

/// Loads the singleton administrator-controlled file logging policy.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the settings cannot be loaded.
pub async fn get_logging_settings(pool: &SqlitePool) -> Result<LoggingSettings, sqlx::Error> {
    sqlx::query_as::<_, LoggingSettings>(
        "SELECT file_enabled, log_level, retention_days, max_file_size_mb, max_files, updated_at \
         FROM logging_settings WHERE id = 1",
    )
    .fetch_one(pool)
    .await
}

/// Replaces the singleton administrator-controlled file logging policy.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the settings cannot be updated or reloaded.
pub async fn update_logging_settings(
    pool: &SqlitePool,
    file_enabled: bool,
    log_level: &str,
    retention_days: i64,
    max_file_size_mb: i64,
    max_files: i64,
) -> Result<LoggingSettings, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE logging_settings \
         SET file_enabled = ?, log_level = ?, retention_days = ?, max_file_size_mb = ?, \
             max_files = ?, updated_at = ? \
         WHERE id = 1",
    )
    .bind(file_enabled)
    .bind(log_level)
    .bind(retention_days)
    .bind(max_file_size_mb)
    .bind(max_files)
    .bind(updated_at)
    .execute(pool)
    .await?;
    get_logging_settings(pool).await
}

/// Lists the administrator-managed outbound network rules in deterministic order.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the rules cannot be loaded.
pub async fn list_network_access_rules(
    pool: &SqlitePool,
) -> Result<Vec<NetworkAccessRule>, sqlx::Error> {
    sqlx::query_as::<_, NetworkAccessRule>(
        "SELECT id, action, scheme, host, port, integration, created_by_user_id, \
                created_at, updated_at \
         FROM network_access_rules \
         ORDER BY action = 'deny' DESC, integration ASC, scheme ASC, host ASC, port ASC",
    )
    .fetch_all(pool)
    .await
}

/// Loads exact-origin rules that apply to one outbound integration.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the rules cannot be loaded.
pub async fn find_network_access_rules(
    pool: &SqlitePool,
    scheme: &str,
    host: &str,
    port: i64,
    integration: &str,
) -> Result<Vec<NetworkAccessRule>, sqlx::Error> {
    sqlx::query_as::<_, NetworkAccessRule>(
        "SELECT id, action, scheme, host, port, integration, created_by_user_id, \
                created_at, updated_at \
         FROM network_access_rules \
         WHERE scheme = ? AND host = ? AND port = ? \
           AND integration IN ('all', ?) \
         ORDER BY action = 'deny' DESC",
    )
    .bind(scheme)
    .bind(host)
    .bind(port)
    .bind(integration)
    .fetch_all(pool)
    .await
}

/// Creates one exact-origin outbound network rule.
///
/// Returns `None` when the same rule already exists or the instance has reached the 128-rule
/// safety limit.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the rule cannot be created or reloaded.
#[allow(clippy::too_many_arguments)]
pub async fn create_network_access_rule(
    pool: &SqlitePool,
    id: &str,
    action: &str,
    scheme: &str,
    host: &str,
    port: i64,
    integration: &str,
    created_by_user_id: &str,
) -> Result<Option<NetworkAccessRule>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "INSERT INTO network_access_rules (\
             id, action, scheme, host, port, integration, created_by_user_id, created_at, updated_at\
         ) \
         SELECT ?, ?, ?, ?, ?, ?, ?, ?, ? \
         WHERE (SELECT COUNT(*) FROM network_access_rules) < 128 \
         ON CONFLICT(action, scheme, host, port, integration) DO NOTHING",
    )
    .bind(id)
    .bind(action)
    .bind(scheme)
    .bind(host)
    .bind(port)
    .bind(integration)
    .bind(created_by_user_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    sqlx::query_as::<_, NetworkAccessRule>(
        "SELECT id, action, scheme, host, port, integration, created_by_user_id, \
                created_at, updated_at \
         FROM network_access_rules WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Deletes one outbound network rule.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the rule cannot be deleted.
pub async fn delete_network_access_rule(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM network_access_rules WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        == 1)
}

/// Changes another user's role while preserving at least one administrator.
///
/// The conditional update keeps the final-administrator check atomic with the write.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the update or outcome lookup fails.
pub async fn update_managed_user_role(
    pool: &SqlitePool,
    actor_id: &str,
    user_id: &str,
    role: &str,
) -> Result<UserMutationOutcome, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE users SET role = ? \
         WHERE id = ? AND id <> ? \
           AND (role <> 'administrator' OR ? = 'administrator' OR \
                (SELECT COUNT(*) FROM users WHERE role = 'administrator') > 1)",
    )
    .bind(role)
    .bind(user_id)
    .bind(actor_id)
    .bind(role)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return diagnose_user_mutation(pool, actor_id, user_id).await;
    }

    let user = sqlx::query_as::<_, ManagedUser>(
        "SELECT users.id, users.email, user_settings.display_name, users.role, users.created_at, \
                users.last_login_at \
         FROM users \
         JOIN user_settings ON user_settings.user_id = users.id \
         WHERE users.id = ?",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(UserMutationOutcome::Updated(user))
}

/// Deletes another user while preserving at least one administrator.
///
/// Related sessions, settings, tasks, and OIDC identities are removed by foreign-key cascades.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete or outcome lookup fails.
pub async fn delete_managed_user(
    pool: &SqlitePool,
    actor_id: &str,
    user_id: &str,
) -> Result<UserMutationOutcome, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM users \
         WHERE id = ? AND id <> ? \
           AND (role <> 'administrator' OR \
                (SELECT COUNT(*) FROM users WHERE role = 'administrator') > 1)",
    )
    .bind(user_id)
    .bind(actor_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return diagnose_user_mutation(pool, actor_id, user_id).await;
    }

    Ok(UserMutationOutcome::Deleted)
}

async fn diagnose_user_mutation(
    pool: &SqlitePool,
    actor_id: &str,
    user_id: &str,
) -> Result<UserMutationOutcome, sqlx::Error> {
    let target_role = sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    let Some(target_role) = target_role else {
        return Ok(UserMutationOutcome::NotFound);
    };
    if actor_id == user_id {
        return Ok(UserMutationOutcome::SelfAction);
    }
    if target_role == "administrator" {
        return Ok(UserMutationOutcome::LastAdministrator);
    }

    Ok(UserMutationOutcome::NotFound)
}

/// Stores the PKCE verifier and nonce for one short-lived OIDC authorization attempt.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the state cannot be persisted.
pub async fn create_oidc_authorization(
    pool: &SqlitePool,
    state: &str,
    pkce_verifier: &str,
    nonce: &str,
    expires_at: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("DELETE FROM oidc_authorizations WHERE expires_at <= ?")
        .bind(&now)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO oidc_authorizations \
         (state, pkce_verifier, nonce, expires_at, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(state)
    .bind(pkce_verifier)
    .bind(nonce)
    .bind(expires_at)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Atomically consumes one unexpired OIDC authorization attempt.
///
/// A second callback with the same state therefore cannot replay the code exchange.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the state cannot be consumed.
pub async fn consume_oidc_authorization(
    pool: &SqlitePool,
    state: &str,
) -> Result<Option<OidcAuthorization>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query_as::<_, OidcAuthorization>(
        "DELETE FROM oidc_authorizations \
         WHERE state = ? AND expires_at > ? \
         RETURNING state, pkce_verifier, nonce, expires_at, created_at",
    )
    .bind(state)
    .bind(now)
    .fetch_optional(pool)
    .await
}

/// Resolves an OIDC identity to an existing verified-email account or creates a new dashboard.
///
/// The caller must verify the ID token signature, nonce, and `email_verified` claim first.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when identity linking or account creation fails.
pub async fn find_or_create_oidc_user(
    pool: &SqlitePool,
    issuer: &str,
    subject: &str,
    email: &str,
    display_name: &str,
    unusable_password_hash: &str,
    registration_enabled: bool,
) -> Result<Option<String>, sqlx::Error> {
    let mut transaction = pool.begin().await?;

    if let Some(user_id) = sqlx::query_scalar::<_, String>(
        "SELECT user_id FROM oidc_identities WHERE issuer = ? AND subject = ?",
    )
    .bind(issuer)
    .bind(subject)
    .fetch_optional(&mut *transaction)
    .await?
    {
        transaction.commit().await?;
        return Ok(Some(user_id));
    }

    let existing_user_id = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE email = ?")
        .bind(email)
        .fetch_optional(&mut *transaction)
        .await?;
    let user_id = existing_user_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let now = chrono::Utc::now().to_rfc3339();

    if existing_user_id.is_none() {
        if !registration_enabled {
            transaction.commit().await?;
            return Ok(None);
        }
        sqlx::query(
            "INSERT INTO users (id, email, password_hash, role, created_at) \
             VALUES (?, ?, ?, 'member', ?)",
        )
        .bind(&user_id)
        .bind(email)
        .bind(unusable_password_hash)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO user_settings \
             (user_id, display_name, location, timezone, temperature_unit, updated_at) \
             VALUES (?, ?, 'London', 'UTC', 'celsius', ?)",
        )
        .bind(&user_id)
        .bind(display_name)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;

        for (title, completed, priority) in [
            ("Review today’s notes", true, "none"),
            ("Plan the next focus block", false, "p1"),
            ("Organize saved references", false, "none"),
        ] {
            sqlx::query(
                "INSERT INTO tasks \
                 (id, title, completed, priority, created_at, updated_at, user_id) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(title)
            .bind(completed)
            .bind(priority)
            .bind(&now)
            .bind(&now)
            .bind(&user_id)
            .execute(&mut *transaction)
            .await?;
        }
        insert_default_workspaces(&mut transaction, &user_id, &now).await?;
        insert_default_widgets(&mut transaction, &user_id, &now).await?;
    }

    sqlx::query(
        "INSERT INTO oidc_identities (issuer, subject, user_id, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(issuer)
    .bind(subject)
    .bind(&user_id)
    .bind(now)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Some(user_id))
}

/// Reports whether the one-time administrator setup has been completed.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the status cannot be loaded.
pub async fn is_onboarding_complete(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_metadata \
         WHERE key = 'onboarding_complete' AND value = 'true')",
    )
    .fetch_one(pool)
    .await
}

/// Atomically claims first-run setup and creates the sole initial administrator account.
///
/// Returns `None` when setup was already completed or an account already exists. The metadata
/// claim and user are committed in one transaction, so competing requests cannot both succeed.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when setup cannot be completed.
pub async fn create_initial_administrator(
    pool: &SqlitePool,
    email: &str,
    password_hash: &str,
    display_name: &str,
) -> Result<Option<(User, UserSettings)>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some(administrator) =
        insert_initial_administrator(&mut transaction, email, password_hash, display_name).await?
    else {
        transaction.rollback().await?;
        return Ok(None);
    };
    transaction.commit().await?;
    Ok(Some(administrator))
}

/// Atomically claims first-run setup for a verified OIDC identity.
///
/// Returns the initial administrator's user ID, or `None` when another setup request won the
/// one-time claim. The account and OIDC identity are committed in the same transaction.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when setup cannot be completed.
pub async fn create_initial_oidc_administrator(
    pool: &SqlitePool,
    issuer: &str,
    subject: &str,
    email: &str,
    display_name: &str,
    unusable_password_hash: &str,
) -> Result<Option<String>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some((administrator, _)) = insert_initial_administrator(
        &mut transaction,
        email,
        unusable_password_hash,
        display_name,
    )
    .await?
    else {
        transaction.rollback().await?;
        return Ok(None);
    };

    sqlx::query(
        "INSERT INTO oidc_identities (issuer, subject, user_id, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(issuer)
    .bind(subject)
    .bind(&administrator.id)
    .bind(&administrator.created_at)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Some(administrator.id))
}

async fn insert_initial_administrator(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    email: &str,
    password_hash: &str,
    display_name: &str,
) -> Result<Option<(User, UserSettings)>, sqlx::Error> {
    let user_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let claim = sqlx::query(
        "INSERT INTO app_metadata (key, value, updated_at) \
         SELECT 'onboarding_complete', 'true', ? \
         WHERE NOT EXISTS (SELECT 1 FROM users) \
         ON CONFLICT(key) DO NOTHING",
    )
    .bind(&now)
    .execute(&mut **transaction)
    .await?;
    if claim.rows_affected() != 1 {
        return Ok(None);
    }

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, role, created_at) \
         VALUES (?, ?, ?, 'administrator', ?)",
    )
    .bind(&user_id)
    .bind(email)
    .bind(password_hash)
    .bind(&now)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "INSERT INTO user_settings \
         (user_id, display_name, location, timezone, temperature_unit, updated_at) \
         VALUES (?, ?, 'London', 'UTC', 'celsius', ?)",
    )
    .bind(&user_id)
    .bind(display_name)
    .bind(&now)
    .execute(&mut **transaction)
    .await?;

    for (title, completed, priority) in [
        ("Review today’s notes", true, "none"),
        ("Plan the next focus block", false, "p1"),
        ("Organize saved references", false, "none"),
    ] {
        sqlx::query(
            "INSERT INTO tasks \
             (id, title, completed, priority, created_at, updated_at, user_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(title)
        .bind(completed)
        .bind(priority)
        .bind(&now)
        .bind(&now)
        .bind(&user_id)
        .execute(&mut **transaction)
        .await?;
    }
    insert_default_workspaces(transaction, &user_id, &now).await?;
    insert_default_widgets(transaction, &user_id, &now).await?;

    Ok(Some((
        User {
            id: user_id.clone(),
            email: email.to_owned(),
            role: "administrator".to_owned(),
            created_at: now.clone(),
        },
        UserSettings {
            user_id,
            display_name: display_name.to_owned(),
            location: "London".to_owned(),
            timezone: "UTC".to_owned(),
            sidebar_timezones: vec!["UTC".to_owned()],
            calendar_week_start: "sunday".to_owned(),
            temperature_unit: "celsius".to_owned(),
            lines_default_visibility: "private".to_owned(),
            podcast_playback_rate: 1.0,
            updated_at: now,
        },
    )))
}

const KANBAN_PERMISSIONS: &[&str] = &[
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

fn kanban_default_permission(role: &str, permission: &str) -> bool {
    match role {
        "admin" => true,
        "member" => matches!(
            permission,
            "workspace:view"
                | "board:view"
                | "board:create"
                | "list:view"
                | "list:create"
                | "list:edit"
                | "list:delete"
                | "card:view"
                | "card:create"
                | "card:edit"
                | "card:delete"
                | "comment:view"
                | "comment:create"
                | "comment:edit"
                | "comment:delete"
                | "member:view"
        ),
        "guest" => matches!(
            permission,
            "workspace:view"
                | "board:view"
                | "list:view"
                | "card:view"
                | "comment:view"
                | "member:view"
        ),
        _ => false,
    }
}

async fn seed_kanban_role_permissions(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
) -> Result<(), sqlx::Error> {
    for role in ["admin", "member", "guest"] {
        for permission in KANBAN_PERMISSIONS {
            sqlx::query(
                "INSERT INTO kanban_role_permissions \
                 (workspace_id, role, permission, granted) VALUES (?, ?, ?, ?)",
            )
            .bind(workspace_id)
            .bind(role)
            .bind(*permission)
            .bind(kanban_default_permission(role, permission))
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(())
}

/// Returns the active workspace role for a Pandan account.
pub async fn kanban_workspace_role(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT role FROM kanban_workspace_members \
         WHERE workspace_id = ? AND user_id = ? AND status = 'active'",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Resolves role permissions plus per-member overrides.
pub async fn kanban_effective_permissions(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let Some(role) = kanban_workspace_role(pool, workspace_id, user_id).await? else {
        return Ok(Vec::new());
    };
    let role_permissions = sqlx::query_scalar::<_, String>(
        "SELECT permission FROM kanban_role_permissions \
         WHERE workspace_id = ? AND role = ? AND granted = 1",
    )
    .bind(workspace_id)
    .bind(&role)
    .fetch_all(pool)
    .await?;
    let overrides = sqlx::query_as::<_, KanbanMemberPermission>(
        "SELECT user_id, permission, granted FROM kanban_member_permissions \
         WHERE workspace_id = ? AND user_id = ?",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut effective = role_permissions
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    for permission_override in overrides {
        if permission_override.granted {
            effective.insert(permission_override.permission);
        } else {
            effective.remove(&permission_override.permission);
        }
    }
    let mut permissions = effective.into_iter().collect::<Vec<_>>();
    permissions.sort();
    Ok(permissions)
}

/// Checks one effective workspace permission.
pub async fn kanban_has_permission(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
    permission: &str,
) -> Result<bool, sqlx::Error> {
    Ok(kanban_effective_permissions(pool, workspace_id, user_id)
        .await?
        .iter()
        .any(|candidate| candidate == permission))
}

#[derive(Debug, FromRow)]
struct KanbanWorkspaceRow {
    id: String,
    name: String,
    description: String,
    role: String,
    member_count: i64,
    board_count: i64,
    created_at: String,
    updated_at: String,
}

async fn hydrate_kanban_workspace(
    pool: &SqlitePool,
    user_id: &str,
    row: KanbanWorkspaceRow,
) -> Result<KanbanWorkspace, sqlx::Error> {
    let permissions = kanban_effective_permissions(pool, &row.id, user_id).await?;
    Ok(KanbanWorkspace {
        id: row.id,
        name: row.name,
        description: row.description,
        role: row.role,
        member_count: row.member_count,
        board_count: row.board_count,
        permissions,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn get_kanban_workspace_row(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
) -> Result<Option<KanbanWorkspaceRow>, sqlx::Error> {
    sqlx::query_as::<_, KanbanWorkspaceRow>(
        "SELECT w.id, w.name, w.description, m.role, \
         (SELECT COUNT(*) FROM kanban_workspace_members active_member \
          WHERE active_member.workspace_id = w.id AND active_member.status = 'active') AS member_count, \
         (SELECT COUNT(*) FROM kanban_boards b WHERE b.workspace_id = w.id) AS board_count, \
         w.created_at, w.updated_at \
         FROM kanban_workspaces w \
         JOIN kanban_workspace_members m ON m.workspace_id = w.id \
         WHERE w.id = ? AND m.user_id = ? AND m.status = 'active'",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Lists active workspaces and pending in-app invitations for one account.
pub async fn kanban_overview(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<KanbanOverview, sqlx::Error> {
    let rows = sqlx::query_as::<_, KanbanWorkspaceRow>(
        "SELECT w.id, w.name, w.description, m.role, \
         (SELECT COUNT(*) FROM kanban_workspace_members active_member \
          WHERE active_member.workspace_id = w.id AND active_member.status = 'active') AS member_count, \
         (SELECT COUNT(*) FROM kanban_boards b WHERE b.workspace_id = w.id) AS board_count, \
         w.created_at, w.updated_at \
         FROM kanban_workspaces w \
         JOIN kanban_workspace_members m ON m.workspace_id = w.id \
         WHERE m.user_id = ? AND m.status = 'active' \
         ORDER BY w.updated_at DESC, w.name COLLATE NOCASE",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut workspaces = Vec::with_capacity(rows.len());
    for row in rows {
        workspaces.push(hydrate_kanban_workspace(pool, user_id, row).await?);
    }
    let invitations = sqlx::query_as::<_, KanbanInvitation>(
        "SELECT w.id AS workspace_id, w.name AS workspace_name, m.role, \
         COALESCE(inviter_settings.display_name, 'Pandan member') AS invited_by_name, \
         m.created_at \
         FROM kanban_workspace_members m \
         JOIN kanban_workspaces w ON w.id = m.workspace_id \
         LEFT JOIN user_settings inviter_settings ON inviter_settings.user_id = m.invited_by_user_id \
         WHERE m.user_id = ? AND m.status = 'invited' \
         ORDER BY m.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(KanbanOverview {
        workspaces,
        invitations,
    })
}

/// Creates a workspace, its immutable admin membership, and the exact Kan role templates.
pub async fn create_kanban_workspace(
    pool: &SqlitePool,
    user_id: &str,
    name: &str,
    description: &str,
) -> Result<KanbanWorkspace, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO kanban_workspaces \
         (id, name, description, created_by_user_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(description)
    .bind(user_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO kanban_workspace_members \
         (workspace_id, user_id, role, status, invited_by_user_id, created_at, updated_at) \
         VALUES (?, ?, 'admin', 'active', ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(user_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    seed_kanban_role_permissions(&mut transaction, &id).await?;
    transaction.commit().await?;
    let row = get_kanban_workspace_row(pool, &id, user_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    hydrate_kanban_workspace(pool, user_id, row).await
}

/// Updates workspace identity fields after authorization by the server.
pub async fn update_kanban_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
    name: &str,
    description: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE kanban_workspaces SET name = ?, description = ?, updated_at = ? WHERE id = ?",
    )
    .bind(name)
    .bind(description)
    .bind(now)
    .bind(workspace_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Deletes a workspace after authorization by the server.
pub async fn delete_kanban_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM kanban_workspaces WHERE id = ?")
        .bind(workspace_id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

/// Loads member and permission settings for an active workspace member.
pub async fn get_kanban_workspace_settings(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
) -> Result<Option<KanbanWorkspaceSettings>, sqlx::Error> {
    let Some(row) = get_kanban_workspace_row(pool, workspace_id, user_id).await? else {
        return Ok(None);
    };
    let workspace = hydrate_kanban_workspace(pool, user_id, row).await?;
    let members = sqlx::query_as::<_, KanbanMember>(
        "SELECT m.user_id, settings.display_name, users.email, m.role, m.status, m.created_at \
         FROM kanban_workspace_members m \
         JOIN users ON users.id = m.user_id \
         JOIN user_settings settings ON settings.user_id = m.user_id \
         WHERE m.workspace_id = ? \
         ORDER BY m.status = 'active' DESC, m.role = 'admin' DESC, settings.display_name COLLATE NOCASE",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    let role_permissions = sqlx::query_as::<_, KanbanRolePermission>(
        "SELECT role, permission, granted FROM kanban_role_permissions \
         WHERE workspace_id = ? ORDER BY role, permission",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    let member_overrides = sqlx::query_as::<_, KanbanMemberPermission>(
        "SELECT user_id, permission, granted FROM kanban_member_permissions \
         WHERE workspace_id = ? ORDER BY user_id, permission",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    Ok(Some(KanbanWorkspaceSettings {
        workspace,
        members,
        role_permissions,
        member_overrides,
    }))
}

/// Reports whether an account belongs to a Kanban workspace in any membership state.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the membership lookup cannot be completed.
pub async fn is_kanban_workspace_member(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM kanban_workspace_members \
         WHERE workspace_id = ? AND user_id = ?)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Searches existing Pandan accounts that are not already workspace members.
pub async fn search_kanban_directory(
    pool: &SqlitePool,
    workspace_id: &str,
    query: &str,
) -> Result<Vec<KanbanDirectoryUser>, sqlx::Error> {
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    sqlx::query_as::<_, KanbanDirectoryUser>(
        "SELECT users.id AS user_id, settings.display_name, users.email \
         FROM users JOIN user_settings settings ON settings.user_id = users.id \
         WHERE (settings.display_name LIKE ? ESCAPE '\\' OR users.email LIKE ? ESCAPE '\\') \
           AND NOT EXISTS (SELECT 1 FROM kanban_workspace_members member \
                           WHERE member.workspace_id = ? AND member.user_id = users.id) \
         ORDER BY settings.display_name COLLATE NOCASE LIMIT 20",
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(workspace_id)
    .fetch_all(pool)
    .await
}

/// Invites one existing Pandan account into a workspace.
pub async fn invite_kanban_member(
    pool: &SqlitePool,
    workspace_id: &str,
    target_user_id: &str,
    role: &str,
    inviter_user_id: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "INSERT INTO kanban_workspace_members \
         (workspace_id, user_id, role, status, invited_by_user_id, created_at, updated_at) \
         SELECT ?, users.id, ?, 'invited', ?, ?, ? FROM users WHERE users.id = ? \
         ON CONFLICT(workspace_id, user_id) DO NOTHING",
    )
    .bind(workspace_id)
    .bind(role)
    .bind(inviter_user_id)
    .bind(&now)
    .bind(&now)
    .bind(target_user_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Accepts or declines the current user's pending invitation.
pub async fn respond_to_kanban_invitation(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
    accept: bool,
) -> Result<bool, sqlx::Error> {
    if accept {
        let now = chrono::Utc::now().to_rfc3339();
        Ok(sqlx::query(
            "UPDATE kanban_workspace_members SET status = 'active', updated_at = ? \
             WHERE workspace_id = ? AND user_id = ? AND status = 'invited'",
        )
        .bind(now)
        .bind(workspace_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected()
            > 0)
    } else {
        Ok(sqlx::query(
            "DELETE FROM kanban_workspace_members \
             WHERE workspace_id = ? AND user_id = ? AND status = 'invited'",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected()
            > 0)
    }
}

async fn kanban_active_admin_count(
    pool: &SqlitePool,
    workspace_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM kanban_workspace_members \
         WHERE workspace_id = ? AND role = 'admin' AND status = 'active'",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await
}

/// Changes a member role, returning `Some(false)` when it would remove the final admin.
pub async fn update_kanban_member_role(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
    role: &str,
) -> Result<Option<bool>, sqlx::Error> {
    let current: Option<String> = sqlx::query_scalar(
        "SELECT role FROM kanban_workspace_members \
         WHERE workspace_id = ? AND user_id = ? AND status = 'active'",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(current) = current else {
        return Ok(None);
    };
    if current == "admin"
        && role != "admin"
        && kanban_active_admin_count(pool, workspace_id).await? <= 1
    {
        return Ok(Some(false));
    }
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE kanban_workspace_members SET role = ?, updated_at = ? \
         WHERE workspace_id = ? AND user_id = ? AND status = 'active'",
    )
    .bind(role)
    .bind(now)
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(Some(true))
}

/// Removes a workspace member while preserving the final administrator.
pub async fn remove_kanban_member(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
) -> Result<Option<bool>, sqlx::Error> {
    let current: Option<String> = sqlx::query_scalar(
        "SELECT role FROM kanban_workspace_members WHERE workspace_id = ? AND user_id = ?",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(current) = current else {
        return Ok(None);
    };
    if current == "admin" && kanban_active_admin_count(pool, workspace_id).await? <= 1 {
        return Ok(Some(false));
    }
    sqlx::query("DELETE FROM kanban_workspace_members WHERE workspace_id = ? AND user_id = ?")
        .bind(workspace_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(Some(true))
}

/// Changes a Member or Guest role permission. Admin grants stay immutable.
pub async fn set_kanban_role_permission(
    pool: &SqlitePool,
    workspace_id: &str,
    role: &str,
    permission: &str,
    granted: bool,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE kanban_role_permissions SET granted = ? \
         WHERE workspace_id = ? AND role = ? AND permission = ? AND role != 'admin'",
    )
    .bind(granted)
    .bind(workspace_id)
    .bind(role)
    .bind(permission)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Upserts a per-member permission override.
pub async fn set_kanban_member_permission(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
    permission: &str,
    granted: bool,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "INSERT INTO kanban_member_permissions \
         (workspace_id, user_id, permission, granted, updated_at) VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(workspace_id, user_id, permission) DO UPDATE SET \
         granted = excluded.granted, updated_at = excluded.updated_at",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(permission)
    .bind(granted)
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Clears all overrides for one workspace member.
pub async fn reset_kanban_member_permissions(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM kanban_member_permissions WHERE workspace_id = ? AND user_id = ?")
        .bind(workspace_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Resolves a board to its workspace without exposing the board first.
pub async fn kanban_board_workspace_id(
    pool: &SqlitePool,
    board_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT workspace_id FROM kanban_boards WHERE id = ?")
        .bind(board_id)
        .fetch_optional(pool)
        .await
}

/// Resolves a column to its workspace.
pub async fn kanban_column_workspace_id(
    pool: &SqlitePool,
    column_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT board.workspace_id FROM kanban_columns column_record \
         JOIN kanban_boards board ON board.id = column_record.board_id \
         WHERE column_record.id = ?",
    )
    .bind(column_id)
    .fetch_optional(pool)
    .await
}

/// Resolves a card to its workspace.
pub async fn kanban_card_workspace_id(
    pool: &SqlitePool,
    card_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT board.workspace_id FROM kanban_cards card \
         JOIN kanban_columns column_record ON column_record.id = card.column_id \
         JOIN kanban_boards board ON board.id = column_record.board_id WHERE card.id = ?",
    )
    .bind(card_id)
    .fetch_optional(pool)
    .await
}

#[derive(Debug, FromRow)]
struct KanbanBoardRecord {
    id: String,
    workspace_id: String,
    name: String,
    description: String,
    visibility: String,
    archived: bool,
    favorite: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct KanbanColumnRecord {
    id: String,
    board_id: String,
    name: String,
    position: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct KanbanCardRecord {
    id: String,
    column_id: String,
    title: String,
    description: String,
    due_date: Option<String>,
    position: i64,
    created_at: String,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
struct KanbanChecklistRecord {
    id: String,
    card_id: String,
    name: String,
    position: i64,
}

/// Lists active or archived boards available through one workspace membership.
pub async fn list_kanban_boards(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
    archived: bool,
) -> Result<Vec<KanbanBoardSummary>, sqlx::Error> {
    sqlx::query_as::<_, KanbanBoardSummary>(
        "SELECT board.id, board.workspace_id, board.name, board.description, board.visibility, \
         board.archived, EXISTS(SELECT 1 FROM kanban_board_favorites favorite \
                               WHERE favorite.board_id = board.id AND favorite.user_id = ?) AS favorite, \
         board.position, \
         (SELECT COUNT(*) FROM kanban_columns column_record WHERE column_record.board_id = board.id) AS column_count, \
         (SELECT COUNT(*) FROM kanban_cards card JOIN kanban_columns column_record ON column_record.id = card.column_id \
          WHERE column_record.board_id = board.id AND card.archived_at IS NULL) AS card_count, \
         board.created_at, board.updated_at \
         FROM kanban_boards board \
         WHERE board.workspace_id = ? AND board.archived = ? \
           AND EXISTS(SELECT 1 FROM kanban_workspace_members member \
                      WHERE member.workspace_id = board.workspace_id AND member.user_id = ? AND member.status = 'active') \
         ORDER BY favorite DESC, board.position ASC, board.created_at ASC",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(archived)
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Creates a board with the default Todo, In Progress, and Finished columns.
pub async fn create_kanban_board(
    pool: &SqlitePool,
    workspace_id: &str,
    user_id: &str,
    name: &str,
    description: &str,
    visibility: &str,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM kanban_boards WHERE workspace_id = ?",
    )
    .bind(workspace_id)
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO kanban_boards \
         (id, workspace_id, name, description, visibility, position, created_by_user_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(name)
    .bind(description)
    .bind(visibility)
    .bind(position)
    .bind(user_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    for (column_position, column_name) in ["Todo", "In Progress", "Finished"].iter().enumerate() {
        sqlx::query(
            "INSERT INTO kanban_columns (id, board_id, name, position, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&id)
        .bind(column_name)
        .bind(i64::try_from(column_position).unwrap_or(i64::MAX))
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(id)
}

/// Updates board metadata and archive state.
pub async fn update_kanban_board(
    pool: &SqlitePool,
    board_id: &str,
    name: &str,
    description: &str,
    visibility: &str,
    archived: bool,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE kanban_boards SET name = ?, description = ?, visibility = ?, archived = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(name)
    .bind(description)
    .bind(visibility)
    .bind(archived)
    .bind(now)
    .bind(board_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Sets a per-user board favorite.
pub async fn set_kanban_board_favorite(
    pool: &SqlitePool,
    board_id: &str,
    user_id: &str,
    favorite: bool,
) -> Result<(), sqlx::Error> {
    if favorite {
        sqlx::query(
            "INSERT OR IGNORE INTO kanban_board_favorites (board_id, user_id, created_at) VALUES (?, ?, ?)",
        )
        .bind(board_id)
        .bind(user_id)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(pool)
        .await?;
    } else {
        sqlx::query("DELETE FROM kanban_board_favorites WHERE board_id = ? AND user_id = ?")
            .bind(board_id)
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Permanently deletes one authorized board and cascading children.
pub async fn delete_kanban_board(pool: &SqlitePool, board_id: &str) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM kanban_boards WHERE id = ?")
        .bind(board_id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

async fn hydrate_kanban_card(
    pool: &SqlitePool,
    record: KanbanCardRecord,
) -> Result<KanbanCard, sqlx::Error> {
    let assignees = sqlx::query_as::<_, KanbanMember>(
        "SELECT member.user_id, settings.display_name, users.email, member.role, member.status, member.created_at \
         FROM kanban_card_assignees assignee \
         JOIN kanban_workspace_members member ON member.workspace_id = assignee.workspace_id AND member.user_id = assignee.user_id \
         JOIN users ON users.id = member.user_id \
         JOIN user_settings settings ON settings.user_id = member.user_id \
         WHERE assignee.card_id = ? AND member.status = 'active' \
         ORDER BY settings.display_name COLLATE NOCASE",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let labels = sqlx::query_as::<_, KanbanLabel>(
        "SELECT label.id, label.board_id, label.name, label.color \
         FROM kanban_card_labels relation JOIN kanban_labels label ON label.id = relation.label_id \
         WHERE relation.card_id = ? ORDER BY label.name COLLATE NOCASE",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let comments = sqlx::query_as::<_, KanbanComment>(
        "SELECT comment.id, comment.card_id, comment.user_id, \
         COALESCE(settings.display_name, 'Former member') AS author_name, comment.content, \
         comment.created_at, comment.updated_at \
         FROM kanban_comments comment \
         LEFT JOIN user_settings settings ON settings.user_id = comment.user_id \
         WHERE comment.card_id = ? ORDER BY comment.created_at ASC",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let checklist_rows = sqlx::query_as::<_, KanbanChecklistRecord>(
        "SELECT id, card_id, name, position \
         FROM kanban_checklists WHERE card_id = ? ORDER BY position ASC",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let mut checklists = Vec::with_capacity(checklist_rows.len());
    for checklist in checklist_rows {
        let items = sqlx::query_as::<_, KanbanChecklistItem>(
            "SELECT id, checklist_id, title, completed, position \
             FROM kanban_checklist_items WHERE checklist_id = ? ORDER BY position ASC",
        )
        .bind(&checklist.id)
        .fetch_all(pool)
        .await?;
        checklists.push(KanbanChecklist {
            id: checklist.id,
            card_id: checklist.card_id,
            name: checklist.name,
            position: checklist.position,
            items,
        });
    }
    let attachments = sqlx::query_as::<_, KanbanAttachment>(
        "SELECT id, card_id, file_name, mime_type, byte_size, created_at \
         FROM kanban_attachments WHERE card_id = ? ORDER BY created_at ASC",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    let activity = sqlx::query_as::<_, KanbanActivity>(
        "SELECT activity.id, activity.card_id, \
         COALESCE(settings.display_name, 'Former member') AS actor_name, activity.action, activity.detail, activity.created_at \
         FROM kanban_card_activity activity \
         LEFT JOIN user_settings settings ON settings.user_id = activity.user_id \
         WHERE activity.card_id = ? ORDER BY activity.created_at DESC LIMIT 100",
    )
    .bind(&record.id)
    .fetch_all(pool)
    .await?;
    Ok(KanbanCard {
        id: record.id,
        column_id: record.column_id,
        title: record.title,
        description: record.description,
        due_date: record.due_date,
        position: record.position,
        assignees,
        labels,
        comments,
        checklists,
        attachments,
        activity,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

/// Loads a fully hydrated card.
pub async fn get_kanban_card(
    pool: &SqlitePool,
    card_id: &str,
) -> Result<Option<KanbanCard>, sqlx::Error> {
    let record = sqlx::query_as::<_, KanbanCardRecord>(
        "SELECT id, column_id, title, description, due_date, position, created_at, updated_at \
         FROM kanban_cards WHERE id = ? AND archived_at IS NULL",
    )
    .bind(card_id)
    .fetch_optional(pool)
    .await?;
    match record {
        Some(record) => Ok(Some(hydrate_kanban_card(pool, record).await?)),
        None => Ok(None),
    }
}

/// Loads a board, its columns, cards, labels, members, and effective permissions.
pub async fn get_kanban_board(
    pool: &SqlitePool,
    board_id: &str,
    user_id: &str,
) -> Result<Option<KanbanBoard>, sqlx::Error> {
    let record = sqlx::query_as::<_, KanbanBoardRecord>(
        "SELECT board.id, board.workspace_id, board.name, board.description, board.visibility, \
         board.archived, EXISTS(SELECT 1 FROM kanban_board_favorites favorite \
                               WHERE favorite.board_id = board.id AND favorite.user_id = ?) AS favorite, \
         board.created_at, board.updated_at \
         FROM kanban_boards board WHERE board.id = ?",
    )
    .bind(user_id)
    .bind(board_id)
    .fetch_optional(pool)
    .await?;
    let Some(record) = record else {
        return Ok(None);
    };
    let permissions = kanban_effective_permissions(pool, &record.workspace_id, user_id).await?;
    if !permissions
        .iter()
        .any(|permission| permission == "board:view")
    {
        return Ok(None);
    }
    let members = sqlx::query_as::<_, KanbanMember>(
        "SELECT member.user_id, settings.display_name, users.email, member.role, member.status, member.created_at \
         FROM kanban_workspace_members member \
         JOIN users ON users.id = member.user_id JOIN user_settings settings ON settings.user_id = member.user_id \
         WHERE member.workspace_id = ? AND member.status = 'active' \
         ORDER BY member.role = 'admin' DESC, settings.display_name COLLATE NOCASE",
    )
    .bind(&record.workspace_id)
    .fetch_all(pool)
    .await?;
    let labels = sqlx::query_as::<_, KanbanLabel>(
        "SELECT id, board_id, name, color FROM kanban_labels WHERE board_id = ? ORDER BY name COLLATE NOCASE",
    )
    .bind(board_id)
    .fetch_all(pool)
    .await?;
    let column_records = sqlx::query_as::<_, KanbanColumnRecord>(
        "SELECT id, board_id, name, position, created_at, updated_at \
         FROM kanban_columns WHERE board_id = ? ORDER BY position ASC, created_at ASC",
    )
    .bind(board_id)
    .fetch_all(pool)
    .await?;
    let mut columns = Vec::with_capacity(column_records.len());
    for column_record in column_records {
        let card_records = sqlx::query_as::<_, KanbanCardRecord>(
            "SELECT id, column_id, title, description, due_date, position, created_at, updated_at \
             FROM kanban_cards WHERE column_id = ? AND archived_at IS NULL ORDER BY position ASC, created_at ASC",
        )
        .bind(&column_record.id)
        .fetch_all(pool)
        .await?;
        let mut cards = Vec::with_capacity(card_records.len());
        for card_record in card_records {
            cards.push(hydrate_kanban_card(pool, card_record).await?);
        }
        columns.push(KanbanColumn {
            id: column_record.id,
            board_id: column_record.board_id,
            name: column_record.name,
            position: column_record.position,
            cards,
            created_at: column_record.created_at,
            updated_at: column_record.updated_at,
        });
    }
    Ok(Some(KanbanBoard {
        id: record.id,
        workspace_id: record.workspace_id,
        name: record.name,
        description: record.description,
        visibility: record.visibility,
        archived: record.archived,
        favorite: record.favorite,
        permissions,
        members,
        labels,
        columns,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }))
}

/// Creates a column at the end of a board.
pub async fn create_kanban_column(
    pool: &SqlitePool,
    board_id: &str,
    name: &str,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM kanban_columns WHERE board_id = ?",
    )
    .bind(board_id)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "INSERT INTO kanban_columns (id, board_id, name, position, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(board_id)
    .bind(name)
    .bind(position)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Renames a column.
pub async fn rename_kanban_column(
    pool: &SqlitePool,
    column_id: &str,
    name: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE kanban_columns SET name = ?, updated_at = ? WHERE id = ?")
            .bind(name)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(column_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Reorders a column within its board.
pub async fn reorder_kanban_column(
    pool: &SqlitePool,
    column_id: &str,
    target_position: i64,
) -> Result<bool, sqlx::Error> {
    let board_id: Option<String> =
        sqlx::query_scalar("SELECT board_id FROM kanban_columns WHERE id = ?")
            .bind(column_id)
            .fetch_optional(pool)
            .await?;
    let Some(board_id) = board_id else {
        return Ok(false);
    };
    let mut identifiers = sqlx::query_scalar::<_, String>(
        "SELECT id FROM kanban_columns WHERE board_id = ? ORDER BY position ASC, created_at ASC",
    )
    .bind(board_id)
    .fetch_all(pool)
    .await?;
    let Some(current) = identifiers.iter().position(|id| id == column_id) else {
        return Ok(false);
    };
    let moved = identifiers.remove(current);
    let target = usize::try_from(target_position.max(0))
        .unwrap_or(usize::MAX)
        .min(identifiers.len());
    identifiers.insert(target, moved);
    let mut transaction = pool.begin().await?;
    let now = chrono::Utc::now().to_rfc3339();
    for (position, id) in identifiers.iter().enumerate() {
        sqlx::query("UPDATE kanban_columns SET position = ?, updated_at = ? WHERE id = ?")
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .bind(&now)
            .bind(id)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(true)
}

/// Deletes an empty column; cards must be moved first.
pub async fn delete_kanban_column(pool: &SqlitePool, column_id: &str) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM kanban_columns WHERE id = ? \
         AND NOT EXISTS(SELECT 1 FROM kanban_cards WHERE column_id = ? AND archived_at IS NULL)",
    )
    .bind(column_id)
    .bind(column_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

async fn record_kanban_activity(
    transaction: &mut Transaction<'_, Sqlite>,
    card_id: &str,
    user_id: &str,
    action: &str,
    detail: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO kanban_card_activity (id, card_id, user_id, action, detail, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(card_id)
    .bind(user_id)
    .bind(action)
    .bind(detail)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn replace_kanban_card_relations(
    transaction: &mut Transaction<'_, Sqlite>,
    card_id: &str,
    draft: &KanbanCardDraft,
) -> Result<(), sqlx::Error> {
    let context: (String, String) = sqlx::query_as(
        "SELECT board.workspace_id, board.id FROM kanban_cards card \
         JOIN kanban_columns column_record ON column_record.id = card.column_id \
         JOIN kanban_boards board ON board.id = column_record.board_id WHERE card.id = ?",
    )
    .bind(card_id)
    .fetch_one(&mut **transaction)
    .await?;
    sqlx::query("DELETE FROM kanban_card_assignees WHERE card_id = ?")
        .bind(card_id)
        .execute(&mut **transaction)
        .await?;
    for assignee_id in &draft.assignee_ids {
        sqlx::query(
            "INSERT INTO kanban_card_assignees (card_id, workspace_id, user_id, created_at) \
             SELECT ?, ?, member.user_id, ? FROM kanban_workspace_members member \
             WHERE member.workspace_id = ? AND member.user_id = ? AND member.status = 'active'",
        )
        .bind(card_id)
        .bind(&context.0)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&context.0)
        .bind(assignee_id)
        .execute(&mut **transaction)
        .await?;
    }
    sqlx::query("DELETE FROM kanban_card_labels WHERE card_id = ?")
        .bind(card_id)
        .execute(&mut **transaction)
        .await?;
    for label_id in &draft.label_ids {
        sqlx::query(
            "INSERT INTO kanban_card_labels (card_id, label_id) \
             SELECT ?, label.id FROM kanban_labels label WHERE label.id = ? AND label.board_id = ?",
        )
        .bind(card_id)
        .bind(label_id)
        .bind(&context.1)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

/// Creates a card at the bottom of a column.
pub async fn create_kanban_card(
    pool: &SqlitePool,
    column_id: &str,
    user_id: &str,
    draft: &KanbanCardDraft,
) -> Result<KanbanCard, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM kanban_cards \
         WHERE column_id = ? AND archived_at IS NULL",
    )
    .bind(column_id)
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO kanban_cards \
         (id, column_id, title, description, due_date, position, created_by_user_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(column_id)
    .bind(&draft.title)
    .bind(&draft.description)
    .bind(&draft.due_date)
    .bind(position)
    .bind(user_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    replace_kanban_card_relations(&mut transaction, &id, draft).await?;
    record_kanban_activity(&mut transaction, &id, user_id, "card.created", "").await?;
    transaction.commit().await?;
    get_kanban_card(pool, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Updates card content, due date, labels, and assignees atomically.
pub async fn update_kanban_card(
    pool: &SqlitePool,
    card_id: &str,
    user_id: &str,
    draft: &KanbanCardDraft,
) -> Result<Option<KanbanCard>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE kanban_cards SET title = ?, description = ?, due_date = ?, updated_at = ? \
         WHERE id = ? AND archived_at IS NULL",
    )
    .bind(&draft.title)
    .bind(&draft.description)
    .bind(&draft.due_date)
    .bind(&now)
    .bind(card_id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() == 0 {
        transaction.rollback().await?;
        return Ok(None);
    }
    replace_kanban_card_relations(&mut transaction, card_id, draft).await?;
    record_kanban_activity(&mut transaction, card_id, user_id, "card.updated", "").await?;
    transaction.commit().await?;
    get_kanban_card(pool, card_id).await
}

async fn rewrite_kanban_card_positions(
    transaction: &mut Transaction<'_, Sqlite>,
    column_id: &str,
    card_ids: &[String],
    updated_at: &str,
) -> Result<(), sqlx::Error> {
    for (position, card_id) in card_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE kanban_cards SET column_id = ?, position = ?, updated_at = ? WHERE id = ?",
        )
        .bind(column_id)
        .bind(i64::try_from(position).unwrap_or(i64::MAX))
        .bind(updated_at)
        .bind(card_id)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

/// Moves and reorders a card within one board in a single transaction.
pub async fn move_kanban_card(
    pool: &SqlitePool,
    card_id: &str,
    target_column_id: &str,
    target_position: i64,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    let context: Option<(String, String, String)> = sqlx::query_as(
        "SELECT card.column_id, source.board_id, target.board_id \
         FROM kanban_cards card \
         JOIN kanban_columns source ON source.id = card.column_id \
         JOIN kanban_columns target ON target.id = ? \
         WHERE card.id = ? AND card.archived_at IS NULL",
    )
    .bind(target_column_id)
    .bind(card_id)
    .fetch_optional(pool)
    .await?;
    let Some((source_column_id, source_board_id, target_board_id)) = context else {
        return Ok(false);
    };
    if source_board_id != target_board_id {
        return Ok(false);
    }
    let mut source_ids = sqlx::query_scalar::<_, String>(
        "SELECT id FROM kanban_cards WHERE column_id = ? AND archived_at IS NULL \
         ORDER BY position ASC, created_at ASC",
    )
    .bind(&source_column_id)
    .fetch_all(pool)
    .await?;
    source_ids.retain(|id| id != card_id);
    let mut target_ids = if source_column_id == target_column_id {
        source_ids.clone()
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM kanban_cards WHERE column_id = ? AND archived_at IS NULL \
             ORDER BY position ASC, created_at ASC",
        )
        .bind(target_column_id)
        .fetch_all(pool)
        .await?
    };
    let target = usize::try_from(target_position.max(0))
        .unwrap_or(usize::MAX)
        .min(target_ids.len());
    target_ids.insert(target, card_id.to_owned());
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    if source_column_id != target_column_id {
        rewrite_kanban_card_positions(&mut transaction, &source_column_id, &source_ids, &now)
            .await?;
    }
    rewrite_kanban_card_positions(&mut transaction, target_column_id, &target_ids, &now).await?;
    record_kanban_activity(
        &mut transaction,
        card_id,
        user_id,
        "card.moved",
        target_column_id,
    )
    .await?;
    transaction.commit().await?;
    Ok(true)
}

/// Archives a card while preserving its collaboration history.
pub async fn archive_kanban_card(
    pool: &SqlitePool,
    card_id: &str,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE kanban_cards SET archived_at = ?, updated_at = ? WHERE id = ? AND archived_at IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(card_id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() > 0 {
        record_kanban_activity(&mut transaction, card_id, user_id, "card.archived", "").await?;
    }
    transaction.commit().await?;
    Ok(result.rows_affected() > 0)
}

/// Creates a board-scoped label.
pub async fn create_kanban_label(
    pool: &SqlitePool,
    board_id: &str,
    name: &str,
    color: &str,
) -> Result<KanbanLabel, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO kanban_labels (id, board_id, name, color, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(board_id)
    .bind(name)
    .bind(color)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(KanbanLabel {
        id,
        board_id: board_id.to_owned(),
        name: name.to_owned(),
        color: color.to_owned(),
    })
}

/// Deletes a board-scoped label and its card relationships.
pub async fn delete_kanban_label(
    pool: &SqlitePool,
    board_id: &str,
    label_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM kanban_labels WHERE id = ? AND board_id = ?")
            .bind(label_id)
            .bind(board_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Resolves a comment to its workspace and author.
pub async fn kanban_comment_context(
    pool: &SqlitePool,
    comment_id: &str,
) -> Result<Option<(String, Option<String>)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT board.workspace_id, comment.user_id FROM kanban_comments comment \
         JOIN kanban_cards card ON card.id = comment.card_id \
         JOIN kanban_columns column_record ON column_record.id = card.column_id \
         JOIN kanban_boards board ON board.id = column_record.board_id WHERE comment.id = ?",
    )
    .bind(comment_id)
    .fetch_optional(pool)
    .await
}

/// Adds a comment and activity entry.
pub async fn create_kanban_comment(
    pool: &SqlitePool,
    card_id: &str,
    user_id: &str,
    content: &str,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO kanban_comments (id, card_id, user_id, content, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(card_id)
    .bind(user_id)
    .bind(content)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    record_kanban_activity(
        &mut transaction,
        card_id,
        user_id,
        "comment.created",
        content,
    )
    .await?;
    transaction.commit().await?;
    Ok(id)
}

/// Updates a comment after author or permission checks by the server.
pub async fn update_kanban_comment(
    pool: &SqlitePool,
    comment_id: &str,
    content: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE kanban_comments SET content = ?, updated_at = ? WHERE id = ?")
            .bind(content)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(comment_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Deletes a comment after author or permission checks by the server.
pub async fn delete_kanban_comment(
    pool: &SqlitePool,
    comment_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM kanban_comments WHERE id = ?")
        .bind(comment_id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

/// Adds a checklist to a card.
pub async fn create_kanban_checklist(
    pool: &SqlitePool,
    card_id: &str,
    name: &str,
    user_id: &str,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM kanban_checklists WHERE card_id = ?",
    )
    .bind(card_id)
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO kanban_checklists (id, card_id, name, position, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(card_id)
    .bind(name)
    .bind(position)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    record_kanban_activity(
        &mut transaction,
        card_id,
        user_id,
        "checklist.created",
        name,
    )
    .await?;
    transaction.commit().await?;
    Ok(id)
}

/// Resolves a checklist to its workspace and card.
pub async fn kanban_checklist_context(
    pool: &SqlitePool,
    checklist_id: &str,
) -> Result<Option<(String, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT board.workspace_id, checklist.card_id FROM kanban_checklists checklist \
         JOIN kanban_cards card ON card.id = checklist.card_id \
         JOIN kanban_columns column_record ON column_record.id = card.column_id \
         JOIN kanban_boards board ON board.id = column_record.board_id WHERE checklist.id = ?",
    )
    .bind(checklist_id)
    .fetch_optional(pool)
    .await
}

/// Adds an item to a checklist.
pub async fn create_kanban_checklist_item(
    pool: &SqlitePool,
    checklist_id: &str,
    title: &str,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM kanban_checklist_items WHERE checklist_id = ?",
    )
    .bind(checklist_id)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "INSERT INTO kanban_checklist_items \
         (id, checklist_id, title, completed, position, created_at, updated_at) \
         VALUES (?, ?, ?, 0, ?, ?, ?)",
    )
    .bind(&id)
    .bind(checklist_id)
    .bind(title)
    .bind(position)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Updates a checklist item title and completion state.
pub async fn update_kanban_checklist_item(
    pool: &SqlitePool,
    checklist_id: &str,
    item_id: &str,
    title: &str,
    completed: bool,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE kanban_checklist_items SET title = ?, completed = ?, updated_at = ? \
         WHERE id = ? AND checklist_id = ?",
    )
    .bind(title)
    .bind(completed)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(item_id)
    .bind(checklist_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

/// Deletes a checklist and all of its items.
pub async fn delete_kanban_checklist(
    pool: &SqlitePool,
    checklist_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM kanban_checklists WHERE id = ?")
        .bind(checklist_id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

/// Stores one workspace-authorized card attachment in SQLite.
pub async fn create_kanban_attachment(
    pool: &SqlitePool,
    card_id: &str,
    user_id: &str,
    file_name: &str,
    mime_type: &str,
    data: &[u8],
) -> Result<KanbanAttachment, sqlx::Error> {
    let attachment = KanbanAttachment {
        id: uuid::Uuid::new_v4().to_string(),
        card_id: card_id.to_owned(),
        file_name: file_name.to_owned(),
        mime_type: mime_type.to_owned(),
        byte_size: i64::try_from(data.len()).unwrap_or(i64::MAX),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO kanban_attachments \
         (id, card_id, user_id, file_name, mime_type, byte_size, file_data, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&attachment.id)
    .bind(card_id)
    .bind(user_id)
    .bind(file_name)
    .bind(mime_type)
    .bind(attachment.byte_size)
    .bind(data)
    .bind(&attachment.created_at)
    .execute(&mut *transaction)
    .await?;
    record_kanban_activity(
        &mut transaction,
        card_id,
        user_id,
        "attachment.created",
        file_name,
    )
    .await?;
    transaction.commit().await?;
    Ok(attachment)
}

/// Loads attachment bytes through the parent card workspace.
pub async fn get_kanban_attachment(
    pool: &SqlitePool,
    attachment_id: &str,
) -> Result<Option<(String, String, String, Vec<u8>)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT board.workspace_id, attachment.file_name, attachment.mime_type, attachment.file_data \
         FROM kanban_attachments attachment \
         JOIN kanban_cards card ON card.id = attachment.card_id \
         JOIN kanban_columns column_record ON column_record.id = card.column_id \
         JOIN kanban_boards board ON board.id = column_record.board_id \
         WHERE attachment.id = ?",
    )
    .bind(attachment_id)
    .fetch_optional(pool)
    .await
}

/// Deletes an attachment after authorization through its workspace.
pub async fn delete_kanban_attachment(
    pool: &SqlitePool,
    attachment_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM kanban_attachments WHERE id = ?")
        .bind(attachment_id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}
