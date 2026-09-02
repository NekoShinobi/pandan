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
            'jellyfin', 'ai'
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

CREATE TABLE ollama_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    base_url TEXT NOT NULL CHECK (length(base_url) BETWEEN 8 AND 2000),
    model TEXT NOT NULL CHECK (length(model) BETWEEN 1 AND 120),
    prompt TEXT NOT NULL CHECK (length(prompt) BETWEEN 1 AND 2000),
    tag_count INTEGER NOT NULL DEFAULT 5 CHECK (tag_count BETWEEN 1 AND 8),
    configured_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    last_verified_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO ollama_settings (
    id, enabled, base_url, model, prompt, tag_count,
    configured_by_user_id, last_verified_at, created_at, updated_at
) VALUES (
    1,
    0,
    'http://localhost:11434',
    'gemma3:4b',
    'Analyze this wallpaper image and return concise, reusable tags. Describe the visible subject matter, setting, mood, palette, and visual style. Use lowercase words or short phrases.',
    5,
    NULL,
    NULL,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
);
