//! Deterministic mock FX history provider, anchored on a frozen
//! publicly-verifiable ECB daily reference rate snapshot.
//!
//! # Fixture provenance
//!
//! - **Source**: European Central Bank, "Euro foreign exchange reference
//!   rates", daily reference rates published at
//!   <https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml>.
//! - **Effective rate date** (the `time=` attribute of the ECB XML
//!   envelope): **2026-06-18**.
//! - **Retrieved on**: 2026-06-19 (during PR004 implementation).
//! - **Anchor currency**: `EUR` — the ECB feed publishes "1 EUR = X" for
//!   every quoted currency.
//!
//! The exact frozen anchor values are inlined as Rust constants below. A
//! future fixture update is an explicit code review (this file is the
//! single source of truth; no automatic refresh, no network call at
//! runtime, no system-clock dependency in pure-function paths).
//!
//! # Historical variation — integer + Decimal triangle wave
//!
//! Every non-EUR currency has its OWN deterministic, bounded daily factor
//! computed from integer arithmetic and `rust_decimal::Decimal`. No
//! `f32`, no `f64`, no trigonometric function, no binary-floating-point
//! conversion is involved in producing a rate.
//!
//! ```text
//! daily_anchor_value(EUR, D) = 1                              (anchor)
//! daily_anchor_value(ccy, D) = anchor[ccy] * factor(ccy, D)
//! factor(ccy, D)             = 1 + Decimal(signed_amp_bps(ccy, D)) / 10_000
//! signed_amp_bps(ccy, D)     ∈ [-amp_bps(ccy), +amp_bps(ccy)]
//! rate(A → B, D)             = daily_anchor_value(B, D) / daily_anchor_value(A, D)
//! ```
//!
//! - `amp_bps(ccy)` ∈ `{0}` for EUR and `{10, 15, ..., 50}` (basis
//!   points) for every other currency — derived from a stable FNV-1a hash
//!   of the currency code and bounded by `MAX_VARIATION_AMP_BPS = 50` bps
//!   (≤ 0.5 % of anchor).
//! - `period_days(ccy)` ∈ `{40, 46, ..., 100}` days — derived from the
//!   same hash; ensures currencies sharing an amplitude still trace
//!   distinct trajectories over time.
//! - `signed_amp_bps(ccy, D)` evaluates a deterministic integer
//!   **triangle wave** of period `period_days(ccy)` and peak amplitude
//!   `amp_bps(ccy)`, anchored at 0 at `D == anchor_rate_date()`.
//! - On the anchor date the integer offset is exactly 0 ⇒ triangle
//!   value 0 ⇒ `factor(ccy, anchor_date) = Decimal::ONE` exactly for
//!   every currency. The frozen anchor vector is therefore recovered
//!   byte-for-byte on the snapshot day.
//!
//! # Cross-rate and triangular consistency
//!
//! All 45 canonical pairs of the 10-currency set are derived from the
//! **same** per-currency daily vector. For any A, B, C:
//!
//! ```text
//! rate(A → B, D) * rate(B → C, D)
//!   = (B(D)/A(D)) * (C(D)/B(D))
//!   = C(D)/A(D)
//!   = rate(A → C, D)
//! ```
//!
//! The shared intermediate currency `B(D)` cancels by construction, so
//! the triangular identity holds exactly on every date (subject to the
//! 12-dp persistence rounding). Reciprocity `rate(A→B) * rate(B→A) = 1`
//! is the special case `A = C`.
//!
//! # No common-factor cancellation
//!
//! An earlier draft applied a SINGLE daily factor to every currency.
//! That factor cancelled out from every cross-rate, producing constant
//! rates across dates. A subsequent draft fixed that with an `f64`
//! `sin()`-based per-currency factor. The current implementation
//! replaces `sin()` with an integer triangle wave so the entire
//! generation path is fixed-decimal — both for portability and to
//! satisfy the "no `f32`/`f64` in rate calculation" contract.
//!
//! # Boundaries
//!
//! Weekends (Saturday / Sunday) return `ProviderDailyRate::NoQuoteForDate`.
//! The provider does not model bank/market holidays; the repository's
//! 7-day carry-forward covers weekends and short holiday gaps.
//!
//! No network calls. No system-clock dependency in any pure-function
//! path. The persisted column is `numeric(28, 12)`.

use crate::domain::fx_rate::{CanonicalFxRate, CanonicalPair, Currency};
use crate::errors::MarketDataError;
use crate::providers::fx_history_provider::{FxHistoryProvider, ProviderDailyRate};
use rust_decimal::Decimal;
use std::str::FromStr;
use time::{Date, OffsetDateTime, Weekday};

/// Stable provider name persisted in `fx_rate_history_cache.provider`.
pub const MOCK_FX_PROVIDER_NAME: &str = "mock_ecb_fixture";

/// Stable dataset version persisted in
/// `fx_rate_history_cache.dataset_version`. Bumping this constant is the
/// **only** mechanism that allows the mock provider's deterministic
/// output to change. It signals to the repository that previously-persisted
/// rows for this provider are out of date and triggers `Updated` outcomes.
///
/// v2 (this constant) — generation-algorithm correction: the integer
/// triangle-wave + Decimal arithmetic replaces the earlier
/// floating-point `sin()` factor. The frozen ECB anchor values
/// (2026-06-18) are **unchanged**; only the per-currency factor function
/// changed. Existing v1 rows are reclassified as `Updated` when the v2
/// provider output is applied.
pub const MOCK_FX_DATASET_VERSION: &str = "mock-ecb-2026-06-18-v2";

/// Anchor rate date: the ECB envelope `time=` attribute of the frozen
/// snapshot. On this exact date the provider returns the raw anchor
/// values (no oscillation applied).
pub fn anchor_rate_date() -> Date {
    Date::from_calendar_date(2026, time::Month::June, 18)
        .expect("anchor rate date is hard-coded and valid")
}

/// Hard-coded provider_as_of for the anchor snapshot. ECB publishes
/// reference rates around 16:00 CET = 14:00 UTC.
pub fn anchor_provider_as_of() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(
        // 2026-06-18T14:00:00Z
        1_781_708_400,
    )
    .expect("anchor timestamp is hard-coded and valid")
}

/// Frozen ECB anchor: how many units of each currency equal 1 EUR on
/// 2026-06-18. EUR is the anchor (1.0 by definition).
///
/// The 10 product-selected currencies are:
/// USD, EUR, JPY, GBP, CNY, CHF, AUD, CAD, HKD, SGD.
struct AnchorEntry {
    code: &'static str,
    units_per_eur: &'static str,
}

const ANCHOR_TABLE: &[AnchorEntry] = &[
    AnchorEntry {
        code: "EUR",
        units_per_eur: "1",
    },
    AnchorEntry {
        code: "USD",
        units_per_eur: "1.1461",
    },
    AnchorEntry {
        code: "JPY",
        units_per_eur: "184.44",
    },
    AnchorEntry {
        code: "GBP",
        units_per_eur: "0.86638",
    },
    AnchorEntry {
        code: "CHF",
        units_per_eur: "0.9218",
    },
    AnchorEntry {
        code: "AUD",
        units_per_eur: "1.6362",
    },
    AnchorEntry {
        code: "CAD",
        units_per_eur: "1.6189",
    },
    AnchorEntry {
        code: "HKD",
        units_per_eur: "8.9827",
    },
    AnchorEntry {
        code: "SGD",
        units_per_eur: "1.4795",
    },
    AnchorEntry {
        code: "CNY",
        units_per_eur: "7.7609",
    },
];

/// List of supported currency codes (uppercase, exactly the 10 product
/// currencies). Exposed for configuration validation and for listing
/// canonical pairs to fill.
pub fn supported_currencies() -> Vec<Currency> {
    ANCHOR_TABLE
        .iter()
        .map(|e| Currency::parse(e.code).expect("anchor code is valid"))
        .collect()
}

fn anchor_units_per_eur(code: Currency) -> Option<Decimal> {
    ANCHOR_TABLE
        .iter()
        .find(|e| e.code == code.as_str())
        .map(|e| Decimal::from_str(e.units_per_eur).expect("anchor literal parses"))
}

/// Maximum allowed amplitude in basis points — strict upper bound for
/// every currency's per-currency amplitude. 50 bps = 0.50 % of anchor.
const MAX_VARIATION_AMP_BPS: i64 = 50;

/// Anchor-relative reference date used to ensure the frozen snapshot itself
/// is returned exactly on the anchor date for every currency: when
/// `(jdn(D) - jdn(anchor)) == 0`, every currency's triangle-wave argument
/// is 0 and the per-currency factor evaluates to exactly 1.
fn anchor_jdn() -> i64 {
    anchor_rate_date().to_julian_day() as i64
}

/// Deterministic 32-bit hash of a 3-letter ASCII currency code.
/// Pure FNV-1a over the three bytes — stable across platforms and Rust
/// versions, and entirely free of std-library hashing randomness.
fn currency_hash(ccy: Currency) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in ccy.as_str().as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Per-currency amplitude in basis points. Always strictly positive for a
/// non-EUR currency and bounded by `MAX_VARIATION_AMP_BPS` (50 bps).
fn currency_amp_bps(ccy: Currency) -> i64 {
    // EUR is the anchor: amplitude 0 ⇒ factor always exactly 1.
    if ccy.as_str() == "EUR" {
        return 0;
    }
    let bucket = (currency_hash(ccy) % 9) as i64; // 0..=8
    let bps = 10 + bucket * 5; // 10, 15, ..., 50
    debug_assert!(bps > 0 && bps <= MAX_VARIATION_AMP_BPS);
    bps
}

/// Per-currency oscillation period in days. Bounded so the variation is
/// always slow-moving compared with the typical history window. Different
/// periods across currencies ensure that two currencies sharing the same
/// amplitude still trace distinct trajectories over time. All bucket
/// values are even so `period / 2` and `period / 4` integer divisions
/// behave intuitively (the triangle wave handles asymmetric quarters
/// correctly when 4 does not divide the period).
fn currency_period_days(ccy: Currency) -> i64 {
    // Periods in [40, 100] days, in 6-day buckets ⇒ 11 buckets.
    let bucket = ((currency_hash(ccy) / 7) % 11) as i64;
    40 + bucket * 6
}

/// Integer triangle-wave amplitude in signed basis points for `ccy` on
/// `date`. Anchored so the value at `date == anchor_rate_date()` is
/// exactly 0 for every currency.
///
/// The waveform on one full period of length `P` (anchored at `t = 0`):
///
/// ```text
///   t in [0,        P/4]   : linear  0      → +amp_bps
///   t in (P/4,      P/2]   : linear +amp_bps → 0
///   t in (P/2,      P/2+Q] : linear  0      → -amp_bps  (Q = P/4)
///   t in (P/2+Q,    P)     : linear -amp_bps → 0
/// ```
///
/// `P` may not be divisible by 4 — segment widths are computed
/// independently so the function remains exact integer arithmetic with
/// no off-by-one drift.
fn signed_amp_bps(ccy: Currency, date: Date) -> i64 {
    let amp = currency_amp_bps(ccy);
    if amp == 0 {
        return 0;
    }
    let period = currency_period_days(ccy);
    let offset = (date.to_julian_day() as i64) - anchor_jdn();
    // Map signed offset into `[0, period)` exactly (rem_euclid handles
    // negative offsets and is integer-only).
    let t = offset.rem_euclid(period);
    let half = period / 2; // even period ⇒ exact
    let quarter = period / 4; // floor
    // Segment widths — these are always > 0 because period >= 40.
    let w0 = quarter; // segment 0: [0, quarter]
    let w1 = half - quarter; // segment 1: [quarter, half]
    let w2 = quarter; // segment 2: [half, half + quarter]
    let w3 = period - half - quarter; // segment 3: [half + quarter, period)
    if t == 0 {
        return 0;
    }
    if t <= w0 {
        // [0, w0]: 0 → +amp
        amp * t / w0
    } else if t <= half {
        // (w0, half]: +amp → 0
        amp * (half - t) / w1
    } else if t <= half + w2 {
        // (half, half+w2]: 0 → -amp
        -amp * (t - half) / w2
    } else {
        // (half+w2, period): -amp → 0
        -amp * (period - t) / w3
    }
}

/// Decimal divisor `10_000` for converting integer basis points into a
/// fractional factor.
fn bps_divisor() -> Decimal {
    Decimal::from(10_000_i64)
}

/// Bounded deterministic factor for `ccy` on `date`, with
/// `factor == Decimal::ONE` at the anchor date.
///
/// The computation is **fully integer + Decimal**: no `f32`, no `f64`,
/// no trigonometric functions, no binary-floating-point conversion.
fn per_currency_factor(ccy: Currency, date: Date) -> Decimal {
    let signed = signed_amp_bps(ccy, date);
    if signed == 0 {
        return Decimal::ONE;
    }
    Decimal::ONE + Decimal::from(signed) / bps_divisor()
}

/// Daily anchor value for `ccy` on `date`. Equal to the frozen anchor on
/// the anchor date for every currency; bounded per-currency variation
/// elsewhere.
fn daily_anchor_value(ccy: Currency, date: Date) -> Option<Decimal> {
    let base = anchor_units_per_eur(ccy)?;
    Some(base * per_currency_factor(ccy, date))
}

/// Compute the canonical rate (quote per base) for a canonical pair on a
/// given business date.
///
/// All cross-rates are derived from the same per-currency daily vector
/// (`daily_anchor_value`), so the triangular identity
/// `rate(A→B) * rate(B→C) == rate(A→C)` holds exactly on every date — the
/// shared third currency cancels by construction.
///
/// Because each currency has its own amplitude / phase / period, the
/// daily vector itself genuinely varies across dates (and so does every
/// cross-rate). On the anchor date, every per-currency factor is exactly
/// 1 and we recover the frozen anchor values.
fn canonical_rate_on(pair: &CanonicalPair, date: Date) -> Option<Decimal> {
    let base_today = daily_anchor_value(pair.base(), date)?;
    let quote_today = daily_anchor_value(pair.quote(), date)?;
    Some((quote_today / base_today).round_dp(12))
}

/// True when the date is a business day (Mon–Fri). Weekends return no
/// new provider rate; the repository carry-forward covers them.
fn is_business_day(date: Date) -> bool {
    !matches!(date.weekday(), Weekday::Saturday | Weekday::Sunday)
}

pub struct MockFxHistoryProvider;

impl FxHistoryProvider for MockFxHistoryProvider {
    fn name(&self) -> &'static str {
        MOCK_FX_PROVIDER_NAME
    }

    fn dataset_version(&self) -> &'static str {
        MOCK_FX_DATASET_VERSION
    }

    fn supports_pair(&self, pair: &CanonicalPair) -> bool {
        anchor_units_per_eur(pair.base()).is_some() && anchor_units_per_eur(pair.quote()).is_some()
    }

    async fn get_canonical_rate(
        &self,
        pair: &CanonicalPair,
        rate_date: Date,
    ) -> Result<ProviderDailyRate, MarketDataError> {
        if !self.supports_pair(pair) {
            return Ok(ProviderDailyRate::NoQuoteForDate);
        }
        if !is_business_day(rate_date) {
            return Ok(ProviderDailyRate::NoQuoteForDate);
        }
        let Some(canonical) = canonical_rate_on(pair, rate_date) else {
            return Ok(ProviderDailyRate::NoQuoteForDate);
        };
        let provider_as_of = if rate_date == anchor_rate_date() {
            anchor_provider_as_of()
        } else {
            // Synthetic instant: end-of-business-day UTC.
            OffsetDateTime::from_unix_timestamp(
                (rate_date.to_julian_day() as i64 - 2_440_588) * 86_400 + 22 * 3_600,
            )
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        };
        let rate = CanonicalFxRate::from_canonical(
            *pair,
            rate_date,
            canonical,
            MOCK_FX_PROVIDER_NAME,
            Some(provider_as_of),
            MOCK_FX_DATASET_VERSION,
        )?;
        Ok(ProviderDailyRate::Rate(rate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fx_rate::Currency;
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use time::Month;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
    }

    fn ccy(s: &str) -> Currency {
        Currency::parse(s).unwrap()
    }

    fn pair(a: &str, b: &str) -> CanonicalPair {
        CanonicalPair::new(ccy(a), ccy(b)).unwrap()
    }

    #[test]
    fn supports_all_ten_currencies() {
        let currencies = supported_currencies();
        assert_eq!(currencies.len(), 10);
        let mut codes: Vec<String> = currencies.iter().map(|c| c.to_string()).collect();
        codes.sort();
        assert_eq!(
            codes,
            vec![
                "AUD", "CAD", "CHF", "CNY", "EUR", "GBP", "HKD", "JPY", "SGD", "USD"
            ]
        );
    }

    #[test]
    fn forty_five_canonical_pairs_supported() {
        let provider = MockFxHistoryProvider;
        let currencies = supported_currencies();
        let mut count = 0_usize;
        for (i, a) in currencies.iter().enumerate() {
            for b in currencies.iter().skip(i + 1) {
                let p = CanonicalPair::new(*a, *b).expect("distinct currencies");
                assert!(provider.supports_pair(&p), "pair {p} should be supported");
                count += 1;
            }
        }
        assert_eq!(count, 45);
    }

    #[tokio::test]
    async fn anchor_date_returns_eur_usd_anchor_exactly() {
        let provider = MockFxHistoryProvider;
        let p = pair("EUR", "USD");
        let res = provider
            .get_canonical_rate(&p, anchor_rate_date())
            .await
            .expect("mock should succeed");
        match res {
            ProviderDailyRate::Rate(r) => {
                // On the anchor date every per-currency factor is exactly 1
                // (sin(0) = 0 ⇒ factor = 1) so EUR/USD = 1.1461 exactly.
                assert_eq!(r.canonical_rate, Decimal::from_str("1.1461").unwrap());
                assert_eq!(r.provider, MOCK_FX_PROVIDER_NAME);
                assert_eq!(r.dataset_version, MOCK_FX_DATASET_VERSION);
            }
            other => panic!("expected Rate, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn anchor_date_returns_every_anchor_exactly() {
        // For every non-EUR currency, "1 EUR = anchor[X] X" must hold on
        // the anchor date — read in the EUR→X direction regardless of the
        // canonical ordering of the pair (some currencies sort before
        // EUR, some after).
        let provider = MockFxHistoryProvider;
        let anchor = anchor_rate_date();
        let expected = [
            ("USD", "1.1461"),
            ("JPY", "184.44"),
            ("GBP", "0.86638"),
            ("CHF", "0.9218"),
            ("AUD", "1.6362"),
            ("CAD", "1.6189"),
            ("HKD", "8.9827"),
            ("SGD", "1.4795"),
            ("CNY", "7.7609"),
        ];
        for (code, anchor_value) in expected {
            let eur_to_x = fetch_rate(&provider, "EUR", code, anchor).await;
            let expected = Decimal::from_str(anchor_value).unwrap();
            // The fetched value goes through up to two 12-dp roundings
            // (canonical division + inverse derivation) when the pair is
            // CHF/EUR-style; rounded to 8 dp the anchor must reappear
            // exactly.
            assert_eq!(
                eur_to_x.round_dp(8),
                expected.round_dp(8),
                "anchor mismatch for EUR→{code}: {eur_to_x} vs {expected}"
            );
        }
    }

    #[tokio::test]
    async fn eur_usd_differs_between_two_business_dates() {
        // Genuine date variation — the bug the previous algorithm had
        // (factor cancelled out for every cross-rate). EUR is the anchor;
        // EUR→USD = daily_anchor_value(USD, D), which varies in D.
        let provider = MockFxHistoryProvider;
        let p = pair("EUR", "USD");
        let r1 = match provider
            .get_canonical_rate(&p, date(2026, 6, 22))
            .await
            .unwrap()
        {
            ProviderDailyRate::Rate(r) => r,
            other => panic!("expected Rate, got {other:?}"),
        };
        let r2 = match provider
            .get_canonical_rate(&p, date(2026, 7, 10))
            .await
            .unwrap()
        {
            ProviderDailyRate::Rate(r) => r,
            other => panic!("expected Rate, got {other:?}"),
        };
        assert_ne!(
            r1.canonical_rate, r2.canonical_rate,
            "EUR/USD must vary across distinct business dates"
        );
    }

    #[tokio::test]
    async fn non_eur_cross_rate_differs_between_dates() {
        // USD/JPY (= JPY_day / USD_day) involves two non-EUR currencies
        // with independent per-currency factors. It must also vary.
        let provider = MockFxHistoryProvider;
        let p = pair("JPY", "USD"); // canonical alphabetical: JPY < USD
        let r1 = match provider
            .get_canonical_rate(&p, date(2026, 6, 22))
            .await
            .unwrap()
        {
            ProviderDailyRate::Rate(r) => r,
            _ => panic!(),
        };
        let r2 = match provider
            .get_canonical_rate(&p, date(2026, 8, 14))
            .await
            .unwrap()
        {
            ProviderDailyRate::Rate(r) => r,
            _ => panic!(),
        };
        assert_ne!(r1.canonical_rate, r2.canonical_rate);
    }

    #[tokio::test]
    async fn variation_stays_within_bounded_envelope() {
        // ±1% is a safe upper envelope for the combined effect of two
        // per-currency factors each bounded by ±0.5%.
        let provider = MockFxHistoryProvider;
        let anchor_eur_usd = Decimal::from_str("1.1461").unwrap();
        let envelope = Decimal::from_str("0.012").unwrap(); // 1.2 % safety
        for offset_days in 1..120 {
            let d = anchor_rate_date()
                .checked_add(time::Duration::days(offset_days))
                .unwrap();
            if !is_business_day(d) {
                continue;
            }
            let res = provider
                .get_canonical_rate(&pair("EUR", "USD"), d)
                .await
                .unwrap();
            let r = match res {
                ProviderDailyRate::Rate(r) => r,
                _ => panic!(),
            };
            let diff = (r.canonical_rate - anchor_eur_usd).abs();
            let ratio = diff / anchor_eur_usd;
            assert!(
                ratio < envelope,
                "EUR/USD drift exceeded envelope at {d}: rate={} ratio={}",
                r.canonical_rate,
                ratio
            );
        }
    }

    #[tokio::test]
    async fn weekend_returns_no_quote() {
        let provider = MockFxHistoryProvider;
        let p = pair("EUR", "USD");
        // 2026-06-20 is Saturday
        let sat = date(2026, 6, 20);
        let res = provider
            .get_canonical_rate(&p, sat)
            .await
            .expect("provider call ok");
        assert!(matches!(res, ProviderDailyRate::NoQuoteForDate));
    }

    #[tokio::test]
    async fn business_day_returns_positive_rate() {
        let provider = MockFxHistoryProvider;
        let p = pair("EUR", "USD");
        let res = provider
            .get_canonical_rate(&p, date(2026, 6, 19))
            .await
            .expect("provider call ok");
        let rate = match res {
            ProviderDailyRate::Rate(r) => r,
            other => panic!("expected Rate, got {other:?}"),
        };
        assert!(rate.canonical_rate > Decimal::ZERO);
        assert!(rate.inverse_rate > Decimal::ZERO);
    }

    #[tokio::test]
    async fn deterministic_across_invocations() {
        let provider = MockFxHistoryProvider;
        let p = pair("EUR", "USD");
        let d = date(2026, 6, 23);
        let r1 = provider.get_canonical_rate(&p, d).await.unwrap();
        let r2 = provider.get_canonical_rate(&p, d).await.unwrap();
        match (r1, r2) {
            (ProviderDailyRate::Rate(a), ProviderDailyRate::Rate(b)) => {
                assert_eq!(a.canonical_rate, b.canonical_rate);
                assert_eq!(a.inverse_rate, b.inverse_rate);
            }
            _ => panic!("expected two Rate results"),
        }
    }

    async fn fetch_rate(
        provider: &MockFxHistoryProvider,
        from: &str,
        to: &str,
        d: Date,
    ) -> Decimal {
        let canonical_pair =
            CanonicalPair::new(Currency::parse(from).unwrap(), Currency::parse(to).unwrap())
                .unwrap();
        let res = provider
            .get_canonical_rate(&canonical_pair, d)
            .await
            .unwrap();
        let canonical = match res {
            ProviderDailyRate::Rate(r) => r,
            _ => panic!("expected Rate"),
        };
        if canonical_pair.base().as_str() == from {
            canonical.canonical_rate
        } else {
            canonical.inverse_rate
        }
    }

    #[tokio::test]
    async fn reciprocal_consistency_across_dates() {
        let provider = MockFxHistoryProvider;
        for offset in 0..30 {
            let d = anchor_rate_date()
                .checked_add(time::Duration::days(offset))
                .unwrap();
            if !is_business_day(d) {
                continue;
            }
            for (from, to) in [("EUR", "USD"), ("USD", "JPY"), ("CHF", "CNY")] {
                let direct = fetch_rate(&provider, from, to, d).await;
                let inverse = fetch_rate(&provider, to, from, d).await;
                let product = (direct * inverse).round_dp(8);
                assert_eq!(
                    product,
                    Decimal::ONE,
                    "reciprocal broken on {d} for {from}/{to}: {direct} * {inverse} = {product}"
                );
            }
        }
    }

    #[tokio::test]
    async fn cross_rate_consistency_triangular_across_dates() {
        // For any A, B, C: rate(A→B) * rate(B→C) == rate(A→C) at every
        // tested date. Because every pair is derived from the same
        // per-currency daily anchor vector, the intermediate currency
        // cancels by construction.
        let provider = MockFxHistoryProvider;
        let dates = [
            anchor_rate_date(),
            date(2026, 6, 23),
            date(2026, 7, 6),
            date(2026, 7, 27),
            date(2026, 8, 17),
        ];
        let triples = [
            ("USD", "EUR", "JPY"),
            ("CHF", "CNY", "GBP"),
            ("AUD", "CAD", "USD"),
            ("HKD", "SGD", "JPY"),
        ];
        for d in dates {
            assert!(is_business_day(d));
            for (a, b, c) in triples {
                let ab = fetch_rate(&provider, a, b, d).await;
                let bc = fetch_rate(&provider, b, c, d).await;
                let ac = fetch_rate(&provider, a, c, d).await;
                let lhs = (ab * bc).round_dp(8);
                let rhs = ac.round_dp(8);
                assert!(
                    (lhs - rhs).abs() < Decimal::from_str("0.00001").unwrap(),
                    "triangular identity broken on {d} for {a}/{b}/{c}: \
                     {a}→{b}={ab} {b}→{c}={bc} ⇒ lhs={lhs} vs {a}→{c}={rhs}"
                );
            }
        }
    }

    #[tokio::test]
    async fn forty_five_pairs_all_unique_on_a_given_date() {
        // Sanity: with 10 currencies (EUR fixed + 9 varying independently),
        // the 45 canonical pairs should produce 45 distinct rate values on
        // a generic business date (no accidental collision). The check is
        // probabilistic against the per-currency hash but stable thanks to
        // determinism.
        let provider = MockFxHistoryProvider;
        let d = date(2026, 6, 23);
        let currencies = supported_currencies();
        let mut rates = std::collections::BTreeSet::new();
        for (i, a) in currencies.iter().enumerate() {
            for b in currencies.iter().skip(i + 1) {
                let p = CanonicalPair::new(*a, *b).unwrap();
                let res = provider.get_canonical_rate(&p, d).await.unwrap();
                let canonical = match res {
                    ProviderDailyRate::Rate(r) => r,
                    _ => panic!(),
                };
                rates.insert(canonical.canonical_rate);
            }
        }
        assert_eq!(rates.len(), 45, "expected 45 distinct rates on {d}");
    }

    /// Source-level guarantee: the rate-generation path (the
    /// `signed_amp_bps` triangle wave and `per_currency_factor`) must
    /// remain free of `f32` / `f64` / trigonometric calls. The
    /// comments / module-level doc are allowed to mention these tokens
    /// (they explicitly document the absence), but no active source
    /// code line outside comments and Rustdoc may match.
    /// Source-level guarantee: the rate-generation path (above the
    /// `#[cfg(test)]` boundary) must remain free of `f32` / `f64` /
    /// trigonometric calls. The module-level Rustdoc and the doc
    /// comments on individual functions are allowed to MENTION these
    /// tokens (they explicitly document their absence); no active line
    /// of production code outside comments may match.
    #[test]
    fn no_floating_point_rate_generation_path() {
        let full_src = include_str!("mock_fx_history.rs");
        // Truncate at the `#[cfg(test)]` boundary so the assertion list
        // inside this very test does not match itself.
        let prod_src = full_src
            .split_once("#[cfg(test)]")
            .map(|(prod, _tests)| prod)
            .unwrap_or(full_src);
        let forbidden = [".sin(", ".cos(", ".tan(", ".powf(", " as f32", " as f64"];
        for (line_no, line) in prod_src.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//!") || trimmed.starts_with("///") || trimmed.starts_with("//")
            {
                continue;
            }
            for token in forbidden {
                assert!(
                    !line.contains(token),
                    "forbidden floating-point token `{token}` found at line {n}: {line}",
                    n = line_no + 1
                );
            }
        }
        // Standalone keywords must not appear as type annotations in the
        // production path (the doc comments are filtered out above).
        for keyword in [": f32", ": f64", "-> f32", "-> f64", "f64::consts"] {
            for (line_no, line) in prod_src.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//!")
                    || trimmed.starts_with("///")
                    || trimmed.starts_with("//")
                {
                    continue;
                }
                assert!(
                    !line.contains(keyword),
                    "forbidden floating-point keyword `{keyword}` found at line {n}: {line}",
                    n = line_no + 1
                );
            }
        }
    }

    #[test]
    fn dataset_version_is_v2() {
        assert_eq!(MOCK_FX_DATASET_VERSION, "mock-ecb-2026-06-18-v2");
    }

    #[test]
    fn provider_name_is_stable() {
        assert_eq!(MOCK_FX_PROVIDER_NAME, "mock_ecb_fixture");
    }

    #[test]
    fn anchor_date_is_2026_06_18() {
        // The frozen ECB snapshot date does NOT change when the
        // generation algorithm is corrected: only the dataset_version
        // does.
        let d = anchor_rate_date();
        assert_eq!(d.year(), 2026);
        assert_eq!(d.month(), Month::June);
        assert_eq!(d.day(), 18);
    }

    #[tokio::test]
    async fn v2_inverse_round_trip_through_repo_precision() {
        // The persisted inverse is `ROUND(1 / canonical_rate, 12)` on
        // the database side. The application-side `derive_inverse`
        // matches this exactly. Verify that on the anchor date the
        // domain round-trip is consistent for one well-known anchor.
        let provider = MockFxHistoryProvider;
        let p = pair("EUR", "USD");
        let res = provider
            .get_canonical_rate(&p, anchor_rate_date())
            .await
            .unwrap();
        let canonical = match res {
            ProviderDailyRate::Rate(r) => r,
            _ => panic!(),
        };
        assert_eq!(
            canonical.canonical_rate,
            Decimal::from_str("1.1461").unwrap()
        );
        assert_eq!(
            canonical.inverse_rate,
            Decimal::from_str("0.872524212547").unwrap()
        );
    }
}
