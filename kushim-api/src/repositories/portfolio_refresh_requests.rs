use crate::domain::portfolio_refresh_request::{PortfolioRefreshRequest, RefreshRequestStatus};
use sqlx::{PgPool, Postgres, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone)]
pub struct PortfolioRefreshRequestRepository {
    pool: PgPool,
}

#[derive(Debug, Error)]
pub enum PortfolioRefreshRequestRepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("invalid refresh request row")]
    InvalidRow,
}

/// Enqueue (or coalesce) a pending portfolio refresh request inside an
/// existing transaction. This MUST be called in the same transaction that
/// posts the operation so the operation state and the refresh request commit
/// atomically.
///
/// Coalescing: the partial unique index
/// `uq_portfolio_refresh_requests_pending_per_portfolio` guarantees at most
/// one pending row per portfolio. If a pending row already exists it is reused
/// (same id returned) and its `requested_at` / triggering operation are
/// refreshed. If the previous request is already `processing`, a fresh pending
/// row is created so an operation posted during processing is not lost.
pub async fn enqueue_refresh_request_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    id_portfolio: Uuid,
    id_triggering_operation: Option<Uuid>,
) -> Result<PortfolioRefreshRequest, PortfolioRefreshRequestRepositoryError> {
    let row = sqlx::query(
        r#"
        INSERT INTO portfolio_refresh_requests (
            id_portfolio,
            id_triggering_operation,
            status,
            requested_at,
            next_attempt_at
        )
        VALUES ($1, $2, 'pending', now(), now())
        ON CONFLICT (id_portfolio) WHERE status = 'pending'
        DO UPDATE SET
            id_triggering_operation = EXCLUDED.id_triggering_operation,
            requested_at = now(),
            next_attempt_at = now(),
            updated_at = now()
        RETURNING
            id_portfolio_refresh_request,
            id_portfolio,
            status,
            attempts,
            requested_at,
            processing_started_at,
            completed_at,
            (last_error IS NOT NULL) AS has_error,
            updated_at
        "#,
    )
    .bind(id_portfolio)
    .bind(id_triggering_operation)
    .fetch_one(&mut **tx)
    .await?;

    refresh_request_from_row(&row)
}

impl PortfolioRefreshRequestRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Look up a refresh request scoped to a portfolio. Ownership of the
    /// portfolio is verified separately at the service layer.
    pub async fn find_by_id_and_portfolio(
        &self,
        id_refresh_request: Uuid,
        id_portfolio: Uuid,
    ) -> Result<Option<PortfolioRefreshRequest>, PortfolioRefreshRequestRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id_portfolio_refresh_request,
                id_portfolio,
                status,
                attempts,
                requested_at,
                processing_started_at,
                completed_at,
                (last_error IS NOT NULL) AS has_error,
                updated_at
            FROM portfolio_refresh_requests
            WHERE id_portfolio_refresh_request = $1
              AND id_portfolio = $2
            "#,
        )
        .bind(id_refresh_request)
        .bind(id_portfolio)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| refresh_request_from_row(&row)).transpose()
    }
}

fn refresh_request_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<PortfolioRefreshRequest, PortfolioRefreshRequestRepositoryError> {
    let status_text: String = row.try_get("status")?;
    let status = RefreshRequestStatus::try_from(status_text.as_str())
        .map_err(|_| PortfolioRefreshRequestRepositoryError::InvalidRow)?;

    Ok(PortfolioRefreshRequest {
        id_portfolio_refresh_request: row.try_get("id_portfolio_refresh_request")?,
        id_portfolio: row.try_get("id_portfolio")?,
        status,
        attempts: row.try_get("attempts")?,
        requested_at: row.try_get("requested_at")?,
        processing_started_at: row.try_get("processing_started_at")?,
        completed_at: row.try_get("completed_at")?,
        has_error: row.try_get("has_error")?,
        updated_at: row.try_get("updated_at")?,
    })
}
