use crate::domain::portfolio_operation::{
    NewPortfolioOperation, OperationStatus, OperationType, PortfolioOperation,
    PortfolioOperationFilters, UpdatePortfolioOperation,
};
use crate::domain::portfolio_refresh_request::PortfolioRefreshRequest;
use crate::repositories::portfolio_refresh_requests::enqueue_refresh_request_in_tx;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Shared INSERT column list / RETURNING projection for portfolio_operations.
const OPERATION_INSERT_SQL: &str = r#"
INSERT INTO portfolio_operations (
    id_portfolio,
    id_asset,
    id_related_asset,
    operation_type,
    operation_status,
    executed_at,
    effective_at,
    quantity,
    related_quantity,
    price_minor,
    gross_amount_minor,
    fees_minor,
    taxes_minor,
    cash_amount_minor,
    currency,
    fx_rate_to_portfolio,
    external_provider,
    external_reference,
    id_corrected_operation,
    notes,
    metadata
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7,
    $8::numeric, $9::numeric, $10, $11, $12, $13, $14, $15,
    $16::numeric, $17, $18, $19, $20, $21
)
RETURNING
    id_portfolio_operation,
    id_portfolio,
    id_asset,
    id_related_asset,
    operation_type,
    operation_status,
    executed_at,
    effective_at,
    quantity::text AS quantity,
    related_quantity::text AS related_quantity,
    price_minor,
    gross_amount_minor,
    fees_minor,
    taxes_minor,
    cash_amount_minor,
    currency,
    fx_rate_to_portfolio::text AS fx_rate_to_portfolio,
    external_provider,
    external_reference,
    id_corrected_operation,
    notes,
    metadata,
    created_at,
    updated_at
"#;

#[derive(Clone)]
pub struct PortfolioOperationRepository {
    pool: PgPool,
}

#[derive(Debug, Error)]
pub enum PortfolioOperationRepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("invalid operation row")]
    InvalidRow,
}

struct PortfolioOperationRow {
    id_portfolio_operation: Uuid,
    id_portfolio: Uuid,
    id_asset: Option<Uuid>,
    id_related_asset: Option<Uuid>,
    operation_type: String,
    operation_status: String,
    executed_at: OffsetDateTime,
    effective_at: Option<OffsetDateTime>,
    quantity: Option<String>,
    related_quantity: Option<String>,
    price_minor: Option<i64>,
    gross_amount_minor: Option<i64>,
    fees_minor: Option<i64>,
    taxes_minor: Option<i64>,
    cash_amount_minor: i64,
    currency: String,
    fx_rate_to_portfolio: Option<String>,
    external_provider: Option<String>,
    external_reference: Option<String>,
    id_corrected_operation: Option<Uuid>,
    notes: Option<String>,
    metadata: Value,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<PortfolioOperationRow> for PortfolioOperation {
    type Error = PortfolioOperationRepositoryError;

    fn try_from(value: PortfolioOperationRow) -> Result<Self, Self::Error> {
        let operation_type = OperationType::try_from(value.operation_type.as_str())
            .map_err(|_| PortfolioOperationRepositoryError::InvalidRow)?;
        let operation_status = OperationStatus::try_from(value.operation_status.as_str())
            .map_err(|_| PortfolioOperationRepositoryError::InvalidRow)?;

        Ok(Self {
            id_portfolio_operation: value.id_portfolio_operation,
            id_portfolio: value.id_portfolio,
            id_asset: value.id_asset,
            id_related_asset: value.id_related_asset,
            operation_type,
            operation_status,
            executed_at: value.executed_at,
            effective_at: value.effective_at,
            quantity: value.quantity,
            related_quantity: value.related_quantity,
            price_minor: value.price_minor,
            gross_amount_minor: value.gross_amount_minor,
            fees_minor: value.fees_minor,
            taxes_minor: value.taxes_minor,
            cash_amount_minor: value.cash_amount_minor,
            currency: value.currency.trim().to_string(),
            fx_rate_to_portfolio: value.fx_rate_to_portfolio,
            external_provider: value.external_provider,
            external_reference: value.external_reference,
            id_corrected_operation: value.id_corrected_operation,
            notes: value.notes,
            metadata: value.metadata,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl PortfolioOperationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert an operation and, when it is created directly as `posted`,
    /// atomically enqueue a portfolio refresh request in the SAME transaction.
    /// For non-posted creations no refresh request is enqueued and `None` is
    /// returned. This is the path used by both direct posted creation and
    /// posted correction creation.
    pub async fn create_with_optional_refresh(
        &self,
        input: &NewPortfolioOperation,
    ) -> Result<
        (PortfolioOperation, Option<PortfolioRefreshRequest>),
        PortfolioOperationRepositoryError,
    > {
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;

        let row = bind_operation_insert(sqlx::query(OPERATION_INSERT_SQL), input)
            .fetch_one(&mut *tx)
            .await?;
        let operation = operation_from_row(&row)?;

        let refresh = if operation.operation_status == OperationStatus::Posted {
            Some(
                enqueue_refresh_request_in_tx(
                    &mut tx,
                    operation.id_portfolio,
                    Some(operation.id_portfolio_operation),
                )
                .await
                .map_err(map_refresh_error)?,
            )
        } else {
            None
        };

        tx.commit().await?;
        Ok((operation, refresh))
    }

    pub async fn create(
        &self,
        input: &NewPortfolioOperation,
    ) -> Result<PortfolioOperation, PortfolioOperationRepositoryError> {
        let row = bind_operation_insert(sqlx::query(OPERATION_INSERT_SQL), input)
            .fetch_one(&self.pool)
            .await?;

        operation_from_row(&row)
    }

    /// Transition a pending operation to `posted` and atomically enqueue a
    /// portfolio refresh request in the SAME transaction. Returns `None` when
    /// the operation does not exist for the given portfolio.
    pub async fn set_status_posted_with_refresh(
        &self,
        id_portfolio_operation: Uuid,
        id_portfolio: Uuid,
    ) -> Result<
        Option<(PortfolioOperation, PortfolioRefreshRequest)>,
        PortfolioOperationRepositoryError,
    > {
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;

        let row = sqlx::query(
            r#"
            UPDATE portfolio_operations
            SET operation_status = 'posted'
            WHERE id_portfolio_operation = $1
              AND id_portfolio = $2
            RETURNING
                id_portfolio_operation,
                id_portfolio,
                id_asset,
                id_related_asset,
                operation_type,
                operation_status,
                executed_at,
                effective_at,
                quantity::text AS quantity,
                related_quantity::text AS related_quantity,
                price_minor,
                gross_amount_minor,
                fees_minor,
                taxes_minor,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio::text AS fx_rate_to_portfolio,
                external_provider,
                external_reference,
                id_corrected_operation,
                notes,
                metadata,
                created_at,
                updated_at
            "#,
        )
        .bind(id_portfolio_operation)
        .bind(id_portfolio)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(row) = row else {
            tx.rollback().await?;
            return Ok(None);
        };

        let operation = operation_from_row(&row)?;
        let refresh = enqueue_refresh_request_in_tx(
            &mut tx,
            operation.id_portfolio,
            Some(operation.id_portfolio_operation),
        )
        .await
        .map_err(map_refresh_error)?;

        tx.commit().await?;
        Ok(Some((operation, refresh)))
    }

    pub async fn list_by_portfolio(
        &self,
        id_portfolio: Uuid,
        filters: &PortfolioOperationFilters,
    ) -> Result<Vec<PortfolioOperation>, PortfolioOperationRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id_portfolio_operation,
                id_portfolio,
                id_asset,
                id_related_asset,
                operation_type,
                operation_status,
                executed_at,
                effective_at,
                quantity::text AS quantity,
                related_quantity::text AS related_quantity,
                price_minor,
                gross_amount_minor,
                fees_minor,
                taxes_minor,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio::text AS fx_rate_to_portfolio,
                external_provider,
                external_reference,
                id_corrected_operation,
                notes,
                metadata,
                created_at,
                updated_at
            FROM portfolio_operations
            WHERE id_portfolio = $1
              AND ($2::varchar IS NULL OR operation_status = $2)
              AND ($3::varchar IS NULL OR operation_type = $3)
              AND ($4::uuid IS NULL OR id_asset = $4)
            ORDER BY executed_at DESC, created_at DESC
            "#,
        )
        .bind(id_portfolio)
        .bind(
            filters
                .operation_status
                .as_ref()
                .map(OperationStatus::as_str),
        )
        .bind(filters.operation_type.as_ref().map(OperationType::as_str))
        .bind(filters.id_asset)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| operation_from_row(&row))
            .collect()
    }

    pub async fn find_by_id_and_portfolio(
        &self,
        id_portfolio_operation: Uuid,
        id_portfolio: Uuid,
    ) -> Result<Option<PortfolioOperation>, PortfolioOperationRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id_portfolio_operation,
                id_portfolio,
                id_asset,
                id_related_asset,
                operation_type,
                operation_status,
                executed_at,
                effective_at,
                quantity::text AS quantity,
                related_quantity::text AS related_quantity,
                price_minor,
                gross_amount_minor,
                fees_minor,
                taxes_minor,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio::text AS fx_rate_to_portfolio,
                external_provider,
                external_reference,
                id_corrected_operation,
                notes,
                metadata,
                created_at,
                updated_at
            FROM portfolio_operations
            WHERE id_portfolio_operation = $1
              AND id_portfolio = $2
            "#,
        )
        .bind(id_portfolio_operation)
        .bind(id_portfolio)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| operation_from_row(&row)).transpose()
    }

    pub async fn update(
        &self,
        id_portfolio_operation: Uuid,
        id_portfolio: Uuid,
        input: &UpdatePortfolioOperation,
    ) -> Result<Option<PortfolioOperation>, PortfolioOperationRepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE portfolio_operations
            SET
                id_asset = $3,
                id_related_asset = $4,
                operation_type = $5,
                executed_at = $6,
                effective_at = $7,
                quantity = $8::numeric,
                related_quantity = $9::numeric,
                price_minor = $10,
                gross_amount_minor = $11,
                fees_minor = $12,
                taxes_minor = $13,
                cash_amount_minor = $14,
                currency = $15,
                fx_rate_to_portfolio = $16::numeric,
                external_provider = $17,
                external_reference = $18,
                id_corrected_operation = $19,
                notes = $20,
                metadata = $21
            WHERE id_portfolio_operation = $1
              AND id_portfolio = $2
            RETURNING
                id_portfolio_operation,
                id_portfolio,
                id_asset,
                id_related_asset,
                operation_type,
                operation_status,
                executed_at,
                effective_at,
                quantity::text AS quantity,
                related_quantity::text AS related_quantity,
                price_minor,
                gross_amount_minor,
                fees_minor,
                taxes_minor,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio::text AS fx_rate_to_portfolio,
                external_provider,
                external_reference,
                id_corrected_operation,
                notes,
                metadata,
                created_at,
                updated_at
            "#,
        )
        .bind(id_portfolio_operation)
        .bind(id_portfolio)
        .bind(input.id_asset)
        .bind(input.id_related_asset)
        .bind(input.operation_type.as_str())
        .bind(input.executed_at)
        .bind(input.effective_at)
        .bind(&input.quantity)
        .bind(&input.related_quantity)
        .bind(input.price_minor)
        .bind(input.gross_amount_minor)
        .bind(input.fees_minor)
        .bind(input.taxes_minor)
        .bind(input.cash_amount_minor)
        .bind(&input.currency)
        .bind(&input.fx_rate_to_portfolio)
        .bind(&input.external_provider)
        .bind(&input.external_reference)
        .bind(input.id_corrected_operation)
        .bind(&input.notes)
        .bind(&input.metadata)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| operation_from_row(&row)).transpose()
    }

    pub async fn set_status(
        &self,
        id_portfolio_operation: Uuid,
        id_portfolio: Uuid,
        status: OperationStatus,
    ) -> Result<Option<PortfolioOperation>, PortfolioOperationRepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE portfolio_operations
            SET operation_status = $3
            WHERE id_portfolio_operation = $1
              AND id_portfolio = $2
            RETURNING
                id_portfolio_operation,
                id_portfolio,
                id_asset,
                id_related_asset,
                operation_type,
                operation_status,
                executed_at,
                effective_at,
                quantity::text AS quantity,
                related_quantity::text AS related_quantity,
                price_minor,
                gross_amount_minor,
                fees_minor,
                taxes_minor,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio::text AS fx_rate_to_portfolio,
                external_provider,
                external_reference,
                id_corrected_operation,
                notes,
                metadata,
                created_at,
                updated_at
            "#,
        )
        .bind(id_portfolio_operation)
        .bind(id_portfolio)
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| operation_from_row(&row)).transpose()
    }

    pub async fn list_corrections_for_operation(
        &self,
        id_portfolio: Uuid,
        id_corrected_operation: Uuid,
    ) -> Result<Vec<PortfolioOperation>, PortfolioOperationRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id_portfolio_operation,
                id_portfolio,
                id_asset,
                id_related_asset,
                operation_type,
                operation_status,
                executed_at,
                effective_at,
                quantity::text AS quantity,
                related_quantity::text AS related_quantity,
                price_minor,
                gross_amount_minor,
                fees_minor,
                taxes_minor,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio::text AS fx_rate_to_portfolio,
                external_provider,
                external_reference,
                id_corrected_operation,
                notes,
                metadata,
                created_at,
                updated_at
            FROM portfolio_operations
            WHERE id_portfolio = $1
              AND id_corrected_operation = $2
            ORDER BY executed_at ASC, created_at ASC
            "#,
        )
        .bind(id_portfolio)
        .bind(id_corrected_operation)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| operation_from_row(&row))
            .collect()
    }

    pub async fn list_primary_operations_page(
        &self,
        id_portfolio: Uuid,
        filters: &PortfolioOperationFilters,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PortfolioOperation>, PortfolioOperationRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id_portfolio_operation,
                id_portfolio,
                id_asset,
                id_related_asset,
                operation_type,
                operation_status,
                executed_at,
                effective_at,
                quantity::text AS quantity,
                related_quantity::text AS related_quantity,
                price_minor,
                gross_amount_minor,
                fees_minor,
                taxes_minor,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio::text AS fx_rate_to_portfolio,
                external_provider,
                external_reference,
                id_corrected_operation,
                notes,
                metadata,
                created_at,
                updated_at
            FROM portfolio_operations
            WHERE id_portfolio = $1
              AND id_corrected_operation IS NULL
              AND ($2::varchar IS NULL OR operation_status = $2)
              AND ($3::varchar IS NULL OR operation_type = $3)
            ORDER BY executed_at DESC, created_at DESC
            LIMIT $4
            OFFSET $5
            "#,
        )
        .bind(id_portfolio)
        .bind(
            filters
                .operation_status
                .as_ref()
                .map(OperationStatus::as_str),
        )
        .bind(filters.operation_type.as_ref().map(OperationType::as_str))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| operation_from_row(&row))
            .collect()
    }

    pub async fn list_corrections_for_operations(
        &self,
        id_portfolio: Uuid,
        id_corrected_operations: &[Uuid],
    ) -> Result<Vec<PortfolioOperation>, PortfolioOperationRepositoryError> {
        if id_corrected_operations.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT
                id_portfolio_operation,
                id_portfolio,
                id_asset,
                id_related_asset,
                operation_type,
                operation_status,
                executed_at,
                effective_at,
                quantity::text AS quantity,
                related_quantity::text AS related_quantity,
                price_minor,
                gross_amount_minor,
                fees_minor,
                taxes_minor,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio::text AS fx_rate_to_portfolio,
                external_provider,
                external_reference,
                id_corrected_operation,
                notes,
                metadata,
                created_at,
                updated_at
            FROM portfolio_operations
            WHERE id_portfolio = $1
              AND id_corrected_operation = ANY($2)
              AND operation_type = 'adjustment'
            ORDER BY executed_at ASC, created_at ASC
            "#,
        )
        .bind(id_portfolio)
        .bind(id_corrected_operations)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| operation_from_row(&row))
            .collect()
    }
}

/// Bind the shared `OPERATION_INSERT_SQL` parameters in declaration order.
/// Works against any executor because it returns the bound query unevaluated.
fn bind_operation_insert<'q>(
    query: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    input: &'q NewPortfolioOperation,
) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
    query
        .bind(input.id_portfolio)
        .bind(input.id_asset)
        .bind(input.id_related_asset)
        .bind(input.operation_type.as_str())
        .bind(input.operation_status.as_str())
        .bind(input.executed_at)
        .bind(input.effective_at)
        .bind(&input.quantity)
        .bind(&input.related_quantity)
        .bind(input.price_minor)
        .bind(input.gross_amount_minor)
        .bind(input.fees_minor)
        .bind(input.taxes_minor)
        .bind(input.cash_amount_minor)
        .bind(&input.currency)
        .bind(&input.fx_rate_to_portfolio)
        .bind(&input.external_provider)
        .bind(&input.external_reference)
        .bind(input.id_corrected_operation)
        .bind(&input.notes)
        .bind(&input.metadata)
}

fn map_refresh_error(
    error: crate::repositories::portfolio_refresh_requests::PortfolioRefreshRequestRepositoryError,
) -> PortfolioOperationRepositoryError {
    use crate::repositories::portfolio_refresh_requests::PortfolioRefreshRequestRepositoryError as RefreshError;
    match error {
        RefreshError::Database(error) => PortfolioOperationRepositoryError::Database(error),
        RefreshError::InvalidRow => PortfolioOperationRepositoryError::InvalidRow,
    }
}

fn operation_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<PortfolioOperation, PortfolioOperationRepositoryError> {
    PortfolioOperationRow {
        id_portfolio_operation: row.try_get("id_portfolio_operation")?,
        id_portfolio: row.try_get("id_portfolio")?,
        id_asset: row.try_get("id_asset")?,
        id_related_asset: row.try_get("id_related_asset")?,
        operation_type: row.try_get("operation_type")?,
        operation_status: row.try_get("operation_status")?,
        executed_at: row.try_get("executed_at")?,
        effective_at: row.try_get("effective_at")?,
        quantity: row.try_get("quantity")?,
        related_quantity: row.try_get("related_quantity")?,
        price_minor: row.try_get("price_minor")?,
        gross_amount_minor: row.try_get("gross_amount_minor")?,
        fees_minor: row.try_get("fees_minor")?,
        taxes_minor: row.try_get("taxes_minor")?,
        cash_amount_minor: row.try_get("cash_amount_minor")?,
        currency: row.try_get("currency")?,
        fx_rate_to_portfolio: row.try_get("fx_rate_to_portfolio")?,
        external_provider: row.try_get("external_provider")?,
        external_reference: row.try_get("external_reference")?,
        id_corrected_operation: row.try_get("id_corrected_operation")?,
        notes: row.try_get("notes")?,
        metadata: row.try_get("metadata")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    }
    .try_into()
}
