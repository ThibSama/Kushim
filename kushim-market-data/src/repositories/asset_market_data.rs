use crate::domain::CurrentQuote;
use sqlx::PgPool;
use uuid::Uuid;

/// Upsert the current market data row for `id_asset`.
///
/// Conflict policy — observable idempotence:
///
/// - First insert always wins.
/// - Strictly older `as_of` → ignored (no-op, `rows_affected = 0`).
/// - Same `as_of` with **identical** `price_minor`, `currency`, `data_source`
///   → ignored, so a deterministic replay never advances `updated_at`
///   (`fetched_at` exposed at the API stays stable).
/// - Same `as_of` with at least one **changed** meaningful field
///   (price/currency/data_source) → accepted as a correction.
/// - Strictly newer `as_of` → accepted unconditionally.
///
/// Returns the number of rows actually written. The job uses this to count
/// real updates vs. silent no-ops.
pub async fn upsert_current(
    pool: &PgPool,
    id_asset: Uuid,
    quote: &CurrentQuote,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO asset_market_data (id_asset, price_minor, currency, data_source, as_of)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (id_asset) DO UPDATE SET
            price_minor = EXCLUDED.price_minor,
            currency = EXCLUDED.currency,
            data_source = EXCLUDED.data_source,
            as_of = EXCLUDED.as_of,
            updated_at = now()
        WHERE
            EXCLUDED.as_of > asset_market_data.as_of
            OR (
                EXCLUDED.as_of = asset_market_data.as_of
                AND (
                    EXCLUDED.price_minor IS DISTINCT FROM asset_market_data.price_minor
                    OR EXCLUDED.currency  IS DISTINCT FROM asset_market_data.currency
                    OR EXCLUDED.data_source IS DISTINCT FROM asset_market_data.data_source
                )
            )
        "#,
    )
    .bind(id_asset)
    .bind(quote.price_minor)
    .bind(&quote.currency)
    .bind(&quote.data_source)
    .bind(quote.as_of)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::upsert_current;
    use crate::{
        domain::CurrentQuote,
        test_utils::{TEST_SYMBOL_PREFIX_UNSUPPORTED, lock_env, unique_test_symbol},
    };
    use sqlx::{PgPool, Row, postgres::PgPoolOptions};
    use time::{Duration, OffsetDateTime};
    use uuid::Uuid;

    async fn pool() -> PgPool {
        let url = {
            let _g = lock_env();
            std::env::var("DATABASE_URL").unwrap_or_default()
        };
        assert!(!url.is_empty(), "DATABASE_URL must be set");
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("test db must be reachable")
    }

    // Fixture asset for the direct `upsert_current` repository tests.
    //
    // These tests exercise the conflict guard by calling `upsert_current`
    // directly with an explicit `id_asset`, so the fixture never needs to be
    // active or provider-supported. It is deliberately created so a concurrent
    // `refresh_current_market_data` job test (which runs the real job against
    // the SHARED CI database and sweeps ALL active, provider-supported assets)
    // can never select it and rewrite this row mid-test:
    //   - `status = 'inactive'` → excluded from `list_active_assets`;
    //   - `TEST_SYMBOL_PREFIX_UNSUPPORTED` symbol → the test provider returns
    //     `None`, so even a future scan that ignored status would skip it.
    // Without this isolation, the sweep wrote a newer `now_utc()` quote over the
    // fixture, advancing `updated_at` / changing the price and intermittently
    // breaking the idempotence assertions. The production conflict SQL is
    // unchanged — this only fixes test-fixture visibility, not a real race.
    async fn insert_asset(pool: &PgPool) -> Uuid {
        let id = Uuid::new_v4();
        let symbol = unique_test_symbol(TEST_SYMBOL_PREFIX_UNSUPPORTED);
        sqlx::query(
            r#"
            INSERT INTO assets (id_asset, asset_class, status, name, native_currency, symbol)
            VALUES ($1, 'equity', 'inactive', $2, 'USD', $3)
            "#,
        )
        .bind(id)
        .bind(format!("upsert_guard_{}", &id.simple().to_string()[..8]))
        .bind(symbol)
        .execute(pool)
        .await
        .expect("asset row should insert");
        id
    }

    async fn cleanup(pool: &PgPool, id: Uuid) {
        sqlx::query("DELETE FROM asset_market_data WHERE id_asset = $1")
            .bind(id)
            .execute(pool)
            .await
            .ok();
        sqlx::query("DELETE FROM assets WHERE id_asset = $1")
            .bind(id)
            .execute(pool)
            .await
            .ok();
    }

    async fn current_row(pool: &PgPool, id: Uuid) -> (i64, String, OffsetDateTime) {
        let row = sqlx::query(
            "SELECT price_minor, data_source, as_of FROM asset_market_data WHERE id_asset = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("row should exist");
        (
            row.try_get("price_minor").unwrap(),
            row.try_get::<Option<String>, _>("data_source")
                .unwrap()
                .unwrap_or_default(),
            row.try_get("as_of").unwrap(),
        )
    }

    async fn updated_at(pool: &PgPool, id: Uuid) -> OffsetDateTime {
        sqlx::query_scalar::<_, OffsetDateTime>(
            "SELECT updated_at FROM asset_market_data WHERE id_asset = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("updated_at should be readable")
    }

    /// Truncate sub-microsecond digits so equality holds round-tripping through
    /// `timestamptz` (Postgres stores microsecond precision; `OffsetDateTime`
    /// keeps nanoseconds).
    fn trunc_to_micros(value: OffsetDateTime) -> OffsetDateTime {
        let nanos = value.nanosecond();
        value
            .replace_nanosecond(nanos / 1_000 * 1_000)
            .expect("nanosecond truncation must stay within bounds")
    }

    #[tokio::test]
    async fn older_incoming_quote_does_not_overwrite_newer_row() {
        let pool = pool().await;
        let id = insert_asset(&pool).await;
        let newer = trunc_to_micros(OffsetDateTime::now_utc());
        let older = newer - Duration::hours(2);

        let written = upsert_current(
            &pool,
            id,
            &CurrentQuote {
                price_minor: 12_500,
                currency: "USD".into(),
                data_source: "provider-newer".into(),
                as_of: newer,
            },
        )
        .await
        .expect("first upsert should succeed");
        assert_eq!(written, 1);

        let skipped = upsert_current(
            &pool,
            id,
            &CurrentQuote {
                price_minor: 999,
                currency: "USD".into(),
                data_source: "provider-older".into(),
                as_of: older,
            },
        )
        .await
        .expect("stale upsert should not error");
        assert_eq!(skipped, 0, "older incoming as_of must not overwrite");

        let (price, source, stored_as_of) = current_row(&pool, id).await;
        assert_eq!(price, 12_500);
        assert_eq!(source, "provider-newer");
        assert_eq!(stored_as_of, newer);

        cleanup(&pool, id).await;
    }

    #[tokio::test]
    async fn newer_incoming_quote_overwrites_older_row() {
        let pool = pool().await;
        let id = insert_asset(&pool).await;
        let now = trunc_to_micros(OffsetDateTime::now_utc());
        let older = now - Duration::hours(2);
        let newer = now;

        let written = upsert_current(
            &pool,
            id,
            &CurrentQuote {
                price_minor: 100,
                currency: "USD".into(),
                data_source: "provider-older".into(),
                as_of: older,
            },
        )
        .await
        .expect("first upsert should succeed");
        assert_eq!(written, 1);

        let written2 = upsert_current(
            &pool,
            id,
            &CurrentQuote {
                price_minor: 22_222,
                currency: "USD".into(),
                data_source: "provider-newer".into(),
                as_of: newer,
            },
        )
        .await
        .expect("second upsert should succeed");
        assert_eq!(written2, 1);

        let (price, source, stored_as_of) = current_row(&pool, id).await;
        assert_eq!(price, 22_222);
        assert_eq!(source, "provider-newer");
        assert_eq!(stored_as_of, newer);

        cleanup(&pool, id).await;
    }

    #[tokio::test]
    async fn identical_repeated_upsert_does_not_rewrite_or_advance_updated_at() {
        let pool = pool().await;
        let id = insert_asset(&pool).await;
        let when = trunc_to_micros(OffsetDateTime::now_utc());
        let quote = CurrentQuote {
            price_minor: 7_777,
            currency: "USD".into(),
            data_source: "provider-stable".into(),
            as_of: when,
        };

        assert_eq!(upsert_current(&pool, id, &quote).await.unwrap(), 1);
        let updated_after_first = updated_at(&pool, id).await;

        // Deterministic replay: same as_of and identical payload must not
        // touch the row — `rows_affected` is 0 and `updated_at` is unchanged.
        // Sleep just enough that `now()` would tick if the guard were absent.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(
            upsert_current(&pool, id, &quote).await.unwrap(),
            0,
            "identical replay must not rewrite the row"
        );

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM asset_market_data WHERE id_asset = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);

        let (price, _, _) = current_row(&pool, id).await;
        assert_eq!(price, 7_777);
        let updated_after_second = updated_at(&pool, id).await;
        assert_eq!(
            updated_after_first, updated_after_second,
            "updated_at must not advance on an identical replay"
        );

        cleanup(&pool, id).await;
    }

    #[tokio::test]
    async fn same_as_of_with_changed_payload_is_accepted_as_correction() {
        let pool = pool().await;
        let id = insert_asset(&pool).await;
        let when = trunc_to_micros(OffsetDateTime::now_utc());

        assert_eq!(
            upsert_current(
                &pool,
                id,
                &CurrentQuote {
                    price_minor: 1_000,
                    currency: "USD".into(),
                    data_source: "provider-a".into(),
                    as_of: when,
                }
            )
            .await
            .unwrap(),
            1
        );
        let updated_after_first = updated_at(&pool, id).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Same as_of, different price_minor: the row is a correction and must
        // be accepted. updated_at must advance because the payload changed.
        assert_eq!(
            upsert_current(
                &pool,
                id,
                &CurrentQuote {
                    price_minor: 2_000,
                    currency: "USD".into(),
                    data_source: "provider-a".into(),
                    as_of: when,
                }
            )
            .await
            .unwrap(),
            1,
            "same-as_of correction with changed payload must be written"
        );

        let (price, _, stored_as_of) = current_row(&pool, id).await;
        assert_eq!(price, 2_000);
        assert_eq!(stored_as_of, when);
        let updated_after_second = updated_at(&pool, id).await;
        assert!(
            updated_after_second > updated_after_first,
            "updated_at must advance when the payload actually changed"
        );

        // A change in data_source alone is also a correction.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(
            upsert_current(
                &pool,
                id,
                &CurrentQuote {
                    price_minor: 2_000,
                    currency: "USD".into(),
                    data_source: "provider-b".into(),
                    as_of: when,
                }
            )
            .await
            .unwrap(),
            1
        );
        let (_, source, _) = current_row(&pool, id).await;
        assert_eq!(source, "provider-b");

        cleanup(&pool, id).await;
    }
}
