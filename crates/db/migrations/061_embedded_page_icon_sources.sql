CREATE TABLE embedded_pages_next (
    id                 TEXT PRIMARY KEY NOT NULL,
    scope              TEXT NOT NULL CHECK(scope IN ('global', 'user')),
    owner_user_id      TEXT REFERENCES users(id) ON DELETE CASCADE,
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    title              TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 80),
    description        TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 280),
    url                TEXT NOT NULL CHECK(length(url) BETWEEN 1 AND 2000),
    icon_kind          TEXT NOT NULL DEFAULT 'favicon'
                       CHECK(icon_kind IN ('favicon', 'lucide', 'custom')),
    icon_value         TEXT CHECK(icon_value IS NULL OR length(icon_value) BETWEEN 1 AND 2000),
    allow_scripts      INTEGER NOT NULL DEFAULT 0 CHECK(allow_scripts IN (0, 1)),
    allow_same_origin  INTEGER NOT NULL DEFAULT 0 CHECK(allow_same_origin IN (0, 1)),
    iframe_height      INTEGER NOT NULL DEFAULT 720 CHECK(iframe_height BETWEEN 320 AND 2400),
    position           INTEGER NOT NULL CHECK(position >= 0),
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL,
    CHECK(
        (scope = 'global' AND owner_user_id IS NULL) OR
        (scope = 'user' AND owner_user_id IS NOT NULL)
    ),
    CHECK(
        (icon_kind = 'favicon' AND icon_value IS NULL) OR
        (icon_kind IN ('lucide', 'custom') AND icon_value IS NOT NULL)
    )
);

INSERT INTO embedded_pages_next (
    id, scope, owner_user_id, created_by_user_id, title, description, url,
    icon_kind, icon_value, allow_scripts, allow_same_origin, iframe_height,
    position, created_at, updated_at
)
SELECT
    id, scope, owner_user_id, created_by_user_id, title, description, url,
    CASE WHEN icon_url IS NULL THEN 'favicon' ELSE 'custom' END,
    icon_url, allow_scripts, allow_same_origin, iframe_height,
    position, created_at, updated_at
FROM embedded_pages;

DROP TABLE embedded_pages;
ALTER TABLE embedded_pages_next RENAME TO embedded_pages;

CREATE INDEX embedded_pages_global_position_idx
ON embedded_pages(position, created_at, id)
WHERE scope = 'global';

CREATE INDEX embedded_pages_user_position_idx
ON embedded_pages(owner_user_id, position, created_at, id)
WHERE scope = 'user';
