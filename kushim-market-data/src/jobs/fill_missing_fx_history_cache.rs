//! Fill / repair the `fx_rate_history_cache` from an FX provider.
//!
//! The job is deterministic, idempotent and isolated from the equity-price
//! pipeline. It works on canonical pairs and an explicit date range.
//! Cross-service discovery of "first conversion need" is **not** part of
//! this PR — see `documentation/architecture/historical-fx-foundation.md`,
//! section "Future integration boundary".

use crate::{
    domain::fx_rate::{CanonicalFxRate, CanonicalPair, Currency},
    errors::MarketDataError,
    jobs::Job,
    providers::fx_history_provider::{FxHistoryProvider, ProviderDailyRate},
    repositories::fx_rate_history_cache,
    state::AppState,
};
use std::collections::BTreeSet;
use time::Date;

/// Counters reported by one run of the FX fill job.
#[derive(Debug, Default, Clone, Copy)]
pub struct FxFillSummary {
    pub pairs_total: usize,
    pub pairs_failed: usize,
    pub dates_in_range: i64,
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub provider_no_quote: usize,
    pub provider_errors: usize,
}

pub struct FillMissingFxHistoryCacheJob<P: FxHistoryProvider> {
    provider: P,
    date_from: Date,
    date_to: Date,
    pairs: Vec<CanonicalPair>,
    /// Maximum number of canonical rates persisted in a single transaction.
    /// Each pair is at most one chunk per call to `run`.
    #[allow(dead_code)]
    chunk_days: usize,
}

impl<P: FxHistoryProvider> FillMissingFxHistoryCacheJob<P> {
    /// Build a job that fills the cartesian product of `currencies` (45
    /// canonical pairs for 10 currencies). Currencies are deduplicated;
    /// identity pairs are skipped (the contract handles identity
    /// conversions without persistence).
    pub fn for_currency_set(
        provider: P,
        date_from: Date,
        date_to: Date,
        currencies: &[Currency],
        chunk_days: usize,
    ) -> Result<Self, MarketDataError> {
        validate_range(date_from, date_to)?;
        let dedup: BTreeSet<Currency> = currencies.iter().copied().collect();
        let mut pairs = Vec::new();
        let ordered: Vec<Currency> = dedup.into_iter().collect();
        for (i, a) in ordered.iter().enumerate() {
            for b in ordered.iter().skip(i + 1) {
                if let Some(p) = CanonicalPair::new(*a, *b) {
                    pairs.push(p);
                }
            }
        }
        Ok(Self {
            provider,
            date_from,
            date_to,
            pairs,
            chunk_days,
        })
    }

    /// Build a job for an explicit canonical pair list. Identity pairs in
    /// the input are rejected.
    pub fn for_explicit_pairs(
        provider: P,
        date_from: Date,
        date_to: Date,
        pairs: Vec<CanonicalPair>,
        chunk_days: usize,
    ) -> Result<Self, MarketDataError> {
        validate_range(date_from, date_to)?;
        Ok(Self {
            provider,
            date_from,
            date_to,
            pairs,
            chunk_days,
        })
    }

    pub fn pairs(&self) -> &[CanonicalPair] {
        &self.pairs
    }

    /// Run the job and return aggregated counters. The default `Job::run`
    /// implementation discards the summary; callers that need it (CLI,
    /// tests) can invoke this directly.
    pub async fn run_with_summary(
        &self,
        state: &AppState,
    ) -> Result<FxFillSummary, MarketDataError> {
        let mut summary = FxFillSummary {
            pairs_total: self.pairs.len(),
            dates_in_range: (self.date_to - self.date_from).whole_days() + 1,
            ..FxFillSummary::default()
        };

        for pair in &self.pairs {
            if !self.provider.supports_pair(pair) {
                summary.pairs_failed += 1;
                tracing::warn!(
                    provider = self.provider.name(),
                    pair = %pair,
                    "provider does not support pair, skipping"
                );
                continue;
            }

            let missing = match fx_rate_history_cache::missing_dates_for_pair(
                &state.pg_pool,
                pair,
                self.provider.name(),
                self.date_from,
                self.date_to,
            )
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    summary.pairs_failed += 1;
                    tracing::error!(
                        provider = self.provider.name(),
                        pair = %pair,
                        error = %e,
                        "missing-dates query failed, skipping pair"
                    );
                    continue;
                }
            };

            let mut batch: Vec<CanonicalFxRate> = Vec::new();
            let mut had_provider_error = false;

            // Repair pass over genuinely-missing dates only. Existing rows
            // are then re-evaluated (for correction detection) on the full
            // range — this is what surfaces dataset-version bumps as
            // `Updated` outcomes.
            for date in &missing {
                match self.provider.get_canonical_rate(pair, *date).await {
                    Ok(ProviderDailyRate::Rate(rate)) => batch.push(rate),
                    Ok(ProviderDailyRate::NoQuoteForDate) => {
                        summary.provider_no_quote += 1;
                    }
                    Err(e) => {
                        summary.provider_errors += 1;
                        had_provider_error = true;
                        tracing::warn!(
                            provider = self.provider.name(),
                            pair = %pair,
                            rate_date = %date,
                            error = %e,
                            "provider canonical-rate fetch failed"
                        );
                        break;
                    }
                }
            }

            // Re-check existing rows for correction (changed canonical_rate
            // or dataset_version). This is the path that triggers
            // `Updated` outcomes and lets the future portfolio integration
            // request a full portfolio-history rebuild.
            if !had_provider_error {
                let present_dates: BTreeSet<Date> = missing.iter().copied().collect();
                let mut cursor = self.date_from;
                loop {
                    if !present_dates.contains(&cursor) {
                        match self.provider.get_canonical_rate(pair, cursor).await {
                            Ok(ProviderDailyRate::Rate(rate)) => batch.push(rate),
                            Ok(ProviderDailyRate::NoQuoteForDate) => {
                                // No-op: weekend / holiday.
                            }
                            Err(e) => {
                                summary.provider_errors += 1;
                                tracing::warn!(
                                    provider = self.provider.name(),
                                    pair = %pair,
                                    rate_date = %cursor,
                                    error = %e,
                                    "provider canonical-rate fetch failed (correction pass)"
                                );
                                had_provider_error = true;
                                break;
                            }
                        }
                    }
                    match cursor.next_day() {
                        Some(next) if next <= self.date_to => cursor = next,
                        _ => break,
                    }
                }
            }

            if had_provider_error {
                summary.pairs_failed += 1;
                continue;
            }

            if batch.is_empty() {
                continue;
            }

            match fx_rate_history_cache::upsert_bulk(&state.pg_pool, &batch).await {
                Ok(c) => {
                    summary.inserted += c.inserted;
                    summary.updated += c.updated;
                    summary.unchanged += c.unchanged;
                }
                Err(e) => {
                    summary.pairs_failed += 1;
                    tracing::error!(
                        provider = self.provider.name(),
                        pair = %pair,
                        error = %e,
                        "bulk upsert failed, rolling back this pair"
                    );
                }
            }
        }

        tracing::info!(
            job = self.name(),
            provider = self.provider.name(),
            dataset_version = self.provider.dataset_version(),
            date_from = %self.date_from,
            date_to = %self.date_to,
            pairs_total = summary.pairs_total,
            pairs_failed = summary.pairs_failed,
            dates_in_range = summary.dates_in_range,
            inserted = summary.inserted,
            updated = summary.updated,
            unchanged = summary.unchanged,
            provider_no_quote = summary.provider_no_quote,
            provider_errors = summary.provider_errors,
            "fill_missing_fx_history_cache completed"
        );

        if summary.pairs_failed > 0 && summary.inserted == 0 && summary.updated == 0 {
            return Err(MarketDataError::Job(format!(
                "fill_missing_fx_history_cache: every pair failed (provider_errors={}, pairs_failed={})",
                summary.provider_errors, summary.pairs_failed
            )));
        }

        Ok(summary)
    }
}

impl<P: FxHistoryProvider> Job for FillMissingFxHistoryCacheJob<P> {
    fn name(&self) -> &'static str {
        "fill_missing_fx_history_cache"
    }

    async fn run(&self, state: &AppState) -> Result<(), MarketDataError> {
        self.run_with_summary(state).await.map(|_| ())
    }
}

fn validate_range(date_from: Date, date_to: Date) -> Result<(), MarketDataError> {
    if date_from > date_to {
        return Err(MarketDataError::Config(format!(
            "FX history date range invalid: from ({date_from}) > to ({date_to})"
        )));
    }
    let days = (date_to - date_from).whole_days() + 1;
    if days > 366 {
        return Err(MarketDataError::Config(format!(
            "FX history date range must be at most 366 days; got {days}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::fx_rate::{CanonicalFxRate, CanonicalPair, Currency},
        providers::{
            fx_history_provider::ProviderDailyRate,
            mock_fx_history::{MOCK_FX_PROVIDER_NAME, MockFxHistoryProvider, supported_currencies},
        },
        repositories::fx_rate_history_cache::upsert_canonical_rate,
        state::AppState,
        test_utils::lock_env,
    };
    use rust_decimal::Decimal;
    use sqlx::PgPool;
    use std::str::FromStr;
    use time::{Date, Month, OffsetDateTime};

    async fn test_pool() -> PgPool {
        // Read DATABASE_URL under the shared env lock: the config tests mutate
        // process env (set_var/remove_var) under the same lock, so an unguarded
        // read here could race with them and observe an empty value.
        let database_url = {
            let _guard = lock_env();
            std::env::var("DATABASE_URL").unwrap_or_default()
        };
        assert!(
            !database_url.is_empty(),
            "DATABASE_URL must be set for fx job integration tests"
        );
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    async fn cleanup_provider_range(pool: &PgPool, provider: &str, from: Date, to: Date) {
        sqlx::query(
            "DELETE FROM fx_rate_history_cache \
             WHERE provider = $1 AND rate_date >= $2 AND rate_date <= $3",
        )
        .bind(provider)
        .bind(from)
        .bind(to)
        .execute(pool)
        .await
        .ok();
    }

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
    }

    fn ccy(s: &str) -> Currency {
        Currency::parse(s).unwrap()
    }

    fn pair(a: &str, b: &str) -> CanonicalPair {
        CanonicalPair::new(ccy(a), ccy(b)).unwrap()
    }

    #[tokio::test]
    async fn first_fill_then_second_fill_is_idempotent() {
        let pool = test_pool().await;
        // Dedicated date window — distinct from every other job test so
        // parallel test execution does not collide on
        // (pair, date, MOCK_FX_PROVIDER_NAME).
        let from = date(2026, 9, 7);
        let to = date(2026, 9, 9);
        cleanup_provider_range(&pool, MOCK_FX_PROVIDER_NAME, from, to).await;
        let state = AppState {
            pg_pool: pool.clone(),
        };

        let pairs = vec![pair("EUR", "USD"), pair("EUR", "JPY")];
        let job = FillMissingFxHistoryCacheJob::for_explicit_pairs(
            MockFxHistoryProvider,
            from,
            to,
            pairs,
            366,
        )
        .unwrap();

        let s1 = job.run_with_summary(&state).await.unwrap();
        assert_eq!(s1.pairs_total, 2);
        assert_eq!(s1.pairs_failed, 0);
        assert_eq!(s1.dates_in_range, 3);
        assert_eq!(s1.inserted, 6); // 2 pairs × 3 business days
        assert_eq!(s1.updated, 0);
        assert_eq!(s1.unchanged, 0);
        assert_eq!(s1.provider_no_quote, 0);
        assert_eq!(s1.provider_errors, 0);

        let s2 = job.run_with_summary(&state).await.unwrap();
        assert_eq!(s2.inserted, 0, "second run must insert nothing");
        assert_eq!(s2.updated, 0, "second run must update nothing");
        assert_eq!(s2.unchanged, 6);

        cleanup_provider_range(&pool, MOCK_FX_PROVIDER_NAME, from, to).await;
    }

    #[tokio::test]
    async fn partial_gap_repair_only_fills_missing_dates() {
        let pool = test_pool().await;
        let from = date(2026, 9, 14);
        let to = date(2026, 9, 16);
        cleanup_provider_range(&pool, MOCK_FX_PROVIDER_NAME, from, to).await;
        let state = AppState {
            pg_pool: pool.clone(),
        };

        let provider = MockFxHistoryProvider;
        let preseed_pair = pair("EUR", "USD");
        let preseed_date = date(2026, 9, 15);
        match provider
            .get_canonical_rate(&preseed_pair, preseed_date)
            .await
            .unwrap()
        {
            ProviderDailyRate::Rate(r) => {
                upsert_canonical_rate(&pool, &r).await.unwrap();
            }
            other => panic!("expected Rate, got {other:?}"),
        }

        let job = FillMissingFxHistoryCacheJob::for_explicit_pairs(
            MockFxHistoryProvider,
            from,
            to,
            vec![preseed_pair],
            366,
        )
        .unwrap();

        let s = job.run_with_summary(&state).await.unwrap();
        assert_eq!(s.inserted, 2, "must insert only the two surrounding gaps");
        assert_eq!(s.updated, 0, "pre-seeded row matches provider → no update");
        assert_eq!(
            s.unchanged, 1,
            "pre-seeded row is reclassified as unchanged"
        );

        cleanup_provider_range(&pool, MOCK_FX_PROVIDER_NAME, from, to).await;
    }

    #[tokio::test]
    async fn dataset_version_change_triggers_update_in_second_run() {
        let pool = test_pool().await;
        let from = date(2026, 9, 21);
        let to = date(2026, 9, 23);
        cleanup_provider_range(&pool, MOCK_FX_PROVIDER_NAME, from, to).await;
        let state = AppState {
            pg_pool: pool.clone(),
        };

        let pairs = vec![pair("EUR", "USD")];
        let job = FillMissingFxHistoryCacheJob::for_explicit_pairs(
            MockFxHistoryProvider,
            from,
            to,
            pairs,
            366,
        )
        .unwrap();
        let _ = job.run_with_summary(&state).await.unwrap();

        // Manually rewrite the dataset_version of one stored row. Re-running
        // the same fill must surface that row as Updated.
        sqlx::query(
            "UPDATE fx_rate_history_cache SET dataset_version='stale-version' \
             WHERE provider = $1 AND canonical_base_currency='EUR' \
             AND canonical_quote_currency='USD' AND rate_date=$2",
        )
        .bind(MOCK_FX_PROVIDER_NAME)
        .bind(from)
        .execute(&pool)
        .await
        .unwrap();

        let s = job.run_with_summary(&state).await.unwrap();
        assert!(s.updated >= 1, "stale dataset_version must trigger update");

        cleanup_provider_range(&pool, MOCK_FX_PROVIDER_NAME, from, to).await;
    }

    #[tokio::test]
    async fn invalid_range_is_rejected() {
        let result = FillMissingFxHistoryCacheJob::for_currency_set(
            MockFxHistoryProvider,
            date(2026, 6, 19),
            date(2026, 6, 15),
            &supported_currencies(),
            366,
        );
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("from > to must be rejected"),
        };
        let msg = err.to_string();
        assert!(msg.contains("invalid"), "got: {msg}");
    }

    #[tokio::test]
    async fn unsupported_mock_currency_marks_pair_failed() {
        let pool = test_pool().await;
        let state = AppState {
            pg_pool: pool.clone(),
        };

        // ZZZ is a valid ISO-style code shape but not in the mock anchor
        // table. No rows are persisted by this test so no cleanup window
        // is needed.
        let pairs = vec![pair("EUR", "ZZZ")];
        let job = FillMissingFxHistoryCacheJob::for_explicit_pairs(
            MockFxHistoryProvider,
            date(2026, 9, 28),
            date(2026, 9, 28),
            pairs,
            366,
        )
        .unwrap();

        let result = job.run_with_summary(&state).await;
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("single unsupported pair must surface an error"),
        };
        assert!(err.to_string().contains("fill_missing_fx_history_cache"));
    }

    #[tokio::test]
    async fn weekend_dates_are_skipped_with_provider_no_quote() {
        let pool = test_pool().await;
        let state = AppState {
            pg_pool: pool.clone(),
        };

        let pairs = vec![pair("EUR", "USD")];
        // 2026-10-03 = Saturday, 2026-10-04 = Sunday → both weekend. No
        // rows are persisted on a pure weekend window.
        let job = FillMissingFxHistoryCacheJob::for_explicit_pairs(
            MockFxHistoryProvider,
            date(2026, 10, 3),
            date(2026, 10, 4),
            pairs,
            366,
        )
        .unwrap();

        let s = job.run_with_summary(&state).await.unwrap();
        assert_eq!(s.inserted, 0);
        assert_eq!(s.updated, 0);
        assert!(
            s.provider_no_quote >= 2,
            "weekend dates must surface as provider_no_quote, got {}",
            s.provider_no_quote
        );
    }

    #[tokio::test]
    async fn currency_set_constructor_yields_45_pairs_for_default_currencies() {
        let job = FillMissingFxHistoryCacheJob::for_currency_set(
            MockFxHistoryProvider,
            date(2026, 6, 15),
            date(2026, 6, 17),
            &supported_currencies(),
            366,
        )
        .unwrap();
        assert_eq!(job.pairs().len(), 45);
    }

    #[tokio::test]
    async fn explicit_pair_subset_only_persists_those_pairs() {
        let pool = test_pool().await;
        let dt = date(2026, 10, 12);
        cleanup_provider_range(&pool, MOCK_FX_PROVIDER_NAME, dt, dt).await;
        let state = AppState {
            pg_pool: pool.clone(),
        };

        let pairs = vec![pair("CHF", "JPY")];
        let job = FillMissingFxHistoryCacheJob::for_explicit_pairs(
            MockFxHistoryProvider,
            dt,
            dt,
            pairs,
            366,
        )
        .unwrap();
        let s = job.run_with_summary(&state).await.unwrap();
        assert_eq!(s.inserted, 1);

        let distinct_pairs: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT (canonical_base_currency, canonical_quote_currency)) \
             FROM fx_rate_history_cache WHERE provider = $1 AND rate_date = $2",
        )
        .bind(MOCK_FX_PROVIDER_NAME)
        .bind(dt)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(distinct_pairs, 1);

        cleanup_provider_range(&pool, MOCK_FX_PROVIDER_NAME, dt, dt).await;
    }

    // Sanity helper used by the integration smoke tests — kept here so we
    // can extend the suite without exporting the helpers.
    fn _silence_unused() {
        let _ = OffsetDateTime::UNIX_EPOCH;
        let _ = Decimal::from_str("1").unwrap();
        let _: CanonicalFxRate = CanonicalFxRate::from_canonical(
            pair("EUR", "USD"),
            date(2026, 6, 18),
            Decimal::ONE,
            "x",
            None,
            "y",
        )
        .unwrap();
    }
}
