ALTER TABLE rss_subscriptions
ADD COLUMN refresh_generation INTEGER NOT NULL DEFAULT 0;

ALTER TABLE rss_items
ADD COLUMN last_seen_generation INTEGER NOT NULL DEFAULT 0;

CREATE INDEX rss_items_current_snapshot_idx
    ON rss_items(subscription_id, last_seen_generation, published_at DESC);
