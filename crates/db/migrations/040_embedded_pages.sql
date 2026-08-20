CREATE TABLE embedded_pages (
    id                 TEXT PRIMARY KEY NOT NULL,
    scope              TEXT NOT NULL CHECK(scope IN ('global', 'user')),
    owner_user_id      TEXT REFERENCES users(id) ON DELETE CASCADE,
    created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    title              TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 80),
    description        TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 280),
    url                TEXT NOT NULL CHECK(length(url) BETWEEN 1 AND 2000),
    position           INTEGER NOT NULL CHECK(position >= 0),
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL,
    CHECK(
        (scope = 'global' AND owner_user_id IS NULL) OR
        (scope = 'user' AND owner_user_id IS NOT NULL)
    )
);

CREATE INDEX embedded_pages_global_position_idx
ON embedded_pages(position, created_at, id)
WHERE scope = 'global';

CREATE INDEX embedded_pages_user_position_idx
ON embedded_pages(owner_user_id, position, created_at, id)
WHERE scope = 'user';
