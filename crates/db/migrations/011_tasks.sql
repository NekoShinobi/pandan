DROP INDEX IF EXISTS tasks_completion_created_idx;
DROP INDEX IF EXISTS tasks_user_completion_created_idx;

ALTER TABLE tasks RENAME TO tasks_legacy;

CREATE TABLE tasks (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 180),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 4000),
    completed INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0, 1)),
    priority TEXT NOT NULL DEFAULT 'none' CHECK(priority IN ('p1', 'p2', 'p3', 'p4', 'none')),
    due_date TEXT,
    repeat_rule TEXT NOT NULL DEFAULT 'none' CHECK(repeat_rule IN ('none', 'daily', 'weekly', 'monthly', 'yearly', 'custom')),
    repeat_interval INTEGER NOT NULL DEFAULT 1 CHECK(repeat_interval BETWEEN 1 AND 365),
    repeat_unit TEXT NOT NULL DEFAULT 'days' CHECK(repeat_unit IN ('days', 'weeks', 'months', 'years')),
    reschedule_from TEXT NOT NULL DEFAULT 'due_date' CHECK(reschedule_from IN ('due_date', 'completion_date')),
    completed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO tasks (
    id, user_id, title, completed, priority, created_at, updated_at
)
SELECT
    id,
    user_id,
    title,
    completed,
    CASE priority WHEN 'high' THEN 'p1' WHEN 'low' THEN 'p4' ELSE 'none' END,
    created_at,
    updated_at
FROM tasks_legacy
WHERE user_id IS NOT NULL;

DROP TABLE tasks_legacy;

CREATE INDEX tasks_user_completion_created_idx
    ON tasks(user_id, completed, created_at);

CREATE TABLE task_labels (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    label TEXT NOT NULL CHECK(length(trim(label)) BETWEEN 1 AND 40),
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (task_id, label)
);

CREATE TABLE task_subtasks (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 180),
    completed INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0, 1)),
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX task_subtasks_task_position_idx ON task_subtasks(task_id, position);

CREATE TABLE task_attachments (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL CHECK(length(trim(file_name)) BETWEEN 1 AND 255),
    mime_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK(byte_size > 0),
    file_data BLOB NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX task_attachments_task_created_idx ON task_attachments(task_id, created_at);
