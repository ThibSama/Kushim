//! Domain types for the historical FX rate cache.
//!
//! These types implement the provider-agnostic contract defined in
//! `documentation/architecture/historical-fx-foundation.md` and applied by
//! migration 004 on table `fx_rate_history_cache`.
//!
//! Currency codes are uppercase, exactly three ASCII letters. A canonical
//! pair has its base currency strictly less than its quote currency
//! (lexicographic UTF-8 / ASCII order), so the pair is unordered: a single
//! row models both directions. The inverse rate is mechanically derived from
//! the canonical rate via a STORED GENERATED column on the database side,
//! and re-derived here when domain values are constructed from a canonical
//! rate (no two independent persisted directions).
//!
//! Rates are `rust_decimal::Decimal` — never `f32` or `f64`.

use rust_decimal::Decimal;
use std::fmt;
use std::str::FromStr;
use time::{Date, OffsetDateTime};

/// Reason a lookup could not return a rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxUnavailableReason {
    /// No row found for this pair/provider on or before the requested date.
    RateMissing,
    /// A row exists but its `rate_date` is older than the carry-forward
    /// tolerance (> `max_age_days`).
    RateStale,
    /// The requested provider is not configured / not registered.
    ProviderNotConfigured,
    /// The mock provider does not support one of the requested currencies.
    UnsupportedMockCurrency,
}

impl FxUnavailableReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RateMissing => "rate_missing",
            Self::RateStale => "rate_stale",
            Self::ProviderNotConfigured => "provider_not_configured",
            Self::UnsupportedMockCurrency => "unsupported_mock_currency",
        }
    }
}

/// ISO-style three-letter uppercase currency code.
///
/// Constructors normalize to uppercase and validate the `^[A-Z]{3}$` shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Currency([u8; 3]);

impl Currency {
    /// Parse a three-letter currency code, uppercasing ASCII input.
    pub fn parse(input: &str) -> Result<Self, FxDomainError> {
        let trimmed = input.trim();
        if trimmed.len() != 3 {
            return Err(FxDomainError::InvalidCurrencyCode(input.to_string()));
        }
        let mut bytes = [0u8; 3];
        for (i, ch) in trimmed.chars().enumerate() {
            if !ch.is_ascii_alphabetic() {
                return Err(FxDomainError::InvalidCurrencyCode(input.to_string()));
            }
            bytes[i] = ch.to_ascii_uppercase() as u8;
        }
        Ok(Self(bytes))
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: bytes are always ASCII A-Z by construction.
        std::str::from_utf8(&self.0).expect("currency bytes are ASCII")
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Currency {
    type Err = FxDomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Canonical unordered currency pair: `base < quote` lexicographically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CanonicalPair {
    base: Currency,
    quote: Currency,
}

impl CanonicalPair {
    /// Build a canonical pair from two currencies in any order. Returns
    /// `None` when both currencies are equal (identity is handled as
    /// `rate = 1` and is never persisted).
    pub fn new(a: Currency, b: Currency) -> Option<Self> {
        match a.cmp(&b) {
            std::cmp::Ordering::Less => Some(Self { base: a, quote: b }),
            std::cmp::Ordering::Greater => Some(Self { base: b, quote: a }),
            std::cmp::Ordering::Equal => None,
        }
    }

    pub fn base(&self) -> Currency {
        self.base
    }

    pub fn quote(&self) -> Currency {
        self.quote
    }
}

impl fmt::Display for CanonicalPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// Direction of a requested conversion relative to the canonical pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairDirection {
    /// Requested `source → target` matches the canonical pair direction
    /// (`source = base`, `target = quote`). The canonical rate applies
    /// directly.
    Direct,
    /// Requested `source → target` is the inverse of the canonical pair
    /// (`source = quote`, `target = base`). The inverse rate applies.
    Inverse,
}

/// A provider-supplied canonical rate for a pair and date.
///
/// `inverse_rate` is always mechanically derived from `canonical_rate`.
/// Constructors guarantee the two directions cannot diverge.
#[derive(Debug, Clone)]
pub struct CanonicalFxRate {
    pub pair: CanonicalPair,
    pub rate_date: Date,
    pub canonical_rate: Decimal,
    pub inverse_rate: Decimal,
    pub provider: String,
    pub provider_as_of: Option<OffsetDateTime>,
    pub dataset_version: String,
}

impl CanonicalFxRate {
    /// Build a canonical rate from positive inputs. The inverse rate is
    /// derived from `canonical_rate` (rounded to 12 fractional digits to
    /// match the persisted STORED GENERATED column).
    pub fn from_canonical(
        pair: CanonicalPair,
        rate_date: Date,
        canonical_rate: Decimal,
        provider: impl Into<String>,
        provider_as_of: Option<OffsetDateTime>,
        dataset_version: impl Into<String>,
    ) -> Result<Self, FxDomainError> {
        if canonical_rate <= Decimal::ZERO {
            return Err(FxDomainError::NonPositiveRate);
        }
        let inverse_rate = derive_inverse(canonical_rate)?;
        Ok(Self {
            pair,
            rate_date,
            canonical_rate,
            inverse_rate,
            provider: provider.into(),
            provider_as_of,
            dataset_version: dataset_version.into(),
        })
    }
}

/// Derive `1 / canonical_rate` rounded to 12 fractional digits. Matches the
/// PostgreSQL `ROUND((1::numeric / canonical_rate)::numeric, 12)` expression.
pub fn derive_inverse(canonical_rate: Decimal) -> Result<Decimal, FxDomainError> {
    if canonical_rate <= Decimal::ZERO {
        return Err(FxDomainError::NonPositiveRate);
    }
    let inverse = Decimal::ONE / canonical_rate;
    Ok(inverse.round_dp(12))
}

/// Result of a successful lookup against the FX cache.
#[derive(Debug, Clone)]
pub struct FxLookupHit {
    pub source: Currency,
    pub target: Currency,
    pub requested_date: Date,
    pub rate_date: Date,
    pub rate: Decimal,
    pub direction: PairDirection,
    pub provider: String,
    pub provider_as_of: Option<OffsetDateTime>,
    pub record_updated_at: OffsetDateTime,
    pub dataset_version: String,
    /// `requested_date - rate_date`, clamped to `>= 0`. Always `0` for an
    /// identity conversion or an exact-date hit.
    pub age_days: i64,
}

impl FxLookupHit {
    pub fn is_inverse_direction(&self) -> bool {
        matches!(self.direction, PairDirection::Inverse)
    }
}

/// Outcome of an FX lookup. The future portfolio worker uses this to
/// distinguish "rate available", "rate missing" and "rate stale" and to
/// trigger the appropriate fallback per the historical performance contract.
#[derive(Debug, Clone)]
pub enum FxLookup {
    Available(FxLookupHit),
    Unavailable {
        source: Currency,
        target: Currency,
        requested_date: Date,
        reason: FxUnavailableReason,
        /// When `reason = RateStale`, the age of the most recent row.
        candidate_age_days: Option<i64>,
    },
}

impl FxLookup {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available(_))
    }

    pub fn unavailable_reason(&self) -> Option<FxUnavailableReason> {
        match self {
            Self::Available(_) => None,
            Self::Unavailable { reason, .. } => Some(*reason),
        }
    }
}

/// Synthesize the identity rate (source == target). The contract requires
/// the identity to be returned without any database access and without
/// any persisted row.
pub fn identity_lookup(currency: Currency, requested_date: Date) -> FxLookupHit {
    FxLookupHit {
        source: currency,
        target: currency,
        requested_date,
        rate_date: requested_date,
        rate: Decimal::ONE,
        direction: PairDirection::Direct,
        provider: "identity".to_string(),
        provider_as_of: None,
        record_updated_at: OffsetDateTime::UNIX_EPOCH,
        dataset_version: "identity".to_string(),
        age_days: 0,
    }
}

/// Result of a repository upsert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxUpsertOutcome {
    Inserted,
    /// An existing row was updated because the canonical rate (or other
    /// provider provenance) changed. The future portfolio integration must
    /// react to this by requesting a full portfolio-history rebuild for
    /// every portfolio that touches the affected currency.
    Updated,
    /// An existing row matched byte-for-byte; no write needed.
    Unchanged,
}

#[derive(Debug, thiserror::Error)]
pub enum FxDomainError {
    #[error("invalid currency code `{0}` — expected three ASCII letters")]
    InvalidCurrencyCode(String),
    #[error("FX rates must be strictly positive")]
    NonPositiveRate,
    #[error("source and target currencies are equal: use identity_lookup instead")]
    IdentityPairNotPersistable,
    #[error("invalid date range: {0}")]
    InvalidDateRange(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use time::Month;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
    }

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal literal should parse")
    }

    #[test]
    fn currency_parses_uppercase() {
        assert_eq!(Currency::parse("eur").unwrap().as_str(), "EUR");
        assert_eq!(Currency::parse("USD").unwrap().as_str(), "USD");
    }

    #[test]
    fn currency_rejects_invalid_codes() {
        assert!(Currency::parse("EU").is_err());
        assert!(Currency::parse("EURO").is_err());
        assert!(Currency::parse("EU1").is_err());
        assert!(Currency::parse("   ").is_err());
    }

    #[test]
    fn canonical_pair_orders_lexicographically() {
        let eur = Currency::parse("EUR").unwrap();
        let usd = Currency::parse("USD").unwrap();
        let pair_a = CanonicalPair::new(eur, usd).unwrap();
        let pair_b = CanonicalPair::new(usd, eur).unwrap();
        assert_eq!(pair_a, pair_b);
        assert_eq!(pair_a.base().as_str(), "EUR");
        assert_eq!(pair_a.quote().as_str(), "USD");
    }

    #[test]
    fn canonical_pair_rejects_identity() {
        let eur = Currency::parse("EUR").unwrap();
        assert!(CanonicalPair::new(eur, eur).is_none());
    }

    #[test]
    fn derive_inverse_matches_postgres_round_12() {
        // 1 / 1.1461 = 0.872524212547..., rounded at 12 dp.
        let inverse = derive_inverse(d("1.1461")).unwrap();
        assert_eq!(inverse, d("0.872524212547"));
    }

    #[test]
    fn derive_inverse_rejects_non_positive() {
        assert!(derive_inverse(Decimal::ZERO).is_err());
        assert!(derive_inverse(d("-1")).is_err());
    }

    #[test]
    fn canonical_fx_rate_constructs_consistent_inverse() {
        let pair = CanonicalPair::new(
            Currency::parse("EUR").unwrap(),
            Currency::parse("USD").unwrap(),
        )
        .unwrap();
        let r = CanonicalFxRate::from_canonical(
            pair,
            date(2026, 6, 18),
            d("1.1461"),
            "mock_ecb_fixture",
            None,
            "ecb-2026-06-18",
        )
        .unwrap();
        assert_eq!(r.canonical_rate, d("1.1461"));
        assert_eq!(r.inverse_rate, d("0.872524212547"));
    }

    #[test]
    fn identity_lookup_returns_unit_rate() {
        let eur = Currency::parse("EUR").unwrap();
        let hit = identity_lookup(eur, date(2026, 6, 18));
        assert_eq!(hit.rate, Decimal::ONE);
        assert_eq!(hit.source, hit.target);
        assert_eq!(hit.age_days, 0);
        assert_eq!(hit.provider, "identity");
    }

    #[test]
    fn unavailable_reason_strings() {
        assert_eq!(FxUnavailableReason::RateMissing.as_str(), "rate_missing");
        assert_eq!(FxUnavailableReason::RateStale.as_str(), "rate_stale");
        assert_eq!(
            FxUnavailableReason::ProviderNotConfigured.as_str(),
            "provider_not_configured"
        );
        assert_eq!(
            FxUnavailableReason::UnsupportedMockCurrency.as_str(),
            "unsupported_mock_currency"
        );
    }
}
