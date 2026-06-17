use crate::{
    calculation::portfolio_replay::rebuild_portfolio_state,
    config::Config,
    domain::{
        portfolio_backfill::BackfillDateRange,
        portfolio_snapshot::{PortfolioDailySnapshotWrite, PortfolioHoldingSnapshotDailyWrite},
        portfolio_state::PortfolioDefinition,
    },
    errors::WorkerError,
    jobs::Job,
    repositories::{
        backfill_snapshots::BackfillSnapshotsRepository,
        snapshot_generation::SnapshotGenerationRepository,
    },
    state::AppState,
};
use async_trait::async_trait;
use std::collections::HashSet;
use time::{Date, Duration, PrimitiveDateTime, Time};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BackfillDailySnapshotsJob {
    target_portfolio_id: Uuid,
    date_range: BackfillDateRange,
}

impl BackfillDailySnapshotsJob {
    pub fn from_config(config: &Config) -> Result<Self, WorkerError> {
        Ok(Self {
            target_portfolio_id: config.target_portfolio_id.ok_or_else(|| {
                WorkerError::Config(
                    "WORKER_TARGET_PORTFOLIO_ID must be set for WORKER_JOB=backfill_daily_snapshots"
                        .to_string(),
                )
            })?,
            date_range: BackfillDateRange {
                date_from: config.backfill_date_from.ok_or_else(|| {
                    WorkerError::Config(
                        "WORKER_BACKFILL_DATE_FROM must be set for WORKER_JOB=backfill_daily_snapshots"
                            .to_string(),
                    )
                })?,
                date_to: config.backfill_date_to.ok_or_else(|| {
                    WorkerError::Config(
                        "WORKER_BACKFILL_DATE_TO must be set for WORKER_JOB=backfill_daily_snapshots"
                            .to_string(),
                    )
                })?,
            },
        })
    }

    fn iter_dates(&self) -> Result<Vec<Date>, WorkerError> {
        let mut dates = Vec::new();
        let mut current = self.date_range.date_from;
        loop {
            dates.push(current);
            if current == self.date_range.date_to {
                break;
            }
            current = current.next_day().ok_or_else(|| {
                WorkerError::Job("backfill date iteration exceeded supported date bounds".into())
            })?;
        }
        Ok(dates)
    }
}

#[async_trait]
impl Job for BackfillDailySnapshotsJob {
    fn name(&self) -> &'static str {
        "backfill_daily_snapshots"
    }

    async fn run(&self, state: &AppState) -> Result<(), WorkerError> {
        tracing::info!(
            worker = %state.worker_name,
            job = self.name(),
            target_portfolio_id = %self.target_portfolio_id,
            date_from = %self.date_range.date_from,
            date_to = %self.date_range.date_to,
            "starting historical daily snapshot backfill job"
        );

        let backfill_repository = BackfillSnapshotsRepository::new(state.pg_pool.clone());
        let snapshot_repository = SnapshotGenerationRepository::new(state.pg_pool.clone());
        let portfolio = backfill_repository
            .find_target_portfolio(self.target_portfolio_id)
            .await?
            .ok_or_else(|| {
                WorkerError::Job(format!(
                    "target portfolio {} does not exist or is soft-deleted",
                    self.target_portfolio_id
                ))
            })?;

        let dates = self.iter_dates()?;
        let created_on = portfolio.created_at.date();
        let mut snapshots_written = 0_usize;
        let mut missing_price_count = 0_usize;
        let mut estimated_snapshot_count = 0_usize;
        let mut skipped_before_creation_count = 0_usize;

        for snapshot_date in dates {
            if created_on > snapshot_date {
                skipped_before_creation_count += 1;
                tracing::info!(
                    worker = %state.worker_name,
                    job = self.name(),
                    id_portfolio = %portfolio.id_portfolio,
                    snapshot_date = %snapshot_date,
                    created_on = %created_on,
                    "skipping backfill date because portfolio was not created yet"
                );
                continue;
            }

            let operations = backfill_repository
                .list_posted_operations_through_date(portfolio.id_portfolio, snapshot_date)
                .await?;

            let asset_ids: Vec<Uuid> = operations
                .iter()
                .flat_map(|operation| [operation.id_asset, operation.id_related_asset])
                .flatten()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            let historical_prices = backfill_repository
                .find_historical_prices_for_assets(
                    &asset_ids,
                    snapshot_date,
                    &portfolio.base_currency,
                )
                .await?;

            let rebuilt = rebuild_portfolio_state(
                PortfolioDefinition {
                    id_portfolio: portfolio.id_portfolio,
                    base_currency: portfolio.base_currency.clone(),
                },
                &operations,
                &historical_prices,
                PrimitiveDateTime::new(snapshot_date, Time::MIDNIGHT).assume_utc()
                    + Duration::hours(23)
                    + Duration::minutes(59)
                    + Duration::seconds(59),
            )?;

            let held_assets: Vec<Uuid> = rebuilt
                .holdings
                .iter()
                .map(|holding| holding.id_asset)
                .collect();
            let date_missing_prices = held_assets
                .iter()
                .filter(|id_asset| !historical_prices.contains_key(id_asset))
                .count();
            missing_price_count += date_missing_prices;
            if rebuilt.summary.is_estimated {
                estimated_snapshot_count += 1;
            }

            let mut transaction = snapshot_repository.begin().await?;
            let snapshot_id = snapshot_repository
                .upsert_daily_snapshot(
                    &mut transaction,
                    &PortfolioDailySnapshotWrite {
                        id_portfolio: rebuilt.summary.id_portfolio,
                        snapshot_date,
                        base_currency: rebuilt.summary.base_currency.clone(),
                        cash_balance_minor: rebuilt.summary.cash_balance_minor,
                        total_value_minor: rebuilt.summary.total_value_minor,
                        total_invested_minor: rebuilt.summary.total_invested_minor,
                        total_pnl_minor: rebuilt.summary.total_pnl_minor,
                        total_pnl_pct: rebuilt.summary.total_pnl_pct.clone(),
                        is_estimated: rebuilt.summary.is_estimated,
                        source_type: "backfill",
                    },
                )
                .await?;

            let holding_snapshots: Vec<PortfolioHoldingSnapshotDailyWrite> = rebuilt
                .holdings
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

            snapshot_repository
                .replace_holding_snapshots(&mut transaction, snapshot_id, &holding_snapshots)
                .await?;
            transaction.commit().await?;

            snapshots_written += 1;
            tracing::info!(
                worker = %state.worker_name,
                job = self.name(),
                id_portfolio = %portfolio.id_portfolio,
                snapshot_date = %snapshot_date,
                holdings_count = holding_snapshots.len(),
                missing_price_count = date_missing_prices,
                is_estimated = rebuilt.summary.is_estimated,
                "backfilled daily snapshot"
            );
        }

        tracing::info!(
            worker = %state.worker_name,
            job = self.name(),
            target_portfolio_id = %self.target_portfolio_id,
            date_from = %self.date_range.date_from,
            date_to = %self.date_range.date_to,
            snapshots_written,
            estimated_snapshot_count,
            missing_price_count,
            skipped_before_creation_count,
            "completed historical daily snapshot backfill job"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BackfillDailySnapshotsJob;
    use crate::{
        config::{Config, WorkerJob, WorkerMode},
        jobs::Job,
        state::AppState,
        test_utils::{cleanup_worker_test_tree, lock_env},
    };
    use sqlx::{PgPool, Row, postgres::PgPoolOptions};
    use std::time::Duration as StdDuration;
    use time::{Date, Duration, Month, OffsetDateTime};
    use uuid::Uuid;

    async fn test_pool() -> PgPool {
        let database_url = {
            let _guard = lock_env();
            crate::test_utils::require_disposable_test_database_url()
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
        let handle = format!("bck{}", suffix);

        sqlx::query(
            "INSERT INTO users (id_user, id_role, username, public_handle, password_hash) VALUES ($1, 1, $2, $3, '$argon2id$backfill')",
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
        created_at: OffsetDateTime,
    ) -> Uuid {
        let id_portfolio = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO portfolios (id_portfolio, id_user, name, base_currency, visibility, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'private', $5, $5)
            "#,
        )
        .bind(id_portfolio)
        .bind(id_user)
        .bind(format!("pf{}", &id_portfolio.simple().to_string()[..12]))
        .bind(base_currency)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("portfolio should be inserted");

        id_portfolio
    }

    async fn create_asset(pool: &PgPool, suffix: &str) -> Uuid {
        let id_asset = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO assets (id_asset, asset_class, status, name, native_currency, symbol) VALUES ($1, 'equity', 'active', $2, 'EUR', $3)",
        )
        .bind(id_asset)
        .bind(format!("Backfill Asset {suffix}"))
        .bind(format!("B{suffix}"))
        .execute(pool)
        .await
        .expect("asset should be inserted");

        id_asset
    }

    async fn insert_historical_price(
        pool: &PgPool,
        id_asset: Uuid,
        price_date: Date,
        currency: &str,
        close_minor: i64,
        source: &str,
        fetched_at: OffsetDateTime,
    ) {
        sqlx::query(
            r#"
            INSERT INTO asset_price_history_cache (
                id_asset, price_date, currency, close_minor, source, fetched_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(id_asset)
        .bind(price_date)
        .bind(currency)
        .bind(close_minor)
        .bind(source)
        .bind(fetched_at)
        .execute(pool)
        .await
        .expect("historical price should be inserted");
    }

    struct OperationFixture<'a> {
        id_asset: Option<Uuid>,
        id_related_asset: Option<Uuid>,
        operation_type: &'a str,
        operation_status: &'a str,
        executed_at: OffsetDateTime,
        quantity: Option<&'a str>,
        related_quantity: Option<&'a str>,
        price_minor: Option<i64>,
        gross_amount_minor: Option<i64>,
        cash_amount_minor: i64,
        currency: &'a str,
        fx_rate_to_portfolio: Option<&'a str>,
    }

    async fn insert_operation(pool: &PgPool, id_portfolio: Uuid, fixture: OperationFixture<'_>) {
        sqlx::query(
            r#"
            INSERT INTO portfolio_operations (
                id_portfolio_operation, id_portfolio, id_asset, id_related_asset,
                operation_type, operation_status, executed_at,
                quantity, related_quantity, price_minor, gross_amount_minor,
                fees_minor, taxes_minor, cash_amount_minor,
                currency, fx_rate_to_portfolio, metadata
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5, $6,
                $7::numeric, $8::numeric, $9, $10,
                NULL, NULL, $11, $12, $13::numeric, '{}'::jsonb
            )
            "#,
        )
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
    }

    fn test_state(pool: PgPool) -> AppState {
        AppState {
            pg_pool: pool,
            worker_name: "test-worker".into(),
        }
    }

    fn backfill_job(
        id_portfolio: Uuid,
        date_from: Date,
        date_to: Date,
    ) -> BackfillDailySnapshotsJob {
        BackfillDailySnapshotsJob::from_config(&Config {
            database_url: "postgresql://postgres:postgres@localhost:5432/test".into(),
            app_env: "test".into(),
            rust_log: "debug".into(),
            worker_name: "worker-test".into(),
            worker_mode: WorkerMode::Once,
            worker_job: WorkerJob::BackfillDailySnapshots,
            worker_poll_interval: StdDuration::from_secs(5),
            target_portfolio_id: Some(id_portfolio),
            snapshot_date: None,
            backfill_date_from: Some(date_from),
            backfill_date_to: Some(date_to),
            redis_url: None,
            health: None,
            refresh_consumer: crate::config::RefreshConsumerConfig::default(),
        })
        .expect("backfill config should be valid")
    }

    #[test]
    fn config_exposes_backfill_job() {
        let date_from = Date::from_calendar_date(2026, Month::June, 1).unwrap();
        let date_to = Date::from_calendar_date(2026, Month::June, 3).unwrap();
        let config = Config {
            database_url: "postgresql://postgres:postgres@localhost:5432/test".into(),
            app_env: "test".into(),
            rust_log: "debug".into(),
            worker_name: "worker-test".into(),
            worker_mode: WorkerMode::Once,
            worker_job: WorkerJob::BackfillDailySnapshots,
            worker_poll_interval: StdDuration::from_secs(5),
            target_portfolio_id: Some(Uuid::new_v4()),
            snapshot_date: None,
            backfill_date_from: Some(date_from),
            backfill_date_to: Some(date_to),
            redis_url: None,
            health: None,
            refresh_consumer: crate::config::RefreshConsumerConfig::default(),
        };

        let job = BackfillDailySnapshotsJob::from_config(&config)
            .expect("backfill job should build from config");
        assert_eq!(job.name(), "backfill_daily_snapshots");
        assert_eq!(job.date_range.date_from, date_from);
        assert_eq!(job.date_range.date_to, date_to);
    }

    #[tokio::test]
    async fn backfill_creates_snapshots_for_date_range_and_is_idempotent() {
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let user = create_user(&pool, &suffix[..8]).await;
        let created_at = Date::from_calendar_date(2026, Month::June, 1)
            .unwrap()
            .midnight()
            .assume_utc();
        let portfolio = create_portfolio(&pool, user, "EUR", created_at).await;
        let asset = create_asset(&pool, &suffix[..6]).await;
        let d1 = Date::from_calendar_date(2026, Month::June, 1).unwrap();
        let d2 = Date::from_calendar_date(2026, Month::June, 2).unwrap();
        let d3 = Date::from_calendar_date(2026, Month::June, 3).unwrap();

        insert_historical_price(&pool, asset, d2, "EUR", 400, "provider_a", created_at).await;
        insert_historical_price(
            &pool,
            asset,
            d2,
            "EUR",
            450,
            "default",
            created_at + Duration::seconds(1),
        )
        .await;
        insert_historical_price(&pool, asset, d3, "EUR", 500, "provider_a", created_at).await;

        insert_operation(
            &pool,
            portfolio,
            OperationFixture {
                id_asset: None,
                id_related_asset: None,
                operation_type: "deposit",
                operation_status: "posted",
                executed_at: created_at + Duration::hours(9),
                quantity: None,
                related_quantity: None,
                price_minor: None,
                gross_amount_minor: Some(1_000),
                cash_amount_minor: 1_000,
                currency: "EUR",
                fx_rate_to_portfolio: None,
            },
        )
        .await;
        insert_operation(
            &pool,
            portfolio,
            OperationFixture {
                id_asset: Some(asset),
                id_related_asset: None,
                operation_type: "buy",
                operation_status: "posted",
                executed_at: created_at + Duration::days(1) + Duration::hours(10),
                quantity: Some("2"),
                related_quantity: None,
                price_minor: Some(300),
                gross_amount_minor: Some(600),
                cash_amount_minor: 600,
                currency: "EUR",
                fx_rate_to_portfolio: None,
            },
        )
        .await;
        insert_operation(
            &pool,
            portfolio,
            OperationFixture {
                id_asset: Some(asset),
                id_related_asset: None,
                operation_type: "buy",
                operation_status: "pending",
                executed_at: created_at + Duration::days(2) + Duration::hours(10),
                quantity: Some("10"),
                related_quantity: None,
                price_minor: Some(100),
                gross_amount_minor: Some(1_000),
                cash_amount_minor: 1_000,
                currency: "EUR",
                fx_rate_to_portfolio: None,
            },
        )
        .await;

        let outside_date = Date::from_calendar_date(2026, Month::June, 10).unwrap();
        sqlx::query(
            r#"
            INSERT INTO portfolio_snapshots_daily (
                id_portfolio, snapshot_date, base_currency, cash_balance_minor,
                total_value_minor, total_invested_minor, total_pnl_minor, total_pnl_pct, is_estimated, source_type
            )
            VALUES ($1, $2, 'EUR', 1, 1, 1, 0, NULL, false, 'backfill')
            "#,
        )
        .bind(portfolio)
        .bind(outside_date)
        .execute(&pool)
        .await
        .expect("outside snapshot should be inserted");

        let job = backfill_job(portfolio, d1, d3);
        job.run(&test_state(pool.clone()))
            .await
            .expect("backfill should succeed");
        job.run(&test_state(pool.clone()))
            .await
            .expect("backfill rerun should stay idempotent");

        let snapshot_counts = sqlx::query(
            r#"
            SELECT snapshot_date::text AS snapshot_date, total_value_minor, is_estimated
            FROM portfolio_snapshots_daily
            WHERE id_portfolio = $1 AND snapshot_date BETWEEN $2 AND $3
            ORDER BY snapshot_date ASC
            "#,
        )
        .bind(portfolio)
        .bind(d1)
        .bind(d3)
        .fetch_all(&pool)
        .await
        .expect("backfill snapshots should exist");
        assert_eq!(snapshot_counts.len(), 3);
        assert_eq!(
            snapshot_counts[0].get::<String, _>("snapshot_date"),
            "2026-06-01"
        );
        assert_eq!(snapshot_counts[0].get::<i64, _>("total_value_minor"), 1000);
        assert!(!snapshot_counts[0].get::<bool, _>("is_estimated"));
        assert_eq!(snapshot_counts[1].get::<i64, _>("total_value_minor"), 1300);
        assert!(!snapshot_counts[1].get::<bool, _>("is_estimated"));
        assert_eq!(snapshot_counts[2].get::<i64, _>("total_value_minor"), 1400);
        assert!(!snapshot_counts[2].get::<bool, _>("is_estimated"));

        let day2_holding = sqlx::query(
            r#"
            SELECT market_value_minor, invested_minor, quantity::text AS quantity
            FROM portfolio_holding_snapshot_daily hs
            JOIN portfolio_snapshots_daily s
                ON s.id_portfolio_snapshot_daily = hs.id_portfolio_snapshot_daily
            WHERE s.id_portfolio = $1 AND s.snapshot_date = $2
            "#,
        )
        .bind(portfolio)
        .bind(d2)
        .fetch_one(&pool)
        .await
        .expect("day 2 holding should exist");
        assert_eq!(day2_holding.get::<i64, _>("market_value_minor"), 900);
        assert_eq!(day2_holding.get::<i64, _>("invested_minor"), 600);
        assert_eq!(day2_holding.get::<String, _>("quantity"), "2.0000000000");

        let outside_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM portfolio_snapshots_daily WHERE id_portfolio = $1 AND snapshot_date = $2",
        )
        .bind(portfolio)
        .bind(outside_date)
        .fetch_one(&pool)
        .await
        .expect("outside snapshot should remain untouched");
        assert_eq!(outside_count, 1);

        let rm_summary_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM rm_portfolio_summary WHERE id_portfolio = $1",
        )
        .bind(portfolio)
        .fetch_one(&pool)
        .await
        .expect("rm summary count should be available");
        assert_eq!(rm_summary_count, 0);

        let operations_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1",
        )
        .bind(portfolio)
        .fetch_one(&pool)
        .await
        .expect("operation count should be available");
        assert_eq!(operations_count, 3);

        cleanup_worker_test_tree(&pool, user).await;
    }

    #[tokio::test]
    async fn backfill_skips_dates_before_creation_and_creates_zero_snapshot_without_operations() {
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let user = create_user(&pool, &suffix[..8]).await;
        let created_at = Date::from_calendar_date(2026, Month::June, 2)
            .unwrap()
            .midnight()
            .assume_utc();
        let portfolio = create_portfolio(&pool, user, "EUR", created_at).await;
        let d1 = Date::from_calendar_date(2026, Month::June, 1).unwrap();
        let d3 = Date::from_calendar_date(2026, Month::June, 3).unwrap();

        let job = backfill_job(portfolio, d1, d3);
        job.run(&test_state(pool.clone()))
            .await
            .expect("backfill should succeed for empty portfolio");

        let snapshots = sqlx::query(
            r#"
            SELECT snapshot_date::text AS snapshot_date, total_value_minor
            FROM portfolio_snapshots_daily
            WHERE id_portfolio = $1
            ORDER BY snapshot_date ASC
            "#,
        )
        .bind(portfolio)
        .fetch_all(&pool)
        .await
        .expect("snapshots should be queryable");

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].get::<String, _>("snapshot_date"), "2026-06-02");
        assert_eq!(snapshots[0].get::<i64, _>("total_value_minor"), 0);
        assert_eq!(snapshots[1].get::<String, _>("snapshot_date"), "2026-06-03");

        cleanup_worker_test_tree(&pool, user).await;
    }

    #[tokio::test]
    async fn backfill_missing_or_wrong_currency_price_falls_back_to_invested_cost_and_marks_estimated()
     {
        // Day 1: a historical price exists but is denominated in USD while the
        // portfolio's base is EUR (currency mismatch → cannot value live).
        // Day 2: no historical price is published at all.
        //
        // Both cases must hit the P0.2 valuation fallback:
        //   market_value_minor = invested_base_minor (trusted ledger)
        //   is_estimated       = true
        //
        // The snapshot stays consistent with cash (deposit 1000 − buy 600 = 400)
        // and the only open holding (invested 600 → estimated 600 → weight 100).
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let user = create_user(&pool, &suffix[..8]).await;
        let created_at = Date::from_calendar_date(2026, Month::June, 1)
            .unwrap()
            .midnight()
            .assume_utc();
        let portfolio = create_portfolio(&pool, user, "EUR", created_at).await;
        let asset = create_asset(&pool, &suffix[..6]).await;
        let d1 = Date::from_calendar_date(2026, Month::June, 1).unwrap();
        let d2 = Date::from_calendar_date(2026, Month::June, 2).unwrap();

        // Wrong-currency price on day 1; nothing for day 2.
        insert_historical_price(&pool, asset, d1, "USD", 999, "default", created_at).await;
        insert_operation(
            &pool,
            portfolio,
            OperationFixture {
                id_asset: None,
                id_related_asset: None,
                operation_type: "deposit",
                operation_status: "posted",
                executed_at: created_at + Duration::hours(9),
                quantity: None,
                related_quantity: None,
                price_minor: None,
                gross_amount_minor: Some(1_000),
                cash_amount_minor: 1_000,
                currency: "EUR",
                fx_rate_to_portfolio: None,
            },
        )
        .await;
        insert_operation(
            &pool,
            portfolio,
            OperationFixture {
                id_asset: Some(asset),
                id_related_asset: None,
                operation_type: "buy",
                operation_status: "posted",
                executed_at: created_at + Duration::hours(10),
                quantity: Some("2"),
                related_quantity: None,
                price_minor: Some(300),
                gross_amount_minor: Some(600),
                cash_amount_minor: 600,
                currency: "EUR",
                fx_rate_to_portfolio: None,
            },
        )
        .await;

        let job = backfill_job(portfolio, d1, d2);
        job.run(&test_state(pool.clone()))
            .await
            .expect("backfill should succeed with incompatible / missing historical prices");

        // --- Day 1: wrong-currency historical price → fallback. ---
        let day1 = sqlx::query(
            r#"
            SELECT total_value_minor, cash_balance_minor, total_invested_minor, is_estimated
            FROM portfolio_snapshots_daily
            WHERE id_portfolio = $1 AND snapshot_date = $2
            "#,
        )
        .bind(portfolio)
        .bind(d1)
        .fetch_one(&pool)
        .await
        .expect("day 1 snapshot should exist");
        assert_eq!(day1.get::<i64, _>("cash_balance_minor"), 400);
        assert_eq!(day1.get::<i64, _>("total_invested_minor"), 1_000);
        assert_eq!(day1.get::<i64, _>("total_value_minor"), 1_000);
        assert!(day1.get::<bool, _>("is_estimated"));

        let day1_holding = sqlx::query(
            r#"
            SELECT hs.market_value_minor, hs.invested_minor, hs.weight_pct::text AS weight_pct,
                   hs.pnl_minor, hs.is_estimated
            FROM portfolio_holding_snapshot_daily hs
            JOIN portfolio_snapshots_daily s
                ON s.id_portfolio_snapshot_daily = hs.id_portfolio_snapshot_daily
            WHERE s.id_portfolio = $1 AND s.snapshot_date = $2
            "#,
        )
        .bind(portfolio)
        .bind(d1)
        .fetch_one(&pool)
        .await
        .expect("day 1 holding should exist");
        assert_eq!(day1_holding.get::<i64, _>("invested_minor"), 600);
        assert_eq!(day1_holding.get::<i64, _>("market_value_minor"), 600);
        // P&L is coherent with the invested-cost fallback (no gain, no loss
        // because the estimated value mirrors the cost basis).
        assert_eq!(day1_holding.get::<i64, _>("pnl_minor"), 0);
        // Single valued holding → weight_pct must be exactly 100 (holdings-only
        // allocation contract from P0.2).
        assert_eq!(day1_holding.get::<String, _>("weight_pct"), "100.0000");
        assert!(day1_holding.get::<bool, _>("is_estimated"));

        // --- Day 2: missing historical price → same fallback. ---
        let day2 = sqlx::query(
            r#"
            SELECT total_value_minor, cash_balance_minor, total_invested_minor, is_estimated
            FROM portfolio_snapshots_daily
            WHERE id_portfolio = $1 AND snapshot_date = $2
            "#,
        )
        .bind(portfolio)
        .bind(d2)
        .fetch_one(&pool)
        .await
        .expect("day 2 snapshot should exist");
        assert_eq!(day2.get::<i64, _>("cash_balance_minor"), 400);
        assert_eq!(day2.get::<i64, _>("total_invested_minor"), 1_000);
        assert_eq!(day2.get::<i64, _>("total_value_minor"), 1_000);
        assert!(day2.get::<bool, _>("is_estimated"));

        let day2_holding = sqlx::query(
            r#"
            SELECT hs.market_value_minor, hs.invested_minor, hs.weight_pct::text AS weight_pct,
                   hs.pnl_minor, hs.is_estimated
            FROM portfolio_holding_snapshot_daily hs
            JOIN portfolio_snapshots_daily s
                ON s.id_portfolio_snapshot_daily = hs.id_portfolio_snapshot_daily
            WHERE s.id_portfolio = $1 AND s.snapshot_date = $2
            "#,
        )
        .bind(portfolio)
        .bind(d2)
        .fetch_one(&pool)
        .await
        .expect("day 2 holding should exist");
        assert_eq!(day2_holding.get::<i64, _>("invested_minor"), 600);
        assert_eq!(day2_holding.get::<i64, _>("market_value_minor"), 600);
        assert_eq!(day2_holding.get::<i64, _>("pnl_minor"), 0);
        assert_eq!(day2_holding.get::<String, _>("weight_pct"), "100.0000");
        assert!(day2_holding.get::<bool, _>("is_estimated"));

        cleanup_worker_test_tree(&pool, user).await;
    }
}
