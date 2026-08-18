CREATE TABLE rss_read_later (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES rss_items(id) ON DELETE CASCADE,
    saved_at TEXT NOT NULL,
    PRIMARY KEY (user_id, item_id)
);

CREATE INDEX rss_read_later_user_saved_idx
    ON rss_read_later(user_id, saved_at DESC);

CREATE TABLE youtube_watch_later (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    video_id TEXT NOT NULL REFERENCES youtube_videos(id) ON DELETE CASCADE,
    saved_at TEXT NOT NULL,
    PRIMARY KEY (user_id, video_id)
);

CREATE INDEX youtube_watch_later_user_saved_idx
    ON youtube_watch_later(user_id, saved_at DESC);
