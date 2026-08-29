CREATE TABLE logging_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    file_enabled INTEGER NOT NULL DEFAULT 1 CHECK (file_enabled IN (0, 1)),
    log_level TEXT NOT NULL DEFAULT 'info' CHECK (log_level IN ('error', 'warn', 'info', 'debug', 'trace')),
    retention_days INTEGER NOT NULL DEFAULT 14 CHECK (retention_days BETWEEN 1 AND 365),
    max_file_size_mb INTEGER NOT NULL DEFAULT 10 CHECK (max_file_size_mb BETWEEN 1 AND 256),
    max_files INTEGER NOT NULL DEFAULT 20 CHECK (max_files BETWEEN 1 AND 100),
    updated_at TEXT NOT NULL
);

INSERT INTO logging_settings (
    id,
    file_enabled,
    log_level,
    retention_days,
    max_file_size_mb,
    max_files,
    updated_at
)
VALUES (1, 1, 'info', 14, 10, 20, CURRENT_TIMESTAMP);
