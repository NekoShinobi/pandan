CREATE TABLE network_access_rules_new (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL CHECK (action IN ('allow', 'deny')),
    scheme TEXT NOT NULL CHECK (scheme IN ('http', 'https')),
    host TEXT NOT NULL CHECK (length(host) BETWEEN 1 AND 253),
    port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
    integration TEXT NOT NULL CHECK (
        integration IN (
            'all', 'rss', 'calendar', 'contacts', 'podcasts',
            'notifications', 'coding', 'images', 'youtube', 'widgets',
            'jellyfin'
        )
    ),
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(action, scheme, host, port, integration)
);

INSERT INTO network_access_rules_new (
    id, action, scheme, host, port, integration,
    created_by_user_id, created_at, updated_at
)
SELECT
    id, action, scheme, host, port, integration,
    created_by_user_id, created_at, updated_at
FROM network_access_rules;

DROP TABLE network_access_rules;
ALTER TABLE network_access_rules_new RENAME TO network_access_rules;

CREATE INDEX idx_network_access_rules_target
    ON network_access_rules(scheme, host, port, integration, action);

CREATE TABLE jellyfin_server_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    base_url TEXT NOT NULL CHECK (length(base_url) BETWEEN 8 AND 2000),
    server_id TEXT NOT NULL CHECK (length(server_id) BETWEEN 1 AND 128),
    server_name TEXT NOT NULL CHECK (length(server_name) BETWEEN 1 AND 120),
    server_version TEXT NOT NULL CHECK (length(server_version) BETWEEN 1 AND 64),
    configured_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE jellyfin_user_connections (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    server_setting_id INTEGER NOT NULL DEFAULT 1
        REFERENCES jellyfin_server_settings(id) ON DELETE CASCADE
        CHECK (server_setting_id = 1),
    jellyfin_user_id TEXT NOT NULL CHECK (length(jellyfin_user_id) BETWEEN 1 AND 128),
    jellyfin_username TEXT NOT NULL CHECK (length(jellyfin_username) BETWEEN 1 AND 120),
    token_ciphertext TEXT NOT NULL CHECK (length(token_ciphertext) BETWEEN 1 AND 8192),
    device_id TEXT NOT NULL CHECK (length(device_id) BETWEEN 1 AND 128),
    last_verified_at TEXT,
    last_error TEXT CHECK (last_error IS NULL OR length(last_error) <= 500),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_jellyfin_user_connections_server
    ON jellyfin_user_connections(server_setting_id, jellyfin_user_id);
