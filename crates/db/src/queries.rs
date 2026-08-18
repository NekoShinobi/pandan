use crate::entities::{
    AppMetadata, CalendarEvent, CalendarEventDraft, CalendarSubscription, CodingCredential,
    CodingProject, DashboardWidget, FeedItem, JournalNode, ManagedUser, OidcAuthorization,
    PaymentSubscription, RssItem, RssItemDraft, RssSubscription, RssSubscriptionDraft,
    SessionAccount, Task, TaskAttachment, TaskDraft, TaskSubtask, User, UserAppearance, UserAvatar,
    UserBackground, UserCredentials, UserSettings, Workspace,
};
pub use crate::youtube_queries::*;
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

const DEFAULT_WIDGETS: &[(&str, i64, i64, &str, i64, i64, i64, i64)] = &[
    ("weather", 0, 0, "wide", 0, 0, 8, 5),
    ("task-summary", 0, 1, "compact", 8, 0, 4, 5),
    ("search", 0, 2, "standard", 0, 5, 6, 4),
    ("focus", 0, 3, "standard", 6, 5, 6, 4),
    ("task-list", 0, 4, "wide", 0, 9, 8, 6),
    ("task-progress", 0, 5, "compact", 8, 9, 4, 6),
    ("feed-list", 0, 6, "wide", 0, 15, 8, 6),
    ("feed-sources", 0, 7, "compact", 8, 15, 4, 6),
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
struct UserSettingsRecord {
    user_id: String,
    display_name: String,
    location: String,
    timezone: String,
    sidebar_timezones_json: String,
    temperature_unit: String,
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
            temperature_unit: self.temperature_unit,
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
            groups + subscriptions + settings
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
         last_fetched_at, last_error, created_at, updated_at \
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
         i.title, i.summary, i.published_at, i.fetched_at, i.read_at \
         FROM rss_items i JOIN rss_subscriptions s ON s.id = i.subscription_id \
         WHERE s.user_id = ? ORDER BY datetime(i.published_at) DESC, i.fetched_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
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
         last_fetched_at, last_error, created_at, updated_at \
         FROM rss_subscriptions WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

async fn upsert_rss_items(
    transaction: &mut Transaction<'_, Sqlite>,
    subscription_id: &str,
    items: &[RssItemDraft],
    fetched_at: &str,
) -> Result<(), sqlx::Error> {
    for item in items {
        sqlx::query(
            "INSERT INTO rss_items \
             (id, subscription_id, external_id, url, title, summary, published_at, fetched_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(subscription_id, external_id) DO UPDATE SET \
             url = excluded.url, title = excluded.title, summary = excluded.summary, \
             published_at = excluded.published_at, fetched_at = excluded.fetched_at",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(subscription_id)
        .bind(&item.external_id)
        .bind(&item.url)
        .bind(&item.title)
        .bind(&item.summary)
        .bind(&item.published_at)
        .bind(fetched_at)
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
          last_fetched_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .execute(&mut *transaction)
    .await?;
    upsert_rss_items(&mut transaction, &id, items, &now).await?;
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
        "UPDATE rss_subscriptions SET title = ?, last_fetched_at = ?, last_error = NULL, \
         updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(title)
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
    upsert_rss_items(&mut transaction, id, items, &now).await?;
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
    sqlx::query(
        "UPDATE rss_subscriptions SET last_error = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(message)
    .bind(chrono::Utc::now().to_rfc3339())
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
         i.title, i.summary, i.published_at, i.fetched_at, i.read_at \
         FROM rss_items i JOIN rss_subscriptions s ON s.id = i.subscription_id \
         WHERE i.id = ? AND s.user_id = ?",
    )
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
        "DELETE FROM rss_items WHERE EXISTS (\
         SELECT 1 FROM rss_subscriptions s WHERE s.id = rss_items.subscription_id \
         AND s.user_id = ? AND s.auto_delete_days IS NOT NULL \
         AND datetime(rss_items.published_at) < datetime('now', '-' || s.auto_delete_days || ' days') \
         AND (s.auto_delete_mode = 'all' OR rss_items.read_at IS NOT NULL))",
    )
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
        "DELETE FROM rss_items WHERE EXISTS (\
         SELECT 1 FROM rss_subscriptions s WHERE s.id = rss_items.subscription_id AND s.user_id = ?) \
         AND datetime(published_at) < datetime('now', '-' || ? || ' days') \
         AND (? = 'all' OR read_at IS NOT NULL)",
    )
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
            temperature_unit: "celsius".to_owned(),
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
    expires_at: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(token)
    .bind(user_id)
    .bind(expires_at)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
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
                user_settings.sidebar_timezones_json, user_settings.temperature_unit, \
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
    temperature_unit: &str,
) -> Result<UserSettings, sqlx::Error> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE user_settings SET display_name = ?, location = ?, timezone = ?, \
         sidebar_timezones_json = ?, temperature_unit = ?, updated_at = ? WHERE user_id = ?",
    )
    .bind(display_name)
    .bind(location)
    .bind(timezone)
    .bind(sidebar_timezones_json)
    .bind(temperature_unit)
    .bind(&updated_at)
    .bind(user_id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, UserSettingsRecord>(
        "SELECT user_id, display_name, location, timezone, sidebar_timezones_json, \
                temperature_unit, updated_at \
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
    .execute(pool)
    .await?;
    Ok(())
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
    Ok(
        sqlx::query("DELETE FROM user_wallpapers WHERE user_id = ? AND slot = ?")
            .bind(user_id)
            .bind(slot)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// Removes the singleton login wallpaper.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the delete cannot be completed.
pub async fn delete_login_wallpaper(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM user_wallpapers WHERE slot = 'login'")
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
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
                  AND user_wallpapers.slot = 'dashboard') AS has_dashboard_wallpaper, \
                EXISTS(SELECT 1 FROM user_wallpapers \
                WHERE user_wallpapers.user_id = user_appearance.user_id \
                  AND user_wallpapers.slot = 'welcome') AS has_welcome_wallpaper, \
                EXISTS(SELECT 1 FROM user_wallpapers \
                WHERE user_wallpapers.user_id = user_appearance.user_id \
                  AND user_wallpapers.slot = 'loading') AS has_loading_wallpaper, \
                EXISTS(SELECT 1 FROM user_wallpapers \
                JOIN users ON users.id = user_wallpapers.user_id \
                WHERE user_wallpapers.slot = 'login' \
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

/// Lists all accounts for the administrator directory.
///
/// # Errors
///
/// Returns the underlying `SQLx` error when the query cannot be completed.
pub async fn list_managed_users(pool: &SqlitePool) -> Result<Vec<ManagedUser>, sqlx::Error> {
    sqlx::query_as::<_, ManagedUser>(
        "SELECT users.id, users.email, user_settings.display_name, users.role, users.created_at \
         FROM users \
         JOIN user_settings ON user_settings.user_id = users.id \
         ORDER BY users.role = 'administrator' DESC, users.created_at ASC",
    )
    .fetch_all(pool)
    .await
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
        "SELECT users.id, users.email, user_settings.display_name, users.role, users.created_at \
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
) -> Result<String, sqlx::Error> {
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
        return Ok(user_id);
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
    Ok(user_id)
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
    let user_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut transaction = pool.begin().await?;
    let claim = sqlx::query(
        "INSERT INTO app_metadata (key, value, updated_at) \
         SELECT 'onboarding_complete', 'true', ? \
         WHERE NOT EXISTS (SELECT 1 FROM users) \
         ON CONFLICT(key) DO NOTHING",
    )
    .bind(&now)
    .execute(&mut *transaction)
    .await?;
    if claim.rows_affected() != 1 {
        transaction.rollback().await?;
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
            temperature_unit: "celsius".to_owned(),
            updated_at: now,
        },
    )))
}
