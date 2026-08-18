ALTER TABLE tasks ADD COLUMN archived_at TEXT;

CREATE INDEX tasks_user_archived_completion_created_idx
    ON tasks(user_id, archived_at, completed, created_at);
