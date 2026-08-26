CREATE TABLE bookmarks (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 120),
    url TEXT NOT NULL CHECK (length(url) BETWEEN 1 AND 2048),
    favicon_content_type TEXT CHECK (
        favicon_content_type IS NULL OR favicon_content_type IN (
            'image/avif', 'image/jpeg', 'image/png', 'image/webp',
            'image/x-icon', 'image/vnd.microsoft.icon'
        )
    ),
    favicon_data BLOB CHECK (
        favicon_data IS NULL OR length(favicon_data) BETWEEN 1 AND 262144
    ),
    favicon_fetched_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id, url),
    CHECK (
        (favicon_content_type IS NULL AND favicon_data IS NULL AND favicon_fetched_at IS NULL)
        OR
        (favicon_content_type IS NOT NULL AND favicon_data IS NOT NULL AND favicon_fetched_at IS NOT NULL)
    )
);

CREATE INDEX idx_bookmarks_user_title
    ON bookmarks(user_id, title COLLATE NOCASE, created_at, id);
