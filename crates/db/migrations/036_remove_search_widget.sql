-- The dashboard web search widget was replaced by the global command palette.
-- Remove placed instances, then rebuild the table without 'search' in the kind check.
DELETE FROM dashboard_widgets WHERE kind = 'search';

CREATE TABLE dashboard_widgets_next (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN (
        'weather', 'task-summary', 'focus', 'task-list', 'task-progress',
        'feed-list', 'feed-sources', 'youtube', 'rss', 'reddit', 'stocks',
        'calendar', 'clock', 'iframe', 'html', 'releases', 'streams', 'bible-verse'
    )),
    workspace INTEGER NOT NULL CHECK (workspace BETWEEN 0 AND 31),
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 127),
    size TEXT NOT NULL CHECK (size IN ('compact', 'standard', 'wide', 'full')),
    config_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(config_json)),
    grid_x INTEGER NOT NULL DEFAULT 0 CHECK (grid_x BETWEEN 0 AND 11),
    grid_y INTEGER NOT NULL DEFAULT 0 CHECK (grid_y BETWEEN 0 AND 255),
    grid_w INTEGER NOT NULL DEFAULT 6 CHECK (grid_w BETWEEN 1 AND 12),
    grid_h INTEGER NOT NULL DEFAULT 4 CHECK (grid_h BETWEEN 1 AND 12),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (user_id, workspace)
        REFERENCES user_workspaces(user_id, workspace) ON DELETE CASCADE
);

-- Close the ordering gap the removed widget leaves behind.
INSERT INTO dashboard_widgets_next
    (id, user_id, kind, workspace, position, size, config_json,
     grid_x, grid_y, grid_w, grid_h, created_at, updated_at)
SELECT id, user_id, kind, workspace,
       ROW_NUMBER() OVER (
           PARTITION BY user_id, workspace ORDER BY position, id
       ) - 1,
       size, config_json, grid_x, grid_y, grid_w, grid_h, created_at, updated_at
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
CREATE INDEX dashboard_widgets_grid_idx
    ON dashboard_widgets (user_id, workspace, grid_y, grid_x);
CREATE INDEX widget_secrets_user_idx ON widget_secrets (user_id);
