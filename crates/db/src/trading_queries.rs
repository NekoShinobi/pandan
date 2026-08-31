use crate::entities::{TradingQuote, TradingQuoteDraft, TradingSettings, TradingWatchlistItem};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Loads one account's private Trading provider settings.
pub async fn get_trading_settings(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Option<TradingSettings>, sqlx::Error> {
    sqlx::query_as::<_, TradingSettings>(
        "SELECT user_id, finnhub_api_key_ciphertext, last_refresh_at, last_refresh_error, \
         created_at, updated_at FROM trading_settings WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Replaces or clears one account's encrypted Finnhub API key.
pub async fn set_trading_finnhub_key(
    pool: &SqlitePool,
    user_id: &str,
    ciphertext: Option<&str>,
) -> Result<TradingSettings, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO trading_settings \
         (user_id, finnhub_api_key_ciphertext, created_at, updated_at) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT(user_id) DO UPDATE SET \
         finnhub_api_key_ciphertext = excluded.finnhub_api_key_ciphertext, \
         updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(ciphertext)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_trading_settings(pool, user_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Records a safe refresh result while retaining the last successful refresh timestamp.
pub async fn set_trading_refresh_status(
    pool: &SqlitePool,
    user_id: &str,
    refreshed_at: Option<&str>,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO trading_settings \
         (user_id, last_refresh_at, last_refresh_error, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(user_id) DO UPDATE SET \
         last_refresh_at = COALESCE(excluded.last_refresh_at, trading_settings.last_refresh_at), \
         last_refresh_error = excluded.last_refresh_error, \
         updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(refreshed_at)
    .bind(error)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Lists one account's watchlist in its persisted display order.
pub async fn list_trading_watchlist(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<TradingWatchlistItem>, sqlx::Error> {
    sqlx::query_as::<_, TradingWatchlistItem>(
        "SELECT id, symbol, position, created_at, updated_at \
         FROM trading_watchlist WHERE user_id = ? ORDER BY position ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Adds one normalized symbol to an account's watchlist.
pub async fn create_trading_watchlist_item(
    pool: &SqlitePool,
    user_id: &str,
    symbol: &str,
) -> Result<TradingWatchlistItem, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO trading_watchlist \
         (id, user_id, symbol, position, created_at, updated_at) \
         SELECT ?, ?, ?, COALESCE(MAX(position) + 1, 0), ?, ? \
         FROM trading_watchlist WHERE user_id = ?",
    )
    .bind(&id)
    .bind(user_id)
    .bind(symbol)
    .bind(&now)
    .bind(&now)
    .bind(user_id)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, TradingWatchlistItem>(
        "SELECT id, symbol, position, created_at, updated_at \
         FROM trading_watchlist WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Deletes one owned symbol and compacts the remaining watchlist positions.
pub async fn delete_trading_watchlist_item(
    pool: &SqlitePool,
    user_id: &str,
    item_id: &str,
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let position = sqlx::query_scalar::<_, i64>(
        "SELECT position FROM trading_watchlist WHERE id = ? AND user_id = ?",
    )
    .bind(item_id)
    .bind(user_id)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(position) = position else {
        transaction.rollback().await?;
        return Ok(false);
    };
    sqlx::query("DELETE FROM trading_watchlist WHERE id = ? AND user_id = ?")
        .bind(item_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "UPDATE trading_watchlist SET position = position - 1, updated_at = ? \
         WHERE user_id = ? AND position > ?",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(user_id)
    .bind(position)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(true)
}

/// Lists the last successful quote snapshot for one account.
pub async fn list_trading_quotes(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<TradingQuote>, sqlx::Error> {
    sqlx::query_as::<_, TradingQuote>(
        "SELECT user_id, symbol, name, price, previous_close, day_open, day_high, day_low, \
         change_percent, currency, market_state, source, quoted_at, refreshed_at \
         FROM trading_quotes WHERE user_id = ? ORDER BY symbol COLLATE NOCASE ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Stores successful provider quotes without deleting cached rows omitted by a partial refresh.
pub async fn upsert_trading_quotes(
    pool: &SqlitePool,
    user_id: &str,
    quotes: &[TradingQuoteDraft],
    refreshed_at: &str,
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    for quote in quotes {
        sqlx::query(
            "INSERT INTO trading_quotes \
             (user_id, symbol, name, price, previous_close, day_open, day_high, day_low, \
              change_percent, currency, market_state, source, quoted_at, refreshed_at) \
             SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ? \
             WHERE EXISTS(SELECT 1 FROM trading_watchlist WHERE user_id = ? AND symbol = ?) \
             ON CONFLICT(user_id, symbol) DO UPDATE SET \
             name = excluded.name, price = excluded.price, \
             previous_close = excluded.previous_close, day_open = excluded.day_open, \
             day_high = excluded.day_high, day_low = excluded.day_low, \
             change_percent = excluded.change_percent, currency = excluded.currency, \
             market_state = excluded.market_state, source = excluded.source, \
             quoted_at = excluded.quoted_at, refreshed_at = excluded.refreshed_at",
        )
        .bind(user_id)
        .bind(&quote.symbol)
        .bind(&quote.name)
        .bind(&quote.price)
        .bind(&quote.previous_close)
        .bind(&quote.day_open)
        .bind(&quote.day_high)
        .bind(&quote.day_low)
        .bind(&quote.change_percent)
        .bind(&quote.currency)
        .bind(&quote.market_state)
        .bind(&quote.source)
        .bind(&quote.quoted_at)
        .bind(refreshed_at)
        .bind(user_id)
        .bind(&quote.symbol)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}
