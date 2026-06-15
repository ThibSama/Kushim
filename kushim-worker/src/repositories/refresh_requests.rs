use crate::errors::WorkerError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Maximum length of a stored `last_error` diagnostic. Longer errors are
/// truncated so a pathological provider/DB message cannot bloat the row.
const MAX_STORED_ERROR_LEN: usize = 2000;

/// A refresh request claimed for processing by this worker.
#[derive(Debug, Clone)]
pub struct ClaimedRefreshRequest {
    pub id_portfolio_refresh_request: Uuid,
    pub id_portfolio: Uuid,
    pub attempts: i32,
}

#[derive(Clone)]
pub struct RefreshRequestRepository {
    pool: PgPool,
}

impl RefreshRequestRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Mark requests that have exhausted their attempt budget as terminally
    /// `failed`, so the consumer never tight-loops on a permanently broken
    /// request. Covers both pending rows and stale `processing` rows (whose
    /// worker died) that are past the lock timeout.
    pub async fn mark_exhausted_failed(
        &self,
        max_attempts: i32,
        lock_timeout_secs: i64,
    ) -> Result<u64, WorkerError> {
        let result = sqlx::query(
            r#"
            UPDATE portfolio_refresh_requests
            SET status = 'failed',
                completed_at = now()
            WHERE attempts >= $1
              AND (
                    status = 'pending'
                    OR (status = 'processing'
                        AND processing_started_at < now() - make_interval(secs => $2::int))
                  )
            "#,
        )
        .bind(max_attempts)
        .bind(lock_timeout_secs as i32)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Atomically claim up to `batch_size` eligible refresh requests using
    /// `FOR UPDATE SKIP LOCKED`, so two workers never claim the same row. The
    /// claim transaction only marks rows `processing` (sets owner, start time,
    /// increments attempts, and pushes `next_attempt_at` past the lock timeout
    /// so a crashed worker's row becomes retryable later). The heavy rebuild is
    /// performed OUTSIDE this transaction.
    ///
    /// Eligible rows are: `pending` whose `next_attempt_at` is due, or
    /// `processing` whose `processing_started_at` is older than the lock
    /// timeout (crash recovery). Rows that already reached `max_attempts` are
    /// excluded (they are failed by `mark_exhausted_failed`).
    pub async fn claim_batch(
        &self,
        worker_name: &str,
        batch_size: i64,
        max_attempts: i32,
        lock_timeout_secs: i64,
    ) -> Result<Vec<ClaimedRefreshRequest>, WorkerError> {
        let mut tx = self.pool.begin().await?;

        let candidate_rows = sqlx::query(
            r#"
            SELECT id_portfolio_refresh_request
            FROM portfolio_refresh_requests
            WHERE attempts < $1
              AND (
                    (status = 'pending' AND next_attempt_at <= now())
                    OR (status = 'processing'
                        AND processing_started_at < now() - make_interval(secs => $2::int))
                  )
            ORDER BY next_attempt_at ASC
            LIMIT $3
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(max_attempts)
        .bind(lock_timeout_secs as i32)
        .bind(batch_size)
        .fetch_all(&mut *tx)
        .await?;

        let candidate_ids: Vec<Uuid> = candidate_rows
            .iter()
            .map(|row| row.get::<Uuid, _>("id_portfolio_refresh_request"))
            .collect();

        if candidate_ids.is_empty() {
            tx.rollback().await?;
            return Ok(Vec::new());
        }

        let claimed_rows = sqlx::query(
            r#"
            UPDATE portfolio_refresh_requests
            SET status = 'processing',
                processing_started_at = now(),
                attempts = attempts + 1,
                locked_by = $2,
                next_attempt_at = now() + make_interval(secs => $3::int)
            WHERE id_portfolio_refresh_request = ANY($1)
            RETURNING id_portfolio_refresh_request, id_portfolio, attempts
            "#,
        )
        .bind(&candidate_ids)
        .bind(worker_name)
        .bind(lock_timeout_secs as i32)
        .fetch_all(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(claimed_rows
            .into_iter()
            .map(|row| ClaimedRefreshRequest {
                id_portfolio_refresh_request: row.get("id_portfolio_refresh_request"),
                id_portfolio: row.get("id_portfolio"),
                attempts: row.get("attempts"),
            })
            .collect())
    }

    /// Mark a request completed after both refresh steps succeeded.
    pub async fn mark_completed(
        &self,
        id_portfolio_refresh_request: Uuid,
    ) -> Result<(), WorkerError> {
        sqlx::query(
            r#"
            UPDATE portfolio_refresh_requests
            SET status = 'completed',
                completed_at = now(),
                last_error = NULL
            WHERE id_portfolio_refresh_request = $1
            "#,
        )
        .bind(id_portfolio_refresh_request)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record a failed attempt. If the attempt budget is exhausted the request
    /// becomes terminally `failed`; otherwise it returns to `pending` with a
    /// backed-off `next_attempt_at` for a bounded retry. The raw error is
    /// truncated and kept only for diagnostics.
    pub async fn mark_failed_or_retry(
        &self,
        request: &ClaimedRefreshRequest,
        error_message: &str,
        max_attempts: i32,
        retry_delay_secs: i64,
    ) -> Result<(), WorkerError> {
        let truncated: String = error_message.chars().take(MAX_STORED_ERROR_LEN).collect();

        if request.attempts >= max_attempts {
            sqlx::query(
                r#"
                UPDATE portfolio_refresh_requests
                SET status = 'failed',
                    completed_at = now(),
                    last_error = $2
                WHERE id_portfolio_refresh_request = $1
                "#,
            )
            .bind(request.id_portfolio_refresh_request)
            .bind(truncated)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE portfolio_refresh_requests
                SET status = 'pending',
                    processing_started_at = NULL,
                    next_attempt_at = now() + make_interval(secs => $3::int),
                    last_error = $2
                WHERE id_portfolio_refresh_request = $1
                "#,
            )
            .bind(request.id_portfolio_refresh_request)
            .bind(truncated)
            .bind(retry_delay_secs as i32)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }
}
