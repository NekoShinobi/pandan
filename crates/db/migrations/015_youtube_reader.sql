CREATE TABLE youtube_channels (
    channel_id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    channel_url TEXT NOT NULL,
    last_fetched_at TEXT,
    refresh_started_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK(length(channel_id) = 24 AND substr(channel_id, 1, 2) = 'UC')
);

CREATE INDEX youtube_channels_refresh_idx
ON youtube_channels(last_fetched_at, refresh_started_at);

CREATE TABLE youtube_videos (
    id TEXT PRIMARY KEY,
    external_id TEXT NOT NULL UNIQUE,
    channel_id TEXT NOT NULL REFERENCES youtube_channels(channel_id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    thumbnail_url TEXT NOT NULL,
    title TEXT NOT NULL,
    published_at TEXT NOT NULL,
    fetched_at TEXT NOT NULL
);

CREATE INDEX youtube_videos_channel_published_idx
ON youtube_videos(channel_id, published_at DESC);

CREATE TABLE youtube_subscriptions (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id TEXT NOT NULL REFERENCES youtube_channels(channel_id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, channel_id)
);

CREATE INDEX youtube_subscriptions_channel_idx
ON youtube_subscriptions(channel_id, user_id);

CREATE TABLE youtube_groups (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE,
    position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 127),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id, name),
    UNIQUE(user_id, position)
);

CREATE TABLE youtube_group_channels (
    group_id TEXT NOT NULL REFERENCES youtube_groups(id) ON DELETE CASCADE,
    channel_id TEXT NOT NULL REFERENCES youtube_channels(channel_id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 127),
    PRIMARY KEY (group_id, channel_id),
    UNIQUE(group_id, position)
);

CREATE TABLE youtube_settings (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    display_mode TEXT NOT NULL DEFAULT 'thumbnails'
        CHECK(display_mode IN ('thumbnails', 'compact')),
    updated_at TEXT NOT NULL
);
