use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanOverview {
    pub workspaces: Vec<KanbanWorkspace>,
    pub invitations: Vec<KanbanInvitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanWorkspace {
    pub id: String,
    pub name: String,
    pub description: String,
    pub role: String,
    pub member_count: i64,
    pub board_count: i64,
    pub permissions: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanInvitation {
    pub workspace_id: String,
    pub workspace_name: String,
    pub role: String,
    pub invited_by_name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanMember {
    pub user_id: String,
    pub display_name: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanDirectoryUser {
    pub user_id: String,
    pub display_name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanWorkspaceSettings {
    pub workspace: KanbanWorkspace,
    pub members: Vec<KanbanMember>,
    pub role_permissions: Vec<KanbanRolePermission>,
    pub member_overrides: Vec<KanbanMemberPermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanRolePermission {
    pub role: String,
    pub permission: String,
    pub granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanMemberPermission {
    pub user_id: String,
    pub permission: String,
    pub granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanBoardSummary {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    pub visibility: String,
    pub archived: bool,
    pub favorite: bool,
    pub position: i64,
    pub column_count: i64,
    pub card_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanBoard {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    pub visibility: String,
    pub archived: bool,
    pub favorite: bool,
    pub permissions: Vec<String>,
    pub members: Vec<KanbanMember>,
    pub labels: Vec<KanbanLabel>,
    pub columns: Vec<KanbanColumn>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanColumn {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub position: i64,
    pub cards: Vec<KanbanCard>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanCard {
    pub id: String,
    pub column_id: String,
    pub title: String,
    pub description: String,
    pub due_date: Option<String>,
    pub position: i64,
    pub assignees: Vec<KanbanMember>,
    pub labels: Vec<KanbanLabel>,
    pub comments: Vec<KanbanComment>,
    pub checklists: Vec<KanbanChecklist>,
    pub attachments: Vec<KanbanAttachment>,
    pub activity: Vec<KanbanActivity>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanLabel {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanComment {
    pub id: String,
    pub card_id: String,
    pub user_id: Option<String>,
    pub author_name: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanbanChecklist {
    pub id: String,
    pub card_id: String,
    pub name: String,
    pub position: i64,
    pub items: Vec<KanbanChecklistItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanChecklistItem {
    pub id: String,
    pub checklist_id: String,
    pub title: String,
    pub completed: bool,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanAttachment {
    pub id: String,
    pub card_id: String,
    pub file_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct KanbanActivity {
    pub id: String,
    pub card_id: String,
    pub actor_name: String,
    pub action: String,
    pub detail: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanCardDraft {
    pub title: String,
    pub description: String,
    pub due_date: Option<String>,
    pub assignee_ids: Vec<String>,
    pub label_ids: Vec<String>,
}
