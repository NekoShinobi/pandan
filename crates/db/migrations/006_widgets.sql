CREATE TABLE IF NOT EXISTS dashboard_widgets (
    id         TEXT PRIMARY KEY NOT NULL,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL CHECK (
        kind IN (
            'weather',
            'task-summary',
            'search',
            'focus',
            'task-list',
            'task-progress',
            'feed-list',
            'feed-sources'
        )
    ),
    workspace  INTEGER NOT NULL CHECK (workspace BETWEEN 0 AND 2),
    position   INTEGER NOT NULL CHECK (position BETWEEN 0 AND 127),
    size       TEXT NOT NULL CHECK (size IN ('compact', 'standard', 'wide', 'full')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS dashboard_widgets_user_workspace_position_idx
    ON dashboard_widgets (user_id, workspace, position);

INSERT OR IGNORE INTO dashboard_widgets
    (id, user_id, kind, workspace, position, size, created_at, updated_at)
SELECT id || '-weather', id, 'weather', 0, 0, 'wide', datetime('now'), datetime('now')
FROM users;

INSERT OR IGNORE INTO dashboard_widgets
    (id, user_id, kind, workspace, position, size, created_at, updated_at)
SELECT id || '-task-summary', id, 'task-summary', 0, 1, 'compact', datetime('now'), datetime('now')
FROM users;

INSERT OR IGNORE INTO dashboard_widgets
    (id, user_id, kind, workspace, position, size, created_at, updated_at)
SELECT id || '-search', id, 'search', 0, 2, 'standard', datetime('now'), datetime('now')
FROM users;

INSERT OR IGNORE INTO dashboard_widgets
    (id, user_id, kind, workspace, position, size, created_at, updated_at)
SELECT id || '-focus', id, 'focus', 0, 3, 'standard', datetime('now'), datetime('now')
FROM users;

INSERT OR IGNORE INTO dashboard_widgets
    (id, user_id, kind, workspace, position, size, created_at, updated_at)
SELECT id || '-task-list', id, 'task-list', 1, 0, 'wide', datetime('now'), datetime('now')
FROM users;

INSERT OR IGNORE INTO dashboard_widgets
    (id, user_id, kind, workspace, position, size, created_at, updated_at)
SELECT id || '-task-progress', id, 'task-progress', 1, 1, 'compact', datetime('now'), datetime('now')
FROM users;

INSERT OR IGNORE INTO dashboard_widgets
    (id, user_id, kind, workspace, position, size, created_at, updated_at)
SELECT id || '-feed-list', id, 'feed-list', 2, 0, 'wide', datetime('now'), datetime('now')
FROM users;

INSERT OR IGNORE INTO dashboard_widgets
    (id, user_id, kind, workspace, position, size, created_at, updated_at)
SELECT id || '-feed-sources', id, 'feed-sources', 2, 1, 'compact', datetime('now'), datetime('now')
FROM users;
