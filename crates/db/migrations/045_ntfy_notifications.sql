CREATE TABLE ntfy_connections (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    base_url TEXT NOT NULL,
    token_ciphertext TEXT,
    last_synced_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE ntfy_topics (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES ntfy_connections(user_id) ON DELETE CASCADE,
    topic TEXT NOT NULL,
    label TEXT NOT NULL,
    last_message_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id, topic)
);

CREATE INDEX idx_ntfy_topics_user ON ntfy_topics(user_id, created_at);

CREATE TABLE ntfy_notifications (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    topic_id TEXT NOT NULL REFERENCES ntfy_topics(id) ON DELETE CASCADE,
    remote_id TEXT NOT NULL,
    occurred_at INTEGER NOT NULL,
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 3,
    tags_json TEXT NOT NULL DEFAULT '[]',
    click_url TEXT,
    actions_json TEXT NOT NULL DEFAULT '[]',
    seen_at TEXT,
    archived_at TEXT,
    received_at TEXT NOT NULL,
    UNIQUE(topic_id, remote_id)
);

CREATE INDEX idx_ntfy_notifications_inbox
    ON ntfy_notifications(user_id, archived_at, occurred_at DESC);
CREATE INDEX idx_ntfy_notifications_topic
    ON ntfy_notifications(topic_id, archived_at, occurred_at DESC);
