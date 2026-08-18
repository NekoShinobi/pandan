CREATE TABLE calendar_subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    url TEXT NOT NULL CHECK(length(trim(url)) BETWEEN 1 AND 2048),
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
    color TEXT NOT NULL DEFAULT 'teal' CHECK(color IN ('teal', 'amber', 'rose', 'blue', 'slate')),
    last_fetched_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id, url)
);

CREATE INDEX calendar_subscriptions_user_name_idx
    ON calendar_subscriptions(user_id, name COLLATE NOCASE);

CREATE TABLE calendar_events (
    id TEXT PRIMARY KEY NOT NULL,
    subscription_id TEXT NOT NULL REFERENCES calendar_subscriptions(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 500),
    description TEXT NOT NULL DEFAULT '',
    location TEXT NOT NULL DEFAULT '',
    url TEXT NOT NULL DEFAULT '',
    start_at TEXT NOT NULL,
    end_at TEXT,
    all_day INTEGER NOT NULL DEFAULT 0 CHECK(all_day IN (0, 1)),
    fetched_at TEXT NOT NULL,
    UNIQUE(subscription_id, external_id, start_at)
);

CREATE INDEX calendar_events_subscription_start_idx
    ON calendar_events(subscription_id, start_at);

CREATE TABLE payment_subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    service TEXT NOT NULL CHECK(length(trim(service)) BETWEEN 1 AND 120),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    frequency TEXT NOT NULL CHECK(length(trim(frequency)) BETWEEN 1 AND 40),
    first_paid_on TEXT NOT NULL CHECK(length(first_paid_on) = 10),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX payment_subscriptions_user_service_idx
    ON payment_subscriptions(user_id, service COLLATE NOCASE);
