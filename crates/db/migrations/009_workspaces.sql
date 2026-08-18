CREATE TABLE user_workspaces (
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace  INTEGER NOT NULL CHECK (workspace BETWEEN 0 AND 31),
    name       TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 40),
    position   INTEGER NOT NULL CHECK (position BETWEEN 0 AND 31),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, workspace),
    UNIQUE (user_id, position)
);

INSERT INTO user_workspaces (user_id, workspace, name, position, created_at, updated_at)
SELECT id, 0, 'Overview', 0, created_at, created_at FROM users
UNION ALL
SELECT id, 1, 'Tasks', 1, created_at, created_at FROM users
UNION ALL
SELECT id, 2, 'Feeds', 2, created_at, created_at FROM users;

CREATE TABLE dashboard_widgets_next (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN (
        'weather', 'task-summary', 'search', 'focus', 'task-list', 'task-progress',
        'feed-list', 'feed-sources', 'youtube', 'rss', 'reddit', 'stocks',
        'calendar', 'clock', 'iframe', 'html', 'releases', 'streams'
    )),
    workspace INTEGER NOT NULL CHECK (workspace BETWEEN 0 AND 31),
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 127),
    size TEXT NOT NULL CHECK (size IN ('compact', 'standard', 'wide', 'full')),
    config_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(config_json)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (user_id, workspace)
        REFERENCES user_workspaces(user_id, workspace) ON DELETE CASCADE
);

INSERT INTO dashboard_widgets_next
    (id, user_id, kind, workspace, position, size, config_json, created_at, updated_at)
SELECT id, user_id, kind, workspace, position, size, config_json, created_at, updated_at
FROM dashboard_widgets;

CREATE TABLE widget_secrets_next (
    widget_id TEXT PRIMARY KEY REFERENCES dashboard_widgets_next(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ciphertext TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO widget_secrets_next (widget_id, user_id, ciphertext, updated_at)
SELECT widget_id, user_id, ciphertext, updated_at FROM widget_secrets;

DROP TABLE widget_secrets;
DROP TABLE dashboard_widgets;
ALTER TABLE dashboard_widgets_next RENAME TO dashboard_widgets;
ALTER TABLE widget_secrets_next RENAME TO widget_secrets;

CREATE INDEX dashboard_widgets_user_workspace_position_idx
    ON dashboard_widgets (user_id, workspace, position);
CREATE INDEX widget_secrets_user_idx ON widget_secrets (user_id);

CREATE TABLE user_backgrounds_next (
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace  INTEGER NOT NULL CHECK (workspace BETWEEN 0 AND 31),
    mime_type  TEXT NOT NULL CHECK (mime_type IN ('image/jpeg', 'image/png', 'image/webp', 'image/avif')),
    image_data BLOB NOT NULL CHECK (length(image_data) BETWEEN 1 AND 8388608),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, workspace),
    FOREIGN KEY (user_id, workspace)
        REFERENCES user_workspaces(user_id, workspace) ON DELETE CASCADE
);

INSERT INTO user_backgrounds_next (user_id, workspace, mime_type, image_data, updated_at)
SELECT user_id, workspace, mime_type, image_data, updated_at FROM user_backgrounds;

DROP TABLE user_backgrounds;
ALTER TABLE user_backgrounds_next RENAME TO user_backgrounds;
