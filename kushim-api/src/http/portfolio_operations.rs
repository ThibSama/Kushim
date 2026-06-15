use crate::{
    auth::AuthenticatedUser,
    domain::portfolio_operation::{OperationStatus, OperationType, PortfolioOperation},
    domain::portfolio_refresh_request::{PortfolioRefreshRequest, RefreshRequestStatus},
    errors::ApiError,
    http::extractors::{ApiJson, ApiPath, ApiQuery},
    services::portfolio_operations::{
        CancelPortfolioOperationInput, CreatePortfolioOperationCorrectionInput,
        CreatePortfolioOperationInput, ListPortfolioOperationsInput, OperationWriteOutcome,
        PortfolioOperationAuditTimelineInput, PortfolioOperationAuditTimelineItemView,
        PortfolioOperationAuditTimelineView, PortfolioOperationAuditView,
        PortfolioOperationCorrectionsView, PortfolioOperationServiceError,
        PostPortfolioOperationInput, UpdatePortfolioOperationInput,
    },
    state::AppState,
};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePortfolioOperationRequest {
    pub id_asset: Option<Uuid>,
    pub id_related_asset: Option<Uuid>,
    pub operation_type: OperationType,
    pub operation_status: Option<OperationStatus>,
    pub executed_at: String,
    pub effective_at: Option<String>,
    pub quantity: Option<String>,
    pub related_quantity: Option<String>,
    pub price_minor: Option<i64>,
    pub gross_amount_minor: Option<i64>,
    pub fees_minor: Option<i64>,
    pub taxes_minor: Option<i64>,
    pub cash_amount_minor: Option<i64>,
    pub currency: String,
    pub fx_rate_to_portfolio: Option<String>,
    pub external_provider: Option<String>,
    pub external_reference: Option<String>,
    pub id_corrected_operation: Option<Uuid>,
    pub notes: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListPortfolioOperationsQuery {
    pub operation_status: Option<OperationStatus>,
    pub operation_type: Option<OperationType>,
    pub id_asset: Option<Uuid>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PortfolioOperationAuditTimelineQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub operation_status: Option<String>,
    pub operation_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePortfolioOperationRequest {
    pub id_asset: Option<Uuid>,
    pub id_related_asset: Option<Uuid>,
    pub operation_type: Option<OperationType>,
    pub executed_at: Option<String>,
    pub effective_at: Option<Option<String>>,
    pub quantity: Option<Option<String>>,
    pub related_quantity: Option<Option<String>>,
    pub price_minor: Option<Option<i64>>,
    pub gross_amount_minor: Option<Option<i64>>,
    pub fees_minor: Option<Option<i64>>,
    pub taxes_minor: Option<Option<i64>>,
    pub cash_amount_minor: Option<i64>,
    pub currency: Option<String>,
    pub fx_rate_to_portfolio: Option<Option<String>>,
    pub external_provider: Option<Option<String>>,
    pub external_reference: Option<Option<String>>,
    pub id_corrected_operation: Option<Option<Uuid>>,
    pub notes: Option<Option<String>>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCorrectionRequest {
    pub operation_status: Option<OperationStatus>,
    pub executed_at: String,
    pub effective_at: Option<String>,
    pub id_asset: Option<Uuid>,
    pub id_related_asset: Option<Uuid>,
    pub quantity: Option<String>,
    pub related_quantity: Option<String>,
    pub price_minor: Option<i64>,
    pub gross_amount_minor: Option<i64>,
    pub fees_minor: Option<i64>,
    pub taxes_minor: Option<i64>,
    pub cash_amount_minor: Option<i64>,
    pub currency: Option<String>,
    pub fx_rate_to_portfolio: Option<String>,
    pub external_provider: Option<String>,
    pub external_reference: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct PortfolioOperationResponse {
    pub id_portfolio_operation: Uuid,
    pub id_portfolio: Uuid,
    pub id_asset: Option<Uuid>,
    pub id_related_asset: Option<Uuid>,
    pub operation_type: String,
    pub operation_status: String,
    pub executed_at: String,
    pub effective_at: Option<String>,
    pub quantity: Option<String>,
    pub related_quantity: Option<String>,
    pub price_minor: Option<i64>,
    pub gross_amount_minor: Option<i64>,
    pub fees_minor: Option<i64>,
    pub taxes_minor: Option<i64>,
    pub cash_amount_minor: i64,
    pub currency: String,
    pub fx_rate_to_portfolio: Option<String>,
    pub external_provider: Option<String>,
    pub external_reference: Option<String>,
    pub id_corrected_operation: Option<Uuid>,
    pub notes: Option<String>,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct PortfolioOperationEnvelope {
    pub operation: PortfolioOperationResponse,
}

/// Compact refresh-request identity returned alongside a write that produced a
/// posted operation. `null` for pending creations.
#[derive(Debug, Serialize)]
pub struct RefreshRequestRef {
    pub id_portfolio_refresh_request: Uuid,
    pub status: String,
    pub requested_at: String,
}

impl From<PortfolioRefreshRequest> for RefreshRequestRef {
    fn from(value: PortfolioRefreshRequest) -> Self {
        Self {
            id_portfolio_refresh_request: value.id_portfolio_refresh_request,
            status: value.status.as_str().to_string(),
            requested_at: format_datetime(value.requested_at),
        }
    }
}

/// Envelope for operation writes: includes the operation and the refresh
/// request when one was enqueued (posted writes), `null` otherwise.
#[derive(Debug, Serialize)]
pub struct PortfolioOperationWriteEnvelope {
    pub operation: PortfolioOperationResponse,
    pub refresh_request: Option<RefreshRequestRef>,
}

impl From<OperationWriteOutcome> for PortfolioOperationWriteEnvelope {
    fn from(value: OperationWriteOutcome) -> Self {
        Self {
            operation: value.operation.into(),
            refresh_request: value.refresh_request.map(RefreshRequestRef::from),
        }
    }
}

/// Full refresh-request status returned by the read endpoint. The raw
/// `last_error` is never exposed; only a safe public `error_code` is surfaced
/// when the request failed.
#[derive(Debug, Serialize)]
pub struct RefreshRequestStatusResponse {
    pub id_portfolio_refresh_request: Uuid,
    pub id_portfolio: Uuid,
    pub status: String,
    pub attempts: i32,
    pub requested_at: String,
    pub processing_started_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: String,
    pub error_code: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct RefreshRequestStatusEnvelope {
    pub refresh_request: RefreshRequestStatusResponse,
}

impl From<PortfolioRefreshRequest> for RefreshRequestStatusResponse {
    fn from(value: PortfolioRefreshRequest) -> Self {
        let error_code = if value.status == RefreshRequestStatus::Failed {
            Some("refresh_failed")
        } else {
            None
        };

        Self {
            id_portfolio_refresh_request: value.id_portfolio_refresh_request,
            id_portfolio: value.id_portfolio,
            status: value.status.as_str().to_string(),
            attempts: value.attempts,
            requested_at: format_datetime(value.requested_at),
            processing_started_at: value.processing_started_at.map(format_datetime),
            completed_at: value.completed_at.map(format_datetime),
            updated_at: format_datetime(value.updated_at),
            error_code,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PortfolioOperationListResponse {
    pub operations: Vec<PortfolioOperationResponse>,
}

#[derive(Debug, Serialize)]
pub struct PortfolioOperationCorrectionsResponse {
    pub operation: PortfolioOperationResponse,
    pub corrections: Vec<PortfolioOperationResponse>,
}

#[derive(Debug, Serialize)]
pub struct PortfolioOperationAuditResponse {
    pub operation: PortfolioOperationResponse,
    pub corrected_operation: Option<PortfolioOperationResponse>,
    pub corrections: Vec<PortfolioOperationResponse>,
    pub correction_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PortfolioOperationAuditTimelineItemResponse {
    pub operation: PortfolioOperationResponse,
    pub corrections: Vec<PortfolioOperationResponse>,
    pub correction_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PortfolioOperationAuditTimelinePaginationResponse {
    pub limit: i64,
    pub offset: i64,
    pub returned: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct PortfolioOperationAuditTimelineResponse {
    pub items: Vec<PortfolioOperationAuditTimelineItemResponse>,
    pub pagination: PortfolioOperationAuditTimelinePaginationResponse,
}

pub async fn create_portfolio_operation(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath(id_portfolio): ApiPath<Uuid>,
    ApiJson(request): ApiJson<CreatePortfolioOperationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let outcome = state
        .portfolio_operation_service
        .create_operation(CreatePortfolioOperationInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            id_asset: request.id_asset,
            id_related_asset: request.id_related_asset,
            operation_type: request.operation_type,
            operation_status: request.operation_status,
            executed_at: request.executed_at,
            effective_at: request.effective_at,
            quantity: request.quantity,
            related_quantity: request.related_quantity,
            price_minor: request.price_minor,
            gross_amount_minor: request.gross_amount_minor,
            fees_minor: request.fees_minor,
            taxes_minor: request.taxes_minor,
            cash_amount_minor: request.cash_amount_minor,
            currency: request.currency,
            fx_rate_to_portfolio: request.fx_rate_to_portfolio,
            external_provider: request.external_provider,
            external_reference: request.external_reference,
            id_corrected_operation: request.id_corrected_operation,
            notes: request.notes,
            metadata: request.metadata,
        })
        .await
        .map_err(map_service_error)?;

    Ok((
        StatusCode::CREATED,
        Json(PortfolioOperationWriteEnvelope::from(outcome)),
    ))
}

pub async fn list_portfolio_operations(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath(id_portfolio): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<ListPortfolioOperationsQuery>,
) -> Result<Json<PortfolioOperationListResponse>, ApiError> {
    let operations = state
        .portfolio_operation_service
        .list_operations(ListPortfolioOperationsInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            operation_status: query.operation_status,
            operation_type: query.operation_type,
            id_asset: query.id_asset,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioOperationListResponse {
        operations: operations
            .into_iter()
            .map(PortfolioOperationResponse::from)
            .collect(),
    }))
}

pub async fn get_portfolio_operation(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath((id_portfolio, id_portfolio_operation)): ApiPath<(Uuid, Uuid)>,
) -> Result<Json<PortfolioOperationEnvelope>, ApiError> {
    let operation = state
        .portfolio_operation_service
        .get_operation(
            authenticated.claims.sub,
            id_portfolio,
            id_portfolio_operation,
        )
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioOperationEnvelope {
        operation: operation.into(),
    }))
}

pub async fn update_portfolio_operation(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath((id_portfolio, id_portfolio_operation)): ApiPath<(Uuid, Uuid)>,
    ApiJson(request): ApiJson<UpdatePortfolioOperationRequest>,
) -> Result<Json<PortfolioOperationEnvelope>, ApiError> {
    let operation = state
        .portfolio_operation_service
        .update_operation(UpdatePortfolioOperationInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            id_portfolio_operation,
            id_asset: request.id_asset,
            id_related_asset: request.id_related_asset,
            operation_type: request.operation_type,
            executed_at: request.executed_at,
            effective_at: request.effective_at,
            quantity: request.quantity,
            related_quantity: request.related_quantity,
            price_minor: request.price_minor,
            gross_amount_minor: request.gross_amount_minor,
            fees_minor: request.fees_minor,
            taxes_minor: request.taxes_minor,
            cash_amount_minor: request.cash_amount_minor,
            currency: request.currency,
            fx_rate_to_portfolio: request.fx_rate_to_portfolio,
            external_provider: request.external_provider,
            external_reference: request.external_reference,
            id_corrected_operation: request.id_corrected_operation,
            notes: request.notes,
            metadata: request.metadata,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioOperationEnvelope {
        operation: operation.into(),
    }))
}

pub async fn cancel_portfolio_operation(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath((id_portfolio, id_portfolio_operation)): ApiPath<(Uuid, Uuid)>,
) -> Result<Json<PortfolioOperationEnvelope>, ApiError> {
    let operation = state
        .portfolio_operation_service
        .cancel_operation(CancelPortfolioOperationInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            id_portfolio_operation,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioOperationEnvelope {
        operation: operation.into(),
    }))
}

pub async fn create_portfolio_operation_correction(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath((id_portfolio, id_portfolio_operation)): ApiPath<(Uuid, Uuid)>,
    ApiJson(request): ApiJson<CreateCorrectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let outcome = state
        .portfolio_operation_service
        .create_correction(CreatePortfolioOperationCorrectionInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            id_portfolio_operation,
            operation_status: request.operation_status,
            executed_at: request.executed_at,
            effective_at: request.effective_at,
            id_asset: request.id_asset,
            id_related_asset: request.id_related_asset,
            quantity: request.quantity,
            related_quantity: request.related_quantity,
            price_minor: request.price_minor,
            gross_amount_minor: request.gross_amount_minor,
            fees_minor: request.fees_minor,
            taxes_minor: request.taxes_minor,
            cash_amount_minor: request.cash_amount_minor,
            currency: request.currency,
            fx_rate_to_portfolio: request.fx_rate_to_portfolio,
            external_provider: request.external_provider,
            external_reference: request.external_reference,
            notes: request.notes,
            metadata: request.metadata,
        })
        .await
        .map_err(map_service_error)?;

    Ok((
        StatusCode::CREATED,
        Json(PortfolioOperationWriteEnvelope::from(outcome)),
    ))
}

pub async fn post_portfolio_operation(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath((id_portfolio, id_portfolio_operation)): ApiPath<(Uuid, Uuid)>,
) -> Result<Json<PortfolioOperationWriteEnvelope>, ApiError> {
    let outcome = state
        .portfolio_operation_service
        .post_operation(PostPortfolioOperationInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            id_portfolio_operation,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioOperationWriteEnvelope::from(outcome)))
}

pub async fn get_portfolio_refresh_request(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath((id_portfolio, id_refresh_request)): ApiPath<(Uuid, Uuid)>,
) -> Result<Json<RefreshRequestStatusEnvelope>, ApiError> {
    let refresh_request = state
        .portfolio_operation_service
        .get_refresh_request(authenticated.claims.sub, id_portfolio, id_refresh_request)
        .await
        .map_err(map_service_error)?;

    Ok(Json(RefreshRequestStatusEnvelope {
        refresh_request: refresh_request.into(),
    }))
}

pub async fn get_portfolio_operation_corrections(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath((id_portfolio, id_portfolio_operation)): ApiPath<(Uuid, Uuid)>,
) -> Result<Json<PortfolioOperationCorrectionsResponse>, ApiError> {
    let view = state
        .portfolio_operation_service
        .get_corrections(
            authenticated.claims.sub,
            id_portfolio,
            id_portfolio_operation,
        )
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioOperationCorrectionsResponse::from(view)))
}

pub async fn get_portfolio_operation_audit(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath((id_portfolio, id_portfolio_operation)): ApiPath<(Uuid, Uuid)>,
) -> Result<Json<PortfolioOperationAuditResponse>, ApiError> {
    let view = state
        .portfolio_operation_service
        .get_audit(
            authenticated.claims.sub,
            id_portfolio,
            id_portfolio_operation,
        )
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioOperationAuditResponse::from(view)))
}

pub async fn get_portfolio_operations_audit_timeline(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath(id_portfolio): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<PortfolioOperationAuditTimelineQuery>,
) -> Result<Json<PortfolioOperationAuditTimelineResponse>, ApiError> {
    let view = state
        .portfolio_operation_service
        .get_audit_timeline(PortfolioOperationAuditTimelineInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            limit: query.limit,
            offset: query.offset,
            operation_status: parse_operation_status_filter(query.operation_status.as_deref())?,
            operation_type: parse_operation_type_filter(query.operation_type.as_deref())?,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioOperationAuditTimelineResponse::from(view)))
}

impl From<PortfolioOperation> for PortfolioOperationResponse {
    fn from(value: PortfolioOperation) -> Self {
        Self {
            id_portfolio_operation: value.id_portfolio_operation,
            id_portfolio: value.id_portfolio,
            id_asset: value.id_asset,
            id_related_asset: value.id_related_asset,
            operation_type: value.operation_type.as_str().to_string(),
            operation_status: value.operation_status.as_str().to_string(),
            executed_at: format_datetime(value.executed_at),
            effective_at: value.effective_at.map(format_datetime),
            quantity: value.quantity,
            related_quantity: value.related_quantity,
            price_minor: value.price_minor,
            gross_amount_minor: value.gross_amount_minor,
            fees_minor: value.fees_minor,
            taxes_minor: value.taxes_minor,
            cash_amount_minor: value.cash_amount_minor,
            currency: value.currency,
            fx_rate_to_portfolio: value.fx_rate_to_portfolio,
            external_provider: value.external_provider,
            external_reference: value.external_reference,
            id_corrected_operation: value.id_corrected_operation,
            notes: value.notes,
            metadata: value.metadata,
            created_at: format_datetime(value.created_at),
            updated_at: format_datetime(value.updated_at),
        }
    }
}

impl From<PortfolioOperationCorrectionsView> for PortfolioOperationCorrectionsResponse {
    fn from(value: PortfolioOperationCorrectionsView) -> Self {
        Self {
            operation: value.operation.into(),
            corrections: value
                .corrections
                .into_iter()
                .map(PortfolioOperationResponse::from)
                .collect(),
        }
    }
}

impl From<PortfolioOperationAuditView> for PortfolioOperationAuditResponse {
    fn from(value: PortfolioOperationAuditView) -> Self {
        let correction_count = value.corrections.len();

        Self {
            operation: value.operation.into(),
            corrected_operation: value
                .corrected_operation
                .map(PortfolioOperationResponse::from),
            corrections: value
                .corrections
                .into_iter()
                .map(PortfolioOperationResponse::from)
                .collect(),
            correction_count,
        }
    }
}

impl From<PortfolioOperationAuditTimelineItemView> for PortfolioOperationAuditTimelineItemResponse {
    fn from(value: PortfolioOperationAuditTimelineItemView) -> Self {
        let correction_count = value.corrections.len();

        Self {
            operation: value.operation.into(),
            corrections: value
                .corrections
                .into_iter()
                .map(PortfolioOperationResponse::from)
                .collect(),
            correction_count,
        }
    }
}

impl From<PortfolioOperationAuditTimelineView> for PortfolioOperationAuditTimelineResponse {
    fn from(value: PortfolioOperationAuditTimelineView) -> Self {
        Self {
            items: value
                .items
                .into_iter()
                .map(PortfolioOperationAuditTimelineItemResponse::from)
                .collect(),
            pagination: PortfolioOperationAuditTimelinePaginationResponse {
                limit: value.pagination.limit,
                offset: value.pagination.offset,
                returned: value.pagination.returned,
                has_more: value.pagination.has_more,
            },
        }
    }
}

fn format_datetime(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("OffsetDateTime should always be serializable as RFC3339")
}

fn parse_operation_status_filter(value: Option<&str>) -> Result<Option<OperationStatus>, ApiError> {
    match value {
        None => Ok(None),
        Some(value) => {
            OperationStatus::try_from(value)
                .map(Some)
                .map_err(|_| ApiError::Validation {
                    code: "invalid_operation_status",
                    message: "operation_status must be one of pending, posted, cancelled",
                })
        }
    }
}

fn parse_operation_type_filter(value: Option<&str>) -> Result<Option<OperationType>, ApiError> {
    match value {
        None => Ok(None),
        Some(value) => OperationType::try_from(value)
            .map(Some)
            .map_err(|_| ApiError::Validation {
                code: "invalid_operation_type",
                message: "operation_type must be a supported portfolio operation type",
            }),
    }
}

fn map_service_error(error: PortfolioOperationServiceError) -> ApiError {
    match error {
        PortfolioOperationServiceError::Validation { code, message } => {
            ApiError::Validation { code, message }
        }
        PortfolioOperationServiceError::UnprocessableEntity { code, message } => {
            ApiError::UnprocessableEntity { code, message }
        }
        PortfolioOperationServiceError::NotFound { code, message } => {
            ApiError::NotFound { code, message }
        }
        PortfolioOperationServiceError::Conflict { code, message } => {
            ApiError::Conflict { code, message }
        }
        PortfolioOperationServiceError::Internal => ApiError::Internal {
            code: "portfolio_operation_service_failed",
            message: "failed to process portfolio operation request",
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        auth::{
            JwtValidator,
            claims::{AuthClaims, TokenType, UserRole},
        },
        repositories::{
            assets::AssetRepository, portfolio_operations::PortfolioOperationRepository,
            portfolio_read_models::PortfolioReadModelRepository,
            portfolio_refresh_requests::PortfolioRefreshRequestRepository,
            portfolio_snapshots::PortfolioSnapshotRepository, portfolios::PortfolioRepository,
        },
        services::{
            assets::AssetService, portfolio_operations::PortfolioOperationService,
            portfolio_read_models::PortfolioReadModelService,
            portfolio_snapshots::PortfolioSnapshotService, portfolios::PortfolioService,
        },
        state::AppState,
    };
    use axum::{
        body::{self, Body},
        http::{Request, StatusCode, header::AUTHORIZATION},
    };
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::{Value, json};
    use sqlx::{PgPool, Row, postgres::PgPoolOptions};
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    async fn test_pool() -> PgPool {
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    async fn ensure_role(pool: &PgPool, id_role: i16, label: &str) {
        // Race-safe under cargo's parallel test runner; see assets.rs notes.
        sqlx::query(
            r#"
            INSERT INTO roles (id_role, label)
            VALUES ($1, $2)
            ON CONFLICT (label) DO NOTHING
            "#,
        )
        .bind(id_role)
        .bind(label)
        .execute(pool)
        .await
        .expect("role should be upserted");
    }

    async fn create_user(pool: &PgPool, public_handle: &str) -> Uuid {
        ensure_role(pool, 1, "user").await;

        let id_user = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id_user, id_role, username, public_handle, password_hash)
            VALUES ($1, 1, $2, $3, $4)
            "#,
        )
        .bind(id_user)
        .bind(public_handle)
        .bind(public_handle)
        .bind("$argon2id$placeholder")
        .execute(pool)
        .await
        .expect("user should be inserted");

        id_user
    }

    async fn create_portfolio(
        pool: &PgPool,
        id_user: Uuid,
        deleted_at: Option<OffsetDateTime>,
    ) -> Uuid {
        let id_portfolio = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO portfolios (id_portfolio, id_user, name, base_currency, visibility, deleted_at)
            VALUES ($1, $2, $3, 'EUR', 'private', $4)
            "#,
        )
        .bind(id_portfolio)
        .bind(id_user)
        .bind(format!("pf{}", &id_portfolio.simple().to_string()[..12]))
        .bind(deleted_at)
        .execute(pool)
        .await
        .expect("portfolio should be inserted");

        id_portfolio
    }

    async fn create_asset_with_status(pool: &PgPool, symbol: &str, status: &str) -> Uuid {
        let id_asset = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO assets (id_asset, asset_class, status, name, native_currency, symbol)
            VALUES ($1, 'equity', $2, $3, 'EUR', $4)
            "#,
        )
        .bind(id_asset)
        .bind(status)
        .bind(format!("Asset {symbol}"))
        .bind(symbol)
        .execute(pool)
        .await
        .expect("asset should be inserted");

        id_asset
    }

    async fn create_asset(pool: &PgPool, symbol: &str) -> Uuid {
        create_asset_with_status(pool, symbol, "active").await
    }

    async fn set_asset_status(pool: &PgPool, id_asset: Uuid, status: &str) {
        sqlx::query("UPDATE assets SET status = $2 WHERE id_asset = $1")
            .bind(id_asset)
            .bind(status)
            .execute(pool)
            .await
            .expect("asset status should be updated");
    }

    async fn insert_operation(
        pool: &PgPool,
        id_portfolio: Uuid,
        operation_status: &str,
        payload: Value,
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
                $1, $2, $3, $4, $5, $6, $7, $8,
                $9::numeric, $10::numeric, $11, $12, $13, $14, $15, $16,
                $17::numeric, $18, $19, $20, $21, $22
            )
            "#,
        )
        .bind(id_portfolio_operation)
        .bind(id_portfolio)
        .bind(
            payload["id_asset"]
                .as_str()
                .and_then(|value| Uuid::parse_str(value).ok()),
        )
        .bind(
            payload["id_related_asset"]
                .as_str()
                .and_then(|value| Uuid::parse_str(value).ok()),
        )
        .bind(payload["operation_type"].as_str().unwrap())
        .bind(operation_status)
        .bind(OffsetDateTime::parse(payload["executed_at"].as_str().unwrap(), &Rfc3339).unwrap())
        .bind(
            payload["effective_at"]
                .as_str()
                .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok()),
        )
        .bind(payload["quantity"].as_str())
        .bind(payload["related_quantity"].as_str())
        .bind(payload["price_minor"].as_i64())
        .bind(payload["gross_amount_minor"].as_i64())
        .bind(payload["fees_minor"].as_i64())
        .bind(payload["taxes_minor"].as_i64())
        .bind(payload["cash_amount_minor"].as_i64().unwrap_or(0))
        .bind(payload["currency"].as_str().unwrap())
        .bind(payload["fx_rate_to_portfolio"].as_str())
        .bind(payload["external_provider"].as_str())
        .bind(payload["external_reference"].as_str())
        .bind(
            payload["id_corrected_operation"]
                .as_str()
                .and_then(|value| Uuid::parse_str(value).ok()),
        )
        .bind(payload["notes"].as_str())
        .bind(
            payload
                .get("metadata")
                .cloned()
                .unwrap_or_else(|| json!({})),
        )
        .execute(pool)
        .await
        .expect("operation should be inserted");

        id_portfolio_operation
    }

    async fn cleanup_user_tree(pool: &PgPool, id_user: Uuid, asset_ids: &[Uuid]) {
        sqlx::query(
            r#"
            DELETE FROM portfolio_operations
            WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
            "#,
        )
        .bind(id_user)
        .execute(pool)
        .await
        .expect("operations should be deleted");

        sqlx::query("DELETE FROM portfolios WHERE id_user = $1")
            .bind(id_user)
            .execute(pool)
            .await
            .expect("portfolios should be deleted");

        sqlx::query("DELETE FROM users WHERE id_user = $1")
            .bind(id_user)
            .execute(pool)
            .await
            .expect("user should be deleted");

        for id_asset in asset_ids {
            sqlx::query("DELETE FROM assets WHERE id_asset = $1")
                .bind(id_asset)
                .execute(pool)
                .await
                .expect("asset should be deleted");
        }
    }

    fn build_access_token(id_user: Uuid, public_handle: &str) -> String {
        build_token(id_user, public_handle, TokenType::Access)
    }

    fn build_refresh_token(id_user: Uuid, public_handle: &str) -> String {
        build_token(id_user, public_handle, TokenType::Refresh)
    }

    fn build_token(id_user: Uuid, public_handle: &str, token_type: TokenType) -> String {
        let now = OffsetDateTime::now_utc();
        let claims = AuthClaims {
            sub: id_user,
            public_handle: public_handle.to_string(),
            role: UserRole::User,
            token_type,
            jti: Uuid::new_v4(),
            iat: now.unix_timestamp(),
            exp: (now + Duration::seconds(900)).unix_timestamp(),
            iss: "kushim-auth".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret("dev_only_change_me_minimum_32_chars".as_bytes()),
        )
        .expect("token should be encoded")
    }

    async fn test_state(pool: PgPool) -> AppState {
        let portfolio_repository = PortfolioRepository::new(pool.clone());
        let asset_service = AssetService::new(AssetRepository::new(pool.clone()));
        let portfolio_service = PortfolioService::new(portfolio_repository.clone());
        let portfolio_operation_service = PortfolioOperationService::new(
            AssetRepository::new(pool.clone()),
            portfolio_repository.clone(),
            PortfolioOperationRepository::new(pool.clone()),
            PortfolioRefreshRequestRepository::new(pool.clone()),
        );
        let portfolio_read_model_service = PortfolioReadModelService::new(
            portfolio_repository.clone(),
            PortfolioReadModelRepository::new(pool.clone()),
        );
        let portfolio_snapshot_service = PortfolioSnapshotService::new(
            portfolio_repository,
            PortfolioSnapshotRepository::new(pool.clone()),
        );

        AppState {
            db_pool: pool,
            jwt_validator: JwtValidator::new(
                "dev_only_change_me_minimum_32_chars",
                "kushim-auth".to_string(),
            ),
            asset_service,
            portfolio_service,
            portfolio_operation_service,
            portfolio_read_model_service,
            portfolio_snapshot_service,
            service_name: "kushim-api",
            service_version: "test",
            routes_version: "api-routes-v1",
            environment: "test".to_string(),
        }
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        serde_json::from_slice(&bytes).expect("body should be valid json")
    }

    fn deposit_payload() -> Value {
        json!({
            "operation_type": "deposit",
            "executed_at": "2026-06-05T10:00:00Z",
            "gross_amount_minor": 100000,
            "cash_amount_minor": 100000,
            "currency": "EUR",
            "metadata": {}
        })
    }

    fn buy_payload(id_asset: Uuid) -> Value {
        json!({
            "id_asset": id_asset,
            "operation_type": "buy",
            "executed_at": "2026-06-05T10:00:00Z",
            "quantity": "10.5000000000",
            "price_minor": 12345,
            "gross_amount_minor": 129622,
            "cash_amount_minor": 129622,
            "currency": "EUR",
            "metadata": {}
        })
    }

    fn correction_payload() -> Value {
        json!({
            "executed_at": "2026-06-06T10:00:00Z",
            "cash_amount_minor": 5000,
            "currency": "EUR",
            "notes": "correction adjustment",
            "metadata": {
                "reason": "manual_correction"
            }
        })
    }

    fn empty_adjustment_payload() -> Value {
        json!({
            "operation_type": "adjustment",
            "executed_at": "2026-06-06T10:00:00Z",
            "currency": "EUR",
            "metadata": {}
        })
    }

    fn assert_rfc3339_string(value: &Value) {
        let as_str = value.as_str().expect("date field should be a JSON string");
        OffsetDateTime::parse(as_str, &Rfc3339).expect("date field should be valid RFC3339");
    }

    fn posted_deposit_payload() -> Value {
        let mut payload = deposit_payload();
        payload["operation_status"] = json!("posted");
        payload
    }

    // Posted operations are immutable (DB trigger), so they cannot be deleted.
    // Tests that create posted operations therefore clean up only the deletable
    // refresh requests and leave the immutable rows in place — the same pattern
    // the worker integration tests follow for posted fixtures.
    async fn cleanup_refresh_requests(pool: &PgPool, id_portfolio: Uuid) {
        sqlx::query("DELETE FROM portfolio_refresh_requests WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .execute(pool)
            .await
            .expect("refresh requests should be deleted");
    }

    async fn count_pending_refresh_requests(pool: &PgPool, id_portfolio: Uuid) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM portfolio_refresh_requests WHERE id_portfolio = $1 AND status = 'pending'",
        )
        .bind(id_portfolio)
        .fetch_one(pool)
        .await
        .expect("count should succeed")
    }

    async fn count_all_refresh_requests(pool: &PgPool, id_portfolio: Uuid) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM portfolio_refresh_requests WHERE id_portfolio = $1",
        )
        .bind(id_portfolio)
        .fetch_one(pool)
        .await
        .expect("count should succeed")
    }

    async fn post_json(
        pool: &PgPool,
        id_user: Uuid,
        handle: &str,
        uri: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let status = response.status();
        (status, response_json(response).await)
    }

    #[tokio::test]
    async fn posted_create_enqueues_refresh_request_atomically() {
        let pool = test_pool().await;
        let handle = format!("prc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            posted_deposit_payload(),
        )
        .await;

        let pending = count_pending_refresh_requests(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["operation"]["operation_status"], "posted");
        assert_eq!(body["refresh_request"]["status"], "pending");
        assert_rfc3339_string(&body["refresh_request"]["requested_at"]);
        assert!(body["refresh_request"]["id_portfolio_refresh_request"].is_string());
        // Operation row AND refresh request row both committed.
        assert_eq!(pending, 1);
    }

    #[tokio::test]
    async fn cross_currency_posted_without_fx_is_rejected_and_no_refresh_enqueued() {
        // P1 contract: a posted operation whose currency differs from the
        // portfolio base currency MUST carry a positive fx_rate_to_portfolio.
        // Reject with 422 unsupported_cross_currency; neither the operation
        // row nor the refresh request must be inserted.
        let pool = test_pool().await;
        let handle = format!("xcc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await; // EUR

        let mut payload = posted_deposit_payload();
        payload["currency"] = json!("USD");

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            payload,
        )
        .await;

        let op_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1")
                .bind(id_portfolio)
                .fetch_one(&pool)
                .await
                .expect("count should succeed");
        let refresh_count = count_all_refresh_requests(&pool, id_portfolio).await;

        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "unsupported_cross_currency");
        assert_eq!(op_count, 0, "no operation row must be inserted");
        assert_eq!(refresh_count, 0, "no refresh request must be enqueued");
    }

    #[tokio::test]
    async fn cross_currency_posted_with_valid_fx_is_accepted() {
        let pool = test_pool().await;
        let handle = format!("xcf{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await; // EUR

        let mut payload = posted_deposit_payload();
        payload["currency"] = json!("USD");
        payload["fx_rate_to_portfolio"] = json!("0.92");

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            payload,
        )
        .await;

        let pending = count_pending_refresh_requests(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["operation"]["operation_status"], "posted");
        assert_eq!(body["operation"]["currency"], "USD");
        // Postgres NUMERIC pads to its column scale, so the round-tripped
        // value is e.g. "0.9200000000" rather than the input "0.92". Parse it
        // numerically to assert equivalence rather than literal string match.
        let fx_str = body["operation"]["fx_rate_to_portfolio"]
            .as_str()
            .expect("fx rate should be a string");
        let parsed: f64 = fx_str.parse().expect("fx rate should parse");
        assert!((parsed - 0.92).abs() < 1e-9, "got {fx_str}");
        assert_eq!(pending, 1);
    }

    #[tokio::test]
    async fn posted_cross_currency_transfer_in_without_fx_is_rejected() {
        // P1.1: transfer_in with a positive monetary leg crosses currencies
        // exactly like a deposit — the worker applies converted_cash. The
        // guard must reject before insertion (no operation row, no refresh
        // request) even though the type is in the "cash" group.
        let pool = test_pool().await;
        let handle = format!("xti{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await; // EUR

        let payload = json!({
            "operation_type": "transfer_in",
            "operation_status": "posted",
            "executed_at": "2026-06-05T10:00:00Z",
            "cash_amount_minor": 100000,
            "currency": "USD",
            "metadata": {}
        });

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            payload,
        )
        .await;

        let op_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1")
                .bind(id_portfolio)
                .fetch_one(&pool)
                .await
                .expect("count should succeed");
        let refresh_count = count_all_refresh_requests(&pool, id_portfolio).await;
        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "unsupported_cross_currency");
        assert_eq!(op_count, 0);
        assert_eq!(refresh_count, 0);
    }

    #[tokio::test]
    async fn posted_cross_currency_transfer_out_without_fx_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("xto{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await; // EUR

        let payload = json!({
            "operation_type": "transfer_out",
            "operation_status": "posted",
            "executed_at": "2026-06-05T10:00:00Z",
            "cash_amount_minor": 100000,
            "currency": "USD",
            "metadata": {}
        });

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            payload,
        )
        .await;

        let op_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1")
                .bind(id_portfolio)
                .fetch_one(&pool)
                .await
                .expect("count should succeed");
        let refresh_count = count_all_refresh_requests(&pool, id_portfolio).await;
        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "unsupported_cross_currency");
        assert_eq!(op_count, 0);
        assert_eq!(refresh_count, 0);
    }

    #[tokio::test]
    async fn posted_cross_currency_transfer_in_with_valid_fx_is_accepted() {
        let pool = test_pool().await;
        let handle = format!("xtf{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await; // EUR

        let payload = json!({
            "operation_type": "transfer_in",
            "operation_status": "posted",
            "executed_at": "2026-06-05T10:00:00Z",
            "cash_amount_minor": 100000,
            "currency": "USD",
            "fx_rate_to_portfolio": "0.92",
            "metadata": {}
        });

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            payload,
        )
        .await;

        let pending = count_pending_refresh_requests(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["operation"]["operation_status"], "posted");
        assert_eq!(body["operation"]["currency"], "USD");
        assert_eq!(pending, 1);
    }

    #[tokio::test]
    async fn posted_cross_currency_zero_cash_transfer_does_not_require_fx() {
        // Worker contract: transfer_in/out with cash_amount_minor = 0 has no
        // monetary leg to convert, so the cross-currency guard must not
        // require an fx_rate_to_portfolio. The backend payload validation
        // already allows zero cash for transfers; the cross-currency guard
        // must agree.
        let pool = test_pool().await;
        let handle = format!("xtz{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await; // EUR

        let payload = json!({
            "operation_type": "transfer_in",
            "operation_status": "posted",
            "executed_at": "2026-06-05T10:00:00Z",
            "cash_amount_minor": 0,
            "currency": "USD",
            "metadata": {}
        });

        let (status, _body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            payload,
        )
        .await;

        let pending = count_pending_refresh_requests(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(pending, 1);
    }

    #[tokio::test]
    async fn unknown_operation_currency_returns_422_unsupported_currency() {
        let pool = test_pool().await;
        let handle = format!("uuc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let mut payload = posted_deposit_payload();
        payload["currency"] = json!("ZZZ");

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            payload,
        )
        .await;

        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "unsupported_currency");
    }

    #[tokio::test]
    async fn lowercase_currency_is_normalized_to_canonical_uppercase() {
        let pool = test_pool().await;
        let handle = format!("lnc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let mut payload = deposit_payload();
        payload["currency"] = json!(" eur ");

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            payload,
        )
        .await;

        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["operation"]["currency"], "EUR");
    }

    #[tokio::test]
    async fn posting_pending_cross_currency_without_fx_is_rejected_atomically() {
        // P1 contract: transitioning pending → posted must revalidate the FX
        // contract. On rejection the row stays pending and no refresh request
        // is enqueued.
        let pool = test_pool().await;
        let handle = format!("ppc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await; // EUR

        let mut pending = deposit_payload();
        pending["currency"] = json!("USD");
        let id_operation = insert_operation(&pool, id_portfolio, "pending", pending).await;

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"),
            json!({}),
        )
        .await;

        let row_status: String = sqlx::query_scalar(
            "SELECT operation_status FROM portfolio_operations WHERE id_portfolio_operation = $1",
        )
        .bind(id_operation)
        .fetch_one(&pool)
        .await
        .expect("status should be readable");
        let refresh_count = count_all_refresh_requests(&pool, id_portfolio).await;

        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "unsupported_cross_currency");
        assert_eq!(row_status, "pending");
        assert_eq!(refresh_count, 0);
    }

    #[tokio::test]
    async fn pending_create_does_not_enqueue_refresh_request() {
        let pool = test_pool().await;
        let handle = format!("pnc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            deposit_payload(),
        )
        .await;

        let total = count_all_refresh_requests(&pool, id_portfolio).await;
        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["operation"]["operation_status"], "pending");
        assert!(body["refresh_request"].is_null());
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn posting_pending_operation_enqueues_refresh_request() {
        let pool = test_pool().await;
        let handle = format!("ppo{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"),
            json!({}),
        )
        .await;

        let pending = count_pending_refresh_requests(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operation"]["operation_status"], "posted");
        assert_eq!(body["refresh_request"]["status"], "pending");
        assert_eq!(pending, 1);
    }

    #[tokio::test]
    async fn cancel_does_not_enqueue_refresh_request() {
        let pool = test_pool().await;
        let handle = format!("cnc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;

        let (status, _body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations/{id_operation}/cancel"),
            json!({}),
        )
        .await;

        let total = count_all_refresh_requests(&pool, id_portfolio).await;
        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn posted_creates_coalesce_into_single_pending_request() {
        let pool = test_pool().await;
        let handle = format!("col{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let uri = format!("/v1/portfolios/{id_portfolio}/operations");
        let (s1, _) = post_json(&pool, id_user, &handle, &uri, posted_deposit_payload()).await;
        let (s2, _) = post_json(&pool, id_user, &handle, &uri, posted_deposit_payload()).await;

        let pending = count_pending_refresh_requests(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;

        assert_eq!(s1, StatusCode::CREATED);
        assert_eq!(s2, StatusCode::CREATED);
        // Two posted operations, but at most one pending refresh request.
        assert_eq!(pending, 1);
    }

    #[tokio::test]
    async fn get_refresh_request_returns_status_without_raw_error() {
        let pool = test_pool().await;
        let handle = format!("grr{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let (_, create_body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            posted_deposit_payload(),
        )
        .await;
        let id_refresh = create_body["refresh_request"]["id_portfolio_refresh_request"]
            .as_str()
            .expect("refresh request id")
            .to_string();

        // Simulate a worker failure with a sensitive diagnostic.
        sqlx::query(
            "UPDATE portfolio_refresh_requests SET status = 'failed', attempts = 3, last_error = $2 WHERE id_portfolio_refresh_request = $1",
        )
        .bind(Uuid::parse_str(&id_refresh).unwrap())
        .bind("SENSITIVE_INTERNAL_DETAIL_should_not_leak")
        .execute(&pool)
        .await
        .expect("update should succeed");

        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/refresh-requests/{id_refresh}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let status = response.status();
        let raw_body = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body readable");
        let raw_text = String::from_utf8_lossy(&raw_body).to_string();
        let body: Value = serde_json::from_slice(&raw_body).expect("json body");

        cleanup_refresh_requests(&pool, id_portfolio).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["refresh_request"]["status"], "failed");
        assert_eq!(body["refresh_request"]["error_code"], "refresh_failed");
        assert_eq!(body["refresh_request"]["attempts"], 3);
        assert!(
            !raw_text.contains("SENSITIVE_INTERNAL_DETAIL_should_not_leak"),
            "raw last_error must never be exposed"
        );
        assert!(
            !raw_text.contains("last_error"),
            "response must not contain a last_error field"
        );
    }

    #[tokio::test]
    async fn refresh_request_from_another_user_is_not_exposed() {
        let pool = test_pool().await;
        let owner_handle = format!("own{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_owner = create_user(&pool, &owner_handle).await;
        let id_portfolio = create_portfolio(&pool, id_owner, None).await;

        let (_, create_body) = post_json(
            &pool,
            id_owner,
            &owner_handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            posted_deposit_payload(),
        )
        .await;
        let id_refresh = create_body["refresh_request"]["id_portfolio_refresh_request"]
            .as_str()
            .expect("refresh request id")
            .to_string();

        let attacker_handle = format!("atk{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_attacker = create_user(&pool, &attacker_handle).await;

        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/refresh-requests/{id_refresh}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!(
                            "Bearer {}",
                            build_access_token(id_attacker, &attacker_handle)
                        ),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let status = response.status();

        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_user_tree(&pool, id_attacker, &[]).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_pending_deposit_operation() {
        let pool = test_pool().await;
        let handle = format!("opd{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(deposit_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["operation"]["operation_type"], "deposit");
        assert_eq!(body["operation"]["operation_status"], "pending");
        assert_rfc3339_string(&body["operation"]["created_at"]);
        assert_rfc3339_string(&body["operation"]["updated_at"]);
        assert_rfc3339_string(&body["operation"]["executed_at"]);
    }

    #[tokio::test]
    async fn create_pending_buy_operation() {
        let pool = test_pool().await;
        let handle = format!("opb{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset(
            &pool,
            &format!("SYM{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(buy_payload(id_asset).to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_asset]).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["operation"]["operation_type"], "buy");
        assert_eq!(body["operation"]["id_asset"], id_asset.to_string());
    }

    #[tokio::test]
    async fn create_buy_with_missing_id_asset_fails_cleanly() {
        let pool = test_pool().await;
        let handle = format!("obm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = buy_payload(Uuid::new_v4());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_asset_reference");
    }

    #[tokio::test]
    async fn create_buy_with_inactive_id_asset_fails_cleanly() {
        let pool = test_pool().await;
        let handle = format!("obi{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset_with_status(
            &pool,
            &format!("INA{}", &Uuid::new_v4().simple().to_string()[..8]),
            "inactive",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(buy_payload(id_asset).to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_asset]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "inactive_asset_reference");
    }

    #[tokio::test]
    async fn create_dividend_with_missing_id_asset_fails_cleanly() {
        let pool = test_pool().await;
        let handle = format!("odm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = json!({
            "id_asset": Uuid::new_v4(),
            "operation_type": "dividend",
            "executed_at": "2026-06-05T10:00:00Z",
            "gross_amount_minor": 1000,
            "cash_amount_minor": 1000,
            "currency": "EUR",
            "metadata": {}
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_asset_reference");
    }

    #[tokio::test]
    async fn create_spin_off_with_missing_related_asset_fails_cleanly() {
        let pool = test_pool().await;
        let handle = format!("osm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset(
            &pool,
            &format!("SPA{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = json!({
            "id_asset": id_asset,
            "id_related_asset": Uuid::new_v4(),
            "operation_type": "spin_off",
            "executed_at": "2026-06-05T10:00:00Z",
            "quantity": "10.0000000000",
            "related_quantity": "5.0000000000",
            "cash_amount_minor": 0,
            "currency": "EUR",
            "metadata": {}
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_asset]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_related_asset_reference");
    }

    #[tokio::test]
    async fn create_symbol_change_with_same_asset_relation_fails_cleanly() {
        let pool = test_pool().await;
        let handle = format!("osc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset(
            &pool,
            &format!("SCA{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = json!({
            "id_asset": id_asset,
            "id_related_asset": id_asset,
            "operation_type": "symbol_change",
            "executed_at": "2026-06-05T10:00:00Z",
            "quantity": "10.0000000000",
            "cash_amount_minor": 0,
            "currency": "EUR",
            "metadata": {}
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_asset]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "same_asset_relation");
    }

    #[tokio::test]
    async fn default_status_is_pending() {
        let pool = test_pool().await;
        let handle = format!("ops{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(deposit_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(body["operation"]["operation_status"], "pending");
    }

    #[tokio::test]
    async fn reject_invalid_operation_type() {
        let pool = test_pool().await;
        let handle = format!("opi{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "operation_type": "unknown",
                            "executed_at": "2026-06-05T10:00:00Z",
                            "gross_amount_minor": 1000,
                            "cash_amount_minor": 1000,
                            "currency": "EUR"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn create_operation_malformed_json_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("omj{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"operation_type":"deposit""#))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let headers = response.headers().clone();
        let body = response_json(response).await;

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(body["error"]["code"], "invalid_json_body");
    }

    #[tokio::test]
    async fn create_operation_missing_required_field_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("omr{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"executed_at":"2026-06-05T10:00:00Z","currency":"EUR"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn create_operation_wrong_numeric_type_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("owt{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"operation_type":"deposit","executed_at":"2026-06-05T10:00:00Z","cash_amount_minor":"abc","currency":"EUR"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn create_operation_unknown_field_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("ouf{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"operation_type":"deposit","executed_at":"2026-06-05T10:00:00Z","cash_amount_minor":1000,"currency":"EUR","unexpected":true}"#,
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn update_operation_wrong_field_type_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("upw{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"price_minor":"broken"}"#))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn update_operation_unknown_field_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("upu{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"unknown_field":true}"#))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn correction_malformed_json_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("cmj{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"executed_at":"2026-06-06T10:00:00Z""#))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_json_body");
    }

    #[tokio::test]
    async fn correction_empty_body_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("ceb{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_json_body");
    }

    #[tokio::test]
    async fn correction_wrong_field_type_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("cwt{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"executed_at":"2026-06-06T10:00:00Z","cash_amount_minor":"oops"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn correction_unknown_field_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("cuf{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"executed_at":"2026-06-06T10:00:00Z","cash_amount_minor":1000,"currency":"EUR","unknown":true}"#,
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn reject_invalid_currency() {
        // P1: lowercase/whitespace input is normalized and (if part of the
        // catalogue) accepted. To exercise the format-level rejection we use
        // a malformed code that cannot normalize to 3 ASCII uppercase
        // letters.
        let pool = test_pool().await;
        let handle = format!("opc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let mut payload = deposit_payload();
        payload["currency"] = json!("EURO");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_currency");
    }

    #[tokio::test]
    async fn reject_create_on_portfolio_owned_by_another_user() {
        let pool = test_pool().await;
        let handle_a = format!("opa{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("opb{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(deposit_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        cleanup_user_tree(&pool, user_b, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn reject_create_on_soft_deleted_portfolio() {
        let pool = test_pool().await;
        let handle = format!("opx{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let deleted_at = OffsetDateTime::now_utc() + Duration::seconds(5);
        let id_portfolio = create_portfolio(&pool, id_user, Some(deleted_at)).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(deposit_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn reject_refresh_token_as_auth() {
        let pool = test_pool().await;
        let handle = format!("opr{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_refresh_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(deposit_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_only_operations_for_owned_portfolio() {
        let pool = test_pool().await;
        let handle = format!("opl{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio_a = create_portfolio(&pool, id_user, None).await;
        let id_portfolio_b = create_portfolio(&pool, id_user, None).await;
        insert_operation(&pool, id_portfolio_a, "pending", deposit_payload()).await;
        insert_operation(&pool, id_portfolio_b, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio_a}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(body["operations"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_does_not_leak_another_users_portfolio_operations() {
        let pool = test_pool().await;
        let handle_a = format!("ol1{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("ol2{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        cleanup_user_tree(&pool, user_b, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn filter_by_operation_status_if_implemented() {
        let pool = test_pool().await;
        let handle = format!("ofs{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations?operation_status=pending"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["operations"].as_array().unwrap().len(), 1);
        assert_eq!(body["operations"][0]["operation_status"], "pending");
    }

    #[tokio::test]
    async fn filter_by_operation_type_if_implemented() {
        let pool = test_pool().await;
        let handle = format!("oft{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset(
            &pool,
            &format!("S{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        insert_operation(&pool, id_portfolio, "pending", buy_payload(id_asset)).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations?operation_type=buy"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_asset]).await;
        assert_eq!(body["operations"].as_array().unwrap().len(), 1);
        assert_eq!(body["operations"][0]["operation_type"], "buy");
    }

    #[tokio::test]
    async fn get_owned_operation() {
        let pool = test_pool().await;
        let handle = format!("ogo{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(
            body["operation"]["id_portfolio_operation"],
            id_operation.to_string()
        );
    }

    #[tokio::test]
    async fn return_404_for_another_users_operation() {
        let pool = test_pool().await;
        let handle_a = format!("og1{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("og2{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        cleanup_user_tree(&pool, user_b, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn return_404_for_operation_from_another_portfolio() {
        let pool = test_pool().await;
        let handle = format!("ogp{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let portfolio_a = create_portfolio(&pool, id_user, None).await;
        let portfolio_b = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, portfolio_b, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_a}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn update_pending_operation_succeeds() {
        let pool = test_pool().await;
        let handle = format!("oup{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "notes": "updated note",
                            "gross_amount_minor": 110000,
                            "cash_amount_minor": 110000
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(body["operation"]["notes"], "updated note");
        assert_eq!(body["operation"]["gross_amount_minor"], 110000);
    }

    #[tokio::test]
    async fn update_pending_operation_to_missing_id_asset_fails() {
        let pool = test_pool().await;
        let handle = format!("oum{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset(
            &pool,
            &format!("UPM{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", buy_payload(id_asset)).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "id_asset": Uuid::new_v4() }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_asset]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_asset_reference");
    }

    #[tokio::test]
    async fn update_pending_operation_to_inactive_id_asset_fails() {
        let pool = test_pool().await;
        let handle = format!("oui{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original_asset = create_asset(
            &pool,
            &format!("UOA{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let id_inactive_asset = create_asset_with_status(
            &pool,
            &format!("UIA{}", &Uuid::new_v4().simple().to_string()[..8]),
            "inactive",
        )
        .await;
        let id_operation = insert_operation(
            &pool,
            id_portfolio,
            "pending",
            buy_payload(id_original_asset),
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "id_asset": id_inactive_asset }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_original_asset, id_inactive_asset]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "inactive_asset_reference");
    }

    #[tokio::test]
    async fn update_pending_operation_with_valid_active_asset_succeeds() {
        let pool = test_pool().await;
        let handle = format!("ouv{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original_asset = create_asset(
            &pool,
            &format!("UVA{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let id_new_asset = create_asset(
            &pool,
            &format!("UVB{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let id_operation = insert_operation(
            &pool,
            id_portfolio,
            "pending",
            buy_payload(id_original_asset),
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "id_asset": id_new_asset }).to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_original_asset, id_new_asset]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operation"]["id_asset"], id_new_asset.to_string());
    }

    #[tokio::test]
    async fn update_posted_operation_rejected_before_db_trigger() {
        let pool = test_pool().await;
        let handle = format!("oup{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "notes": "forbidden" }).to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "posted_operation_immutable");
    }

    #[tokio::test]
    async fn update_cancelled_operation_rejected() {
        let pool = test_pool().await;
        let handle = format!("ouc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "cancelled", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "notes": "forbidden" }).to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn update_another_users_operation_returns_404() {
        let pool = test_pool().await;
        let handle_a = format!("ou1{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("ou2{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "notes": "forbidden" }).to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        cleanup_user_tree(&pool, user_b, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn cancel_pending_operation_succeeds() {
        let pool = test_pool().await;
        let handle = format!("ocp{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/cancel"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(body["operation"]["operation_status"], "cancelled");
    }

    #[tokio::test]
    async fn cancel_posted_operation_rejected() {
        let pool = test_pool().await;
        let handle = format!("ocx{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/cancel"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn cancel_already_cancelled_operation_is_idempotent_success() {
        let pool = test_pool().await;
        let handle = format!("oci{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "cancelled", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/cancel"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operation"]["operation_status"], "cancelled");
    }

    #[tokio::test]
    async fn cancel_another_users_operation_returns_404() {
        let pool = test_pool().await;
        let handle_a = format!("oc1{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("oc2{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/cancel"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        cleanup_user_tree(&pool, user_b, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_correction_for_posted_operation_succeeds() {
        let pool = test_pool().await;
        let handle = format!("occ{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        let original = sqlx::query(
            r#"
            SELECT operation_type, operation_status, id_corrected_operation, cash_amount_minor
            FROM portfolio_operations
            WHERE id_portfolio_operation = $1
            "#,
        )
        .bind(id_operation)
        .fetch_one(&pool)
        .await
        .expect("original operation should still exist");

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["operation"]["operation_type"], "adjustment");
        assert_eq!(body["operation"]["operation_status"], "pending");
        assert_eq!(
            body["operation"]["id_corrected_operation"],
            id_operation.to_string()
        );
        assert_rfc3339_string(&body["operation"]["created_at"]);
        assert_rfc3339_string(&body["operation"]["updated_at"]);
        assert_rfc3339_string(&body["operation"]["executed_at"]);
        assert_eq!(
            original.get::<String, _>("operation_type"),
            "deposit".to_string()
        );
        assert_eq!(
            original.get::<String, _>("operation_status"),
            "posted".to_string()
        );
        assert_eq!(
            original.get::<Option<Uuid>, _>("id_corrected_operation"),
            None
        );
        assert_eq!(original.get::<i64, _>("cash_amount_minor"), 100000);
    }

    #[tokio::test]
    async fn create_adjustment_correction_with_existing_active_id_asset_succeeds() {
        let pool = test_pool().await;
        let handle = format!("oca{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let id_asset = create_asset(
            &pool,
            &format!("CAA{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = json!({
            "executed_at": "2026-06-06T10:00:00Z",
            "id_asset": id_asset,
            "quantity": "1.0000000000",
            "cash_amount_minor": 5000,
            "currency": "EUR",
            "metadata": { "reason": "asset_adjustment" }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["operation"]["id_asset"], id_asset.to_string());
        assert_eq!(body["operation"]["operation_type"], "adjustment");
    }

    #[tokio::test]
    async fn create_adjustment_correction_with_missing_id_asset_fails() {
        let pool = test_pool().await;
        let handle = format!("ocm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = json!({
            "executed_at": "2026-06-06T10:00:00Z",
            "id_asset": Uuid::new_v4(),
            "quantity": "1.0000000000",
            "cash_amount_minor": 5000,
            "currency": "EUR",
            "metadata": { "reason": "asset_adjustment" }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_asset_reference");
    }

    #[tokio::test]
    async fn create_adjustment_correction_with_inactive_id_asset_fails() {
        let pool = test_pool().await;
        let handle = format!("oci{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let id_asset = create_asset_with_status(
            &pool,
            &format!("CIA{}", &Uuid::new_v4().simple().to_string()[..8]),
            "inactive",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = json!({
            "executed_at": "2026-06-06T10:00:00Z",
            "id_asset": id_asset,
            "quantity": "1.0000000000",
            "cash_amount_minor": 5000,
            "currency": "EUR",
            "metadata": { "reason": "asset_adjustment" }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "inactive_asset_reference");
    }

    #[tokio::test]
    async fn correction_appears_in_operation_list() {
        let pool = test_pool().await;
        let handle = format!("ocl{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let correction_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("correction response should be built");
        let correction_body = response_json(correction_response).await;

        let list_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("list response should be built");

        let list_body = response_json(list_response).await;
        let operations = list_body["operations"]
            .as_array()
            .expect("operations array");
        assert_eq!(operations.len(), 2);
        assert_eq!(
            operations[0]["id_corrected_operation"],
            id_operation.to_string()
        );
        assert_eq!(
            correction_body["operation"]["id_portfolio_operation"],
            operations[0]["id_portfolio_operation"]
        );
    }

    #[tokio::test]
    async fn correcting_pending_operation_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("ocp{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body["error"]["code"],
            "correction_requires_posted_operation"
        );
    }

    #[tokio::test]
    async fn correcting_cancelled_operation_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("ocz{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "cancelled", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body["error"]["code"],
            "correction_requires_posted_operation"
        );
    }

    #[tokio::test]
    async fn correcting_another_users_operation_returns_404() {
        let pool = test_pool().await;
        let handle_a = format!("oca{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("ocb{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn correcting_operation_from_another_portfolio_returns_404() {
        let pool = test_pool().await;
        let handle = format!("ocm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let portfolio_a = create_portfolio(&pool, id_user, None).await;
        let portfolio_b = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, portfolio_b, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{portfolio_a}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn correction_with_refresh_token_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("ocr{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_refresh_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn correction_with_invalid_currency_is_rejected() {
        // P1: lowercase normalizes to uppercase; to exercise format rejection
        // we use a malformed code.
        let pool = test_pool().await;
        let handle = format!("ocu{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let mut payload = correction_payload();
        payload["currency"] = json!("EURO");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "invalid_currency");
    }

    #[tokio::test]
    async fn correction_with_external_provider_but_no_external_reference_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("oce{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let mut payload = correction_payload();
        payload["external_provider"] = json!("manual");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "invalid_external_reference");
    }

    #[tokio::test]
    async fn correction_with_external_reference_but_no_external_provider_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("ocf{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let mut payload = correction_payload();
        payload["external_reference"] = json!("reference-only");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "invalid_external_reference");
    }

    #[tokio::test]
    async fn correction_with_no_meaningful_adjustment_data_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("ocn{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = json!({
            "executed_at": "2026-06-06T10:00:00Z",
            "metadata": {}
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "empty_correction");
    }

    #[tokio::test]
    async fn post_pending_operation_succeeds() {
        let pool = test_pool().await;
        let handle = format!("opp{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operation"]["operation_status"], "posted");
        assert_rfc3339_string(&body["operation"]["created_at"]);
        assert_rfc3339_string(&body["operation"]["updated_at"]);
        assert_rfc3339_string(&body["operation"]["executed_at"]);
    }

    #[tokio::test]
    async fn post_pending_operation_with_active_asset_succeeds() {
        let pool = test_pool().await;
        let handle = format!("opa{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset(
            &pool,
            &format!("PAA{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", buy_payload(id_asset)).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operation"]["operation_status"], "posted");
        assert_eq!(body["operation"]["id_asset"], id_asset.to_string());
    }

    #[tokio::test]
    async fn post_pending_operation_whose_asset_became_inactive_fails_cleanly() {
        let pool = test_pool().await;
        let handle = format!("opi{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset(
            &pool,
            &format!("PIA{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", buy_payload(id_asset)).await;
        set_asset_status(&pool, id_asset, "inactive").await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_asset]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "inactive_asset_reference");
    }

    #[tokio::test]
    async fn after_posting_patch_update_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("opu{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("post response should be built");

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "notes": "forbidden" }).to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "posted_operation_immutable");
    }

    #[tokio::test]
    async fn after_posting_cancel_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("opc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("post response should be built");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/cancel"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("cancel response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "posted_operation_immutable");
    }

    #[tokio::test]
    async fn db_trigger_prevents_direct_mutation_of_posted_operation() {
        let pool = test_pool().await;
        let handle = format!("opd{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let _ = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("post response should be built");

        let error = sqlx::query(
            r#"
            UPDATE portfolio_operations
            SET notes = 'direct mutation'
            WHERE id_portfolio_operation = $1
            "#,
        )
        .bind(id_operation)
        .execute(&pool)
        .await
        .expect_err("direct update on posted operation must fail");

        assert!(
            error
                .to_string()
                .contains("posted portfolio_operations are immutable"),
            "unexpected DB error: {error}"
        );
    }

    #[tokio::test]
    async fn post_already_posted_operation_returns_409() {
        let pool = test_pool().await;
        let handle = format!("opo{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "operation_already_posted");
    }

    #[tokio::test]
    async fn post_cancelled_operation_returns_409() {
        let pool = test_pool().await;
        let handle = format!("opx{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "cancelled", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body["error"]["code"],
            "cancelled_operation_cannot_be_posted"
        );
    }

    #[tokio::test]
    async fn post_operation_from_another_user_returns_404() {
        let pool = test_pool().await;
        let handle_a = format!("opy{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("opz{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        cleanup_user_tree(&pool, user_b, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn post_operation_from_another_portfolio_returns_404() {
        let pool = test_pool().await;
        let handle = format!("opm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let portfolio_a = create_portfolio(&pool, id_user, None).await;
        let portfolio_b = create_portfolio(&pool, id_user, None).await;
        let id_operation = insert_operation(&pool, portfolio_b, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{portfolio_a}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn post_with_refresh_token_returns_401() {
        let pool = test_pool().await;
        let handle = format!("opr{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_refresh_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn valid_adjustment_correction_can_be_posted() {
        let pool = test_pool().await;
        let handle = format!("opa{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let id_asset = create_asset(
            &pool,
            &format!("APA{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = json!({
            "executed_at": "2026-06-06T10:00:00Z",
            "id_asset": id_asset,
            "quantity": "1.0000000000",
            "cash_amount_minor": 5000,
            "currency": "EUR",
            "metadata": {
                "reason": "manual_correction"
            }
        });

        let correction_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("correction response should be built");
        let correction_body = response_json(correction_response).await;
        let id_correction = correction_body["operation"]["id_portfolio_operation"]
            .as_str()
            .expect("correction id")
            .to_string();

        let post_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_correction}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("post response should be built");

        let status = post_response.status();
        let body = response_json(post_response).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operation"]["operation_status"], "posted");
        assert_eq!(body["operation"]["operation_type"], "adjustment");
        assert_eq!(body["operation"]["id_asset"], id_asset.to_string());
    }

    #[tokio::test]
    async fn invalid_pending_adjustment_cannot_be_posted() {
        let pool = test_pool().await;
        let handle = format!("opi{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_operation =
            insert_operation(&pool, id_portfolio, "pending", empty_adjustment_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}/post"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "empty_correction");
    }

    #[tokio::test]
    async fn corrections_endpoint_returns_corrections_for_original_operation() {
        let pool = test_pool().await;
        let handle = format!("orc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("correction response should be built");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(
            body["operation"]["id_portfolio_operation"],
            id_original.to_string()
        );
        assert_eq!(body["corrections"].as_array().unwrap().len(), 1);
        assert_eq!(body["corrections"][0]["operation_type"], "adjustment");
        assert_eq!(
            body["corrections"][0]["id_corrected_operation"],
            id_original.to_string()
        );
        assert_rfc3339_string(&body["operation"]["created_at"]);
        assert_rfc3339_string(&body["corrections"][0]["created_at"]);
        assert_rfc3339_string(&body["corrections"][0]["executed_at"]);
    }

    #[tokio::test]
    async fn corrections_endpoint_returns_empty_array_when_none_exist() {
        let pool = test_pool().await;
        let handle = format!("ore{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert!(body["corrections"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn corrections_endpoint_sorts_by_executed_at_asc_then_created_at_asc() {
        let pool = test_pool().await;
        let handle = format!("ors{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let correction_a = json!({
            "executed_at": "2026-06-07T10:00:00Z",
            "cash_amount_minor": 5000,
            "currency": "EUR",
            "metadata": { "reason": "late" }
        });
        let correction_b = json!({
            "executed_at": "2026-06-06T10:00:00Z",
            "cash_amount_minor": 4000,
            "currency": "EUR",
            "metadata": { "reason": "early" }
        });
        let app = crate::http::router(test_state(pool.clone()).await);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_a.to_string()))
                    .unwrap(),
            )
            .await
            .expect("first correction response should be built");

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_b.to_string()))
                    .unwrap(),
            )
            .await
            .expect("second correction response should be built");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let corrections = body["corrections"].as_array().unwrap();
        assert_eq!(corrections.len(), 2);
        assert_eq!(corrections[0]["executed_at"], "2026-06-06T10:00:00Z");
        assert_eq!(corrections[1]["executed_at"], "2026-06-07T10:00:00Z");
    }

    #[tokio::test]
    async fn corrections_endpoint_cross_user_returns_404() {
        let pool = test_pool().await;
        let handle_a = format!("or1{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("or2{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn corrections_endpoint_wrong_portfolio_returns_404() {
        let pool = test_pool().await;
        let handle = format!("orw{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let portfolio_a = create_portfolio(&pool, id_user, None).await;
        let portfolio_b = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, portfolio_b, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_a}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn corrections_endpoint_refresh_token_rejected() {
        let pool = test_pool().await;
        let handle = format!("orr{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_refresh_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn audit_for_original_operation_includes_corrections() {
        let pool = test_pool().await;
        let handle = format!("oao{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("correction response should be built");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/audit"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(
            body["operation"]["id_portfolio_operation"],
            id_original.to_string()
        );
        assert!(body["corrected_operation"].is_null());
        assert_eq!(body["correction_count"], 1);
        assert_eq!(body["corrections"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn audit_for_adjustment_includes_corrected_operation() {
        let pool = test_pool().await;
        let handle = format!("oaa{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let correction_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("correction response should be built");
        let correction_body = response_json(correction_response).await;
        let id_correction = correction_body["operation"]["id_portfolio_operation"]
            .as_str()
            .unwrap()
            .to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_correction}/audit"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["operation"]["operation_type"], "adjustment");
        assert_eq!(
            body["corrected_operation"]["id_portfolio_operation"],
            id_original.to_string()
        );
        assert_eq!(body["correction_count"], 1);
        assert_eq!(body["corrections"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn audit_cross_user_returns_404() {
        let pool = test_pool().await;
        let handle_a = format!("oa1{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("oa2{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_original}/audit"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn audit_wrong_portfolio_returns_404() {
        let pool = test_pool().await;
        let handle = format!("oaw{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let portfolio_a = create_portfolio(&pool, id_user, None).await;
        let portfolio_b = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, portfolio_b, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_a}/operations/{id_original}/audit"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn audit_timeline_returns_primary_operations_with_nested_corrections() {
        let pool = test_pool().await;
        let handle = format!("atl{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_primary = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_primary}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("correction response should be built");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations/audit"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0]["operation"]["id_portfolio_operation"],
            id_primary.to_string()
        );
        assert_eq!(items[0]["operation"]["operation_type"], "deposit");
        assert_eq!(items[0]["correction_count"], 1);
        assert_eq!(items[0]["corrections"].as_array().unwrap().len(), 1);
        assert_eq!(items[0]["corrections"][0]["operation_type"], "adjustment");
        assert_eq!(
            items[0]["corrections"][0]["id_corrected_operation"],
            id_primary.to_string()
        );
        assert_rfc3339_string(&items[0]["operation"]["created_at"]);
        assert_rfc3339_string(&items[0]["operation"]["updated_at"]);
        assert_rfc3339_string(&items[0]["operation"]["executed_at"]);
        assert_rfc3339_string(&items[0]["corrections"][0]["created_at"]);
        assert_rfc3339_string(&items[0]["corrections"][0]["updated_at"]);
        assert_rfc3339_string(&items[0]["corrections"][0]["executed_at"]);
    }

    #[tokio::test]
    async fn audit_timeline_returns_empty_items_for_empty_portfolio() {
        let pool = test_pool().await;
        let handle = format!("ate{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations/audit"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert!(body["items"].as_array().unwrap().is_empty());
        assert_eq!(body["pagination"]["limit"], 50);
        assert_eq!(body["pagination"]["offset"], 0);
        assert_eq!(body["pagination"]["returned"], 0);
        assert_eq!(body["pagination"]["has_more"], false);
        cleanup_user_tree(&pool, id_user, &[]).await;
    }

    #[tokio::test]
    async fn audit_timeline_keeps_adjustments_nested_only() {
        let pool = test_pool().await;
        let handle = format!("atn{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_primary = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let correction_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_primary}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("correction response should be built");
        let correction_body = response_json(correction_response).await;
        let id_correction = correction_body["operation"]["id_portfolio_operation"]
            .as_str()
            .unwrap()
            .to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations/audit"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_ne!(
            items[0]["operation"]["id_portfolio_operation"],
            id_correction
        );
    }

    #[tokio::test]
    async fn audit_timeline_sorts_top_level_by_executed_at_desc_then_created_at_desc() {
        let pool = test_pool().await;
        let handle = format!("ats{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let older = json!({
            "id_asset": null,
            "id_related_asset": null,
            "operation_type": "deposit",
            "executed_at": "2026-06-05T10:00:00Z",
            "gross_amount_minor": 1000,
            "cash_amount_minor": 1000,
            "currency": "EUR",
            "metadata": {}
        });
        let newer = json!({
            "id_asset": null,
            "id_related_asset": null,
            "operation_type": "deposit",
            "executed_at": "2026-06-06T10:00:00Z",
            "gross_amount_minor": 2000,
            "cash_amount_minor": 2000,
            "currency": "EUR",
            "metadata": {}
        });

        let older_id = insert_operation(&pool, id_portfolio, "posted", older).await;
        let newer_id = insert_operation(&pool, id_portfolio, "posted", newer).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations/audit"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(
            items[0]["operation"]["id_portfolio_operation"],
            newer_id.to_string()
        );
        assert_eq!(
            items[1]["operation"]["id_portfolio_operation"],
            older_id.to_string()
        );
    }

    #[tokio::test]
    async fn audit_timeline_sorts_corrections_by_executed_at_asc_then_created_at_asc() {
        let pool = test_pool().await;
        let handle = format!("atc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_primary = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let early = json!({
            "executed_at": "2026-06-06T10:00:00Z",
            "cash_amount_minor": 1000,
            "currency": "EUR",
            "metadata": { "order": "early" }
        });
        let late = json!({
            "executed_at": "2026-06-07T10:00:00Z",
            "cash_amount_minor": 2000,
            "currency": "EUR",
            "metadata": { "order": "late" }
        });

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_primary}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(late.to_string()))
                    .unwrap(),
            )
            .await
            .expect("late correction response should be built");

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_primary}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(early.to_string()))
                    .unwrap(),
            )
            .await
            .expect("early correction response should be built");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations/audit"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let corrections = body["items"][0]["corrections"].as_array().unwrap();
        assert_eq!(corrections.len(), 2);
        assert_eq!(corrections[0]["executed_at"], "2026-06-06T10:00:00Z");
        assert_eq!(corrections[1]["executed_at"], "2026-06-07T10:00:00Z");
    }

    #[tokio::test]
    async fn audit_timeline_pagination_limit_offset_and_has_more_work() {
        let pool = test_pool().await;
        let handle = format!("atp{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let first = json!({
            "operation_type": "deposit",
            "executed_at": "2026-06-05T10:00:00Z",
            "gross_amount_minor": 1000,
            "cash_amount_minor": 1000,
            "currency": "EUR",
            "metadata": {}
        });
        let second = json!({
            "operation_type": "deposit",
            "executed_at": "2026-06-06T10:00:00Z",
            "gross_amount_minor": 2000,
            "cash_amount_minor": 2000,
            "currency": "EUR",
            "metadata": {}
        });
        let third = json!({
            "operation_type": "deposit",
            "executed_at": "2026-06-07T10:00:00Z",
            "gross_amount_minor": 3000,
            "cash_amount_minor": 3000,
            "currency": "EUR",
            "metadata": {}
        });

        let first_id = insert_operation(&pool, id_portfolio, "posted", first).await;
        let second_id = insert_operation(&pool, id_portfolio, "posted", second).await;
        let third_id = insert_operation(&pool, id_portfolio, "posted", third).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?limit=2"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(body["pagination"]["limit"], 2);
        assert_eq!(body["pagination"]["offset"], 0);
        assert_eq!(body["pagination"]["returned"], 2);
        assert_eq!(body["pagination"]["has_more"], true);
        assert_eq!(
            items[0]["operation"]["id_portfolio_operation"],
            third_id.to_string()
        );
        assert_eq!(
            items[1]["operation"]["id_portfolio_operation"],
            second_id.to_string()
        );

        let second_page = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?limit=2&offset=2"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("second page response should be built");

        let second_page_body = response_json(second_page).await;
        let second_page_items = second_page_body["items"].as_array().unwrap();
        assert_eq!(second_page_items.len(), 1);
        assert_eq!(second_page_body["pagination"]["has_more"], false);
        assert_eq!(
            second_page_items[0]["operation"]["id_portfolio_operation"],
            first_id.to_string()
        );
    }

    #[tokio::test]
    async fn audit_timeline_filters_by_operation_status() {
        let pool = test_pool().await;
        let handle = format!("afs{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let _ = insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let posted_id = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?operation_status=posted"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0]["operation"]["id_portfolio_operation"],
            posted_id.to_string()
        );
        assert_eq!(items[0]["operation"]["operation_status"], "posted");
    }

    #[tokio::test]
    async fn audit_timeline_filters_by_operation_type() {
        let pool = test_pool().await;
        let handle = format!("aft{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let deposit_id = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let fee = json!({
            "operation_type": "fee",
            "executed_at": "2026-06-06T10:00:00Z",
            "gross_amount_minor": 1000,
            "cash_amount_minor": 1000,
            "currency": "EUR",
            "metadata": {}
        });
        let _ = insert_operation(&pool, id_portfolio, "posted", fee).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?operation_type=deposit"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0]["operation"]["id_portfolio_operation"],
            deposit_id.to_string()
        );
        assert_eq!(items[0]["operation"]["operation_type"], "deposit");
    }

    #[tokio::test]
    async fn audit_timeline_filters_by_combined_operation_status_and_type() {
        let pool = test_pool().await;
        let handle = format!("afc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let _ = insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let _ = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let fee = json!({
            "operation_type": "fee",
            "executed_at": "2026-06-06T10:00:00Z",
            "gross_amount_minor": 1000,
            "cash_amount_minor": 1000,
            "currency": "EUR",
            "metadata": {}
        });
        let fee_posted_id = insert_operation(&pool, id_portfolio, "posted", fee).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?operation_status=posted&operation_type=fee"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0]["operation"]["id_portfolio_operation"],
            fee_posted_id.to_string()
        );
        assert_eq!(items[0]["operation"]["operation_status"], "posted");
        assert_eq!(items[0]["operation"]["operation_type"], "fee");
    }

    #[tokio::test]
    async fn audit_timeline_invalid_operation_status_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("aiv{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?operation_status=invalid"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_operation_status");
    }

    #[tokio::test]
    async fn audit_timeline_invalid_operation_type_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("ait{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?operation_type=invalid"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_operation_type");
    }

    #[tokio::test]
    async fn audit_timeline_keeps_corrections_nested_when_filter_matches_primary_only() {
        let pool = test_pool().await;
        let handle = format!("afn{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_primary = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_primary}/corrections"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(correction_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("correction response should be built");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?operation_status=posted&operation_type=deposit"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["correction_count"], 1);
        assert_eq!(items[0]["corrections"].as_array().unwrap().len(), 1);
        assert_eq!(items[0]["corrections"][0]["operation_type"], "adjustment");
    }

    #[tokio::test]
    async fn audit_timeline_pagination_still_works_with_filters() {
        let pool = test_pool().await;
        let handle = format!("afp{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let posted_a = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let pending_b = insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        let posted_c = insert_operation(
            &pool,
            id_portfolio,
            "posted",
            json!({
                "operation_type": "deposit",
                "executed_at": "2026-06-07T10:00:00Z",
                "gross_amount_minor": 3000,
                "cash_amount_minor": 3000,
                "currency": "EUR",
                "metadata": {}
            }),
        )
        .await;
        let _ = pending_b;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?operation_status=posted&limit=1"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["pagination"]["returned"], 1);
        assert_eq!(body["pagination"]["has_more"], true);
        assert_eq!(
            body["items"][0]["operation"]["id_portfolio_operation"],
            posted_c.to_string()
        );

        let second_page = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?operation_status=posted&limit=1&offset=1"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let second_page_body = response_json(second_page).await;
        assert_eq!(second_page_body["pagination"]["returned"], 1);
        assert_eq!(second_page_body["pagination"]["has_more"], false);
        assert_eq!(
            second_page_body["items"][0]["operation"]["id_portfolio_operation"],
            posted_a.to_string()
        );
    }

    #[tokio::test]
    async fn audit_timeline_invalid_limit_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("atl{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/audit?limit=101"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_limit");
    }

    #[tokio::test]
    async fn audit_timeline_refresh_token_rejected() {
        let pool = test_pool().await;
        let handle = format!("atr{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations/audit"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_refresh_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn audit_timeline_cross_user_returns_404() {
        let pool = test_pool().await;
        let handle_a = format!("at1{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("at2{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = create_portfolio(&pool, user_b, None).await;
        let _ = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations/audit"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_a, &handle_a)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, user_a, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn audit_timeline_soft_deleted_portfolio_returns_404() {
        let pool = test_pool().await;
        let handle = format!("atd{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(
            &pool,
            id_user,
            Some(OffsetDateTime::now_utc() + Duration::minutes(1)),
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations/audit"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn repository_does_not_reference_worker_tables() {
        let repository_source = include_str!("../repositories/portfolio_operations.rs");
        let service_source = include_str!("../services/portfolio_operations.rs");

        for forbidden in [
            "rm_portfolio_summary",
            "rm_portfolio_holdings",
            "portfolio_snapshots_daily",
            "portfolio_holding_snapshot_daily",
        ] {
            assert!(!repository_source.contains(forbidden));
            assert!(!service_source.contains(forbidden));
        }
    }
}
