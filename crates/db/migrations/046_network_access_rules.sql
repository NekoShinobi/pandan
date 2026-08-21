CREATE TABLE network_access_rules (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL CHECK (action IN ('allow', 'deny')),
    scheme TEXT NOT NULL CHECK (scheme IN ('http', 'https')),
    host TEXT NOT NULL CHECK (length(host) BETWEEN 1 AND 253),
    port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
    integration TEXT NOT NULL CHECK (
        integration IN (
            'all', 'rss', 'calendar', 'contacts', 'podcasts',
            'notifications', 'coding', 'images', 'youtube', 'widgets'
        )
    ),
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(action, scheme, host, port, integration)
);

CREATE INDEX idx_network_access_rules_target
    ON network_access_rules(scheme, host, port, integration, action);
