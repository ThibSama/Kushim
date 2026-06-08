use crate::{
    config::Config,
    domain::portfolio_snapshot::{PortfolioDailySnapshotWrite, PortfolioHoldingSnapshotDailyWrite},
    errors::WorkerError,
    jobs::Job,
    repositories::snapshot_generation::SnapshotGenerationRepository,
    state::AppState,
};
use async_trait::async_trait;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GenerateDailySnapshotsJob {
    target_portfolio_id: Option<Uuid>,
    snapshot_date: Option<Date>,
}

impl GenerateDailySnapshotsJob {
    pub fn from_config(config: &Config) -> Self {
        Self {
            target_portfolio_id: config.target_portfolio_id,
            snapshot_date: config.snapshot_date,
        }
    }

    pub fn effective_snapshot_date(&self) -> Date {
        self.snapshot_date
            .unwrap_or_else(|| OffsetDateTime::now_utc().date())
    }
}

#[async_trait]
impl Job for GenerateDailySnapshotsJob {
    fn name(&self) -> &'static str {
        "generate_daily_snapshots"
    }

    async fn run(&self, state: &AppState) -> Result<(), WorkerError> {
        let snapshot_date = self.effective_snapshot_date();
        tracing::info!(
            worker = %state.worker_name,
            job = self.name(),
            snapshot_date = %snapshot_date,
            target_portfolio_id = ?self.target_portfolio_id,
            "starting generate daily snapshots job"
        );

        let repository = SnapshotGenerationRepository::new(state.pg_pool.clone());
        let portfolios = repository
            .list_portfolios_for_snapshot_generation(self.target_portfolio_id)
            .await?;

        let mut processed_count = 0_usize;
        let mut skipped_missing_summary_count = 0_usize;

        for portfolio in portfolios {
            let mut transaction = repository.begin().await?;
            let Some(summary) = repository
                .find_current_summary_for_portfolio(&mut transaction, portfolio.id_portfolio)
                .await?
            else {
                tracing::warn!(
                    worker = %state.worker_name,
                    job = self.name(),
                    id_portfolio = %portfolio.id_portfolio,
                    snapshot_date = %snapshot_date,
                    "skipping daily snapshot generation because current summary is missing"
                );
                skipped_missing_summary_count += 1;
                transaction.rollback().await?;
                continue;
            };

            let holdings = repository
                .list_current_holdings_for_portfolio(&mut transaction, portfolio.id_portfolio)
                .await?;

            let snapshot_id = repository
                .upsert_daily_snapshot(
                    &mut transaction,
                    &PortfolioDailySnapshotWrite {
                        id_portfolio: summary.id_portfolio,
                        snapshot_date,
                        base_currency: summary.base_currency.clone(),
                        cash_balance_minor: summary.cash_balance_minor,
                        total_value_minor: summary.total_value_minor,
                        total_invested_minor: summary.total_invested_minor,
                        total_pnl_minor: summary.total_pnl_minor,
                        total_pnl_pct: summary.total_pnl_pct.clone(),
                        is_estimated: summary.is_estimated,
                        source_type: "daily_job",
                    },
                )
                .await?;

            let holding_snapshots: Vec<PortfolioHoldingSnapshotDailyWrite> = holdings
                .into_iter()
                .map(|holding| PortfolioHoldingSnapshotDailyWrite {
                    id_asset: holding.id_asset,
                    base_currency: holding.base_currency,
                    quantity: holding.quantity,
                    avg_cost_minor: holding.avg_cost_minor,
                    invested_minor: holding.invested_base_minor,
                    market_value_minor: holding.market_value_minor,
                    pnl_minor: holding.pnl_base_minor,
                    pnl_pct: holding.pnl_pct,
                    weight_pct: holding.weight_pct,
                    is_estimated: holding.is_estimated,
                })
                .collect();

            repository
                .replace_holding_snapshots(&mut transaction, snapshot_id, &holding_snapshots)
                .await?;
            transaction.commit().await?;

            tracing::info!(
                worker = %state.worker_name,
                job = self.name(),
                id_portfolio = %portfolio.id_portfolio,
                snapshot_date = %snapshot_date,
                holdings_count = holding_snapshots.len(),
                "generated daily snapshot from current read models"
            );
            processed_count += 1;
        }

        tracing::info!(
            worker = %state.worker_name,
            job = self.name(),
            snapshot_date = %snapshot_date,
            processed_count,
            skipped_missing_summary_count,
            "completed generate daily snapshots job"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::GenerateDailySnapshotsJob;
    use crate::{config::Config, jobs::Job, state::AppState, test_utils::lock_env};
    use sqlx::{PgPool, Row};
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
        let handle = format!("snap{}", suffix);

        sqlx::query(
            r#"
            INSERT INTO users (id_user, id_role, username, public_handle, password_hash)
            VALUES ($1, 1, $2, $3, '$argon2id$snapshot')
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
        .bind(format!("Snapshot Asset {suffix}"))
        .bind(format!("S{suffix}"))
        .execute(pool)
        .await
        .expect("asset should be inserted");

        id_asset
    }

    struct SummaryFixture<'a> {
        total_value_minor: i64,
        cash_balance_minor: i64,
        total_invested_minor: i64,
        total_pnl_minor: i64,
        total_pnl_pct: Option<&'a str>,
        is_estimated: bool,
    }

    async fn insert_summary(pool: &PgPool, id_portfolio: Uuid, fixture: SummaryFixture<'_>) {
        sqlx::query(
            r#"
            INSERT INTO rm_portfolio_summary (
                id_portfolio,
                base_currency,
                total_value_minor,
                cash_balance_minor,
                total_invested_minor,
                total_pnl_minor,
                total_pnl_pct,
                portfolio_status,
                is_estimated,
                as_of
            )
            VALUES ($1, 'EUR', $2, $3, $4, $5, $6::numeric, 'active', $7, now())
            ON CONFLICT (id_portfolio) DO UPDATE
            SET
                total_value_minor = EXCLUDED.total_value_minor,
                cash_balance_minor = EXCLUDED.cash_balance_minor,
                total_invested_minor = EXCLUDED.total_invested_minor,
                total_pnl_minor = EXCLUDED.total_pnl_minor,
                total_pnl_pct = EXCLUDED.total_pnl_pct,
                is_estimated = EXCLUDED.is_estimated,
                as_of = EXCLUDED.as_of
            "#,
        )
        .bind(id_portfolio)
        .bind(fixture.total_value_minor)
        .bind(fixture.cash_balance_minor)
        .bind(fixture.total_invested_minor)
        .bind(fixture.total_pnl_minor)
        .bind(fixture.total_pnl_pct)
        .bind(fixture.is_estimated)
        .execute(pool)
        .await
        .expect("summary should be inserted");
    }

    struct HoldingFixture<'a> {
        quantity: &'a str,
        avg_cost_minor: Option<i64>,
        invested_base_minor: i64,
        market_value_minor: i64,
        pnl_base_minor: i64,
        pnl_pct: Option<&'a str>,
        weight_pct: Option<&'a str>,
        is_estimated: bool,
    }

    async fn insert_holding(
        pool: &PgPool,
        id_portfolio: Uuid,
        id_asset: Uuid,
        fixture: HoldingFixture<'_>,
    ) {
        sqlx::query(
            r#"
            INSERT INTO rm_portfolio_holdings (
                id_portfolio,
                id_asset,
                base_currency,
                quantity,
                avg_cost_minor,
                invested_base_minor,
                market_value_minor,
                pnl_base_minor,
                pnl_pct,
                weight_pct,
                position_status,
                is_estimated,
                as_of
            )
            VALUES (
                $1, $2, 'EUR', $3::numeric, $4, $5, $6, $7, $8::numeric, $9::numeric, 'open', $10, now()
            )
            ON CONFLICT (id_portfolio, id_asset) DO UPDATE
            SET
                quantity = EXCLUDED.quantity,
                avg_cost_minor = EXCLUDED.avg_cost_minor,
                invested_base_minor = EXCLUDED.invested_base_minor,
                market_value_minor = EXCLUDED.market_value_minor,
                pnl_base_minor = EXCLUDED.pnl_base_minor,
                pnl_pct = EXCLUDED.pnl_pct,
                weight_pct = EXCLUDED.weight_pct,
                is_estimated = EXCLUDED.is_estimated,
                as_of = EXCLUDED.as_of
            "#,
        )
        .bind(id_portfolio)
        .bind(id_asset)
        .bind(fixture.quantity)
        .bind(fixture.avg_cost_minor)
        .bind(fixture.invested_base_minor)
        .bind(fixture.market_value_minor)
        .bind(fixture.pnl_base_minor)
        .bind(fixture.pnl_pct)
        .bind(fixture.weight_pct)
        .bind(fixture.is_estimated)
        .execute(pool)
        .await
        .expect("holding should be inserted");
    }

    async fn test_state(pool: PgPool) -> AppState {
        AppState {
            pg_pool: pool,
            worker_name: "worker-test".to_string(),
        }
    }

    fn job(
        target_portfolio_id: Option<Uuid>,
        snapshot_date: Option<Date>,
    ) -> GenerateDailySnapshotsJob {
        GenerateDailySnapshotsJob {
            target_portfolio_id,
            snapshot_date,
        }
    }

    async fn snapshot_count_for_portfolio_and_date(
        pool: &PgPool,
        id_portfolio: Uuid,
        snapshot_date: Date,
    ) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_snapshots_daily WHERE id_portfolio = $1 AND snapshot_date = $2",
        )
        .bind(id_portfolio)
        .bind(snapshot_date)
        .fetch_one(pool)
        .await
        .expect("snapshot count should succeed")
    }

    async fn snapshot_id_for_portfolio_and_date(
        pool: &PgPool,
        id_portfolio: Uuid,
        snapshot_date: Date,
    ) -> Option<Uuid> {
        sqlx::query_scalar(
            "SELECT id_portfolio_snapshot_daily FROM portfolio_snapshots_daily WHERE id_portfolio = $1 AND snapshot_date = $2",
        )
        .bind(id_portfolio)
        .bind(snapshot_date)
        .fetch_optional(pool)
        .await
        .expect("snapshot id query should succeed")
    }

    async fn holding_snapshot_count(pool: &PgPool, id_snapshot: Uuid) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_holding_snapshot_daily WHERE id_portfolio_snapshot_daily = $1",
        )
        .bind(id_snapshot)
        .fetch_one(pool)
        .await
        .expect("holding snapshot count should succeed")
    }

    #[tokio::test]
    async fn generates_daily_snapshot_from_summary_and_holdings() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let id_asset = create_asset(&pool, &Uuid::new_v4().simple().to_string()[..8]).await;
        let snapshot_date = Date::from_calendar_date(2026, time::Month::June, 6).unwrap();

        insert_summary(
            &pool,
            id_portfolio,
            SummaryFixture {
                total_value_minor: 1_200,
                cash_balance_minor: 400,
                total_invested_minor: 1_000,
                total_pnl_minor: 200,
                total_pnl_pct: Some("20.0000"),
                is_estimated: false,
            },
        )
        .await;
        insert_holding(
            &pool,
            id_portfolio,
            id_asset,
            HoldingFixture {
                quantity: "2.0000000000",
                avg_cost_minor: Some(300),
                invested_base_minor: 600,
                market_value_minor: 800,
                pnl_base_minor: 200,
                pnl_pct: Some("33.3333"),
                weight_pct: Some("66.6667"),
                is_estimated: false,
            },
        )
        .await;

        job(Some(id_portfolio), Some(snapshot_date))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("snapshot generation should succeed");

        let snapshot = sqlx::query(
            r#"
            SELECT cash_balance_minor, total_value_minor, total_invested_minor, total_pnl_minor, source_type
            FROM portfolio_snapshots_daily
            WHERE id_portfolio = $1 AND snapshot_date = $2
            "#,
        )
        .bind(id_portfolio)
        .bind(snapshot_date)
        .fetch_one(&pool)
        .await
        .expect("snapshot should exist");

        assert_eq!(snapshot.get::<i64, _>("cash_balance_minor"), 400);
        assert_eq!(snapshot.get::<i64, _>("total_value_minor"), 1_200);
        assert_eq!(snapshot.get::<i64, _>("total_invested_minor"), 1_000);
        assert_eq!(snapshot.get::<i64, _>("total_pnl_minor"), 200);
        assert_eq!(snapshot.get::<String, _>("source_type"), "daily_job");

        let id_snapshot = snapshot_id_for_portfolio_and_date(&pool, id_portfolio, snapshot_date)
            .await
            .expect("snapshot id should exist");
        let holding = sqlx::query(
            r#"
            SELECT quantity::text AS quantity, invested_minor, market_value_minor, pnl_minor
            FROM portfolio_holding_snapshot_daily
            WHERE id_portfolio_snapshot_daily = $1 AND id_asset = $2
            "#,
        )
        .bind(id_snapshot)
        .bind(id_asset)
        .fetch_one(&pool)
        .await
        .expect("holding snapshot should exist");

        assert_eq!(holding.get::<String, _>("quantity"), "2.0000000000");
        assert_eq!(holding.get::<i64, _>("invested_minor"), 600);
        assert_eq!(holding.get::<i64, _>("market_value_minor"), 800);
        assert_eq!(holding.get::<i64, _>("pnl_minor"), 200);
    }

    #[tokio::test]
    async fn summary_without_holdings_creates_snapshot_with_zero_holding_rows() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let snapshot_date = Date::from_calendar_date(2026, time::Month::June, 7).unwrap();

        insert_summary(
            &pool,
            id_portfolio,
            SummaryFixture {
                total_value_minor: 500,
                cash_balance_minor: 500,
                total_invested_minor: 500,
                total_pnl_minor: 0,
                total_pnl_pct: Some("0.0000"),
                is_estimated: false,
            },
        )
        .await;

        job(Some(id_portfolio), Some(snapshot_date))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("snapshot generation should succeed");

        let id_snapshot = snapshot_id_for_portfolio_and_date(&pool, id_portfolio, snapshot_date)
            .await
            .expect("snapshot id should exist");
        assert_eq!(holding_snapshot_count(&pool, id_snapshot).await, 0);
    }

    #[tokio::test]
    async fn missing_summary_is_skipped() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let snapshot_date = Date::from_calendar_date(2026, time::Month::June, 8).unwrap();

        job(Some(id_portfolio), Some(snapshot_date))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("snapshot generation should succeed even when summary is missing");

        assert_eq!(
            snapshot_count_for_portfolio_and_date(&pool, id_portfolio, snapshot_date).await,
            0
        );
    }

    #[tokio::test]
    async fn target_portfolio_limits_generation_to_one_portfolio() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let target_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let other_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let snapshot_date = Date::from_calendar_date(2026, time::Month::June, 9).unwrap();

        insert_summary(
            &pool,
            target_portfolio,
            SummaryFixture {
                total_value_minor: 100,
                cash_balance_minor: 100,
                total_invested_minor: 100,
                total_pnl_minor: 0,
                total_pnl_pct: Some("0.0000"),
                is_estimated: false,
            },
        )
        .await;
        insert_summary(
            &pool,
            other_portfolio,
            SummaryFixture {
                total_value_minor: 200,
                cash_balance_minor: 200,
                total_invested_minor: 200,
                total_pnl_minor: 0,
                total_pnl_pct: Some("0.0000"),
                is_estimated: false,
            },
        )
        .await;

        job(Some(target_portfolio), Some(snapshot_date))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("snapshot generation should succeed");

        assert_eq!(
            snapshot_count_for_portfolio_and_date(&pool, target_portfolio, snapshot_date).await,
            1
        );
        assert_eq!(
            snapshot_count_for_portfolio_and_date(&pool, other_portfolio, snapshot_date).await,
            0
        );
    }

    #[tokio::test]
    async fn rerunning_same_date_is_idempotent_and_replaces_holding_rows() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let id_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let id_asset = create_asset(&pool, &Uuid::new_v4().simple().to_string()[..8]).await;
        let snapshot_date = Date::from_calendar_date(2026, time::Month::June, 10).unwrap();

        insert_summary(
            &pool,
            id_portfolio,
            SummaryFixture {
                total_value_minor: 1_000,
                cash_balance_minor: 400,
                total_invested_minor: 800,
                total_pnl_minor: 200,
                total_pnl_pct: Some("25.0000"),
                is_estimated: false,
            },
        )
        .await;
        insert_holding(
            &pool,
            id_portfolio,
            id_asset,
            HoldingFixture {
                quantity: "1.0000000000",
                avg_cost_minor: Some(600),
                invested_base_minor: 600,
                market_value_minor: 800,
                pnl_base_minor: 200,
                pnl_pct: Some("33.3333"),
                weight_pct: Some("80.0000"),
                is_estimated: false,
            },
        )
        .await;

        let state = test_state(pool.clone()).await;
        let job = job(Some(id_portfolio), Some(snapshot_date));
        job.run(&state).await.expect("first run should succeed");

        insert_summary(
            &pool,
            id_portfolio,
            SummaryFixture {
                total_value_minor: 1_200,
                cash_balance_minor: 500,
                total_invested_minor: 900,
                total_pnl_minor: 300,
                total_pnl_pct: Some("33.3333"),
                is_estimated: false,
            },
        )
        .await;
        insert_holding(
            &pool,
            id_portfolio,
            id_asset,
            HoldingFixture {
                quantity: "2.0000000000",
                avg_cost_minor: Some(450),
                invested_base_minor: 900,
                market_value_minor: 700,
                pnl_base_minor: -200,
                pnl_pct: Some("-22.2222"),
                weight_pct: Some("58.3333"),
                is_estimated: true,
            },
        )
        .await;

        job.run(&state).await.expect("second run should succeed");

        assert_eq!(
            snapshot_count_for_portfolio_and_date(&pool, id_portfolio, snapshot_date).await,
            1
        );

        let id_snapshot = snapshot_id_for_portfolio_and_date(&pool, id_portfolio, snapshot_date)
            .await
            .expect("snapshot id should exist");
        assert_eq!(holding_snapshot_count(&pool, id_snapshot).await, 1);

        let holding = sqlx::query(
            r#"
            SELECT quantity::text AS quantity, invested_minor, market_value_minor, pnl_minor, is_estimated
            FROM portfolio_holding_snapshot_daily
            WHERE id_portfolio_snapshot_daily = $1 AND id_asset = $2
            "#,
        )
        .bind(id_snapshot)
        .bind(id_asset)
        .fetch_one(&pool)
        .await
        .expect("updated holding snapshot should exist");

        assert_eq!(holding.get::<String, _>("quantity"), "2.0000000000");
        assert_eq!(holding.get::<i64, _>("invested_minor"), 900);
        assert_eq!(holding.get::<i64, _>("market_value_minor"), 700);
        assert_eq!(holding.get::<i64, _>("pnl_minor"), -200);
        assert!(holding.get::<bool, _>("is_estimated"));
    }

    #[tokio::test]
    async fn does_not_write_read_models_operations_or_market_data_and_skips_soft_deleted() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let active_portfolio = create_portfolio(&pool, id_user, "EUR", false).await;
        let deleted_portfolio = create_portfolio(&pool, id_user, "EUR", true).await;
        let id_asset = create_asset(&pool, &Uuid::new_v4().simple().to_string()[..8]).await;
        let snapshot_date = Date::from_calendar_date(2026, time::Month::June, 11).unwrap();

        insert_summary(
            &pool,
            active_portfolio,
            SummaryFixture {
                total_value_minor: 1_000,
                cash_balance_minor: 400,
                total_invested_minor: 700,
                total_pnl_minor: 300,
                total_pnl_pct: Some("42.8571"),
                is_estimated: false,
            },
        )
        .await;
        insert_summary(
            &pool,
            deleted_portfolio,
            SummaryFixture {
                total_value_minor: 500,
                cash_balance_minor: 500,
                total_invested_minor: 500,
                total_pnl_minor: 0,
                total_pnl_pct: Some("0.0000"),
                is_estimated: false,
            },
        )
        .await;
        insert_holding(
            &pool,
            active_portfolio,
            id_asset,
            HoldingFixture {
                quantity: "1.0000000000",
                avg_cost_minor: Some(700),
                invested_base_minor: 700,
                market_value_minor: 600,
                pnl_base_minor: -100,
                pnl_pct: Some("-14.2857"),
                weight_pct: Some("60.0000"),
                is_estimated: false,
            },
        )
        .await;

        let read_model_counts_before: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM rm_portfolio_summary WHERE id_portfolio IN ($1, $2)), (SELECT COUNT(*) FROM rm_portfolio_holdings WHERE id_portfolio = $1)",
        )
        .bind(active_portfolio)
        .bind(deleted_portfolio)
        .fetch_one(&pool)
        .await
        .expect("read model counts should succeed");

        let operations_before: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio IN ($1, $2)",
        )
        .bind(active_portfolio)
        .bind(deleted_portfolio)
        .fetch_one(&pool)
        .await
        .expect("operation count should succeed");

        let market_data_before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM asset_market_data WHERE id_asset = $1")
                .bind(id_asset)
                .fetch_one(&pool)
                .await
                .expect("market data count should succeed");

        let price_cache_before: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM asset_price_history_cache WHERE id_asset = $1",
        )
        .bind(id_asset)
        .fetch_one(&pool)
        .await
        .expect("price cache count should succeed");

        job(None, Some(snapshot_date))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("snapshot generation should succeed");

        let read_model_counts_after: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM rm_portfolio_summary WHERE id_portfolio IN ($1, $2)), (SELECT COUNT(*) FROM rm_portfolio_holdings WHERE id_portfolio = $1)",
        )
        .bind(active_portfolio)
        .bind(deleted_portfolio)
        .fetch_one(&pool)
        .await
        .expect("read model counts should succeed");

        assert_eq!(read_model_counts_before, read_model_counts_after);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio IN ($1, $2)",
            )
            .bind(active_portfolio)
            .bind(deleted_portfolio)
            .fetch_one(&pool)
            .await
            .expect("operation count should succeed"),
            operations_before
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM asset_market_data WHERE id_asset = $1"
            )
            .bind(id_asset)
            .fetch_one(&pool)
            .await
            .expect("market data count should succeed"),
            market_data_before
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM asset_price_history_cache WHERE id_asset = $1",
            )
            .bind(id_asset)
            .fetch_one(&pool)
            .await
            .expect("price cache count should succeed"),
            price_cache_before
        );

        assert_eq!(
            snapshot_count_for_portfolio_and_date(&pool, active_portfolio, snapshot_date).await,
            1
        );
        assert_eq!(
            snapshot_count_for_portfolio_and_date(&pool, deleted_portfolio, snapshot_date).await,
            0
        );
    }

    #[tokio::test]
    async fn multiple_portfolios_work() {
        let pool = test_pool().await;
        let id_user = create_user(&pool, &Uuid::new_v4().simple().to_string()[..12]).await;
        let first = create_portfolio(&pool, id_user, "EUR", false).await;
        let second = create_portfolio(&pool, id_user, "EUR", false).await;
        let snapshot_date = Date::from_calendar_date(2026, time::Month::June, 12).unwrap();

        insert_summary(
            &pool,
            first,
            SummaryFixture {
                total_value_minor: 100,
                cash_balance_minor: 100,
                total_invested_minor: 100,
                total_pnl_minor: 0,
                total_pnl_pct: Some("0.0000"),
                is_estimated: false,
            },
        )
        .await;
        insert_summary(
            &pool,
            second,
            SummaryFixture {
                total_value_minor: 200,
                cash_balance_minor: 50,
                total_invested_minor: 150,
                total_pnl_minor: 50,
                total_pnl_pct: Some("33.3333"),
                is_estimated: true,
            },
        )
        .await;

        job(None, Some(snapshot_date))
            .run(&test_state(pool.clone()).await)
            .await
            .expect("snapshot generation should succeed");

        assert_eq!(
            snapshot_count_for_portfolio_and_date(&pool, first, snapshot_date).await,
            1
        );
        assert_eq!(
            snapshot_count_for_portfolio_and_date(&pool, second, snapshot_date).await,
            1
        );
    }

    #[test]
    fn config_exposes_generate_daily_snapshots_job() {
        let _ = Config {
            database_url: "postgresql://localhost/kushim".into(),
            app_env: "test".into(),
            rust_log: "info".into(),
            worker_name: "worker".into(),
            worker_mode: crate::config::WorkerMode::Once,
            worker_job: crate::config::WorkerJob::GenerateDailySnapshots,
            worker_poll_interval: std::time::Duration::from_secs(1),
            target_portfolio_id: None,
            snapshot_date: Some(Date::from_calendar_date(2026, time::Month::June, 6).unwrap()),
            backfill_date_from: None,
            backfill_date_to: None,
            redis_url: None,
            health: None,
        };
    }
}
