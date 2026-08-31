use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct TradingSettings {
    pub user_id: String,
    pub finnhub_api_key_ciphertext: Option<String>,
    pub last_refresh_at: Option<String>,
    pub last_refresh_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct TradingWatchlistItem {
    pub id: String,
    pub symbol: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct TradingQuote {
    pub user_id: String,
    pub symbol: String,
    pub name: String,
    pub price: String,
    pub previous_close: Option<String>,
    pub day_open: Option<String>,
    pub day_high: Option<String>,
    pub day_low: Option<String>,
    pub change_percent: Option<String>,
    pub currency: String,
    pub market_state: Option<String>,
    pub source: String,
    pub quoted_at: String,
    pub refreshed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradingQuoteDraft {
    pub symbol: String,
    pub name: String,
    pub price: String,
    pub previous_close: Option<String>,
    pub day_open: Option<String>,
    pub day_high: Option<String>,
    pub day_low: Option<String>,
    pub change_percent: Option<String>,
    pub currency: String,
    pub market_state: Option<String>,
    pub source: String,
    pub quoted_at: String,
}
