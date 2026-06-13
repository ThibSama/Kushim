use std::sync::{Mutex, MutexGuard, OnceLock};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Technical symbol prefixes reserved for integration-test fixtures.
///
/// Real provider allowlists (mock, Finnhub, …) never resolve symbols
/// starting with these prefixes, so an interrupted test that leaves an
/// active fixture behind cannot collide with a canonical catalogue
/// entry such as AAPL/MSFT/NVDA.
// All prefixes are exactly 10 characters so that prefix + 10 hex chars
// fits the `assets.symbol`/`assets.ticker` varchar(20) constraint while
// still leaving 16^10 combinations per prefix for uniqueness.
pub const TEST_SYMBOL_PREFIX_CURRENT: &str = "TEST_CURR_";
pub const TEST_SYMBOL_PREFIX_HISTORY: &str = "TEST_HIST_";
pub const TEST_SYMBOL_PREFIX_UNSUPPORTED: &str = "TEST_NONE_";
pub const TEST_TICKER_PREFIX: &str = "TEST_TICK_";

/// Build a fresh unique technical symbol for an integration test fixture.
///
/// The returned value is always 20 characters: a 10-char prefix plus
/// 10 hex characters drawn from a v4 UUID. This fits `varchar(20)`.
pub fn unique_test_symbol(prefix: &str) -> String {
    debug_assert_eq!(prefix.len(), 10, "test symbol prefix must be 10 chars");
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}{}", &suffix[..10])
}

/// Deterministic in-test market-data provider.
///
/// - Returns `Some` for any symbol/ticker starting with
///   [`TEST_SYMBOL_PREFIX_CURRENT`], [`TEST_SYMBOL_PREFIX_HISTORY`] or
///   [`TEST_TICKER_PREFIX`].
/// - Returns `None` otherwise (which covers
///   [`TEST_SYMBOL_PREFIX_UNSUPPORTED`] and any unknown symbol).
///
/// This provider is `#[cfg(test)]` only; it never reaches production
/// binaries and is not used by the canonical catalogue.
pub mod providers {
    use super::*;
    use crate::domain::{ActiveAsset, CurrentQuote, HistoricalQuote};
    use crate::errors::MarketDataError;
    use crate::providers::MarketDataProvider;
    use time::{Date, OffsetDateTime};

    pub const TEST_CURRENT_PRICE_MINOR: i64 = 12_345;

    pub struct DeterministicTestProvider;

    fn supports(key: &str) -> bool {
        key.starts_with(TEST_SYMBOL_PREFIX_CURRENT)
            || key.starts_with(TEST_SYMBOL_PREFIX_HISTORY)
            || key.starts_with(TEST_TICKER_PREFIX)
    }

    fn lookup_key(asset: &ActiveAsset) -> Option<&str> {
        asset.symbol.as_deref().or(asset.ticker.as_deref())
    }

    fn deterministic_history_close(date: Date) -> i64 {
        // Stable per-date jitter on top of a base; varies with date,
        // identical across two calls for the same date.
        let ordinal = date.ordinal() as i64;
        let year = (date.year() % 100) as i64;
        12_000 + ((ordinal * 7 + year * 13) % 500)
    }

    impl MarketDataProvider for DeterministicTestProvider {
        fn name(&self) -> &'static str {
            "test-static"
        }

        async fn get_quote(
            &self,
            asset: &ActiveAsset,
        ) -> Result<Option<CurrentQuote>, MarketDataError> {
            let Some(key) = lookup_key(asset) else {
                return Ok(None);
            };
            if !supports(key) {
                return Ok(None);
            }
            Ok(Some(CurrentQuote {
                price_minor: TEST_CURRENT_PRICE_MINOR,
                currency: "USD".to_string(),
                data_source: "test-static".to_string(),
                as_of: OffsetDateTime::now_utc(),
            }))
        }

        async fn get_historical_quote(
            &self,
            asset: &ActiveAsset,
            date: Date,
        ) -> Result<Option<HistoricalQuote>, MarketDataError> {
            let Some(key) = lookup_key(asset) else {
                return Ok(None);
            };
            if !supports(key) {
                return Ok(None);
            }
            Ok(Some(HistoricalQuote {
                close_minor: deterministic_history_close(date),
                currency: "USD".to_string(),
                data_source: "test-static".to_string(),
                price_date: date,
            }))
        }
    }
}
