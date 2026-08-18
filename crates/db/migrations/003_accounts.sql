CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY NOT NULL,
    email         TEXT NOT NULL COLLATE NOCASE UNIQUE,
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_settings (
    user_id          TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    display_name     TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 60),
    location         TEXT NOT NULL DEFAULT 'London' CHECK (length(trim(location)) BETWEEN 1 AND 80),
    timezone         TEXT NOT NULL DEFAULT 'UTC' CHECK (length(trim(timezone)) BETWEEN 1 AND 80),
    temperature_unit TEXT NOT NULL DEFAULT 'celsius' CHECK (temperature_unit IN ('celsius', 'fahrenheit')),
    updated_at       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    token      TEXT PRIMARY KEY NOT NULL,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS sessions_user_idx ON sessions (user_id);
CREATE INDEX IF NOT EXISTS sessions_expiry_idx ON sessions (expires_at);

ALTER TABLE tasks ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS tasks_user_completion_created_idx
    ON tasks (user_id, completed, created_at);
