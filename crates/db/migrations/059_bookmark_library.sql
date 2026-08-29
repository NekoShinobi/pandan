CREATE TABLE bookmark_library_categories (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL CHECK (scope IN ('global', 'personal')),
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (
        (scope = 'global' AND user_id IS NULL)
        OR
        (scope = 'personal' AND user_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_bookmark_library_categories_global_name
    ON bookmark_library_categories(name COLLATE NOCASE)
    WHERE scope = 'global';

CREATE UNIQUE INDEX idx_bookmark_library_categories_personal_name
    ON bookmark_library_categories(user_id, name COLLATE NOCASE)
    WHERE scope = 'personal';

CREATE INDEX idx_bookmark_library_categories_visible
    ON bookmark_library_categories(scope, user_id, name COLLATE NOCASE, created_at, id);

CREATE TABLE bookmark_library_items (
    id TEXT PRIMARY KEY,
    category_id TEXT NOT NULL REFERENCES bookmark_library_categories(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 120),
    url TEXT NOT NULL CHECK (length(url) BETWEEN 1 AND 2048),
    icon_kind TEXT NOT NULL CHECK (icon_kind IN ('favicon', 'lucide', 'custom')),
    icon_value TEXT CHECK (icon_value IS NULL OR length(icon_value) BETWEEN 1 AND 2048),
    icon_content_type TEXT CHECK (
        icon_content_type IS NULL OR icon_content_type IN (
            'image/avif', 'image/jpeg', 'image/png', 'image/webp',
            'image/x-icon', 'image/vnd.microsoft.icon'
        )
    ),
    icon_data BLOB CHECK (
        icon_data IS NULL OR length(icon_data) BETWEEN 1 AND 262144
    ),
    icon_fetched_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(category_id, url),
    CHECK (
        (icon_kind = 'favicon' AND icon_value IS NULL)
        OR
        (icon_kind IN ('lucide', 'custom') AND icon_value IS NOT NULL)
    ),
    CHECK (
        (icon_content_type IS NULL AND icon_data IS NULL AND icon_fetched_at IS NULL)
        OR
        (icon_kind IN ('favicon', 'custom')
            AND icon_content_type IS NOT NULL
            AND icon_data IS NOT NULL
            AND icon_fetched_at IS NOT NULL)
    )
);

CREATE INDEX idx_bookmark_library_items_category_title
    ON bookmark_library_items(category_id, title COLLATE NOCASE, created_at, id);
