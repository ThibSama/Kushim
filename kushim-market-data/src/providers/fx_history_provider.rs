//! Provider abstraction for historical FX rates.
//!
//! This trait is **separate** from `MarketDataProvider` (which models equity
//! quotes) on purpose: an FX provider has a different shape (no symbol /
//! exchange / asset class, the "instrument" is a canonical currency pair,
//! and rates form a cross-rate matrix that must remain triangular-arbitrage
//! consistent).
//!
//! Only the mock provider is registered in this PR. Real provider selection
//! is deferred — see
//! `documentation/architecture/historical-fx-foundation.md`.

use crate::domain::fx_rate::{CanonicalFxRate, CanonicalPair, FxDomainError};
use crate::errors::MarketDataError;
use std::future::Future;
use time::Date;

/// Result of asking a provider for a daily canonical rate.
#[derive(Debug, Clone)]
pub enum ProviderDailyRate {
    /// The provider produced a rate for the requested pair and date.
    Rate(CanonicalFxRate),
    /// The provider has no rate for that date (e.g. weekend / market
    /// holiday). The caller should rely on repository carry-forward for
    /// downstream consumers; it should not insert a fabricated rate.
    NoQuoteForDate,
}

pub trait FxHistoryProvider: Send + Sync {
    /// Stable provider identifier persisted in `fx_rate_history_cache.provider`.
    fn name(&self) -> &'static str;

    /// Dataset version persisted in `fx_rate_history_cache.dataset_version`.
    /// Identifies the frozen snapshot or upstream feed version. Must be
    /// stable so that re-runs over the same range are idempotent.
    fn dataset_version(&self) -> &'static str;

    /// True when the provider can produce rates for this canonical pair.
    fn supports_pair(&self, pair: &CanonicalPair) -> bool;

    /// Request the canonical rate for `pair` on `rate_date`.
    fn get_canonical_rate(
        &self,
        pair: &CanonicalPair,
        rate_date: Date,
    ) -> impl Future<Output = Result<ProviderDailyRate, MarketDataError>> + Send;
}

impl From<FxDomainError> for MarketDataError {
    fn from(value: FxDomainError) -> Self {
        MarketDataError::Provider(value.to_string())
    }
}
