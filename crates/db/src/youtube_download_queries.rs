use crate::entities::{
    NewYoutubeDownloadJob, YoutubeDownloadFileRef, YoutubeDownloadJob, YoutubeDownloadSettings,
};
use sqlx::SqlitePool;

const INSERT_JOB: &str = "INSERT INTO youtube_download_jobs (
    id, user_id, source_url, youtube_video_id, title, channel_name, duration_seconds,
    media_kind, output_format, max_height, status, created_at, updated_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'queued', ?, ?)
RETURNING id, user_id, source_url, youtube_video_id, title, channel_name,
    duration_seconds, media_kind, output_format, max_height, status, progress_percent,
    downloaded_bytes, total_bytes, speed_bytes_per_second, eta_seconds, storage_file_name,
    display_file_name, mime_type, byte_size, attempts, error_code, last_error, lease_started_at,
    created_at, started_at, completed_at, updated_at";

const SELECT_JOB: &str = "SELECT id, user_id, source_url, youtube_video_id, title, channel_name,
    duration_seconds, media_kind, output_format, max_height, status, progress_percent,
    downloaded_bytes, total_bytes, speed_bytes_per_second, eta_seconds, storage_file_name,
    display_file_name, mime_type, byte_size, attempts, error_code, last_error, lease_started_at,
    created_at, started_at, completed_at, updated_at
FROM youtube_download_jobs WHERE id = ? AND user_id = ?";

const LIST_JOBS: &str = "SELECT id, user_id, source_url, youtube_video_id, title, channel_name,
    duration_seconds, media_kind, output_format, max_height, status, progress_percent,
    downloaded_bytes, total_bytes, speed_bytes_per_second, eta_seconds, storage_file_name,
    display_file_name, mime_type, byte_size, attempts, error_code, last_error, lease_started_at,
    created_at, started_at, completed_at, updated_at
FROM youtube_download_jobs
WHERE user_id = ? AND (? IS NULL OR status = ?) AND (? IS NULL OR created_at < ?)
ORDER BY created_at DESC LIMIT ?";

const LIST_ACTIVE_JOBS: &str =
    "SELECT id, user_id, source_url, youtube_video_id, title, channel_name,
    duration_seconds, media_kind, output_format, max_height, status, progress_percent,
    downloaded_bytes, total_bytes, speed_bytes_per_second, eta_seconds, storage_file_name,
    display_file_name, mime_type, byte_size, attempts, error_code, last_error, lease_started_at,
    created_at, started_at, completed_at, updated_at
FROM youtube_download_jobs WHERE user_id = ?
AND status IN ('queued', 'inspecting', 'downloading', 'postprocessing')
ORDER BY created_at ASC";

const CLAIM_JOB: &str = "UPDATE youtube_download_jobs
SET status = 'downloading', attempts = attempts + 1, lease_started_at = ?,
    started_at = COALESCE(started_at, ?), updated_at = ?, progress_percent = NULL,
    downloaded_bytes = 0, total_bytes = NULL, speed_bytes_per_second = NULL,
    eta_seconds = NULL, error_code = NULL, last_error = NULL
WHERE id = (
    SELECT queued.id FROM youtube_download_jobs AS queued
    WHERE queued.status = 'queued' AND queued.attempts < 3
      AND (
          SELECT COUNT(*) FROM youtube_download_jobs AS active
          WHERE active.user_id = queued.user_id
            AND active.status IN ('inspecting', 'downloading', 'postprocessing')
      ) < ?
    ORDER BY (
        SELECT COUNT(*) FROM youtube_download_jobs AS active
        WHERE active.user_id = queued.user_id
          AND active.status IN ('inspecting', 'downloading', 'postprocessing')
    ) ASC, queued.created_at ASC LIMIT 1
) AND status = 'queued'
RETURNING id, user_id, source_url, youtube_video_id, title, channel_name,
    duration_seconds, media_kind, output_format, max_height, status, progress_percent,
    downloaded_bytes, total_bytes, speed_bytes_per_second, eta_seconds, storage_file_name,
    display_file_name, mime_type, byte_size, attempts, error_code, last_error, lease_started_at,
    created_at, started_at, completed_at, updated_at";

pub async fn get_settings(pool: &SqlitePool) -> Result<YoutubeDownloadSettings, sqlx::Error> {
    sqlx::query_as::<_, YoutubeDownloadSettings>(
        "SELECT member_downloads_enabled, storage_budget_bytes, per_user_budget_bytes,  max_output_bytes, global_concurrency, per_user_concurrency, max_batch_urls,  max_queued_per_user, updated_at FROM youtube_download_settings WHERE id = 1",
    )
    .fetch_one(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn update_settings(
    pool: &SqlitePool,
    member_downloads_enabled: bool,
    storage_budget_bytes: i64,
    per_user_budget_bytes: i64,
    max_output_bytes: i64,
    global_concurrency: i64,
    per_user_concurrency: i64,
    max_batch_urls: i64,
    max_queued_per_user: i64,
) -> Result<YoutubeDownloadSettings, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE youtube_download_settings SET member_downloads_enabled = ?,  storage_budget_bytes = ?, per_user_budget_bytes = ?, max_output_bytes = ?,  global_concurrency = ?, per_user_concurrency = ?, max_batch_urls = ?,  max_queued_per_user = ?, updated_at = ? WHERE id = 1",
    )
    .bind(member_downloads_enabled)
    .bind(storage_budget_bytes)
    .bind(per_user_budget_bytes)
    .bind(max_output_bytes)
    .bind(global_concurrency)
    .bind(per_user_concurrency)
    .bind(max_batch_urls)
    .bind(max_queued_per_user)
    .bind(now)
    .execute(pool)
    .await?;
    get_settings(pool).await
}

pub async fn create_job(
    pool: &SqlitePool,
    draft: &NewYoutubeDownloadJob,
) -> Result<YoutubeDownloadJob, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query_as::<_, YoutubeDownloadJob>(INSERT_JOB)
        .bind(&draft.id)
        .bind(&draft.user_id)
        .bind(&draft.source_url)
        .bind(&draft.youtube_video_id)
        .bind(&draft.title)
        .bind(&draft.channel_name)
        .bind(draft.duration_seconds)
        .bind(&draft.media_kind)
        .bind(&draft.output_format)
        .bind(draft.max_height)
        .bind(&now)
        .bind(&now)
        .fetch_one(pool)
        .await
}

pub async fn get_job(
    pool: &SqlitePool,
    user_id: &str,
    job_id: &str,
) -> Result<Option<YoutubeDownloadJob>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeDownloadJob>(SELECT_JOB)
        .bind(job_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

pub async fn list_jobs(
    pool: &SqlitePool,
    user_id: &str,
    status: Option<&str>,
    before: Option<&str>,
    limit: i64,
) -> Result<Vec<YoutubeDownloadJob>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeDownloadJob>(LIST_JOBS)
        .bind(user_id)
        .bind(status)
        .bind(status)
        .bind(before)
        .bind(before)
        .bind(limit)
        .fetch_all(pool)
        .await
}

pub async fn list_active_jobs(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<YoutubeDownloadJob>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeDownloadJob>(LIST_ACTIVE_JOBS)
        .bind(user_id)
        .fetch_all(pool)
        .await
}

pub async fn count_unsettled_jobs(pool: &SqlitePool, user_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM youtube_download_jobs WHERE user_id = ?  AND status IN ('queued', 'inspecting', 'downloading', 'postprocessing')",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

pub async fn claim_next_job(
    pool: &SqlitePool,
    per_user_concurrency: i64,
) -> Result<Option<YoutubeDownloadJob>, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query_as::<_, YoutubeDownloadJob>(CLAIM_JOB)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(per_user_concurrency)
        .fetch_optional(pool)
        .await
}

#[allow(clippy::too_many_arguments)]
pub async fn update_progress(
    pool: &SqlitePool,
    job_id: &str,
    progress_percent: Option<f64>,
    downloaded_bytes: i64,
    total_bytes: Option<i64>,
    speed_bytes_per_second: Option<f64>,
    eta_seconds: Option<i64>,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE youtube_download_jobs SET progress_percent = ?, downloaded_bytes = ?,  total_bytes = ?, speed_bytes_per_second = ?, eta_seconds = ?, updated_at = ?  WHERE id = ? AND status = 'downloading'",
    )
    .bind(progress_percent)
    .bind(downloaded_bytes)
    .bind(total_bytes)
    .bind(speed_bytes_per_second)
    .bind(eta_seconds)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(job_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

pub async fn mark_postprocessing(pool: &SqlitePool, job_id: &str) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE youtube_download_jobs SET status = 'postprocessing', progress_percent = 100,  speed_bytes_per_second = NULL, eta_seconds = NULL, updated_at = ?  WHERE id = ? AND status IN ('downloading', 'postprocessing')",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(job_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

pub async fn mark_complete(
    pool: &SqlitePool,
    job_id: &str,
    storage_file_name: &str,
    display_file_name: &str,
    mime_type: &str,
    byte_size: i64,
) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query(
        "UPDATE youtube_download_jobs SET status = 'complete', progress_percent = 100,  downloaded_bytes = ?, total_bytes = ?, speed_bytes_per_second = NULL, eta_seconds = NULL,  storage_file_name = ?, display_file_name = ?, mime_type = ?, byte_size = ?,  error_code = NULL, last_error = NULL, lease_started_at = NULL, completed_at = ?, updated_at = ?  WHERE id = ? AND status = 'postprocessing'",
    )
    .bind(byte_size)
    .bind(byte_size)
    .bind(storage_file_name)
    .bind(display_file_name)
    .bind(mime_type)
    .bind(byte_size)
    .bind(&now)
    .bind(&now)
    .bind(job_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

pub async fn mark_failed(
    pool: &SqlitePool,
    job_id: &str,
    error_code: &str,
    message: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE youtube_download_jobs SET status = 'failed', error_code = ?, last_error = ?,  lease_started_at = NULL, speed_bytes_per_second = NULL, eta_seconds = NULL, updated_at = ?  WHERE id = ? AND status IN ('queued', 'inspecting', 'downloading', 'postprocessing')",
    )
    .bind(error_code)
    .bind(message)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(job_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

pub async fn mark_cancelled(
    pool: &SqlitePool,
    user_id: &str,
    job_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE youtube_download_jobs SET status = 'cancelled', error_code = NULL,  last_error = NULL, lease_started_at = NULL, speed_bytes_per_second = NULL, eta_seconds = NULL,  updated_at = ? WHERE id = ? AND user_id = ?  AND status IN ('queued', 'inspecting', 'downloading', 'postprocessing')",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(job_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}

pub async fn delete_job(
    pool: &SqlitePool,
    user_id: &str,
    job_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM youtube_download_jobs WHERE id = ? AND user_id = ?")
            .bind(job_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

pub async fn user_storage_used(pool: &SqlitePool, user_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(byte_size), 0) FROM youtube_download_jobs  WHERE user_id = ? AND status = 'complete'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

pub async fn instance_storage_used(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(byte_size), 0) FROM youtube_download_jobs WHERE status = 'complete'",
    )
    .fetch_one(pool)
    .await
}

pub async fn list_user_job_ids(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM youtube_download_jobs WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await
}

pub async fn reset_interrupted(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let failed = sqlx::query(
        "UPDATE youtube_download_jobs SET status = 'failed', error_code = 'interrupted',  last_error = 'Download was interrupted too many times.', lease_started_at = NULL, updated_at = ?  WHERE status IN ('inspecting', 'downloading', 'postprocessing') AND attempts >= 3",
    )
    .bind(&now)
    .execute(pool)
    .await?
    .rows_affected();
    let queued = sqlx::query(
        "UPDATE youtube_download_jobs SET status = 'queued', progress_percent = NULL, downloaded_bytes = 0,  total_bytes = NULL, speed_bytes_per_second = NULL, eta_seconds = NULL, lease_started_at = NULL,  error_code = NULL, last_error = NULL, updated_at = ?  WHERE status IN ('inspecting', 'downloading', 'postprocessing') AND attempts < 3",
    )
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(failed + queued)
}

pub async fn list_complete_files(
    pool: &SqlitePool,
) -> Result<Vec<YoutubeDownloadFileRef>, sqlx::Error> {
    sqlx::query_as::<_, YoutubeDownloadFileRef>(
        "SELECT id, user_id, storage_file_name FROM youtube_download_jobs  WHERE status = 'complete' AND storage_file_name <> ''",
    )
    .fetch_all(pool)
    .await
}

pub async fn invalidate_missing_file(pool: &SqlitePool, job_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE youtube_download_jobs SET status = 'failed', error_code = 'file_missing',  last_error = 'The completed file is no longer available.', storage_file_name = '',  display_file_name = '', mime_type = '', byte_size = 0, updated_at = ?  WHERE id = ? AND status = 'complete'",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}
