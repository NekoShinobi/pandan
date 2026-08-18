PRAGMA foreign_keys = OFF;

CREATE TABLE dashboard_widgets_next (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN (
        'weather', 'task-summary', 'search', 'focus', 'task-list', 'task-progress',
        'feed-list', 'feed-sources', 'youtube', 'rss', 'reddit', 'stocks',
        'calendar', 'clock', 'iframe', 'html', 'releases', 'streams'
    )),
    workspace INTEGER NOT NULL CHECK (workspace BETWEEN 0 AND 2),
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 127),
    size TEXT NOT NULL CHECK (size IN ('compact', 'standard', 'wide', 'full')),
    config_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(config_json)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO dashboard_widgets_next
    (id, user_id, kind, workspace, position, size, config_json, created_at, updated_at)
SELECT id, user_id, kind, workspace, position, size, '{}', created_at, updated_at
FROM dashboard_widgets;

DROP TABLE dashboard_widgets;
ALTER TABLE dashboard_widgets_next RENAME TO dashboard_widgets;

CREATE INDEX dashboard_widgets_user_workspace_position_idx
    ON dashboard_widgets (user_id, workspace, position);

CREATE TABLE widget_secrets (
    widget_id TEXT PRIMARY KEY REFERENCES dashboard_widgets(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ciphertext TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX widget_secrets_user_idx ON widget_secrets (user_id);

PRAGMA foreign_keys = ON;
