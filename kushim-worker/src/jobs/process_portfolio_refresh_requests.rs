use crate::{
    config::{Config, RefreshConsumerConfig},
    errors::WorkerError,
    jobs::{Job, refresh_current_portfolio_state::RefreshCurrentPortfolioStateJob},
    repositories::refresh_requests::{ClaimedRefreshRequest, RefreshRequestRepository},
    state::AppState,
};
use async_trait::async_trait;
use time::OffsetDateTime;

/// Automatic consumer for the durable `portfolio_refresh_requests` queue.
///
/// On each invocation it:
///  1. fails requests that exhausted their attempt budget (no tight loop);
///  2. claims a batch of eligible requests with `FOR UPDATE SKIP LOCKED`;
///  3. for each claim, runs the existing per-portfolio composite refresh
///     (rebuild current read models + generate the current daily snapshot);
///  4. marks the request completed, or schedules a bounded retry / terminal
///     failure.
///
/// PostgreSQL is the durable queue — no Redis/queue infrastructure is used.
#[derive(Debug, Clone)]
pub struct ProcessPortfolioRefreshRequestsJob {
    refresh: RefreshConsumerConfig,
}

impl ProcessPortfolioRefreshRequestsJob {
    pub fn from_config(config: &Config) -> Self {
        Self {
            refresh: config.refresh_consumer,
        }
    }
}

#[async_trait]
impl Job for ProcessPortfolioRefreshRequestsJob {
    fn name(&self) -> &'static str {
        "process_portfolio_refresh_requests"
    }

    async fn run(&self, state: &AppState) -> Result<(), WorkerError> {
        let repository = RefreshRequestRepository::new(state.pg_pool.clone());
        let lock_timeout_secs = self.refresh.lock_timeout.as_secs() as i64;
        let retry_delay_secs = self.refresh.retry_delay.as_secs() as i64;

        // 1. Terminal-fail exhausted / abandoned-and-exhausted requests first.
        let failed_exhausted = repository
            .mark_exhausted_failed(self.refresh.max_attempts, lock_timeout_secs)
            .await?;
        if failed_exhausted > 0 {
            tracing::warn!(
                worker = %state.worker_name,
                job = self.name(),
                count = failed_exhausted,
                "marked refresh requests as failed after exhausting attempts"
            );
        }

        // 2. Claim a batch (also recovers stale 'processing' rows past the lock timeout).
        let claimed = repository
            .claim_batch(
                &state.worker_name,
                self.refresh.batch_size,
                self.refresh.max_attempts,
                lock_timeout_secs,
            )
            .await?;

        if claimed.is_empty() {
            return Ok(());
        }

        tracing::info!(
            worker = %state.worker_name,
            job = self.name(),
            claimed = claimed.len(),
            "claimed portfolio refresh requests"
        );

        let mut completed = 0_usize;
        let mut retried_or_failed = 0_usize;

        for request in claimed {
            match self.process_one(state, &request).await {
                Ok(()) => {
                    repository
                        .mark_completed(request.id_portfolio_refresh_request)
                        .await?;
                    completed += 1;
                    tracing::info!(
                        worker = %state.worker_name,
                        job = self.name(),
                        id_portfolio_refresh_request = %request.id_portfolio_refresh_request,
                        id_portfolio = %request.id_portfolio,
                        "completed portfolio refresh request"
                    );
                }
                Err(error) => {
                    let message = error.to_string();
                    tracing::error!(
                        worker = %state.worker_name,
                        job = self.name(),
                        id_portfolio_refresh_request = %request.id_portfolio_refresh_request,
                        id_portfolio = %request.id_portfolio,
                        attempts = request.attempts,
                        error = %message,
                        "portfolio refresh request processing failed"
                    );
                    repository
                        .mark_failed_or_retry(
                            &request,
                            &message,
                            self.refresh.max_attempts,
                            retry_delay_secs,
                        )
                        .await?;
                    retried_or_failed += 1;
                }
            }
        }

        tracing::info!(
            worker = %state.worker_name,
            job = self.name(),
            completed,
            retried_or_failed,
            "completed process portfolio refresh requests pass"
        );

        Ok(())
    }
}

impl ProcessPortfolioRefreshRequestsJob {
    /// Run the existing composite refresh for the request's target portfolio
    /// only. Reuses `RefreshCurrentPortfolioStateJob` (rebuild + snapshot)
    /// unchanged. The snapshot date is "today" (UTC).
    async fn process_one(
        &self,
        state: &AppState,
        request: &ClaimedRefreshRequest,
    ) -> Result<(), WorkerError> {
        let snapshot_date = OffsetDateTime::now_utc().date();
        let job =
            RefreshCurrentPortfolioStateJob::for_portfolio(request.id_portfolio, snapshot_date);
        job.run(state).await
    }
}

#[cfg(test)]
mod tests {
    use super::ProcessPortfolioRefreshRequestsJob;
    use crate::{
        config::RefreshConsumerConfig,
        jobs::Job,
        repositories::refresh_requests::{ClaimedRefreshRequest, RefreshRequestRepository},
        state::AppState,
        test_utils::lock_env,
    };
    use sqlx::{PgPool, Row, postgres::PgPoolOptions};
    use std::sync::OnceLock;
    use time::OffsetDateTime;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    // The consumer claims/fails refresh requests GLOBALLY (no portfolio filter),
    // which is correct in production but means these tests cannot run in
    // parallel against the shared dev database without claiming each other's
    // rows. Serialize them and scope every assertion to the test's own request.
    static CONSUMER_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    async fn serial_guard() -> tokio::sync::MutexGuard<'static, ()> {
        CONSUMER_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .await
    }

    fn contains_request(claimed: &[ClaimedRefreshRequest], id: Uuid) -> bool {
        claimed.iter().any(|r| r.id_portfolio_refresh_request == id)
    }

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
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    fn test_state(pool: PgPool) -> AppState {
        AppState {
            pg_pool: pool,
            worker_name: "refresh-consumer-test".to_string(),
        }
    }

    fn job_with(refresh: RefreshConsumerConfig) -> ProcessPortfolioRefreshRequestsJob {
        ProcessPortfolioRefreshRequestsJob { refresh }
    }

    async fn create_user(pool: &PgPool, suffix: &str) -> Uuid {
        crate::test_utils::ensure_canonical_user_role(pool).await;
        let id_user = Uuid::new_v4();
        let handle = format!("prr{}", suffix);
        sqlx::query(
            "INSERT INTO users (id_user, id_role, username, public_handle, password_hash) VALUES ($1, 1, $2, $3, '$argon2id$prr')",
        )
        .bind(id_user)
        .bind(&handle)
        .bind(&handle)
        .execute(pool)
        .await
        .expect("user should be inserted");
        id_user
    }

    async fn create_portfolio(pool: &PgPool, id_user: Uuid) -> Uuid {
        let id_portfolio = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO portfolios (id_portfolio, id_user, name, base_currency, visibility) VALUES ($1, $2, $3, 'EUR', 'private')",
        )
        .bind(id_portfolio)
        .bind(id_user)
        .bind(format!("pf{}", &id_portfolio.simple().to_string()[..12]))
        .execute(pool)
        .await
        .expect("portfolio should be inserted");
        id_portfolio
    }

    async fn insert_posted_deposit(pool: &PgPool, id_portfolio: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO portfolio_operations (
                id_portfolio_operation, id_portfolio, operation_type, operation_status,
                executed_at, gross_amount_minor, cash_amount_minor, currency, metadata
            )
            VALUES ($1, $2, 'deposit', 'posted', $3, 1000, 1000, 'EUR', '{}'::jsonb)
            "#,
        )
        .bind(id)
        .bind(id_portfolio)
        .bind(OffsetDateTime::now_utc())
        .execute(pool)
        .await
        .expect("deposit should be inserted");
        id
    }

    async fn enqueue_pending(pool: &PgPool, id_portfolio: Uuid) -> Uuid {
        let row = sqlx::query(
            "INSERT INTO portfolio_refresh_requests (id_portfolio, status) VALUES ($1, 'pending') RETURNING id_portfolio_refresh_request",
        )
        .bind(id_portfolio)
        .fetch_one(pool)
        .await
        .expect("pending request inserted");
        row.get("id_portfolio_refresh_request")
    }

    async fn request_status(pool: &PgPool, id: Uuid) -> String {
        sqlx::query(
            "SELECT status FROM portfolio_refresh_requests WHERE id_portfolio_refresh_request = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("request should exist")
        .get::<String, _>("status")
    }

    async fn cleanup(pool: &PgPool, id_portfolio: Uuid) {
        // Refresh requests, read models and snapshots are deletable; posted
        // operations are immutable and intentionally left in place.
        sqlx::query("DELETE FROM portfolio_refresh_requests WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .execute(pool)
            .await
            .ok();
    }

    #[tokio::test]
    async fn job_completes_request_and_builds_read_models() {
        let _serial = serial_guard().await;
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let id_user = create_user(&pool, &suffix[..10]).await;
        let id_portfolio = create_portfolio(&pool, id_user).await;
        insert_posted_deposit(&pool, id_portfolio).await;
        let id_request = enqueue_pending(&pool, id_portfolio).await;

        let job = job_with(RefreshConsumerConfig::default());
        job.run(&test_state(pool.clone()))
            .await
            .expect("consumer pass should succeed");

        let status = request_status(&pool, id_request).await;
        let summary_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM rm_portfolio_summary WHERE id_portfolio = $1",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .unwrap();
        let snapshot_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM portfolio_snapshots_daily WHERE id_portfolio = $1",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .unwrap();

        cleanup(&pool, id_portfolio).await;

        assert_eq!(status, "completed");
        assert_eq!(
            summary_count, 1,
            "current read model summary should be built"
        );
        assert_eq!(
            snapshot_count, 1,
            "current daily snapshot should be created"
        );
    }

    #[tokio::test]
    async fn claim_batch_is_exclusive_and_marks_processing() {
        let _serial = serial_guard().await;
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let id_user = create_user(&pool, &suffix[..10]).await;
        let id_portfolio = create_portfolio(&pool, id_user).await;
        let id_request = enqueue_pending(&pool, id_portfolio).await;

        let repo = RefreshRequestRepository::new(pool.clone());
        let first = repo
            .claim_batch("worker-a", 10, 5, 300)
            .await
            .expect("first claim should succeed");
        let second = repo
            .claim_batch("worker-b", 10, 5, 300)
            .await
            .expect("second claim should succeed");

        let status = request_status(&pool, id_request).await;
        cleanup(&pool, id_portfolio).await;

        assert!(
            contains_request(&first, id_request),
            "first claim takes our pending request"
        );
        assert!(
            !contains_request(&second, id_request),
            "a second concurrent claim must not re-claim the same processing row"
        );
        assert_eq!(status, "processing");
    }

    #[tokio::test]
    async fn mark_failed_or_retry_reschedules_then_fails() {
        let _serial = serial_guard().await;
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let id_user = create_user(&pool, &suffix[..10]).await;
        let id_portfolio = create_portfolio(&pool, id_user).await;
        let id_request = enqueue_pending(&pool, id_portfolio).await;
        let repo = RefreshRequestRepository::new(pool.clone());

        // attempts (1) < max (3): retryable -> back to pending.
        repo.mark_failed_or_retry(
            &ClaimedRefreshRequest {
                id_portfolio_refresh_request: id_request,
                id_portfolio,
                attempts: 1,
            },
            "boom",
            3,
            30,
        )
        .await
        .unwrap();
        assert_eq!(request_status(&pool, id_request).await, "pending");

        // attempts (3) >= max (3): terminal failure.
        repo.mark_failed_or_retry(
            &ClaimedRefreshRequest {
                id_portfolio_refresh_request: id_request,
                id_portfolio,
                attempts: 3,
            },
            "boom again",
            3,
            30,
        )
        .await
        .unwrap();
        assert_eq!(request_status(&pool, id_request).await, "failed");

        cleanup(&pool, id_portfolio).await;
    }

    #[tokio::test]
    async fn stale_processing_request_is_reclaimable() {
        let _serial = serial_guard().await;
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let id_user = create_user(&pool, &suffix[..10]).await;
        let id_portfolio = create_portfolio(&pool, id_user).await;
        // A 'processing' row abandoned by a dead worker 10 minutes ago, attempts below max.
        let id_request = sqlx::query(
            r#"
            INSERT INTO portfolio_refresh_requests
                (id_portfolio, status, attempts, requested_at, processing_started_at, next_attempt_at, locked_by)
            VALUES ($1, 'processing', 1, now() - interval '20 minutes', now() - interval '10 minutes', now() - interval '10 minutes', 'dead-worker')
            RETURNING id_portfolio_refresh_request
            "#,
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get::<Uuid, _>("id_portfolio_refresh_request");

        let repo = RefreshRequestRepository::new(pool.clone());
        // lock timeout 60s -> the 10-minute-old processing row is reclaimable.
        let claimed = repo.claim_batch("worker-c", 10, 5, 60).await.unwrap();
        let ours = claimed
            .iter()
            .find(|r| r.id_portfolio_refresh_request == id_request)
            .cloned();

        cleanup(&pool, id_portfolio).await;

        let ours = ours.expect("stale processing row should be reclaimed");
        assert_eq!(ours.attempts, 2, "attempts incremented on reclaim");
    }

    #[tokio::test]
    async fn request_posted_during_processing_remains_pending() {
        let _serial = serial_guard().await;
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let id_user = create_user(&pool, &suffix[..10]).await;
        let id_portfolio = create_portfolio(&pool, id_user).await;

        // A fresh 'processing' row (worker still alive) plus a new 'pending' row
        // for the same portfolio (operation posted during processing).
        sqlx::query(
            "INSERT INTO portfolio_refresh_requests (id_portfolio, status, attempts, processing_started_at, next_attempt_at, locked_by) VALUES ($1, 'processing', 1, now(), now() + interval '5 minutes', 'alive-worker')",
        )
        .bind(id_portfolio)
        .execute(&pool)
        .await
        .unwrap();
        let id_pending = enqueue_pending(&pool, id_portfolio).await;

        let repo = RefreshRequestRepository::new(pool.clone());
        let claimed = repo.claim_batch("worker-d", 10, 5, 300).await.unwrap();
        // Our fresh processing row must remain pending-only-claimable: the
        // pending row is claimed, the alive processing row is not.
        let processing_for_portfolio = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM portfolio_refresh_requests WHERE id_portfolio = $1 AND status = 'processing' AND locked_by = 'alive-worker'",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .unwrap();

        cleanup(&pool, id_portfolio).await;

        assert!(
            contains_request(&claimed, id_pending),
            "the pending row is claimable"
        );
        assert_eq!(
            processing_for_portfolio, 1,
            "the alive processing row is untouched"
        );
    }

    #[tokio::test]
    async fn mark_exhausted_failed_terminates_stale_exhausted_processing() {
        let _serial = serial_guard().await;
        let pool = test_pool().await;
        let suffix = Uuid::new_v4().simple().to_string();
        let id_user = create_user(&pool, &suffix[..10]).await;
        let id_portfolio = create_portfolio(&pool, id_user).await;
        let id_request = sqlx::query(
            r#"
            INSERT INTO portfolio_refresh_requests
                (id_portfolio, status, attempts, requested_at, processing_started_at, next_attempt_at, locked_by)
            VALUES ($1, 'processing', 5, now() - interval '20 minutes', now() - interval '10 minutes', now() - interval '10 minutes', 'dead-worker')
            RETURNING id_portfolio_refresh_request
            "#,
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get::<Uuid, _>("id_portfolio_refresh_request");

        let repo = RefreshRequestRepository::new(pool.clone());
        let failed = repo.mark_exhausted_failed(5, 60).await.unwrap();
        let status = request_status(&pool, id_request).await;

        cleanup(&pool, id_portfolio).await;

        assert!(failed >= 1);
        assert_eq!(status, "failed");
    }
}
