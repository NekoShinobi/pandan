CREATE TABLE IF NOT EXISTS user_backgrounds (
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace  INTEGER NOT NULL CHECK (workspace BETWEEN 0 AND 2),
    mime_type  TEXT NOT NULL CHECK (mime_type IN ('image/jpeg', 'image/png', 'image/webp', 'image/avif')),
    image_data BLOB NOT NULL CHECK (length(image_data) BETWEEN 1 AND 8388608),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, workspace)
);
