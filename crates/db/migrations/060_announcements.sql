CREATE TABLE announcements (
    id TEXT PRIMARY KEY NOT NULL,
    author_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 160),
    content TEXT NOT NULL CHECK(length(trim(content)) BETWEEN 1 AND 50000),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX announcements_feed_idx
    ON announcements(created_at DESC, id DESC);

CREATE TABLE announcement_images (
    id TEXT PRIMARY KEY NOT NULL,
    announcement_id TEXT NOT NULL REFERENCES announcements(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL CHECK(length(trim(file_name)) BETWEEN 1 AND 255),
    mime_type TEXT NOT NULL
        CHECK(mime_type IN ('image/jpeg', 'image/png', 'image/webp', 'image/avif')),
    byte_size INTEGER NOT NULL CHECK(byte_size BETWEEN 1 AND 10485760),
    image_data BLOB NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX announcement_images_announcement_created_idx
    ON announcement_images(announcement_id, created_at ASC, id ASC);

CREATE TABLE announcement_reactions (
    announcement_id TEXT NOT NULL REFERENCES announcements(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    emoji TEXT NOT NULL CHECK(length(emoji) BETWEEN 1 AND 32),
    created_at TEXT NOT NULL,
    PRIMARY KEY (announcement_id, user_id, emoji)
);

CREATE INDEX announcement_reactions_announcement_idx
    ON announcement_reactions(announcement_id, created_at ASC);
