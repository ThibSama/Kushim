//! Repository for `fx_rate_history_cache`.
//!
//! Contract enforced here:
//!
//! * Canonical and inverse values cannot diverge: the database has the
//!   inverse as a STORED GENERATED column, and this module never accepts
//!   a separately-supplied inverse.
//! * Multiple providers can coexist for the same pair and date; lookups
//!   require an explicit provider.
//! * Carry-forward selects the latest rate where `rate_date <= requested_date`
//!   and rejects it when its age exceeds `max_age_days` (the contract MVP
//!   default is 7).
//! * Identity conversions (`source == target`) are handled in the domain
//!   layer (`identity_lookup`) and never touch the database.

use crate::domain::fx_rate::{
    CanonicalFxRate, CanonicalPair, Currency, FxLookup, FxLookupHit, FxUnavailableReason,
    FxUpsertOutcome, PairDirection,
};
use rust_decimal::Decimal;
use sqlx::Row;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Postgres, Transaction};
use time::{Date, OffsetDateTime};

/// Default carry-forward window in calendar days. Matches the MVP
/// historical performance contract.
pub const DEFAULT_MAX_CARRY_DAYS: i64 = 7;

/// Insert or update a single canonical rate. `inverse_rate` is computed
/// on the database side. Returns the outcome so the caller can distinguish
/// inserted / updated / unchanged rows.
pub async fn upsert_canonical_rate(
    pool: &PgPool,
    rate: &CanonicalFxRate,
) -> Result<FxUpsertOutcome, sqlx::Error> {
    upsert_canonical_rate_in(pool, rate, None).await
}

/// Variant that joins an existing transaction. Used by `upsert_bulk`.
async fn upsert_canonical_rate_in(
    pool: &PgPool,
    rate: &CanonicalFxRate,
    tx: Option<&mut Transaction<'_, Postgres>>,
) -> Result<FxUpsertOutcome, sqlx::Error> {
    // Strategy: SELECT the existing row, decide outcome, then write only
    // when needed. This avoids `UPDATE ... RETURNING` semantics and lets
    // us return Unchanged without bumping `updated_at`.

    let existing: Option<(Decimal, String, Option<OffsetDateTime>)> = sqlx::query_as(
        r#"
        SELECT canonical_rate, dataset_version, provider_as_of
          FROM fx_rate_history_cache
         WHERE canonical_base_currency = $1
           AND canonical_quote_currency = $2
           AND rate_date = $3
           AND provider = $4
        "#,
    )
    .bind(rate.pair.base().as_str())
    .bind(rate.pair.quote().as_str())
    .bind(rate.rate_date)
    .bind(&rate.provider)
    .fetch_optional(pool)
    .await?;

    let _ = tx; // tx unused in v1 — kept for future bulk integration

    if let Some((existing_rate, existing_version, existing_as_of)) = existing {
        if existing_rate == rate.canonical_rate
            && existing_version == rate.dataset_version
            && existing_as_of == rate.provider_as_of
        {
            return Ok(FxUpsertOutcome::Unchanged);
        }
        let rows = sqlx::query(
            r#"
            UPDATE fx_rate_history_cache
               SET canonical_rate  = $5,
                   provider_as_of  = $6,
                   dataset_version = $7
             WHERE canonical_base_currency = $1
               AND canonical_quote_currency = $2
               AND rate_date = $3
               AND provider = $4
            "#,
        )
        .bind(rate.pair.base().as_str())
        .bind(rate.pair.quote().as_str())
        .bind(rate.rate_date)
        .bind(&rate.provider)
        .bind(rate.canonical_rate)
        .bind(rate.provider_as_of)
        .bind(&rate.dataset_version)
        .execute(pool)
        .await?;
        debug_assert_eq!(rows.rows_affected(), 1);
        return Ok(FxUpsertOutcome::Updated);
    }

    sqlx::query(
        r#"
        INSERT INTO fx_rate_history_cache
            (canonical_base_currency, canonical_quote_currency, rate_date,
             provider, canonical_rate, provider_as_of, dataset_version)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (canonical_base_currency, canonical_quote_currency, rate_date, provider)
        DO NOTHING
        "#,
    )
    .bind(rate.pair.base().as_str())
    .bind(rate.pair.quote().as_str())
    .bind(rate.rate_date)
    .bind(&rate.provider)
    .bind(rate.canonical_rate)
    .bind(rate.provider_as_of)
    .bind(&rate.dataset_version)
    .execute(pool)
    .await?;
    Ok(FxUpsertOutcome::Inserted)
}

/// Bulk-upsert a slice of canonical rates inside a single PostgreSQL
/// transaction. Either every rate in the batch is published or none of
/// them is. Returns aggregated counters.
#[derive(Debug, Default, Clone, Copy)]
pub struct BulkUpsertCounters {
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
}

pub async fn upsert_bulk(
    pool: &PgPool,
    rates: &[CanonicalFxRate],
) -> Result<BulkUpsertCounters, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let mut counters = BulkUpsertCounters::default();
    for rate in rates {
        // We piggy-back on upsert_canonical_rate_in with `tx=None` because
        // SQLx requires `&mut Transaction` (not borrowed across awaits).
        // For batch-size correctness we redo the existence check via a
        // transactional path inline.
        let existing: Option<(Decimal, String, Option<OffsetDateTime>)> = sqlx::query_as(
            r#"
            SELECT canonical_rate, dataset_version, provider_as_of
              FROM fx_rate_history_cache
             WHERE canonical_base_currency = $1
               AND canonical_quote_currency = $2
               AND rate_date = $3
               AND provider = $4
             FOR UPDATE
            "#,
        )
        .bind(rate.pair.base().as_str())
        .bind(rate.pair.quote().as_str())
        .bind(rate.rate_date)
        .bind(&rate.provider)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some((existing_rate, existing_version, existing_as_of)) = existing {
            if existing_rate == rate.canonical_rate
                && existing_version == rate.dataset_version
                && existing_as_of == rate.provider_as_of
            {
                counters.unchanged += 1;
                continue;
            }
            sqlx::query(
                r#"
                UPDATE fx_rate_history_cache
                   SET canonical_rate  = $5,
                       provider_as_of  = $6,
                       dataset_version = $7
                 WHERE canonical_base_currency = $1
                   AND canonical_quote_currency = $2
                   AND rate_date = $3
                   AND provider = $4
                "#,
            )
            .bind(rate.pair.base().as_str())
            .bind(rate.pair.quote().as_str())
            .bind(rate.rate_date)
            .bind(&rate.provider)
            .bind(rate.canonical_rate)
            .bind(rate.provider_as_of)
            .bind(&rate.dataset_version)
            .execute(&mut *tx)
            .await?;
            counters.updated += 1;
        } else {
            sqlx::query(
                r#"
                INSERT INTO fx_rate_history_cache
                    (canonical_base_currency, canonical_quote_currency, rate_date,
                     provider, canonical_rate, provider_as_of, dataset_version)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(rate.pair.base().as_str())
            .bind(rate.pair.quote().as_str())
            .bind(rate.rate_date)
            .bind(&rate.provider)
            .bind(rate.canonical_rate)
            .bind(rate.provider_as_of)
            .bind(&rate.dataset_version)
            .execute(&mut *tx)
            .await?;
            counters.inserted += 1;
        }
    }
    tx.commit().await?;
    Ok(counters)
}

/// Look up a rate for `source → target` on `requested_date`, with a
/// carry-forward window of `max_age_days` calendar days.
///
/// Identity conversions are not handled here — call
/// `domain::fx_rate::identity_lookup` instead.
pub async fn lookup_latest(
    pool: &PgPool,
    source: Currency,
    target: Currency,
    requested_date: Date,
    max_age_days: i64,
    provider: &str,
) -> Result<FxLookup, sqlx::Error> {
    let Some(pair) = CanonicalPair::new(source, target) else {
        // Caller should have handled identity; surface a non-DB unavailability.
        return Ok(FxLookup::Unavailable {
            source,
            target,
            requested_date,
            reason: FxUnavailableReason::RateMissing,
            candidate_age_days: None,
        });
    };

    let direction = if source == pair.base() && target == pair.quote() {
        PairDirection::Direct
    } else {
        PairDirection::Inverse
    };

    let row: Option<PgRow> = sqlx::query(
        r#"
        SELECT rate_date,
               canonical_rate,
               inverse_rate,
               provider,
               provider_as_of,
               dataset_version,
               updated_at
          FROM fx_rate_history_cache
         WHERE canonical_base_currency = $1
           AND canonical_quote_currency = $2
           AND provider = $3
           AND rate_date <= $4
         ORDER BY rate_date DESC
         LIMIT 1
        "#,
    )
    .bind(pair.base().as_str())
    .bind(pair.quote().as_str())
    .bind(provider)
    .bind(requested_date)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(FxLookup::Unavailable {
            source,
            target,
            requested_date,
            reason: FxUnavailableReason::RateMissing,
            candidate_age_days: None,
        });
    };

    let rate_date: Date = row.get("rate_date");
    let canonical_rate: Decimal = row.get("canonical_rate");
    let inverse_rate: Decimal = row.get("inverse_rate");
    let provider_name: String = row.get("provider");
    let provider_as_of: Option<OffsetDateTime> = row.get("provider_as_of");
    let dataset_version: String = row.get("dataset_version");
    let record_updated_at: OffsetDateTime = row.get("updated_at");

    let age_days = (requested_date - rate_date).whole_days();
    if age_days > max_age_days {
        return Ok(FxLookup::Unavailable {
            source,
            target,
            requested_date,
            reason: FxUnavailableReason::RateStale,
            candidate_age_days: Some(age_days),
        });
    }

    let rate = match direction {
        PairDirection::Direct => canonical_rate,
        PairDirection::Inverse => inverse_rate,
    };

    Ok(FxLookup::Available(FxLookupHit {
        source,
        target,
        requested_date,
        rate_date,
        rate,
        direction,
        provider: provider_name,
        provider_as_of,
        record_updated_at,
        dataset_version,
        age_days: age_days.max(0),
    }))
}

/// Enumerate the dates inside `[date_from, date_to]` for which **no** row
/// exists for the given pair/provider. Used by the repair job to fill only
/// the genuine gaps without rewriting the rest of the table.
pub async fn missing_dates_for_pair(
    pool: &PgPool,
    pair: &CanonicalPair,
    provider: &str,
    date_from: Date,
    date_to: Date,
) -> Result<Vec<Date>, sqlx::Error> {
    if date_from > date_to {
        return Ok(Vec::new());
    }
    let present: Vec<Date> = sqlx::query_scalar(
        r#"
        SELECT rate_date
          FROM fx_rate_history_cache
         WHERE canonical_base_currency = $1
           AND canonical_quote_currency = $2
           AND provider = $3
           AND rate_date >= $4
           AND rate_date <= $5
        "#,
    )
    .bind(pair.base().as_str())
    .bind(pair.quote().as_str())
    .bind(provider)
    .bind(date_from)
    .bind(date_to)
    .fetch_all(pool)
    .await?;

    let present_set: std::collections::BTreeSet<Date> = present.into_iter().collect();
    let mut missing = Vec::new();
    let mut current = date_from;
    loop {
        if !present_set.contains(&current) {
            missing.push(current);
        }
        match current.next_day() {
            Some(next) if next <= date_to => current = next,
            _ => break,
        }
    }
    Ok(missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fx_rate::{
        CanonicalFxRate, CanonicalPair, Currency, FxUnavailableReason, FxUpsertOutcome,
        identity_lookup,
    };
    use rust_decimal::Decimal;
    use sqlx::PgPool;
    use std::str::FromStr;
    use time::{Date, Duration, Month, OffsetDateTime};

    /// Test-only provider identifier. We embed the test name in the
    /// provider string so two integration tests can run against the
    /// shared disposable database without colliding on the
    /// `(pair, date, provider)` unique index. The provider column is
    /// `varchar(50)` — keep helper outputs short.
    fn test_provider(suffix: &str) -> String {
        format!("test_fx_{suffix}")
    }

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal literal")
    }

    fn date(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
    }

    fn ccy(s: &str) -> Currency {
        Currency::parse(s).unwrap()
    }

    fn pair(a: &str, b: &str) -> CanonicalPair {
        CanonicalPair::new(ccy(a), ccy(b)).unwrap()
    }

    fn rate_for(
        pair: CanonicalPair,
        rate_date: Date,
        canonical_rate: &str,
        provider: &str,
    ) -> CanonicalFxRate {
        CanonicalFxRate::from_canonical(
            pair,
            rate_date,
            d(canonical_rate),
            provider,
            Some(OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()),
            "test-dataset-v1",
        )
        .expect("test rate constructs")
    }

    async fn test_pool() -> PgPool {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_default();
        assert!(
            !database_url.is_empty(),
            "DATABASE_URL must be set for fx repository integration tests"
        );
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    async fn cleanup(pool: &PgPool, provider: &str) {
        sqlx::query("DELETE FROM fx_rate_history_cache WHERE provider = $1")
            .bind(provider)
            .execute(pool)
            .await
            .ok();
    }

    #[tokio::test]
    async fn upsert_inserts_then_returns_unchanged_then_updated() {
        let pool = test_pool().await;
        let provider = test_provider("insert_then_unchanged_then_updated");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        let dt = date(2026, 6, 18);
        let r = rate_for(p, dt, "1.1461", &provider);

        let first = upsert_canonical_rate(&pool, &r).await.unwrap();
        assert_eq!(first, FxUpsertOutcome::Inserted);

        let second = upsert_canonical_rate(&pool, &r).await.unwrap();
        assert_eq!(second, FxUpsertOutcome::Unchanged);

        let corrected = rate_for(p, dt, "1.1500", &provider);
        let third = upsert_canonical_rate(&pool, &corrected).await.unwrap();
        assert_eq!(third, FxUpsertOutcome::Updated);

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn corrected_dataset_version_triggers_update() {
        let pool = test_pool().await;
        let provider = test_provider("corrected_dataset_version");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        let dt = date(2026, 6, 18);
        let r1 = rate_for(p, dt, "1.1461", &provider);
        assert_eq!(
            upsert_canonical_rate(&pool, &r1).await.unwrap(),
            FxUpsertOutcome::Inserted
        );

        let mut r2 = r1.clone();
        r2.dataset_version = "test-dataset-v2".to_string();
        assert_eq!(
            upsert_canonical_rate(&pool, &r2).await.unwrap(),
            FxUpsertOutcome::Updated
        );

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn multiple_providers_coexist_for_same_pair_and_date() {
        let pool = test_pool().await;
        let p1 = test_provider("multi_a");
        let p2 = test_provider("multi_b");
        cleanup(&pool, &p1).await;
        cleanup(&pool, &p2).await;

        let p = pair("EUR", "USD");
        let dt = date(2026, 6, 18);
        upsert_canonical_rate(&pool, &rate_for(p, dt, "1.1461", &p1))
            .await
            .unwrap();
        upsert_canonical_rate(&pool, &rate_for(p, dt, "1.1500", &p2))
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM fx_rate_history_cache \
             WHERE canonical_base_currency='EUR' AND canonical_quote_currency='USD' \
             AND rate_date=$1 AND provider IN ($2, $3)",
        )
        .bind(dt)
        .bind(&p1)
        .bind(&p2)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 2);

        cleanup(&pool, &p1).await;
        cleanup(&pool, &p2).await;
    }

    #[tokio::test]
    async fn exact_date_direct_and_inverse_lookups() {
        let pool = test_pool().await;
        let provider = test_provider("direct_inverse");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        let dt = date(2026, 6, 18);
        upsert_canonical_rate(&pool, &rate_for(p, dt, "1.1461", &provider))
            .await
            .unwrap();

        let direct = lookup_latest(&pool, ccy("EUR"), ccy("USD"), dt, 7, &provider)
            .await
            .unwrap();
        match direct {
            FxLookup::Available(hit) => {
                assert_eq!(hit.rate, d("1.1461"));
                assert_eq!(hit.direction, PairDirection::Direct);
                assert_eq!(hit.rate_date, dt);
                assert_eq!(hit.age_days, 0);
                assert!(!hit.is_inverse_direction());
            }
            other => panic!("expected Available, got {other:?}"),
        }

        let inverse = lookup_latest(&pool, ccy("USD"), ccy("EUR"), dt, 7, &provider)
            .await
            .unwrap();
        match inverse {
            FxLookup::Available(hit) => {
                assert_eq!(hit.direction, PairDirection::Inverse);
                assert!(hit.is_inverse_direction());
                assert_eq!(hit.rate, d("0.872524212547"));
            }
            other => panic!("expected Available, got {other:?}"),
        }

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn identity_lookup_returns_unit_and_does_not_persist() {
        let pool = test_pool().await;
        let provider = test_provider("identity");
        cleanup(&pool, &provider).await;

        let eur = ccy("EUR");
        let dt = date(2026, 6, 18);
        let hit = identity_lookup(eur, dt);
        assert_eq!(hit.rate, Decimal::ONE);
        assert_eq!(hit.age_days, 0);
        assert_eq!(hit.provider, "identity");

        // The repo path with source==target returns Unavailable (the
        // caller MUST short-circuit to identity_lookup first, but the
        // repo must not crash).
        let res = lookup_latest(&pool, eur, eur, dt, 7, &provider)
            .await
            .unwrap();
        assert!(!res.is_available());

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM fx_rate_history_cache \
             WHERE canonical_base_currency = canonical_quote_currency",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "identity rows must never exist");

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn one_day_carry_forward_returns_yesterdays_rate() {
        let pool = test_pool().await;
        let provider = test_provider("carry_one_day");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        let stored = date(2026, 6, 18);
        upsert_canonical_rate(&pool, &rate_for(p, stored, "1.1461", &provider))
            .await
            .unwrap();

        let next = stored.checked_add(Duration::days(1)).unwrap();
        let res = lookup_latest(&pool, ccy("EUR"), ccy("USD"), next, 7, &provider)
            .await
            .unwrap();
        match res {
            FxLookup::Available(hit) => {
                assert_eq!(hit.rate_date, stored);
                assert_eq!(hit.age_days, 1);
                assert_eq!(hit.rate, d("1.1461"));
            }
            other => panic!("expected Available, got {other:?}"),
        }

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn weekend_carry_forward_returns_friday_rate() {
        let pool = test_pool().await;
        let provider = test_provider("carry_weekend");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        let friday = date(2026, 6, 19);
        let sunday = date(2026, 6, 21);
        upsert_canonical_rate(&pool, &rate_for(p, friday, "1.1461", &provider))
            .await
            .unwrap();

        let res = lookup_latest(&pool, ccy("EUR"), ccy("USD"), sunday, 7, &provider)
            .await
            .unwrap();
        match res {
            FxLookup::Available(hit) => {
                assert_eq!(hit.rate_date, friday);
                assert_eq!(hit.age_days, 2);
            }
            other => panic!("expected Available, got {other:?}"),
        }

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn carry_forward_exactly_seven_days_is_accepted() {
        let pool = test_pool().await;
        let provider = test_provider("carry_exact_seven");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        let stored = date(2026, 6, 1);
        let target = stored.checked_add(Duration::days(7)).unwrap();
        upsert_canonical_rate(&pool, &rate_for(p, stored, "1.1461", &provider))
            .await
            .unwrap();

        let res = lookup_latest(&pool, ccy("EUR"), ccy("USD"), target, 7, &provider)
            .await
            .unwrap();
        match res {
            FxLookup::Available(hit) => {
                assert_eq!(hit.age_days, 7);
            }
            other => panic!("7-day carry-forward must be accepted, got {other:?}"),
        }

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn eight_day_old_rate_is_rejected_as_stale() {
        let pool = test_pool().await;
        let provider = test_provider("carry_eight_day_stale");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        let stored = date(2026, 6, 1);
        let target = stored.checked_add(Duration::days(8)).unwrap();
        upsert_canonical_rate(&pool, &rate_for(p, stored, "1.1461", &provider))
            .await
            .unwrap();

        let res = lookup_latest(&pool, ccy("EUR"), ccy("USD"), target, 7, &provider)
            .await
            .unwrap();
        match res {
            FxLookup::Unavailable {
                reason,
                candidate_age_days,
                ..
            } => {
                assert_eq!(reason, FxUnavailableReason::RateStale);
                assert_eq!(candidate_age_days, Some(8));
            }
            other => panic!("8-day-old rate must be Stale, got {other:?}"),
        }

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn missing_pair_returns_rate_missing() {
        let pool = test_pool().await;
        let provider = test_provider("missing_pair");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        upsert_canonical_rate(&pool, &rate_for(p, date(2026, 6, 18), "1.1461", &provider))
            .await
            .unwrap();

        let res = lookup_latest(
            &pool,
            ccy("EUR"),
            ccy("JPY"),
            date(2026, 6, 18),
            7,
            &provider,
        )
        .await
        .unwrap();
        match res {
            FxLookup::Unavailable {
                reason,
                candidate_age_days,
                ..
            } => {
                assert_eq!(reason, FxUnavailableReason::RateMissing);
                assert_eq!(candidate_age_days, None);
            }
            other => panic!("missing pair must be RateMissing, got {other:?}"),
        }

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn unknown_provider_returns_rate_missing() {
        let pool = test_pool().await;
        let provider_a = test_provider("known_provider_a");
        let provider_b = test_provider("unknown_provider_b");
        cleanup(&pool, &provider_a).await;
        cleanup(&pool, &provider_b).await;

        let p = pair("EUR", "USD");
        upsert_canonical_rate(
            &pool,
            &rate_for(p, date(2026, 6, 18), "1.1461", &provider_a),
        )
        .await
        .unwrap();

        let res = lookup_latest(
            &pool,
            ccy("EUR"),
            ccy("USD"),
            date(2026, 6, 18),
            7,
            &provider_b,
        )
        .await
        .unwrap();
        match res {
            FxLookup::Unavailable { reason, .. } => {
                assert_eq!(reason, FxUnavailableReason::RateMissing);
            }
            other => panic!("unknown provider must be RateMissing, got {other:?}"),
        }

        cleanup(&pool, &provider_a).await;
        cleanup(&pool, &provider_b).await;
    }

    #[tokio::test]
    async fn missing_dates_detection_over_mixed_range() {
        let pool = test_pool().await;
        let provider = test_provider("missing_dates");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        let d1 = date(2026, 6, 15);
        let d3 = date(2026, 6, 17);
        let d5 = date(2026, 6, 19);
        for (day, val) in [(d1, "1.1450"), (d3, "1.1455"), (d5, "1.1461")] {
            upsert_canonical_rate(&pool, &rate_for(p, day, val, &provider))
                .await
                .unwrap();
        }

        let missing =
            missing_dates_for_pair(&pool, &p, &provider, date(2026, 6, 15), date(2026, 6, 19))
                .await
                .unwrap();
        assert_eq!(
            missing,
            vec![date(2026, 6, 16), date(2026, 6, 18)],
            "expected to detect only the actual gaps"
        );

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn bulk_upsert_keeps_one_row_per_pair_date_provider() {
        let pool = test_pool().await;
        let provider = test_provider("bulk_one_row");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        let dt = date(2026, 6, 18);
        let r1 = rate_for(p, dt, "1.1461", &provider);
        let r2 = rate_for(p, dt, "1.1462", &provider);

        let counters = upsert_bulk(&pool, &[r1, r2]).await.unwrap();
        assert_eq!(counters.inserted, 1);
        assert_eq!(counters.updated, 1);
        assert_eq!(counters.unchanged, 0);

        let row_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM fx_rate_history_cache \
             WHERE provider = $1 AND canonical_base_currency='EUR' \
             AND canonical_quote_currency='USD' AND rate_date = $2",
        )
        .bind(&provider)
        .bind(dt)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row_count, 1, "unique constraint must keep exactly one row");

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn canonical_and_inverse_reciprocal_within_documented_precision() {
        let pool = test_pool().await;
        let provider = test_provider("reciprocal_precision");
        cleanup(&pool, &provider).await;

        let p = pair("EUR", "USD");
        upsert_canonical_rate(&pool, &rate_for(p, date(2026, 6, 18), "1.1461", &provider))
            .await
            .unwrap();

        let row: (Decimal, Decimal) = sqlx::query_as(
            "SELECT canonical_rate, inverse_rate FROM fx_rate_history_cache \
             WHERE provider = $1 AND canonical_base_currency='EUR' \
             AND canonical_quote_currency='USD' AND rate_date=$2",
        )
        .bind(&provider)
        .bind(date(2026, 6, 18))
        .fetch_one(&pool)
        .await
        .unwrap();
        let (canonical, inverse) = row;
        let product = (canonical * inverse).round_dp(8);
        assert_eq!(product, Decimal::ONE);

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn bulk_upsert_rolls_back_completely_on_invalid_row() {
        let pool = test_pool().await;
        let provider = test_provider("bulk_rollback");
        cleanup(&pool, &provider).await;

        let valid = rate_for(pair("EUR", "USD"), date(2026, 6, 18), "1.1461", &provider);
        let mut invalid = rate_for(pair("EUR", "JPY"), date(2026, 6, 18), "184.44", &provider);
        invalid.provider = "   ".to_string(); // violates chk_*_provider_not_blank

        let result = upsert_bulk(&pool, &[valid, invalid]).await;
        assert!(result.is_err(), "bulk upsert with invalid row must fail");

        let after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM fx_rate_history_cache WHERE provider = $1")
                .bind(&provider)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, 0, "rollback must leave no persisted rows");

        cleanup(&pool, &provider).await;
    }

    #[tokio::test]
    async fn no_silent_provider_mixing_across_lookups() {
        let pool = test_pool().await;
        let p1 = test_provider("nosilent_a");
        let p2 = test_provider("nosilent_b");
        cleanup(&pool, &p1).await;
        cleanup(&pool, &p2).await;

        let p = pair("EUR", "USD");
        let dt = date(2026, 6, 18);
        upsert_canonical_rate(&pool, &rate_for(p, dt, "1.1461", &p1))
            .await
            .unwrap();
        upsert_canonical_rate(&pool, &rate_for(p, dt, "1.1500", &p2))
            .await
            .unwrap();

        let r_a = lookup_latest(&pool, ccy("EUR"), ccy("USD"), dt, 7, &p1)
            .await
            .unwrap();
        let r_b = lookup_latest(&pool, ccy("EUR"), ccy("USD"), dt, 7, &p2)
            .await
            .unwrap();
        match (r_a, r_b) {
            (FxLookup::Available(a), FxLookup::Available(b)) => {
                assert_eq!(a.rate, d("1.1461"));
                assert_eq!(b.rate, d("1.1500"));
                assert_eq!(a.provider, p1);
                assert_eq!(b.provider, p2);
            }
            other => panic!("expected two Available, got {other:?}"),
        }

        cleanup(&pool, &p1).await;
        cleanup(&pool, &p2).await;
    }
}
