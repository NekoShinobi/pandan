use actix_web::{HttpRequest, HttpResponse, http::header, web};
use db::entities::{TradingQuote, TradingQuoteDraft};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{Duration, Instant, Interval, MissedTickBehavior, interval_at};

use crate::{ApiError, AppState, authenticated_account};

const MAX_TRADING_SYMBOLS: usize = 10;
const MAX_FINNHUB_KEY_CHARS: usize = 256;
const FINNHUB_REFRESH_SECONDS: u64 = 20;

#[derive(Debug, Clone, Serialize)]
pub struct TradingResponse {
    watchlist: Vec<TradingWatchlistResponse>,
    provider: &'static str,
    has_finnhub_api_key: bool,
    secret_storage_enabled: bool,
    last_refresh_at: Option<String>,
    last_refresh_error: Option<String>,
    stream_interval_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct TradingWatchlistResponse {
    id: String,
    symbol: String,
    position: i64,
    quote: Option<TradingQuoteResponse>,
}

#[derive(Debug, Clone, Serialize)]
struct TradingQuoteResponse {
    symbol: String,
    name: String,
    price: String,
    previous_close: Option<String>,
    day_open: Option<String>,
    day_high: Option<String>,
    day_low: Option<String>,
    change_percent: Option<String>,
    currency: String,
    market_state: Option<String>,
    source: String,
    quoted_at: String,
    refreshed_at: String,
}

#[derive(Debug, Deserialize)]
struct TradingSymbolInput {
    symbol: String,
}

#[derive(Debug, Deserialize)]
struct FinnhubKeyInput {
    api_key: String,
}

#[derive(Debug, Serialize)]
struct TradingStreamEvent {
    kind: &'static str,
    snapshot: TradingResponse,
}

#[derive(Debug, Clone, Copy)]
enum RefreshMode {
    Automatic,
    FinnhubOnly,
}

struct TradingStreamState {
    state: web::Data<AppState>,
    user_id: String,
    interval: Interval,
    finished: bool,
}

pub(crate) fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/trading", web::get().to(get_trading))
        .route("/trading/refresh", web::post().to(refresh_trading))
        .route("/trading/events", web::get().to(trading_events))
        .route("/trading/symbols", web::post().to(create_symbol))
        .route(
            "/trading/symbols/{item_id}",
            web::delete().to(delete_symbol),
        )
        .route("/trading/finnhub-key", web::put().to(save_finnhub_key))
        .route("/trading/finnhub-key", web::delete().to(delete_finnhub_key));
}

async fn get_trading(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<TradingResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    trading_snapshot(&state, &account.id).await.map(web::Json)
}

async fn refresh_trading(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<TradingResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    refresh_account(&state, &account.id, RefreshMode::Automatic)
        .await
        .map(web::Json)
}

async fn create_symbol(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<TradingSymbolInput>,
) -> Result<web::Json<TradingResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let symbol = validate_symbol(&payload.symbol)?;
    if db::trading_queries::list_trading_watchlist(&state.pool, &account.id)
        .await?
        .len()
        >= MAX_TRADING_SYMBOLS
    {
        return Err(ApiError::BadRequest(
            "a Trading watchlist can contain at most 10 symbols",
        ));
    }
    db::trading_queries::create_trading_watchlist_item(&state.pool, &account.id, &symbol)
        .await
        .map_err(map_watchlist_insert_error)?;
    trading_snapshot(&state, &account.id).await.map(web::Json)
}

async fn delete_symbol(
    state: web::Data<AppState>,
    request: HttpRequest,
    item_id: web::Path<String>,
) -> Result<web::Json<TradingResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !db::trading_queries::delete_trading_watchlist_item(&state.pool, &account.id, &item_id)
        .await?
    {
        return Err(ApiError::NotFound("Trading symbol not found"));
    }
    trading_snapshot(&state, &account.id).await.map(web::Json)
}

async fn save_finnhub_key(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<FinnhubKeyInput>,
) -> Result<web::Json<TradingResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !state.widget_integrations.secrets_enabled() {
        return Err(ApiError::Conflict(
            "encrypted credential storage must be configured first",
        ));
    }
    let api_key = payload.api_key.trim();
    if api_key.is_empty()
        || api_key.chars().count() > MAX_FINNHUB_KEY_CHARS
        || api_key.chars().any(char::is_whitespace)
        || api_key.chars().any(char::is_control)
    {
        return Err(ApiError::BadRequest(
            "Finnhub API key must contain 1 to 256 non-whitespace characters",
        ));
    }
    let ciphertext = state
        .widget_integrations
        .encrypt_secret(api_key)
        .map_err(ApiError::Integration)?;
    state
        .widget_integrations
        .fetch_finnhub_stock_quotes(&["AAPL".to_owned()], &ciphertext)
        .await
        .map_err(|_| {
            ApiError::BadRequest("Finnhub rejected this API key or quote access is unavailable")
        })?;
    db::trading_queries::set_trading_finnhub_key(&state.pool, &account.id, Some(&ciphertext))
        .await?;
    trading_snapshot(&state, &account.id).await.map(web::Json)
}

async fn delete_finnhub_key(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<TradingResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::trading_queries::set_trading_finnhub_key(&state.pool, &account.id, None).await?;
    trading_snapshot(&state, &account.id).await.map(web::Json)
}

async fn trading_events(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let has_key = db::trading_queries::get_trading_settings(&state.pool, &account.id)
        .await?
        .and_then(|settings| settings.finnhub_api_key_ciphertext)
        .is_some();
    if !has_key {
        return Err(ApiError::Conflict(
            "configure a Finnhub API key before opening the live feed",
        ));
    }
    let mut interval = interval_at(Instant::now(), Duration::from_secs(FINNHUB_REFRESH_SECONDS));
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let events = stream::unfold(
        TradingStreamState {
            state,
            user_id: account.id,
            interval,
            finished: false,
        },
        next_trading_event,
    );
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/event-stream"))
        .insert_header((header::CACHE_CONTROL, "private, no-store, no-transform"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(events))
}

async fn next_trading_event(
    mut live: TradingStreamState,
) -> Option<(Result<web::Bytes, actix_web::Error>, TradingStreamState)> {
    if live.finished {
        return None;
    }
    live.interval.tick().await;
    let snapshot = match refresh_account(&live.state, &live.user_id, RefreshMode::FinnhubOnly).await
    {
        Ok(snapshot) => snapshot,
        Err(ApiError::Conflict(_)) => {
            live.finished = true;
            return Some((
                Ok(web::Bytes::from_static(
                    b"event: stream-error\ndata: {\"message\":\"Finnhub live feed stopped\"}\n\n",
                )),
                live,
            ));
        }
        Err(error) => {
            tracing::error!(user_id = %live.user_id, %error, "Trading live refresh failed");
            return Some((
                Ok(web::Bytes::from_static(
                    b"event: stream-error\ndata: {\"message\":\"Live refresh interrupted\"}\n\n",
                )),
                live,
            ));
        }
    };
    let payload = serde_json::to_string(&TradingStreamEvent {
        kind: "snapshot",
        snapshot,
    })
    .unwrap_or_else(|_| "{\"kind\":\"stream-error\"}".to_owned());
    Some((
        Ok(web::Bytes::from(format!(
            "event: snapshot\ndata: {payload}\n\n"
        ))),
        live,
    ))
}

async fn refresh_account(
    state: &AppState,
    user_id: &str,
    mode: RefreshMode,
) -> Result<TradingResponse, ApiError> {
    let watchlist = db::trading_queries::list_trading_watchlist(&state.pool, user_id).await?;
    if watchlist.is_empty() {
        db::trading_queries::set_trading_refresh_status(&state.pool, user_id, None, None).await?;
        return trading_snapshot(state, user_id).await;
    }
    let settings = db::trading_queries::get_trading_settings(&state.pool, user_id).await?;
    let encrypted_key = settings
        .as_ref()
        .and_then(|settings| settings.finnhub_api_key_ciphertext.as_deref());
    if matches!(mode, RefreshMode::FinnhubOnly) && encrypted_key.is_none() {
        return Err(ApiError::Conflict(
            "Finnhub API key is no longer configured",
        ));
    }
    let symbols = watchlist
        .iter()
        .map(|item| item.symbol.clone())
        .collect::<Vec<_>>();
    let provider = if encrypted_key.is_some() {
        "finnhub"
    } else {
        "yahoo"
    };
    let result = if let Some(ciphertext) = encrypted_key {
        state
            .widget_integrations
            .fetch_finnhub_stock_quotes(&symbols, ciphertext)
            .await
    } else {
        state
            .widget_integrations
            .fetch_yahoo_stock_quotes(&symbols)
            .await
    };
    let quotes = match result {
        Ok(quotes) => quotes,
        Err(error) => {
            tracing::warn!(%user_id, provider, %error, "Trading provider refresh failed");
            let message = if provider == "finnhub" {
                "Finnhub refresh failed; cached prices are still shown."
            } else {
                "Yahoo Finance refresh failed; cached prices are still shown."
            };
            db::trading_queries::set_trading_refresh_status(
                &state.pool,
                user_id,
                None,
                Some(message),
            )
            .await?;
            return trading_snapshot(state, user_id).await;
        }
    };
    let existing = db::trading_queries::list_trading_quotes(&state.pool, user_id)
        .await?
        .into_iter()
        .map(|quote| (quote.symbol.clone(), quote))
        .collect::<HashMap<_, _>>();
    let refreshed_at = chrono::Utc::now().to_rfc3339();
    let drafts = quotes
        .into_iter()
        .map(|quote| {
            let cached = existing.get(&quote.symbol);
            TradingQuoteDraft {
                symbol: quote.symbol.clone(),
                name: quote
                    .name
                    .or_else(|| cached.map(|quote| quote.name.clone()))
                    .unwrap_or_else(|| quote.symbol.clone()),
                price: quote.price,
                previous_close: quote
                    .previous_close
                    .or_else(|| cached.and_then(|quote| quote.previous_close.clone())),
                day_open: quote
                    .day_open
                    .or_else(|| cached.and_then(|quote| quote.day_open.clone())),
                day_high: quote
                    .day_high
                    .or_else(|| cached.and_then(|quote| quote.day_high.clone())),
                day_low: quote
                    .day_low
                    .or_else(|| cached.and_then(|quote| quote.day_low.clone())),
                change_percent: quote
                    .change_percent
                    .or_else(|| cached.and_then(|quote| quote.change_percent.clone())),
                currency: if quote.currency.is_empty() {
                    cached
                        .map(|quote| quote.currency.clone())
                        .unwrap_or_default()
                } else {
                    quote.currency
                },
                market_state: quote.market_state,
                source: provider.to_owned(),
                quoted_at: quote.quoted_at,
            }
        })
        .collect::<Vec<_>>();
    db::trading_queries::upsert_trading_quotes(&state.pool, user_id, &drafts, &refreshed_at)
        .await?;
    let partial_error = (drafts.len() < symbols.len()).then_some(if provider == "finnhub" {
        "Some Finnhub symbols could not be refreshed; cached prices remain."
    } else {
        "Some Yahoo Finance symbols could not be refreshed; cached prices remain."
    });
    db::trading_queries::set_trading_refresh_status(
        &state.pool,
        user_id,
        Some(&refreshed_at),
        partial_error,
    )
    .await?;
    trading_snapshot(state, user_id).await
}

async fn trading_snapshot(state: &AppState, user_id: &str) -> Result<TradingResponse, ApiError> {
    let (settings, watchlist, quotes) = tokio::try_join!(
        db::trading_queries::get_trading_settings(&state.pool, user_id),
        db::trading_queries::list_trading_watchlist(&state.pool, user_id),
        db::trading_queries::list_trading_quotes(&state.pool, user_id)
    )?;
    let has_finnhub_api_key = settings
        .as_ref()
        .and_then(|settings| settings.finnhub_api_key_ciphertext.as_ref())
        .is_some();
    let mut quotes = quotes
        .into_iter()
        .map(|quote| (quote.symbol.clone(), quote))
        .collect::<HashMap<_, _>>();
    Ok(TradingResponse {
        watchlist: watchlist
            .into_iter()
            .map(|item| {
                let quote = quotes.remove(&item.symbol).map(TradingQuoteResponse::from);
                TradingWatchlistResponse {
                    id: item.id,
                    symbol: item.symbol,
                    position: item.position,
                    quote,
                }
            })
            .collect(),
        provider: if has_finnhub_api_key {
            "finnhub"
        } else {
            "yahoo"
        },
        has_finnhub_api_key,
        secret_storage_enabled: state.widget_integrations.secrets_enabled(),
        last_refresh_at: settings
            .as_ref()
            .and_then(|settings| settings.last_refresh_at.clone()),
        last_refresh_error: settings.and_then(|settings| settings.last_refresh_error),
        stream_interval_seconds: has_finnhub_api_key.then_some(FINNHUB_REFRESH_SECONDS),
    })
}

impl From<TradingQuote> for TradingQuoteResponse {
    fn from(quote: TradingQuote) -> Self {
        Self {
            symbol: quote.symbol,
            name: quote.name,
            price: quote.price,
            previous_close: quote.previous_close,
            day_open: quote.day_open,
            day_high: quote.day_high,
            day_low: quote.day_low,
            change_percent: quote.change_percent,
            currency: quote.currency,
            market_state: quote.market_state,
            source: quote.source,
            quoted_at: quote.quoted_at,
            refreshed_at: quote.refreshed_at,
        }
    }
}

fn validate_symbol(value: &str) -> Result<String, ApiError> {
    let symbol = value.trim().to_ascii_uppercase();
    if symbol.is_empty()
        || symbol.chars().count() > 16
        || !symbol.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '^' | '=')
        })
    {
        return Err(ApiError::BadRequest(
            "stock symbol must contain 1 to 16 letters, numbers, periods, carets, equals signs, or hyphens",
        ));
    }
    Ok(symbol)
}

fn map_watchlist_insert_error(error: sqlx::Error) -> ApiError {
    let message = error
        .as_database_error()
        .map(|database_error| database_error.message())
        .unwrap_or_default();
    if message.contains("trading_watchlist.user_id, trading_watchlist.symbol") {
        ApiError::Conflict("that symbol is already in the watchlist")
    } else if message.contains("trading_watchlist.position") {
        ApiError::BadRequest("a Trading watchlist can contain at most 10 symbols")
    } else {
        ApiError::Database(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trading_symbols_are_normalized_without_accepting_paths_or_whitespace() {
        assert_eq!(
            validate_symbol(" brk-b ").expect("symbol validates"),
            "BRK-B"
        );
        assert_eq!(validate_symbol("^gspc").expect("index validates"), "^GSPC");
        assert!(validate_symbol("../AAPL").is_err());
        assert!(validate_symbol("AAPL MSFT").is_err());
        assert!(validate_symbol("").is_err());
    }
}
