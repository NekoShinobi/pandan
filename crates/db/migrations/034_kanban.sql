CREATE TABLE kanban_workspaces (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 1000),
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE kanban_workspace_members (
    workspace_id TEXT NOT NULL REFERENCES kanban_workspaces(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('admin', 'member', 'guest')),
    status TEXT NOT NULL DEFAULT 'invited' CHECK(status IN ('invited', 'active')),
    invited_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (workspace_id, user_id)
);

CREATE INDEX kanban_workspace_members_user_status_idx
    ON kanban_workspace_members(user_id, status, workspace_id);

CREATE TABLE kanban_role_permissions (
    workspace_id TEXT NOT NULL REFERENCES kanban_workspaces(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('admin', 'member', 'guest')),
    permission TEXT NOT NULL CHECK(permission IN (
        'workspace:view', 'workspace:edit', 'workspace:delete', 'workspace:manage',
        'board:view', 'board:create', 'board:edit', 'board:delete',
        'list:view', 'list:create', 'list:edit', 'list:delete',
        'card:view', 'card:create', 'card:edit', 'card:delete',
        'comment:view', 'comment:create', 'comment:edit', 'comment:delete',
        'member:view', 'member:invite', 'member:edit', 'member:remove'
    )),
    granted INTEGER NOT NULL DEFAULT 1 CHECK(granted IN (0, 1)),
    PRIMARY KEY (workspace_id, role, permission)
);

CREATE TABLE kanban_member_permissions (
    workspace_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    permission TEXT NOT NULL CHECK(permission IN (
        'workspace:view', 'workspace:edit', 'workspace:delete', 'workspace:manage',
        'board:view', 'board:create', 'board:edit', 'board:delete',
        'list:view', 'list:create', 'list:edit', 'list:delete',
        'card:view', 'card:create', 'card:edit', 'card:delete',
        'comment:view', 'comment:create', 'comment:edit', 'comment:delete',
        'member:view', 'member:invite', 'member:edit', 'member:remove'
    )),
    granted INTEGER NOT NULL CHECK(granted IN (0, 1)),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (workspace_id, user_id, permission),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES kanban_workspace_members(workspace_id, user_id) ON DELETE CASCADE
);

CREATE TABLE kanban_boards (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES kanban_workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    visibility TEXT NOT NULL DEFAULT 'private' CHECK(visibility IN ('private', 'public')),
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0, 1)),
    position INTEGER NOT NULL DEFAULT 0 CHECK(position >= 0),
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX kanban_boards_workspace_archive_position_idx
    ON kanban_boards(workspace_id, archived, position);

CREATE TABLE kanban_board_favorites (
    board_id TEXT NOT NULL REFERENCES kanban_boards(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (board_id, user_id)
);

CREATE TABLE kanban_columns (
    id TEXT PRIMARY KEY NOT NULL,
    board_id TEXT NOT NULL REFERENCES kanban_boards(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    position INTEGER NOT NULL CHECK(position >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX kanban_columns_board_position_idx
    ON kanban_columns(board_id, position);

CREATE TABLE kanban_cards (
    id TEXT PRIMARY KEY NOT NULL,
    column_id TEXT NOT NULL REFERENCES kanban_columns(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 240),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 100000),
    due_date TEXT,
    position INTEGER NOT NULL CHECK(position >= 0),
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    archived_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX kanban_cards_column_archive_position_idx
    ON kanban_cards(column_id, archived_at, position);

CREATE TABLE kanban_card_assignees (
    card_id TEXT NOT NULL REFERENCES kanban_cards(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (card_id, user_id),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES kanban_workspace_members(workspace_id, user_id) ON DELETE CASCADE
);

CREATE TABLE kanban_labels (
    id TEXT PRIMARY KEY NOT NULL,
    board_id TEXT NOT NULL REFERENCES kanban_boards(id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE CHECK(length(trim(name)) BETWEEN 1 AND 40),
    color TEXT NOT NULL DEFAULT 'accent'
        CHECK(color IN ('accent', 'blue', 'amber', 'red', 'violet', 'gray')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(board_id, name)
);

CREATE TABLE kanban_card_labels (
    card_id TEXT NOT NULL REFERENCES kanban_cards(id) ON DELETE CASCADE,
    label_id TEXT NOT NULL REFERENCES kanban_labels(id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, label_id)
);

CREATE TABLE kanban_comments (
    id TEXT PRIMARY KEY NOT NULL,
    card_id TEXT NOT NULL REFERENCES kanban_cards(id) ON DELETE CASCADE,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    content TEXT NOT NULL CHECK(length(trim(content)) BETWEEN 1 AND 10000),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX kanban_comments_card_created_idx
    ON kanban_comments(card_id, created_at);

CREATE TABLE kanban_checklists (
    id TEXT PRIMARY KEY NOT NULL,
    card_id TEXT NOT NULL REFERENCES kanban_cards(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
    position INTEGER NOT NULL CHECK(position >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE kanban_checklist_items (
    id TEXT PRIMARY KEY NOT NULL,
    checklist_id TEXT NOT NULL REFERENCES kanban_checklists(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 500),
    completed INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0, 1)),
    position INTEGER NOT NULL CHECK(position >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX kanban_checklist_items_checklist_position_idx
    ON kanban_checklist_items(checklist_id, position);

CREATE TABLE kanban_attachments (
    id TEXT PRIMARY KEY NOT NULL,
    card_id TEXT NOT NULL REFERENCES kanban_cards(id) ON DELETE CASCADE,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    file_name TEXT NOT NULL CHECK(length(trim(file_name)) BETWEEN 1 AND 255),
    mime_type TEXT NOT NULL CHECK(length(trim(mime_type)) BETWEEN 1 AND 120),
    byte_size INTEGER NOT NULL CHECK(byte_size BETWEEN 1 AND 10485760),
    file_data BLOB NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX kanban_attachments_card_created_idx
    ON kanban_attachments(card_id, created_at);

CREATE TABLE kanban_card_activity (
    id TEXT PRIMARY KEY NOT NULL,
    card_id TEXT NOT NULL REFERENCES kanban_cards(id) ON DELETE CASCADE,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL CHECK(length(trim(action)) BETWEEN 1 AND 80),
    detail TEXT NOT NULL DEFAULT '' CHECK(length(detail) <= 2000),
    created_at TEXT NOT NULL
);

CREATE INDEX kanban_card_activity_card_created_idx
    ON kanban_card_activity(card_id, created_at DESC);
