ALTER TABLE tasks ADD COLUMN completion_count INTEGER NOT NULL DEFAULT 0
    CHECK(completion_count >= 0);

ALTER TABLE tasks ADD COLUMN last_completed_on TEXT;
