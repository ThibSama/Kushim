use crate::{
    config::Config,
    errors::WorkerError,
    jobs::{
        Job, generate_daily_snapshots::GenerateDailySnapshotsJob,
        rebuild_current_read_models::RebuildCurrentReadModelsJob,
    },
    state::AppState,
};
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct RefreshCurrentPortfolioStateJob {
    rebuild_job: RebuildCurrentReadModelsJob,
    snapshot_job: GenerateDailySnapshotsJob,
}

impl RefreshCurrentPortfolioStateJob {
    pub fn from_config(config: &Config) -> Self {
        Self {
            rebuild_job: RebuildCurrentReadModelsJob::from_config(config),
            snapshot_job: GenerateDailySnapshotsJob::from_config(config),
        }
    }

    /// Build a composite refresh scoped to a single portfolio and snapshot
    /// date. Reuses the existing rebuild + snapshot jobs unchanged.
    pub fn for_portfolio(id_portfolio: uuid::Uuid, snapshot_date: time::Date) -> Self {
        Self {
            rebuild_job: RebuildCurrentReadModelsJob::for_portfolio(id_portfolio),
            snapshot_job: GenerateDailySnapshotsJob::for_portfolio_on(id_portfolio, snapshot_date),
        }
    }
}

async fn run_composite_steps(
    state: &AppState,
    rebuild_job: &dyn Job,
    snapshot_job: &dyn Job,
    target_portfolio_id: Option<uuid::Uuid>,
    snapshot_date: Option<time::Date>,
) -> Result<(), WorkerError> {
    tracing::info!(
        worker = %state.worker_name,
        job = "refresh_current_portfolio_state",
        target_portfolio_id = ?target_portfolio_id,
        snapshot_date = ?snapshot_date,
        "starting composite current portfolio state refresh job"
    );

    tracing::info!(
        worker = %state.worker_name,
        job = "refresh_current_portfolio_state",
        step = rebuild_job.name(),
        "starting composite rebuild step"
    );
    rebuild_job.run(state).await.map_err(|error| {
        tracing::error!(
            worker = %state.worker_name,
            job = "refresh_current_portfolio_state",
            step = rebuild_job.name(),
            error = %error,
            "composite job failed during rebuild step"
        );
        error
    })?;
    tracing::info!(
        worker = %state.worker_name,
        job = "refresh_current_portfolio_state",
        step = rebuild_job.name(),
        "completed composite rebuild step"
    );

    tracing::info!(
        worker = %state.worker_name,
        job = "refresh_current_portfolio_state",
        step = snapshot_job.name(),
        "starting composite snapshot step"
    );
    snapshot_job.run(state).await.map_err(|error| {
        tracing::error!(
            worker = %state.worker_name,
            job = "refresh_current_portfolio_state",
            step = snapshot_job.name(),
            error = %error,
            "composite job failed during snapshot step"
        );
        error
    })?;
    tracing::info!(
        worker = %state.worker_name,
        job = "refresh_current_portfolio_state",
        step = snapshot_job.name(),
        "completed composite snapshot step"
    );

    tracing::info!(
        worker = %state.worker_name,
        job = "refresh_current_portfolio_state",
        target_portfolio_id = ?target_portfolio_id,
        snapshot_date = ?snapshot_date,
        "completed composite current portfolio state refresh job"
    );

    Ok(())
}

#[async_trait]
impl Job for RefreshCurrentPortfolioStateJob {
    fn name(&self) -> &'static str {
        "refresh_current_portfolio_state"
    }

    async fn run(&self, state: &AppState) -> Result<(), WorkerError> {
        run_composite_steps(
            state,
            &self.rebuild_job,
            &self.snapshot_job,
            self.rebuild_job.target_portfolio_id(),
            Some(self.snapshot_job.effective_snapshot_date()),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::{RefreshCurrentPortfolioStateJob, run_composite_steps};
    use crate::{
        config::{Config, WorkerJob},
        errors::WorkerError,
        jobs::Job,
        state::AppState,
        test_utils::lock_env,
    };
    use async_trait::async_trait;
    use sqlx::{PgPool, Row, postgres::PgPoolOptions};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration as StdDuration,
    };
    use time::{Date, Duration, OffsetDateTime};
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

        PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    async fn create_user(pool: &PgPool, suffix: &str) -> Uuid {
        crate::test_utils::ensure_canonical_user_role(pool).await;
        let id_user = Uuid::new_v4();
        let handle = format!("cmp{}", suffix);

        sqlx::query(
            r#"
            INSERT INTO users (id_user, id_role, username, public_handle, password_hash)
            VALUES ($1, 1, $2, $3, '$argon2id$composite')
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
        deleted: bool,
    ) -> Uuid {
        let id_portfolio = Uuid::new_v4();
        let deleted_at = if deleted {
            Some(OffsetDateTime::now_utc() + Duration::seconds(1))
        } else {
            None
        };

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
        .bind(format!("Composite Asset {suffix}"))
        .bind(format!("C{suffix}"))
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

    fn test_state(pool: PgPool) -> AppState {
        AppState {
            pg_pool: pool,
            worker_name: "test-worker".to_string(),
        }
    }

    #[derive(Clone)]
    struct RecordingJob {
        name: &'static str,
        count: Arc<AtomicUsize>,
        fail: bool,
    }

    #[async_trait]
    impl Job for RecordingJob {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn run(&self, _state: &AppState) -> Result<(), WorkerError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(WorkerError::Job(format!("{} failed", self.name)))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn config_exposes_refresh_current_portfolio_state_job() {
        let config = Config {
            database_url: "postgresql://postgres:postgres@localhost:5432/test".to_string(),
            app_env: "test".to_string(),
            rust_log: "debug".to_string(),
            worker_name: "worker-test".to_string(),
            worker_mode: crate::config::WorkerMode::Once,
            worker_job: WorkerJob::RefreshCurrentPortfolioState,
            worker_poll_interval: StdDuration::from_secs(5),
            target_portfolio_id: None,
            snapshot_date: Some(Date::from_calendar_date(2026, time::Month::June, 6).unwrap()),
            backfill_date_from: None,
            backfill_date_to: None,
            redis_url: None,
            health: None,
            refresh_consumer: crate::config::RefreshConsumerConfig::default(),
        };

        let job = RefreshCurrentPortfolioStateJob::from_config(&config);
        assert_eq!(job.name(), "refresh_current_portfolio_state");
        assert_eq!(
            job.snapshot_job.effective_snapshot_date(),
            Date::from_calendar_date(2026, time::Month::June, 6).unwrap()
        );
    }

    #[tokio::test]
    async fn composite_job_stops_before_snapshot_when_rebuild_fails() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://postgres:postgres@localhost:5432/kushim")
            .expect("lazy pool should build");
        let state = test_state(pool);
        let rebuild_count = Arc::new(AtomicUsize::new(0));
        let snapshot_count = Arc::new(AtomicUsize::new(0));

        let result = run_composite_steps(
            &state,
            &RecordingJob {
                name: "rebuild_current_read_models",
                count: rebuild_count.clone(),
                fail: true,
            },
            &RecordingJob {
                name: "generate_daily_snapshots",
                count: snapshot_count.clone(),
                fail: false,
            },
            None,
            None,
        )
        .await;

        assert!(result.is_err(), "composite job should fail");
        assert_eq!(rebuild_count.load(Ordering::SeqCst), 1);
        assert_eq!(snapshot_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn composite_job_builds_read_models_and_daily_snapshots() {
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let id_user = create_user(&pool, &suffix[..10]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let id_asset = create_asset(&pool, &suffix[..6]).await;
        insert_asset_market_data(&pool, id_asset, 400).await;

        let deposit_id = insert_operation(
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
        insert_operation(
            &pool,
            id_portfolio,
            OperationFixture {
                id_asset: Some(id_asset),
                id_related_asset: None,
                operation_type: "buy",
                operation_status: "posted",
                quantity: Some("2"),
                related_quantity: None,
                price_minor: Some(300),
                gross_amount_minor: Some(600),
                cash_amount_minor: 600,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc() + Duration::seconds(1),
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
                operation_status: "pending",
                quantity: Some("10"),
                related_quantity: None,
                price_minor: Some(100),
                gross_amount_minor: Some(1_000),
                cash_amount_minor: 1_000,
                currency: "EUR",
                fx_rate_to_portfolio: None,
                executed_at: OffsetDateTime::now_utc() + Duration::seconds(2),
            },
        )
        .await;

        let snapshot_date = Date::from_calendar_date(2026, time::Month::June, 6).unwrap();
        let job = RefreshCurrentPortfolioStateJob::from_config(&Config {
            database_url: "postgresql://postgres:postgres@localhost:5432/test".to_string(),
            app_env: "test".to_string(),
            rust_log: "debug".to_string(),
            worker_name: "worker-test".to_string(),
            worker_mode: crate::config::WorkerMode::Once,
            worker_job: WorkerJob::RefreshCurrentPortfolioState,
            worker_poll_interval: StdDuration::from_secs(5),
            target_portfolio_id: Some(id_portfolio),
            snapshot_date: Some(snapshot_date),
            backfill_date_from: None,
            backfill_date_to: None,
            redis_url: None,
            health: None,
            refresh_consumer: crate::config::RefreshConsumerConfig::default(),
        });

        job.run(&test_state(pool.clone()))
            .await
            .expect("composite job should succeed");
        job.run(&test_state(pool.clone()))
            .await
            .expect("composite job rerun should stay idempotent");

        let summary = sqlx::query(
            "SELECT total_value_minor, cash_balance_minor, total_invested_minor, total_pnl_minor FROM rm_portfolio_summary WHERE id_portfolio = $1",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .expect("summary should exist");
        assert_eq!(summary.get::<i64, _>("total_value_minor"), 1_200);
        assert_eq!(summary.get::<i64, _>("cash_balance_minor"), 400);
        assert_eq!(summary.get::<i64, _>("total_invested_minor"), 1_000);
        assert_eq!(summary.get::<i64, _>("total_pnl_minor"), 200);

        let holdings = sqlx::query(
            "SELECT COUNT(*) AS count FROM rm_portfolio_holdings WHERE id_portfolio = $1",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .expect("holdings count should be available");
        assert_eq!(holdings.get::<i64, _>("count"), 1);

        let snapshot = sqlx::query(
            "SELECT COUNT(*) AS count FROM portfolio_snapshots_daily WHERE id_portfolio = $1 AND snapshot_date = $2",
        )
        .bind(id_portfolio)
        .bind(snapshot_date)
        .fetch_one(&pool)
        .await
        .expect("snapshot count should be available");
        assert_eq!(snapshot.get::<i64, _>("count"), 1);

        let snapshot_holdings = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM portfolio_holding_snapshot_daily hs
            JOIN portfolio_snapshots_daily s
                ON s.id_portfolio_snapshot_daily = hs.id_portfolio_snapshot_daily
            WHERE s.id_portfolio = $1 AND s.snapshot_date = $2
            "#,
        )
        .bind(id_portfolio)
        .bind(snapshot_date)
        .fetch_one(&pool)
        .await
        .expect("snapshot holding count should be available");
        assert_eq!(snapshot_holdings.get::<i64, _>("count"), 1);

        let operations = sqlx::query(
            "SELECT COUNT(*) AS count FROM portfolio_operations WHERE id_portfolio = $1",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .expect("operation count should be available");
        assert_eq!(operations.get::<i64, _>("count"), 3);

        let original_operation = sqlx::query(
            "SELECT operation_status FROM portfolio_operations WHERE id_portfolio_operation = $1",
        )
        .bind(deposit_id)
        .fetch_one(&pool)
        .await
        .expect("deposit operation should still exist");
        assert_eq!(
            original_operation.get::<String, _>("operation_status"),
            "posted"
        );
    }

    #[tokio::test]
    async fn composite_job_respects_target_and_skips_soft_deleted_portfolios() {
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let id_user = create_user(&pool, &suffix[..10]).await;
        let id_target_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let id_other_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let id_deleted_portfolio = create_portfolio(&pool, id_user, "EUR", true).await;
        let id_asset = create_asset(&pool, &suffix[..6]).await;
        insert_asset_market_data(&pool, id_asset, 250).await;

        for id_portfolio in [
            id_target_portfolio,
            id_other_portfolio,
            id_deleted_portfolio,
        ] {
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
                    gross_amount_minor: Some(500),
                    cash_amount_minor: 500,
                    currency: "EUR",
                    fx_rate_to_portfolio: None,
                    executed_at: OffsetDateTime::now_utc(),
                },
            )
            .await;
        }

        let snapshot_date = Date::from_calendar_date(2026, time::Month::June, 7).unwrap();
        let job = RefreshCurrentPortfolioStateJob::from_config(&Config {
            database_url: "postgresql://postgres:postgres@localhost:5432/test".to_string(),
            app_env: "test".to_string(),
            rust_log: "debug".to_string(),
            worker_name: "worker-test".to_string(),
            worker_mode: crate::config::WorkerMode::Once,
            worker_job: WorkerJob::RefreshCurrentPortfolioState,
            worker_poll_interval: StdDuration::from_secs(5),
            target_portfolio_id: Some(id_target_portfolio),
            snapshot_date: Some(snapshot_date),
            backfill_date_from: None,
            backfill_date_to: None,
            redis_url: None,
            health: None,
            refresh_consumer: crate::config::RefreshConsumerConfig::default(),
        });

        job.run(&test_state(pool.clone()))
            .await
            .expect("targeted composite job should succeed");

        let target_snapshot_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM portfolio_snapshots_daily WHERE id_portfolio = $1 AND snapshot_date = $2",
        )
        .bind(id_target_portfolio)
        .bind(snapshot_date)
        .fetch_one(&pool)
        .await
        .expect("target snapshot count should be available");
        assert_eq!(target_snapshot_count.get::<i64, _>("count"), 1);

        let other_snapshot_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM portfolio_snapshots_daily WHERE id_portfolio = $1 AND snapshot_date = $2",
        )
        .bind(id_other_portfolio)
        .bind(snapshot_date)
        .fetch_one(&pool)
        .await
        .expect("other snapshot count should be available");
        assert_eq!(other_snapshot_count.get::<i64, _>("count"), 0);

        let deleted_snapshot_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM portfolio_snapshots_daily WHERE id_portfolio = $1 AND snapshot_date = $2",
        )
        .bind(id_deleted_portfolio)
        .bind(snapshot_date)
        .fetch_one(&pool)
        .await
        .expect("deleted snapshot count should be available");
        assert_eq!(deleted_snapshot_count.get::<i64, _>("count"), 0);
    }
}
