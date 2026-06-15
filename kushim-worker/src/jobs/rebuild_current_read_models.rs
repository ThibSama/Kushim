use crate::{
    calculation::portfolio_replay::rebuild_portfolio_state, config::Config, errors::WorkerError,
    jobs::Job, repositories::read_model_rebuild::ReadModelRebuildRepository, state::AppState,
};
use async_trait::async_trait;
use std::collections::HashSet;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RebuildCurrentReadModelsJob {
    target_portfolio_id: Option<Uuid>,
}

impl RebuildCurrentReadModelsJob {
    pub fn from_config(config: &Config) -> Self {
        Self {
            target_portfolio_id: config.target_portfolio_id,
        }
    }

    /// Build a rebuild job scoped to a single portfolio, used by the
    /// per-request refresh consumer.
    pub fn for_portfolio(id_portfolio: Uuid) -> Self {
        Self {
            target_portfolio_id: Some(id_portfolio),
        }
    }

    pub fn target_portfolio_id(&self) -> Option<Uuid> {
        self.target_portfolio_id
    }
}

#[async_trait]
impl Job for RebuildCurrentReadModelsJob {
    fn name(&self) -> &'static str {
        "rebuild_current_read_models"
    }

    async fn run(&self, state: &AppState) -> Result<(), WorkerError> {
        tracing::info!(
            worker = %state.worker_name,
            job = self.name(),
            target_portfolio_id = ?self.target_portfolio_id,
            "starting rebuild current portfolio read models job"
        );

        let repository = ReadModelRebuildRepository::new(state.pg_pool.clone());
        let portfolios = repository
            .list_portfolios_for_rebuild(self.target_portfolio_id)
            .await?;

        let run_started_at = OffsetDateTime::now_utc();
        let mut rebuilt_count = 0_usize;

        for portfolio in portfolios {
            let operations = repository
                .list_posted_operations_for_portfolio(portfolio.id_portfolio)
                .await?;

            let asset_ids: Vec<Uuid> = operations
                .iter()
                .flat_map(|operation| [operation.id_asset, operation.id_related_asset])
                .flatten()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            let market_data = repository.find_market_data_for_assets(&asset_ids).await?;
            let rebuilt = rebuild_portfolio_state(
                portfolio.clone(),
                &operations,
                &market_data,
                run_started_at,
            )?;

            repository
                .replace_read_models_for_portfolio(&rebuilt)
                .await?;

            tracing::info!(
                worker = %state.worker_name,
                job = self.name(),
                id_portfolio = %rebuilt.summary.id_portfolio,
                holdings_count = rebuilt.holdings.len(),
                total_value_minor = rebuilt.summary.total_value_minor,
                is_estimated = rebuilt.summary.is_estimated,
                "rebuilt current portfolio read models"
            );
            rebuilt_count += 1;
        }

        tracing::info!(
            worker = %state.worker_name,
            job = self.name(),
            rebuilt_count,
            "completed rebuild current portfolio read models job"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RebuildCurrentReadModelsJob;
    use crate::{
        config::Config, jobs::Job, repositories::read_model_rebuild::ReadModelRebuildRepository,
        state::AppState, test_utils::lock_env,
    };
    use sqlx::{PgPool, Row};
    use time::{Duration, OffsetDateTime};
    use uuid::Uuid;

    async fn test_pool() -> PgPool {
        let database_url = {
            let _guard = lock_env();
            std::env::var("DATABASE_URL").unwrap_or_default()
        };

        assert!(
            !database_url.is_empty(),
            "DATABASE_URL must be set for worker integration tests"
        );

        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    async fn ensure_role(pool: &PgPool) {
        sqlx::query(
            r#"
            INSERT INTO roles (id_role, label)
            VALUES (1, 'user')
            ON CONFLICT (id_role) DO UPDATE SET label = EXCLUDED.label
            "#,
        )
        .execute(pool)
        .await
        .expect("role should exist");
    }

    async fn create_user(pool: &PgPool, suffix: &str) -> Uuid {
        ensure_role(pool).await;
        let id_user = Uuid::new_v4();
        let handle = format!("wrk{}", suffix);

        sqlx::query(
            r#"
            INSERT INTO users (id_user, id_role, username, public_handle, password_hash)
            VALUES ($1, 1, $2, $3, '$argon2id$worker')
            "#,
        )
        .bind(id_user)
        .bind(&handle)
        .bind(&handle)
        .execute(pool)
        .await
        .expect("user should be inserted");

        id_user
    }

    async fn create_portfolio(
        pool: &PgPool,
        id_user: Uuid,
        base_currency: &str,
        deleted_at: Option<OffsetDateTime>,
    ) -> Uuid {
        let id_portfolio = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO portfolios (id_portfolio, id_user, name, base_currency, visibility, deleted_at)
            VALUES ($1, $2, $3, $4, 'private', $5)
            "#,
        )
        .bind(id_portfolio)
        .bind(id_user)
        .bind(format!("pf{}", &id_portfolio.simple().to_string()[..12]))
        .bind(base_currency)
        .bind(deleted_at)
        .execute(pool)
        .await
        .expect("portfolio should be inserted");

        id_portfolio
    }

    async fn create_asset(pool: &PgPool, suffix: &str) -> Uuid {
        let id_asset = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO assets (id_asset, asset_class, status, name, native_currency, symbol)
            VALUES ($1, 'equity', 'active', $2, 'EUR', $3)
            "#,
        )
        .bind(id_asset)
        .bind(format!("Worker Asset {suffix}"))
        .bind(format!("W{suffix}"))
        .execute(pool)
        .await
        .expect("asset should be inserted");

        id_asset
    }

    async fn insert_asset_market_data(pool: &PgPool, id_asset: Uuid, price_minor: i64) {
        sqlx::query(
            r#"
            INSERT INTO asset_market_data (id_asset, price_minor, currency, as_of, data_source)
            VALUES ($1, $2, 'EUR', $3, 'worker-test')
            ON CONFLICT (id_asset) DO UPDATE
            SET
                price_minor = EXCLUDED.price_minor,
                currency = EXCLUDED.currency,
                as_of = EXCLUDED.as_of,
                data_source = EXCLUDED.data_source
            "#,
        )
        .bind(id_asset)
        .bind(price_minor)
        .bind(OffsetDateTime::now_utc())
        .execute(pool)
        .await
        .expect("market data should be upserted");
    }

    struct OperationFixture<'a> {
        id_asset: Option<Uuid>,
        id_related_asset: Option<Uuid>,
        operation_type: &'a str,
        operation_status: &'a str,
        quantity: Option<&'a str>,
        related_quantity: Option<&'a str>,
        price_minor: Option<i64>,
        gross_amount_minor: Option<i64>,
        cash_amount_minor: i64,
        currency: &'a str,
        fx_rate_to_portfolio: Option<&'a str>,
        executed_at: OffsetDateTime,
    }

    async fn insert_operation(
        pool: &PgPool,
        id_portfolio: Uuid,
        fixture: OperationFixture<'_>,
    ) -> Uuid {
        let id_portfolio_operation = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO portfolio_operations (
                id_portfolio_operation,
                id_portfolio,
                id_asset,
                id_related_asset,
                operation_type,
                operation_status,
                executed_at,
                quantity,
                related_quantity,
                price_minor,
                gross_amount_minor,
                fees_minor,
                taxes_minor,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio,
                metadata
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7,
                $8::numeric, $9::numeric, $10, $11, NULL, NULL, $12, $13, $14::numeric, '{}'::jsonb
            )
            "#,
        )
        .bind(id_portfolio_operation)
        .bind(id_portfolio)
        .bind(fixture.id_asset)
        .bind(fixture.id_related_asset)
        .bind(fixture.operation_type)
        .bind(fixture.operation_status)
        .bind(fixture.executed_at)
        .bind(fixture.quantity)
        .bind(fixture.related_quantity)
        .bind(fixture.price_minor)
        .bind(fixture.gross_amount_minor)
        .bind(fixture.cash_amount_minor)
        .bind(fixture.currency)
        .bind(fixture.fx_rate_to_portfolio)
        .execute(pool)
        .await
        .expect("operation should be inserted");

        id_portfolio_operation
    }

    async fn test_state(pool: PgPool) -> AppState {
        AppState {
            pg_pool: pool,
            worker_name: "worker-test".to_string(),
        }
    }

    fn job_with_target(id_portfolio: Option<Uuid>) -> RebuildCurrentReadModelsJob {
        RebuildCurrentReadModelsJob {
            target_portfolio_id: id_portfolio,
        }
    }

    async fn fetch_summary(
        pool: &PgPool,
        id_portfolio: Uuid,
    ) -> Option<(i64, i64, i64, i64, String, bool)> {
        sqlx::query(
            r#"
            SELECT total_value_minor, cash_balance_minor, total_invested_minor, total_pnl_minor, portfolio_status, is_estimated
            FROM rm_portfolio_summary
            WHERE id_portfolio = $1
            "#,
        )
        .bind(id_portfolio)
        .fetch_optional(pool)
        .await
        .expect("summary query should succeed")
        .map(|row| {
            (
                row.get("total_value_minor"),
                row.get("cash_balance_minor"),
                row.get("total_invested_minor"),
                row.get("total_pnl_minor"),
                row.get::<String, _>("portfolio_status"),
                row.get("is_estimated"),
            )
        })
    }

    async fn fetch_holdings_count(pool: &PgPool, id_portfolio: Uuid) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM rm_portfolio_holdings WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .fetch_one(pool)
            .await
            .expect("holdings count should succeed")
    }

    #[tokio::test]
    async fn rebuild_empty_portfolio_creates_zero_summary_and_no_holdings() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", None).await;

        job_with_target(Some(id_portfolio))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("rebuild should succeed");

        let summary = fetch_summary(&pool, id_portfolio)
            .await
            .expect("summary should exist");
        assert_eq!(summary.0, 0);
        assert_eq!(summary.1, 0);
        assert_eq!(summary.2, 0);
        assert_eq!(summary.3, 0);
        assert_eq!(summary.4, "empty");
        assert_eq!(fetch_holdings_count(&pool, id_portfolio).await, 0);
    }

    #[tokio::test]
    async fn rebuild_deposit_only_creates_cash_summary() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", None).await;
        insert_operation(
            &pool,
            id_portfolio,
            OperationFixture {
                id_asset: None,
                id_related_asset: None,
                operation_type: "deposit",
                operation_status: "posted",
                quantity: None,
                related_quantity: None,
                price_minor: None,
                gross_amount_minor: Some(1_000),
                cash_amount_minor: 1_000,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc(),
            },
        )
        .await;

        job_with_target(Some(id_portfolio))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("rebuild should succeed");

        let summary = fetch_summary(&pool, id_portfolio)
            .await
            .expect("summary should exist");
        assert_eq!(summary.0, 1_000);
        assert_eq!(summary.1, 1_000);
        assert_eq!(summary.2, 1_000);
        assert_eq!(summary.3, 0);
        assert_eq!(summary.4, "active");
    }

    #[tokio::test]
    async fn rebuild_buy_with_market_data_creates_summary_and_holding() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", None).await;
        let id_asset = create_asset(&pool, &Uuid::new_v4().simple().to_string()[..8]).await;
        insert_asset_market_data(&pool, id_asset, 400).await;

        insert_operation(
            &pool,
            id_portfolio,
            OperationFixture {
                id_asset: None,
                id_related_asset: None,
                operation_type: "deposit",
                operation_status: "posted",
                quantity: None,
                related_quantity: None,
                price_minor: None,
                gross_amount_minor: Some(1_000),
                cash_amount_minor: 1_000,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc() - Duration::days(1),
            },
        )
        .await;

        insert_operation(
            &pool,
            id_portfolio,
            OperationFixture {
                id_asset: Some(id_asset),
                id_related_asset: None,
                operation_type: "buy",
                operation_status: "posted",
                quantity: Some("2.0000000000"),
                related_quantity: None,
                price_minor: Some(300),
                gross_amount_minor: Some(600),
                cash_amount_minor: 600,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc(),
            },
        )
        .await;

        job_with_target(Some(id_portfolio))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("rebuild should succeed");

        let summary = fetch_summary(&pool, id_portfolio)
            .await
            .expect("summary should exist");
        assert_eq!(summary.0, 1_200);
        assert_eq!(summary.1, 400);
        assert_eq!(summary.2, 1_000);
        assert_eq!(summary.3, 200);
        assert!(!summary.5);

        let holding = sqlx::query(
            r#"
            SELECT quantity::text AS quantity, invested_base_minor, market_value_minor, pnl_base_minor
            FROM rm_portfolio_holdings
            WHERE id_portfolio = $1 AND id_asset = $2
            "#,
        )
        .bind(id_portfolio)
        .bind(id_asset)
        .fetch_one(&pool)
        .await
        .expect("holding should exist");

        assert_eq!(holding.get::<String, _>("quantity"), "2.0000000000");
        assert_eq!(holding.get::<i64, _>("invested_base_minor"), 600);
        assert_eq!(holding.get::<i64, _>("market_value_minor"), 800);
        assert_eq!(holding.get::<i64, _>("pnl_base_minor"), 200);
    }

    #[tokio::test]
    async fn rebuild_ignores_pending_and_cancelled_operations() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", None).await;

        insert_operation(
            &pool,
            id_portfolio,
            OperationFixture {
                id_asset: None,
                id_related_asset: None,
                operation_type: "deposit",
                operation_status: "posted",
                quantity: None,
                related_quantity: None,
                price_minor: None,
                gross_amount_minor: Some(100),
                cash_amount_minor: 100,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc() - Duration::hours(2),
            },
        )
        .await;
        insert_operation(
            &pool,
            id_portfolio,
            OperationFixture {
                id_asset: None,
                id_related_asset: None,
                operation_type: "deposit",
                operation_status: "pending",
                quantity: None,
                related_quantity: None,
                price_minor: None,
                gross_amount_minor: Some(200),
                cash_amount_minor: 200,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc() - Duration::hours(1),
            },
        )
        .await;
        insert_operation(
            &pool,
            id_portfolio,
            OperationFixture {
                id_asset: None,
                id_related_asset: None,
                operation_type: "fee",
                operation_status: "cancelled",
                quantity: None,
                related_quantity: None,
                price_minor: None,
                gross_amount_minor: Some(50),
                cash_amount_minor: 50,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc(),
            },
        )
        .await;

        job_with_target(Some(id_portfolio))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("rebuild should succeed");

        let summary = fetch_summary(&pool, id_portfolio)
            .await
            .expect("summary should exist");
        assert_eq!(summary.1, 100);
        assert_eq!(summary.2, 100);
    }

    #[tokio::test]
    async fn rebuild_removes_stale_holdings() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", None).await;
        let id_asset = create_asset(&pool, &Uuid::new_v4().simple().to_string()[..8]).await;

        sqlx::query(
            r#"
            INSERT INTO rm_portfolio_holdings (
                id_portfolio, id_asset, base_currency, quantity, invested_base_minor,
                market_value_minor, pnl_base_minor, position_status, is_estimated, as_of
            )
            VALUES ($1, $2, 'EUR', '1.0000000000'::numeric, 100, 100, 0, 'open', false, now())
            "#,
        )
        .bind(id_portfolio)
        .bind(id_asset)
        .execute(&pool)
        .await
        .expect("stale holding should be inserted");

        job_with_target(Some(id_portfolio))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("rebuild should succeed");

        assert_eq!(fetch_holdings_count(&pool, id_portfolio).await, 0);
    }

    #[tokio::test]
    async fn rebuild_is_idempotent() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", None).await;

        insert_operation(
            &pool,
            id_portfolio,
            OperationFixture {
                id_asset: None,
                id_related_asset: None,
                operation_type: "deposit",
                operation_status: "posted",
                quantity: None,
                related_quantity: None,
                price_minor: None,
                gross_amount_minor: Some(100),
                cash_amount_minor: 100,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc(),
            },
        )
        .await;

        let job = job_with_target(Some(id_portfolio));
        let state = test_state(pool.clone()).await;
        job.run(&state).await.expect("first rebuild should succeed");
        job.run(&state)
            .await
            .expect("second rebuild should succeed");

        let summaries: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM rm_portfolio_summary WHERE id_portfolio = $1")
                .bind(id_portfolio)
                .fetch_one(&pool)
                .await
                .expect("summary count should succeed");
        assert_eq!(summaries, 1);
    }

    #[tokio::test]
    async fn rebuild_does_not_write_snapshots_or_mutate_operations() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", None).await;
        let id_operation = insert_operation(
            &pool,
            id_portfolio,
            OperationFixture {
                id_asset: None,
                id_related_asset: None,
                operation_type: "deposit",
                operation_status: "posted",
                quantity: None,
                related_quantity: None,
                price_minor: None,
                gross_amount_minor: Some(100),
                cash_amount_minor: 100,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc(),
            },
        )
        .await;

        let snapshots_before: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_snapshots_daily WHERE id_portfolio = $1",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .expect("snapshot count should succeed");

        job_with_target(Some(id_portfolio))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("rebuild should succeed");

        let snapshots_after: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_snapshots_daily WHERE id_portfolio = $1",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .expect("snapshot count should succeed");
        assert_eq!(snapshots_before, snapshots_after);

        let operation = sqlx::query(
            "SELECT operation_status, cash_amount_minor FROM portfolio_operations WHERE id_portfolio_operation = $1",
        )
        .bind(id_operation)
        .fetch_one(&pool)
        .await
        .expect("operation should still exist");
        assert_eq!(operation.get::<String, _>("operation_status"), "posted");
        assert_eq!(operation.get::<i64, _>("cash_amount_minor"), 100);
    }

    #[tokio::test]
    async fn list_portfolios_for_rebuild_skips_soft_deleted_portfolios() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let active_portfolio = create_portfolio(&pool, id_user, "EUR", None).await;
        let deleted_portfolio = create_portfolio(
            &pool,
            id_user,
            "EUR",
            Some(OffsetDateTime::now_utc() + Duration::seconds(1)),
        )
        .await;

        let repository = ReadModelRebuildRepository::new(pool.clone());
        let portfolios = repository
            .list_portfolios_for_rebuild(None)
            .await
            .expect("portfolio listing should succeed");

        assert!(
            portfolios
                .iter()
                .any(|portfolio| portfolio.id_portfolio == active_portfolio)
        );
        assert!(
            !portfolios
                .iter()
                .any(|portfolio| portfolio.id_portfolio == deleted_portfolio)
        );
    }

    #[test]
    fn config_exposes_rebuild_job() {
        let _ = Config {
            database_url: "postgresql://localhost/kushim".into(),
            app_env: "test".into(),
            rust_log: "info".into(),
            worker_name: "worker".into(),
            worker_mode: crate::config::WorkerMode::Once,
            worker_job: crate::config::WorkerJob::RebuildCurrentReadModels,
            worker_poll_interval: std::time::Duration::from_secs(1),
            target_portfolio_id: None,
            snapshot_date: None,
            backfill_date_from: None,
            backfill_date_to: None,
            redis_url: None,
            health: None,
            refresh_consumer: crate::config::RefreshConsumerConfig::default(),
        };
    }
}
