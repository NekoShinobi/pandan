-- Walls: a moderated wallpaper collection shared across the instance.
--
-- Any account submits an image; an administrator approves or rejects it. Approved walls
-- form the collection every account can apply to its own wallpaper slots, and that an
-- administrator can promote to the global login screen.
CREATE TABLE walls (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 120),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 500),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'approved', 'rejected')),
    decision_note TEXT NOT NULL DEFAULT '' CHECK(length(decision_note) <= 500),
    decided_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    decided_at TEXT,
    mime_type TEXT NOT NULL
        CHECK(mime_type IN ('image/jpeg', 'image/png', 'image/webp', 'image/avif')),
    byte_size INTEGER NOT NULL CHECK(byte_size BETWEEN 1 AND 31457280),
    width INTEGER NOT NULL CHECK(width > 0),
    height INTEGER NOT NULL CHECK(height > 0),
    image_data BLOB NOT NULL,
    thumbnail_mime TEXT NOT NULL CHECK(thumbnail_mime = 'image/jpeg'),
    thumbnail_data BLOB NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX walls_review_idx ON walls(status, created_at DESC, id DESC);

CREATE INDEX walls_submitter_idx ON walls(user_id, created_at DESC, id DESC);

CREATE TABLE wall_tags (
    wall_id TEXT NOT NULL REFERENCES walls(id) ON DELETE CASCADE,
    tag TEXT NOT NULL COLLATE NOCASE CHECK(length(trim(tag)) BETWEEN 1 AND 32),
    PRIMARY KEY (wall_id, tag)
);

CREATE INDEX wall_tags_tag_idx ON wall_tags(tag, wall_id);

-- Points one wallpaper slot at a wall instead of an uploaded blob. A slot resolves
-- selection first, then the uploaded image in user_wallpapers, then the packaged default.
-- Deleting a wall cascades the selection away so affected slots fall back on their own.
CREATE TABLE user_wallpaper_selections (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    slot TEXT NOT NULL CHECK(slot IN ('dashboard', 'welcome', 'loading', 'login')),
    wall_id TEXT NOT NULL REFERENCES walls(id) ON DELETE CASCADE,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, slot)
);

CREATE INDEX user_wallpaper_selections_wall_idx
    ON user_wallpaper_selections(wall_id);
