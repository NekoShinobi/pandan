CREATE TABLE rss_subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    url TEXT NOT NULL CHECK(length(trim(url)) BETWEEN 1 AND 2048),
    base_url TEXT NOT NULL CHECK(length(trim(base_url)) BETWEEN 1 AND 512),
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 180),
    category TEXT NOT NULL DEFAULT 'Uncategorized' CHECK(length(trim(category)) BETWEEN 1 AND 40),
    auto_delete_days INTEGER CHECK(auto_delete_days IS NULL OR auto_delete_days BETWEEN 1 AND 3650),
    auto_delete_mode TEXT NOT NULL DEFAULT 'read' CHECK(auto_delete_mode IN ('read', 'all')),
    last_fetched_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id, url)
);

CREATE INDEX rss_subscriptions_user_category_idx
    ON rss_subscriptions(user_id, category, created_at);

CREATE TABLE rss_items (
    id TEXT PRIMARY KEY NOT NULL,
    subscription_id TEXT NOT NULL REFERENCES rss_subscriptions(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,
    url TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 500),
    summary TEXT NOT NULL DEFAULT '',
    published_at TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    read_at TEXT,
    UNIQUE(subscription_id, external_id)
);

CREATE INDEX rss_items_subscription_published_idx
    ON rss_items(subscription_id, published_at DESC);

CREATE INDEX rss_items_read_published_idx
    ON rss_items(read_at, published_at DESC);
