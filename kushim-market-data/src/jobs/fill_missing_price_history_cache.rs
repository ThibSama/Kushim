use crate::{
    errors::MarketDataError,
    jobs::Job,
    providers::MarketDataProvider,
    repositories::{assets, price_history_cache},
    state::AppState,
    symbol_filter::{SymbolAllowlist, select_assets_by_allowlist},
};
use time::Date;

pub struct FillMissingPriceHistoryCacheJob<P: MarketDataProvider> {
    provider: P,
    date_from: Date,
    date_to: Date,
    symbol_allowlist: Option<SymbolAllowlist>,
}

impl<P: MarketDataProvider> FillMissingPriceHistoryCacheJob<P> {
    pub fn new(provider: P, date_from: Date, date_to: Date) -> Self {
        Self {
            provider,
            date_from,
            date_to,
            symbol_allowlist: None,
        }
    }

    pub fn new_with_symbol_allowlist(
        provider: P,
        date_from: Date,
        date_to: Date,
        symbol_allowlist: SymbolAllowlist,
    ) -> Self {
        Self {
            provider,
            date_from,
            date_to,
            symbol_allowlist: Some(symbol_allowlist),
        }
    }
}

impl<P: MarketDataProvider> Job for FillMissingPriceHistoryCacheJob<P> {
    fn name(&self) -> &'static str {
        "fill_missing_price_history_cache"
    }

    async fn run(&self, state: &AppState) -> Result<(), MarketDataError> {
        let active_assets = assets::list_active_assets(&state.pg_pool)
            .await
            .map_err(MarketDataError::Database)?;

        let total_assets = active_assets.len();
        let selection = select_assets_by_allowlist(&active_assets, self.symbol_allowlist.as_ref());
        let selected_assets = selection.selected.len();
        let mut inserted = 0_usize;
        let mut already_present = 0_usize;
        let mut skipped = 0_usize;
        let mut provider_errors = 0_usize;

        for symbol in selection.missing_symbols {
            tracing::warn!(
                symbol,
                "allowlisted symbol is not present in active assets, skipping"
            );
        }

        for asset in &selection.selected {
            let mut current = self.date_from;
            loop {
                match self.provider.get_historical_quote(asset, current).await {
                    Ok(Some(quote)) => {
                        match price_history_cache::insert_if_missing(
                            &state.pg_pool,
                            asset.id_asset,
                            &quote,
                        )
                        .await
                        {
                            Ok(true) => inserted += 1,
                            Ok(false) => already_present += 1,
                            Err(sqlx::Error::Database(db_err))
                                if db_err.code().as_deref() == Some("23503") =>
                            {
                                tracing::warn!(
                                    id_asset = %asset.id_asset,
                                    "asset deleted between read and write, skipping"
                                );
                                skipped += 1;
                                break;
                            }
                            Err(e) => return Err(MarketDataError::Database(e)),
                        }
                    }
                    Ok(None) => {
                        skipped += 1;
                        break;
                    }
                    Err(e) => {
                        provider_errors += 1;
                        tracing::warn!(
                            id_asset = %asset.id_asset,
                            price_date = %current,
                            error = %e,
                            "provider historical lookup failed, skipping asset"
                        );
                        break;
                    }
                }

                match current.next_day() {
                    Some(next) if next <= self.date_to => current = next,
                    _ => break,
                }
            }
        }

        let date_count = date_range_len(self.date_from, self.date_to);

        tracing::info!(
            job = self.name(),
            provider = self.provider.name(),
            total_assets,
            selected_assets,
            date_count,
            inserted,
            already_present,
            skipped,
            provider_errors,
            "fill_missing_price_history_cache completed"
        );

        Ok(())
    }
}

fn date_range_len(from: Date, to: Date) -> i64 {
    (to - from).whole_days() + 1
}

#[cfg(test)]
mod tests {
    use super::FillMissingPriceHistoryCacheJob;
    use crate::{
        jobs::Job,
        state::AppState,
        test_utils::{
            TEST_SYMBOL_PREFIX_HISTORY, TEST_SYMBOL_PREFIX_UNSUPPORTED, lock_env,
            providers::DeterministicTestProvider, unique_test_symbol,
        },
    };
    use sqlx::{PgPool, Row};
    use time::{Date, Month};
    use uuid::Uuid;

    async fn test_pool() -> PgPool {
        let database_url = {
            let _guard = lock_env();
            std::env::var("DATABASE_URL").unwrap_or_default()
        };

        assert!(
            !database_url.is_empty(),
            "DATABASE_URL must be set for integration tests"
        );

        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    /// Insert a temporary catalogue row using a technical symbol that no
    /// real provider allowlist resolves. Even if a test panics before its
    /// cleanup runs, the leftover row cannot collide with AAPL/MSFT/NVDA
    /// or any other canonical provider symbol.
    async fn create_test_asset(
        pool: &PgPool,
        symbol: Option<&str>,
        ticker: Option<&str>,
        exchange: Option<&str>,
        status: &str,
    ) -> Uuid {
        let id_asset = Uuid::new_v4();
        let name = format!(
            "test_history_{}",
            symbol
                .or(ticker)
                .unwrap_or(&id_asset.simple().to_string()[..8])
        );

        sqlx::query(
            r#"
            INSERT INTO assets (id_asset, asset_class, status, name, native_currency, symbol, ticker, exchange)
            VALUES ($1, 'equity', $2, $3, 'USD', $4, $5, $6)
            "#,
        )
        .bind(id_asset)
        .bind(status)
        .bind(&name)
        .bind(symbol)
        .bind(ticker)
        .bind(exchange)
        .execute(pool)
        .await
        .expect("asset should be inserted");

        id_asset
    }

    async fn cleanup_test_asset(pool: &PgPool, id_asset: Uuid) {
        sqlx::query("DELETE FROM asset_price_history_cache WHERE id_asset = $1")
            .bind(id_asset)
            .execute(pool)
            .await
            .ok();
        sqlx::query("DELETE FROM asset_market_data WHERE id_asset = $1")
            .bind(id_asset)
            .execute(pool)
            .await
            .ok();
        sqlx::query("DELETE FROM assets WHERE id_asset = $1")
            .bind(id_asset)
            .execute(pool)
            .await
            .ok();
    }

    async fn count_history_rows_in_range(
        pool: &PgPool,
        id_asset: Uuid,
        from: Date,
        to: Date,
    ) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM asset_price_history_cache WHERE id_asset = $1 AND price_date >= $2 AND price_date <= $3",
        )
        .bind(id_asset)
        .bind(from)
        .bind(to)
        .fetch_one(pool)
        .await
        .expect("count should succeed")
    }

    async fn get_history_close(pool: &PgPool, id_asset: Uuid, date: Date) -> Option<i64> {
        sqlx::query(
            "SELECT close_minor FROM asset_price_history_cache WHERE id_asset = $1 AND price_date = $2",
        )
        .bind(id_asset)
        .bind(date)
        .fetch_optional(pool)
        .await
        .expect("query should succeed")
        .map(|row| row.get("close_minor"))
    }

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
    }

    #[tokio::test]
    async fn job_inserts_missing_rows() {
        let pool = test_pool().await;
        let symbol = unique_test_symbol(TEST_SYMBOL_PREFIX_HISTORY);
        let id = create_test_asset(&pool, Some(&symbol), None, None, "active").await;

        let job = FillMissingPriceHistoryCacheJob::new(
            DeterministicTestProvider,
            date(2026, 1, 1),
            date(2026, 1, 3),
        );
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("job should succeed");

        assert_eq!(
            count_history_rows_in_range(&pool, id, date(2026, 1, 1), date(2026, 1, 3)).await,
            3
        );

        let close = get_history_close(&pool, id, date(2026, 1, 1)).await;
        assert!(close.is_some());
        assert!(close.unwrap() > 0);

        cleanup_test_asset(&pool, id).await;
    }

    #[tokio::test]
    async fn job_is_idempotent() {
        let pool = test_pool().await;
        let symbol = unique_test_symbol(TEST_SYMBOL_PREFIX_HISTORY);
        let id = create_test_asset(&pool, Some(&symbol), None, None, "active").await;

        let job = FillMissingPriceHistoryCacheJob::new(
            DeterministicTestProvider,
            date(2026, 2, 1),
            date(2026, 2, 3),
        );
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("first run should succeed");
        let close_first = get_history_close(&pool, id, date(2026, 2, 1))
            .await
            .unwrap();

        job.run(&state).await.expect("second run should succeed");
        let close_second = get_history_close(&pool, id, date(2026, 2, 1))
            .await
            .unwrap();

        assert_eq!(
            count_history_rows_in_range(&pool, id, date(2026, 2, 1), date(2026, 2, 3)).await,
            3
        );
        assert_eq!(close_first, close_second);

        cleanup_test_asset(&pool, id).await;
    }

    #[tokio::test]
    async fn job_skips_unsupported_asset() {
        let pool = test_pool().await;
        let symbol = unique_test_symbol(TEST_SYMBOL_PREFIX_UNSUPPORTED);
        let id = create_test_asset(&pool, Some(&symbol), None, None, "active").await;

        let job = FillMissingPriceHistoryCacheJob::new(
            DeterministicTestProvider,
            date(2026, 1, 1),
            date(2026, 1, 3),
        );
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("job should succeed");

        assert_eq!(
            count_history_rows_in_range(&pool, id, date(2026, 1, 1), date(2026, 1, 3)).await,
            0
        );

        cleanup_test_asset(&pool, id).await;
    }

    #[tokio::test]
    async fn job_skips_inactive_assets() {
        let pool = test_pool().await;
        let symbol = unique_test_symbol(TEST_SYMBOL_PREFIX_HISTORY);
        let id = create_test_asset(&pool, Some(&symbol), None, None, "inactive").await;

        let job = FillMissingPriceHistoryCacheJob::new(
            DeterministicTestProvider,
            date(2026, 3, 1),
            date(2026, 3, 3),
        );
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("job should succeed");

        assert_eq!(
            count_history_rows_in_range(&pool, id, date(2026, 3, 1), date(2026, 3, 3)).await,
            0
        );

        cleanup_test_asset(&pool, id).await;
    }

    #[tokio::test]
    async fn existing_rows_remain_untouched() {
        let pool = test_pool().await;
        let symbol = unique_test_symbol(TEST_SYMBOL_PREFIX_HISTORY);
        let id = create_test_asset(&pool, Some(&symbol), None, None, "active").await;
        let target_date = date(2026, 5, 10);

        sqlx::query(
            r#"
            INSERT INTO asset_price_history_cache
                (id_asset, price_date, currency, close_minor, source)
            VALUES ($1, $2, 'USD', 999_999, 'test-static')
            "#,
        )
        .bind(id)
        .bind(target_date)
        .execute(&pool)
        .await
        .expect("seed row should be inserted");

        let job = FillMissingPriceHistoryCacheJob::new(
            DeterministicTestProvider,
            date(2026, 5, 10),
            date(2026, 5, 10),
        );
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("job should succeed");

        let close = get_history_close(&pool, id, target_date).await.unwrap();
        assert_eq!(close, 999_999, "pre-existing row must not be overwritten");

        cleanup_test_asset(&pool, id).await;
    }
}
