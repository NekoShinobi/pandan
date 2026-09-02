-- Move every Dashboard surface into the persisted widget canvas. Existing user layouts are
-- preserved; the former fixed utilities are appended so the owner can reposition or remove them.
PRAGMA foreign_keys = OFF;

CREATE TABLE dashboard_widgets_next (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN (
        'welcome', 'local-time', 'calendar-overview', 'bookmarks',
        'section-header', 'divider',
        'weather', 'task-summary', 'focus', 'task-list',
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

INSERT INTO dashboard_widgets_next
    (id, user_id, kind, workspace, position, size, config_json,
     grid_x, grid_y, grid_w, grid_h, created_at, updated_at)
SELECT id, user_id, kind, workspace, position, size, config_json,
       grid_x, grid_y, grid_w, grid_h, created_at, updated_at
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

-- The previously hidden tracker joins the canvas below the owner's existing layout.
UPDATE dashboard_widgets AS tracker
SET config_json = json_remove(config_json, '$.placement'),
    size = 'compact',
    grid_x = 8,
    grid_y = MIN(255, COALESCE((
        SELECT MAX(sibling.grid_y + sibling.grid_h)
        FROM dashboard_widgets AS sibling
        WHERE sibling.user_id = tracker.user_id
          AND sibling.workspace = 0
          AND sibling.id <> tracker.id
    ), 0)),
    grid_w = 4,
    grid_h = 4,
    updated_at = datetime('now')
WHERE tracker.kind = 'streams'
  AND json_extract(tracker.config_json, '$.placement') = 'utility_rail';

WITH layout_base AS (
    SELECT users.id AS user_id,
           MIN(252, COALESCE(MAX(widgets.grid_y + widgets.grid_h), 0)) AS grid_y,
           MIN(123, COALESCE(MAX(widgets.position) + 1, 0)) AS position
    FROM users
    LEFT JOIN dashboard_widgets AS widgets
      ON widgets.user_id = users.id AND widgets.workspace = 0
    GROUP BY users.id
)
INSERT INTO dashboard_widgets
    (id, user_id, kind, workspace, position, size, config_json,
     grid_x, grid_y, grid_w, grid_h, created_at, updated_at)
SELECT user_id || '-dashboard-welcome', user_id, 'welcome', 0, position,
       'full', '{}', 0, grid_y, 12, 3, datetime('now'), datetime('now')
FROM layout_base
UNION ALL
SELECT user_id || '-dashboard-local-time', user_id, 'local-time', 0, position + 1,
       'compact', '{}', 0, MIN(255, grid_y + 3), 4, 4, datetime('now'), datetime('now')
FROM layout_base
UNION ALL
SELECT user_id || '-dashboard-calendar', user_id, 'calendar-overview', 0, position + 2,
       'compact', '{}', 4, MIN(255, grid_y + 3), 4, 6, datetime('now'), datetime('now')
FROM layout_base
UNION ALL
SELECT user_id || '-dashboard-bookmarks', user_id, 'bookmarks', 0, position + 3,
       'compact', '{}', 8, MIN(255, grid_y + 3), 4, 4, datetime('now'), datetime('now')
FROM layout_base;

CREATE INDEX dashboard_widgets_user_workspace_position_idx
    ON dashboard_widgets (user_id, workspace, position);
CREATE INDEX dashboard_widgets_grid_idx
    ON dashboard_widgets (user_id, workspace, grid_y, grid_x);
CREATE INDEX widget_secrets_user_idx ON widget_secrets (user_id);

PRAGMA foreign_keys = ON;
