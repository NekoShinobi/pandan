CREATE TABLE task_completions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    task_title TEXT NOT NULL CHECK(length(trim(task_title)) BETWEEN 1 AND 180),
    priority TEXT NOT NULL CHECK(priority IN ('p1', 'p2', 'p3', 'p4', 'none')),
    was_recurring INTEGER NOT NULL CHECK(was_recurring IN (0, 1)),
    completed_on TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX task_completions_user_created_idx
    ON task_completions(user_id, created_at DESC);

INSERT INTO task_completions (
    id,
    user_id,
    task_id,
    task_title,
    priority,
    was_recurring,
    completed_on,
    created_at
)
SELECT
    lower(hex(randomblob(16))),
    user_id,
    id,
    title,
    priority,
    repeat_rule != 'none',
    last_completed_on,
    COALESCE(completed_at, updated_at)
FROM tasks
WHERE completion_count > 0
  AND last_completed_on IS NOT NULL;
