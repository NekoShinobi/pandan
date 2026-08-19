-- Podcasts: an administrator-curated instance catalogue with a member request queue.
--
-- Episode audio is NOT stored here. Approved episodes are downloaded once to the media
-- root on disk (PANDAN_MEDIA_DIR, default `data/podcasts`) and streamed from there so the
-- server can answer HTTP Range requests without materialising a whole episode in memory.
-- `podcast_downloads` is the index over those files and doubles as the download work queue.

CREATE TABLE podcasts (
    id TEXT PRIMARY KEY NOT NULL,
    feed_url TEXT NOT NULL CHECK(length(trim(feed_url)) BETWEEN 1 AND 2048),
    normalized_url TEXT NOT NULL UNIQUE CHECK(length(trim(normalized_url)) BETWEEN 1 AND 2048),
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 300),
    description TEXT NOT NULL DEFAULT '',
    author TEXT NOT NULL DEFAULT '',
    site_url TEXT NOT NULL DEFAULT '',
    language TEXT NOT NULL DEFAULT '',
    artwork_url TEXT NOT NULL DEFAULT '',
    artwork_content_type TEXT NOT NULL DEFAULT '',
    artwork_data BLOB,
    artwork_fetched_at TEXT,
    auto_download_count INTEGER NOT NULL DEFAULT 3
        CHECK(auto_download_count BETWEEN 0 AND 25),
    max_retained_episodes INTEGER NOT NULL DEFAULT 50
        CHECK(max_retained_episodes BETWEEN 1 AND 1000),
    added_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    last_fetched_at TEXT,
    refresh_started_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX podcasts_refresh_idx
    ON podcasts(last_fetched_at, refresh_started_at);

CREATE INDEX podcasts_title_idx
    ON podcasts(title COLLATE NOCASE);

-- Member requests. Decided rows are retained as history so a rejected feed keeps its
-- reason visible to the requester instead of being silently re-requested.
CREATE TABLE podcast_requests (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    feed_url TEXT NOT NULL CHECK(length(trim(feed_url)) BETWEEN 1 AND 2048),
    normalized_url TEXT NOT NULL CHECK(length(trim(normalized_url)) BETWEEN 1 AND 2048),
    resolved_title TEXT NOT NULL DEFAULT '',
    resolved_author TEXT NOT NULL DEFAULT '',
    resolved_artwork_url TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT '' CHECK(length(note) <= 500),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'approved', 'rejected', 'withdrawn')),
    decision_note TEXT NOT NULL DEFAULT '' CHECK(length(decision_note) <= 500),
    decided_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    decided_at TEXT,
    podcast_id TEXT REFERENCES podcasts(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- One open request per user per feed. Decided rows are exempt so history accumulates.
CREATE UNIQUE INDEX podcast_requests_open_idx
    ON podcast_requests(user_id, normalized_url) WHERE status = 'pending';

CREATE INDEX podcast_requests_review_idx
    ON podcast_requests(status, created_at DESC);

CREATE INDEX podcast_requests_user_idx
    ON podcast_requests(user_id, created_at DESC);

CREATE TABLE podcast_episodes (
    id TEXT PRIMARY KEY NOT NULL,
    podcast_id TEXT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    guid TEXT NOT NULL CHECK(length(trim(guid)) BETWEEN 1 AND 2048),
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 500),
    description TEXT NOT NULL DEFAULT '',
    episode_url TEXT NOT NULL DEFAULT '',
    enclosure_url TEXT NOT NULL CHECK(length(trim(enclosure_url)) BETWEEN 1 AND 2048),
    enclosure_type TEXT NOT NULL DEFAULT '',
    enclosure_bytes INTEGER CHECK(enclosure_bytes IS NULL OR enclosure_bytes >= 0),
    duration_seconds INTEGER CHECK(duration_seconds IS NULL OR duration_seconds >= 0),
    published_at TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    UNIQUE(podcast_id, guid)
);

CREATE INDEX podcast_episodes_podcast_published_idx
    ON podcast_episodes(podcast_id, published_at DESC);

-- One row per cached file, and the download work queue. `requested_by` deliberately uses
-- SET NULL: cached audio is a shared instance resource and must outlive the account that
-- first asked for it.
CREATE TABLE podcast_downloads (
    episode_id TEXT PRIMARY KEY NOT NULL
        REFERENCES podcast_episodes(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK(status IN ('queued', 'downloading', 'ready', 'failed')),
    requested_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    file_name TEXT NOT NULL DEFAULT '',
    content_type TEXT NOT NULL DEFAULT '',
    byte_size INTEGER NOT NULL DEFAULT 0 CHECK(byte_size >= 0),
    downloaded_bytes INTEGER NOT NULL DEFAULT 0 CHECK(downloaded_bytes >= 0),
    pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0, 1)),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0),
    last_error TEXT NOT NULL DEFAULT '',
    lease_started_at TEXT,
    last_accessed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX podcast_downloads_queue_idx
    ON podcast_downloads(status, created_at);

CREATE INDEX podcast_downloads_evict_idx
    ON podcast_downloads(status, pinned, last_accessed_at);

CREATE TABLE podcast_subscriptions (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    podcast_id TEXT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, podcast_id)
);

CREATE INDEX podcast_subscriptions_podcast_idx
    ON podcast_subscriptions(podcast_id, user_id);

CREATE TABLE podcast_episode_progress (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    episode_id TEXT NOT NULL REFERENCES podcast_episodes(id) ON DELETE CASCADE,
    position_seconds INTEGER NOT NULL DEFAULT 0 CHECK(position_seconds >= 0),
    completed_at TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, episode_id)
);

CREATE INDEX podcast_episode_progress_user_updated_idx
    ON podcast_episode_progress(user_id, updated_at DESC);

-- Play order, not a saved collection. UNIQUE(user_id, position) is checked per statement,
-- so reordering parks rows in the 256..511 band before writing final 0..255 positions.
-- The two ranges cannot overlap, which is what makes a full reversal safe.
CREATE TABLE podcast_queue (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    episode_id TEXT NOT NULL REFERENCES podcast_episodes(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 511),
    added_at TEXT NOT NULL,
    PRIMARY KEY (user_id, episode_id)
);

CREATE UNIQUE INDEX podcast_queue_order_idx
    ON podcast_queue(user_id, position);

CREATE TABLE podcast_saved_episodes (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    episode_id TEXT NOT NULL REFERENCES podcast_episodes(id) ON DELETE CASCADE,
    saved_at TEXT NOT NULL,
    PRIMARY KEY (user_id, episode_id)
);

CREATE INDEX podcast_saved_episodes_user_saved_idx
    ON podcast_saved_episodes(user_id, saved_at DESC);

-- Singleton administrator policy, seeded exactly as authentication_settings is.
CREATE TABLE podcast_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    requests_enabled INTEGER NOT NULL DEFAULT 1 CHECK (requests_enabled IN (0, 1)),
    member_downloads_enabled INTEGER NOT NULL DEFAULT 1
        CHECK (member_downloads_enabled IN (0, 1)),
    max_pending_requests_per_user INTEGER NOT NULL DEFAULT 5
        CHECK (max_pending_requests_per_user BETWEEN 0 AND 100),
    storage_budget_bytes INTEGER NOT NULL DEFAULT 21474836480
        CHECK (storage_budget_bytes BETWEEN 0 AND 1099511627776),
    max_episode_bytes INTEGER NOT NULL DEFAULT 524288000
        CHECK (max_episode_bytes BETWEEN 1048576 AND 5368709120),
    default_auto_download_count INTEGER NOT NULL DEFAULT 3
        CHECK (default_auto_download_count BETWEEN 0 AND 25),
    updated_at TEXT NOT NULL
);

INSERT INTO podcast_settings (
    id,
    requests_enabled,
    member_downloads_enabled,
    max_pending_requests_per_user,
    storage_budget_bytes,
    max_episode_bytes,
    default_auto_download_count,
    updated_at
) VALUES (1, 1, 1, 5, 21474836480, 524288000, 3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

ALTER TABLE user_settings
ADD COLUMN podcast_playback_rate REAL NOT NULL DEFAULT 1.0
CHECK(podcast_playback_rate BETWEEN 0.5 AND 3.0);
