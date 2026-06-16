// P3 durable operation idempotency repository.
//
// Backs the `portfolio_operation_idempotency` table. Provides two
// transactional primitives used by the service layer:
//
//   * `claim_or_conflict_in_tx`: try to claim the (id_user, idempotency_key)
//     slot inside the caller's transaction. Returns `Claimed` on the winning
//     path (the caller proceeds with the operation insert and finalizes the
//     record with `finalize_in_tx`), or `Conflict` when the row already
//     existed. ON CONFLICT DO NOTHING resolves concurrent races: only one
//     transaction can be the winner; losers see `Conflict` and replay.
//   * `find_by_user_and_key`: read-only lookup used to load a previously
//     committed record and replay its result.

use crate::domain::portfolio_operation::PortfolioOperation;
use crate::domain::portfolio_refresh_request::PortfolioRefreshRequest;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Stored record for a successful (or in-progress, in the winning transaction)
/// idempotency claim.
#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    pub id_portfolio_operation_idempotency: Uuid,
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub idempotency_key: Uuid,
    pub request_kind: IdempotencyRequestKind,
    pub id_corrected_operation: Option<Uuid>,
    pub request_fingerprint: Value,
    pub id_portfolio_operation: Option<Uuid>,
    pub id_portfolio_refresh_request: Option<Uuid>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotencyRequestKind {
    CreateOperation,
    CreateCorrection,
}

impl IdempotencyRequestKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateOperation => "create_operation",
            Self::CreateCorrection => "create_correction",
        }
    }

    pub fn try_from_str(value: &str) -> Option<Self> {
        match value {
            "create_operation" => Some(Self::CreateOperation),
            "create_correction" => Some(Self::CreateCorrection),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum IdempotencyRepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("invalid idempotency row")]
    InvalidRow,
}

/// Outcome of the in-transaction claim attempt.
pub enum ClaimOutcome {
    /// We won the race: the placeholder row is now ours and the caller must
    /// finalize it within the same transaction (`finalize_in_tx`).
    Claimed { id_record: Uuid },
    /// Another committed transaction already owns this `(id_user, key)`.
    /// The caller must abort and replay using `find_by_user_and_key`.
    Conflict,
}

#[derive(Clone)]
pub struct PortfolioOperationIdempotencyRepository {
    pool: PgPool,
}

impl PortfolioOperationIdempotencyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Read-only lookup by (id_user, idempotency_key). Used by the service
    /// layer to replay a previously committed result. Returns `None` if no
    /// record exists for that user/key pair.
    ///
    /// The user scoping is intentional: an idempotency key reused by a
    /// DIFFERENT user must not collide with another user's row, and must not
    /// leak the existence of that row.
    pub async fn find_by_user_and_key(
        &self,
        id_user: Uuid,
        idempotency_key: Uuid,
    ) -> Result<Option<IdempotencyRecord>, IdempotencyRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id_portfolio_operation_idempotency,
                id_user,
                id_portfolio,
                idempotency_key,
                request_kind,
                id_corrected_operation,
                request_fingerprint,
                id_portfolio_operation,
                id_portfolio_refresh_request,
                created_at
            FROM portfolio_operation_idempotency
            WHERE id_user = $1
              AND idempotency_key = $2
            "#,
        )
        .bind(id_user)
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        row.map(record_from_row).transpose()
    }

    /// Attempt to claim `(id_user, idempotency_key)` inside the caller's
    /// transaction. Returns `Conflict` when the slot already exists; otherwise
    /// returns the freshly-claimed row id so the caller can `finalize_in_tx`
    /// once it has inserted the operation (and optional refresh request).
    ///
    /// Concurrency: two simultaneous claims with the same key serialize on the
    /// unique index. The loser sees `Conflict` only after the winner commits,
    /// which is why a `Conflict` reply is always safe to replay.
    pub async fn claim_or_conflict_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        id_user: Uuid,
        id_portfolio: Uuid,
        idempotency_key: Uuid,
        request_kind: IdempotencyRequestKind,
        id_corrected_operation: Option<Uuid>,
        request_fingerprint: &Value,
    ) -> Result<ClaimOutcome, IdempotencyRepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO portfolio_operation_idempotency (
                id_user,
                id_portfolio,
                idempotency_key,
                request_kind,
                id_corrected_operation,
                request_fingerprint
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id_user, idempotency_key) DO NOTHING
            RETURNING id_portfolio_operation_idempotency
            "#,
        )
        .bind(id_user)
        .bind(id_portfolio)
        .bind(idempotency_key)
        .bind(request_kind.as_str())
        .bind(id_corrected_operation)
        .bind(request_fingerprint)
        .fetch_optional(&mut **tx)
        .await?;

        match row {
            Some(row) => Ok(ClaimOutcome::Claimed {
                id_record: row.try_get("id_portfolio_operation_idempotency")?,
            }),
            None => Ok(ClaimOutcome::Conflict),
        }
    }

    /// Finalize a claimed record by attaching the freshly-inserted operation
    /// id and (optional) refresh-request id. Must run inside the SAME
    /// transaction as the operation insert.
    pub async fn finalize_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        id_record: Uuid,
        operation: &PortfolioOperation,
        refresh: Option<&PortfolioRefreshRequest>,
    ) -> Result<(), IdempotencyRepositoryError> {
        sqlx::query(
            r#"
            UPDATE portfolio_operation_idempotency
            SET
                id_portfolio_operation = $2,
                id_portfolio_refresh_request = $3
            WHERE id_portfolio_operation_idempotency = $1
            "#,
        )
        .bind(id_record)
        .bind(operation.id_portfolio_operation)
        .bind(refresh.map(|r| r.id_portfolio_refresh_request))
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}

fn record_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<IdempotencyRecord, IdempotencyRepositoryError> {
    let kind_text: String = row.try_get("request_kind")?;
    let request_kind = IdempotencyRequestKind::try_from_str(&kind_text)
        .ok_or(IdempotencyRepositoryError::InvalidRow)?;

    Ok(IdempotencyRecord {
        id_portfolio_operation_idempotency: row.try_get("id_portfolio_operation_idempotency")?,
        id_user: row.try_get("id_user")?,
        id_portfolio: row.try_get("id_portfolio")?,
        idempotency_key: row.try_get("idempotency_key")?,
        request_kind,
        id_corrected_operation: row.try_get("id_corrected_operation")?,
        request_fingerprint: row.try_get("request_fingerprint")?,
        id_portfolio_operation: row.try_get("id_portfolio_operation")?,
        id_portfolio_refresh_request: row.try_get("id_portfolio_refresh_request")?,
        created_at: row.try_get("created_at")?,
    })
}
