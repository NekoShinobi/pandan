CREATE TABLE trading_settings (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    finnhub_api_key_ciphertext TEXT,
    last_refresh_at TEXT,
    last_refresh_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE trading_watchlist (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL COLLATE NOCASE CHECK(length(trim(symbol)) BETWEEN 1 AND 16),
    position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 9),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id, symbol),
    UNIQUE(user_id, position)
);

CREATE INDEX trading_watchlist_user_position_idx
    ON trading_watchlist(user_id, position);

CREATE TABLE trading_quotes (
    user_id TEXT NOT NULL,
    symbol TEXT NOT NULL COLLATE NOCASE,
    name TEXT NOT NULL,
    price TEXT NOT NULL,
    previous_close TEXT,
    day_open TEXT,
    day_high TEXT,
    day_low TEXT,
    change_percent TEXT,
    currency TEXT NOT NULL,
    market_state TEXT,
    source TEXT NOT NULL CHECK(source IN ('yahoo', 'finnhub')),
    quoted_at TEXT NOT NULL,
    refreshed_at TEXT NOT NULL,
    PRIMARY KEY(user_id, symbol),
    FOREIGN KEY(user_id, symbol)
        REFERENCES trading_watchlist(user_id, symbol)
        ON DELETE CASCADE
);

CREATE INDEX trading_quotes_user_refreshed_idx
    ON trading_quotes(user_id, refreshed_at DESC);
