ALTER TABLE rss_subscriptions
ADD COLUMN current_entry_limit INTEGER NOT NULL DEFAULT 25
CHECK(current_entry_limit BETWEEN 1 AND 200);
