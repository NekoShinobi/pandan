-- Private, account-scoped yt-dlp jobs and the administrator-owned instance policy.
CREATE TABLE youtube_download_jobs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source_url TEXT NOT NULL CHECK(length(source_url) BETWEEN 1 AND 2048),
    youtube_video_id TEXT NOT NULL CHECK(length(youtube_video_id) BETWEEN 6 AND 32),
    title TEXT NOT NULL CHECK(length(title) <= 500),
    channel_name TEXT NOT NULL CHECK(length(channel_name) <= 300),
    duration_seconds INTEGER CHECK(duration_seconds IS NULL OR duration_seconds >= 0),
    media_kind TEXT NOT NULL CHECK(media_kind IN ('video', 'audio')),
    output_format TEXT NOT NULL CHECK(output_format IN ('mp4', 'mkv', 'webm', 'm4a', 'mp3', 'opus')),
    max_height INTEGER CHECK(max_height IS NULL OR max_height > 0),
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK(status IN ('queued', 'inspecting', 'downloading', 'postprocessing', 'complete', 'failed', 'cancelled')),
    progress_percent REAL CHECK(progress_percent IS NULL OR progress_percent BETWEEN 0 AND 100),
    downloaded_bytes INTEGER NOT NULL DEFAULT 0 CHECK(downloaded_bytes >= 0),
    total_bytes INTEGER CHECK(total_bytes IS NULL OR total_bytes >= 0),
    speed_bytes_per_second REAL CHECK(speed_bytes_per_second IS NULL OR speed_bytes_per_second >= 0),
    eta_seconds INTEGER CHECK(eta_seconds IS NULL OR eta_seconds >= 0),
    storage_file_name TEXT NOT NULL DEFAULT '',
    display_file_name TEXT NOT NULL DEFAULT '',
    mime_type TEXT NOT NULL DEFAULT '',
    byte_size INTEGER NOT NULL DEFAULT 0 CHECK(byte_size >= 0),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 8),
    error_code TEXT,
    last_error TEXT,
    lease_started_at TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX youtube_download_jobs_user_created_idx
    ON youtube_download_jobs(user_id, created_at DESC);
CREATE INDEX youtube_download_jobs_queue_idx
    ON youtube_download_jobs(status, created_at);
CREATE INDEX youtube_download_jobs_lease_idx
    ON youtube_download_jobs(status, lease_started_at);
CREATE UNIQUE INDEX youtube_download_jobs_active_profile_idx
    ON youtube_download_jobs(user_id, youtube_video_id, media_kind, output_format, COALESCE(max_height, 0))
    WHERE status IN ('queued', 'inspecting', 'downloading', 'postprocessing');

CREATE TABLE youtube_download_settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    member_downloads_enabled INTEGER NOT NULL DEFAULT 1 CHECK(member_downloads_enabled IN (0, 1)),
    storage_budget_bytes INTEGER NOT NULL DEFAULT 21474836480 CHECK(storage_budget_bytes > 0),
    per_user_budget_bytes INTEGER NOT NULL DEFAULT 10737418240 CHECK(per_user_budget_bytes > 0),
    max_output_bytes INTEGER NOT NULL DEFAULT 2147483648 CHECK(max_output_bytes > 0),
    global_concurrency INTEGER NOT NULL DEFAULT 2 CHECK(global_concurrency BETWEEN 1 AND 8),
    per_user_concurrency INTEGER NOT NULL DEFAULT 1 CHECK(per_user_concurrency BETWEEN 1 AND 4),
    max_batch_urls INTEGER NOT NULL DEFAULT 10 CHECK(max_batch_urls BETWEEN 1 AND 50),
    max_queued_per_user INTEGER NOT NULL DEFAULT 50 CHECK(max_queued_per_user BETWEEN 1 AND 200),
    updated_at TEXT NOT NULL
);

INSERT INTO youtube_download_settings (
    id, member_downloads_enabled, storage_budget_bytes, per_user_budget_bytes,
    max_output_bytes, global_concurrency, per_user_concurrency, max_batch_urls,
    max_queued_per_user, updated_at
) VALUES (1, 1, 21474836480, 10737418240, 2147483648, 2, 1, 10, 50, CURRENT_TIMESTAMP);
