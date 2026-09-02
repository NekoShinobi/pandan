ALTER TABLE user_settings
ADD COLUMN lines_default_visibility TEXT NOT NULL DEFAULT 'public'
CHECK(lines_default_visibility IN ('private', 'public'));

CREATE TABLE line_posts (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL CHECK(length(trim(content)) BETWEEN 1 AND 2000),
    visibility TEXT NOT NULL DEFAULT 'public'
        CHECK(visibility IN ('private', 'public')),
    reply_to_post_id TEXT REFERENCES line_posts(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX line_posts_feed_idx
    ON line_posts(created_at DESC, id DESC);
CREATE INDEX line_posts_user_feed_idx
    ON line_posts(user_id, created_at DESC, id DESC);
CREATE INDEX line_posts_reply_idx
    ON line_posts(reply_to_post_id, created_at ASC);

CREATE TABLE line_post_tags (
    post_id TEXT NOT NULL REFERENCES line_posts(id) ON DELETE CASCADE,
    tag TEXT NOT NULL COLLATE NOCASE CHECK(length(trim(tag)) BETWEEN 1 AND 64),
    PRIMARY KEY (post_id, tag)
);

CREATE INDEX line_post_tags_tag_idx ON line_post_tags(tag, post_id);

CREATE TABLE line_post_reactions (
    post_id TEXT NOT NULL REFERENCES line_posts(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    emoji TEXT NOT NULL CHECK(length(emoji) BETWEEN 1 AND 32),
    created_at TEXT NOT NULL,
    PRIMARY KEY (post_id, user_id, emoji)
);

CREATE INDEX line_post_reactions_post_idx
    ON line_post_reactions(post_id, created_at ASC);

CREATE TABLE line_post_attachments (
    id TEXT PRIMARY KEY NOT NULL,
    post_id TEXT NOT NULL REFERENCES line_posts(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL CHECK(length(trim(file_name)) BETWEEN 1 AND 255),
    mime_type TEXT NOT NULL CHECK(length(trim(mime_type)) BETWEEN 1 AND 120),
    byte_size INTEGER NOT NULL CHECK(byte_size BETWEEN 1 AND 10485760),
    file_data BLOB NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX line_post_attachments_post_created_idx
    ON line_post_attachments(post_id, created_at ASC);
