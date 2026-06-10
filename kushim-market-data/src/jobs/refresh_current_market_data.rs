use crate::{
    errors::MarketDataError,
    jobs::Job,
    providers::MarketDataProvider,
    repositories::{asset_market_data, assets},
    state::AppState,
};

pub struct RefreshCurrentMarketDataJob<P: MarketDataProvider> {
    provider: P,
}

impl<P: MarketDataProvider> RefreshCurrentMarketDataJob<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }
}

impl<P: MarketDataProvider> Job for RefreshCurrentMarketDataJob<P> {
    fn name(&self) -> &'static str {
        "refresh_current_market_data"
    }

    async fn run(&self, state: &AppState) -> Result<(), MarketDataError> {
        let active_assets = assets::list_active_assets(&state.pg_pool)
            .await
            .map_err(MarketDataError::Database)?;

        let total = active_assets.len();
        let mut updated = 0_usize;
        let mut skipped = 0_usize;

        for asset in &active_assets {
            match self.provider.get_quote(asset) {
                Some(quote) => {
                    match asset_market_data::upsert_current(&state.pg_pool, asset.id_asset, &quote)
                        .await
                    {
                        Ok(()) => {
                            updated += 1;
                        }
                        Err(sqlx::Error::Database(db_err))
                            if db_err.code().as_deref() == Some("23503") =>
                        {
                            tracing::warn!(
                                id_asset = %asset.id_asset,
                                "asset deleted between read and write, skipping"
                            );
                            skipped += 1;
                        }
                        Err(e) => return Err(MarketDataError::Database(e)),
                    }
                }
                None => {
                    skipped += 1;
                }
            }
        }

        tracing::info!(
            job = self.name(),
            provider = self.provider.name(),
            total,
            updated,
            skipped,
            "refresh_current_market_data completed"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RefreshCurrentMarketDataJob;
    use crate::{jobs::Job, providers::mock::MockProvider, state::AppState, test_utils::lock_env};
    use sqlx::{PgPool, Row};
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

    async fn create_test_asset(
        pool: &PgPool,
        symbol: Option<&str>,
        ticker: Option<&str>,
        exchange: Option<&str>,
        status: &str,
    ) -> Uuid {
        let id_asset = Uuid::new_v4();
        let name = format!(
            "test_{}",
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

    async fn count_market_data_rows(pool: &PgPool, id_asset: Uuid) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM asset_market_data WHERE id_asset = $1")
            .bind(id_asset)
            .fetch_one(pool)
            .await
            .expect("count should succeed")
    }

    async fn get_market_data_price(pool: &PgPool, id_asset: Uuid) -> Option<i64> {
        sqlx::query("SELECT price_minor FROM asset_market_data WHERE id_asset = $1")
            .bind(id_asset)
            .fetch_optional(pool)
            .await
            .expect("query should succeed")
            .map(|row| row.get("price_minor"))
    }

    async fn get_market_data_source(pool: &PgPool, id_asset: Uuid) -> Option<String> {
        sqlx::query("SELECT data_source FROM asset_market_data WHERE id_asset = $1")
            .bind(id_asset)
            .fetch_optional(pool)
            .await
            .expect("query should succeed")
            .map(|row| row.get("data_source"))
    }

    #[tokio::test]
    async fn job_upserts_supported_asset() {
        let pool = test_pool().await;
        let id = create_test_asset(&pool, Some("AAPL"), None, None, "active").await;

        let job = RefreshCurrentMarketDataJob::new(MockProvider);
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("job should succeed");

        let price = get_market_data_price(&pool, id).await;
        assert_eq!(price, Some(19_523));

        let source = get_market_data_source(&pool, id).await;
        assert_eq!(source.as_deref(), Some("mock"));

        cleanup_test_asset(&pool, id).await;
    }

    #[tokio::test]
    async fn job_skips_unsupported_asset() {
        let pool = test_pool().await;
        let id = create_test_asset(&pool, Some("UNKNOWN_XYZ"), None, None, "active").await;

        let job = RefreshCurrentMarketDataJob::new(MockProvider);
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("job should succeed");

        assert_eq!(count_market_data_rows(&pool, id).await, 0);

        cleanup_test_asset(&pool, id).await;
    }

    #[tokio::test]
    async fn job_skips_inactive_assets() {
        let pool = test_pool().await;
        let id = create_test_asset(&pool, Some("MSFT"), None, None, "inactive").await;

        let job = RefreshCurrentMarketDataJob::new(MockProvider);
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("job should succeed");

        assert_eq!(count_market_data_rows(&pool, id).await, 0);

        cleanup_test_asset(&pool, id).await;
    }

    #[tokio::test]
    async fn job_is_idempotent() {
        let pool = test_pool().await;
        let id = create_test_asset(&pool, Some("NVDA"), None, None, "active").await;

        let job = RefreshCurrentMarketDataJob::new(MockProvider);
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("first run should succeed");
        job.run(&state).await.expect("second run should succeed");

        assert_eq!(count_market_data_rows(&pool, id).await, 1);
        assert_eq!(get_market_data_price(&pool, id).await, Some(87_640));

        cleanup_test_asset(&pool, id).await;
    }

    // NOTE: cross-table negative test (job_does_not_write_price_history) removed.
    // Concurrent fill_missing_price_history_cache integration tests write to
    // price_history_cache for all active supported assets, making the before/after
    // count assertion racy. The guarantee is structural: this job imports only
    // asset_market_data::upsert_current, never price_history_cache::insert_if_missing.

    #[tokio::test]
    async fn job_resolves_via_ticker_when_no_symbol() {
        let pool = test_pool().await;
        let id = create_test_asset(&pool, None, Some("VTI"), Some("NYSE"), "active").await;

        let job = RefreshCurrentMarketDataJob::new(MockProvider);
        let state = AppState {
            pg_pool: pool.clone(),
        };

        job.run(&state).await.expect("job should succeed");

        assert_eq!(get_market_data_price(&pool, id).await, Some(26_410));

        cleanup_test_asset(&pool, id).await;
    }
}
