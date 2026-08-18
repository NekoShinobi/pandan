CREATE TABLE user_avatars (
    user_id     TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    mime_type   TEXT NOT NULL CHECK (mime_type IN ('image/jpeg', 'image/png', 'image/webp', 'image/avif')),
    image_data  BLOB NOT NULL CHECK (length(image_data) BETWEEN 1 AND 10485760),
    updated_at  TEXT NOT NULL
);
