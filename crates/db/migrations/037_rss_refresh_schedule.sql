ALTER TABLE rss_subscriptions ADD COLUMN last_attempted_at TEXT;

UPDATE rss_subscriptions SET last_attempted_at = last_fetched_at;
