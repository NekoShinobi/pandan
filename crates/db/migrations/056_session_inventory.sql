CREATE TABLE sessions_next (
    token        TEXT PRIMARY KEY NOT NULL,
    id           TEXT NOT NULL UNIQUE,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_agent   TEXT NOT NULL CHECK (length(user_agent) <= 512),
    ip_address   TEXT NOT NULL CHECK (length(ip_address) <= 64),
    expires_at   TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    last_seen_at TEXT NOT NULL
);

INSERT INTO sessions_next (
    token,
    id,
    user_id,
    user_agent,
    ip_address,
    expires_at,
    created_at,
    last_seen_at
)
SELECT
    token,
    lower(hex(randomblob(16))),
    user_id,
    '',
    '',
    expires_at,
    created_at,
    created_at
FROM sessions;

DROP TABLE sessions;
ALTER TABLE sessions_next RENAME TO sessions;

CREATE INDEX sessions_user_idx ON sessions (user_id);
CREATE INDEX sessions_expiry_idx ON sessions (expires_at);
