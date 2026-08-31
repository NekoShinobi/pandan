ALTER TABLE rss_subscriptions
ADD COLUMN custom_name TEXT
CHECK(custom_name IS NULL OR length(trim(custom_name)) BETWEEN 1 AND 80);
