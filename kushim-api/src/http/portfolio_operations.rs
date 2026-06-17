use crate::{
    auth::AuthenticatedUser,
    domain::asset::AssetIdentity,
    domain::portfolio_operation::{OperationStatus, OperationType},
    domain::portfolio_refresh_request::{PortfolioRefreshRequest, RefreshRequestStatus},
    errors::{ApiError, IDEMPOTENCY_REPLAYED_HEADER_LOWER},
    http::extractors::{ApiJson, ApiPath, ApiQuery},
    http::idempotency::IdempotencyKey,
    services::portfolio_operations::{
        CancelPortfolioOperationInput, CreatePortfolioOperationCorrectionInput,
        CreatePortfolioOperationInput, IdempotentOperationWriteOutcome,
        ListPortfolioOperationsInput, OperationWriteOutcome, PortfolioOperationAuditTimelineInput,
        PortfolioOperationAuditTimelineItemView, PortfolioOperationAuditTimelineView,
        PortfolioOperationAuditView, PortfolioOperationCorrectionsView,
        PortfolioOperationServiceError, PortfolioOperationView, PostPortfolioOperationInput,
        UpdatePortfolioOperationInput,
    },
    state::AppState,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
};
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

/// Compact asset identity embedded on every operation response.
///
/// Only the fields the Transactions UI needs to render a row are surfaced:
/// `id_asset`, the canonical `name`, the optional `ticker`, and `status` so
/// callers can reason about whether the referenced asset is current. The
/// frontend prefers the ticker, then the name, then a safe fallback.
#[derive(Debug, Serialize)]
pub struct AssetIdentityResponse {
    pub id_asset: Uuid,
    pub name: String,
    pub ticker: Option<String>,
    pub status: String,
}

impl From<AssetIdentity> for AssetIdentityResponse {
    fn from(value: AssetIdentity) -> Self {
        Self {
            id_asset: value.id_asset,
            name: value.name,
            ticker: value.ticker,
            status: value.status.as_str().to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PortfolioOperationResponse {
    pub id_portfolio_operation: Uuid,
    pub id_portfolio: Uuid,
    pub id_asset: Option<Uuid>,
    pub id_related_asset: Option<Uuid>,
    /// Compact identity for `id_asset`, or `null` for cash-only operations and
    /// for operations whose referenced asset row could not be resolved (legacy
    /// or corrupt data). Backward-compatible: `id_asset` remains populated.
    pub asset: Option<AssetIdentityResponse>,
    /// Compact identity for `id_related_asset`, with the same semantics.
    pub related_asset: Option<AssetIdentityResponse>,
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
    IdempotencyKey(idempotency_key): IdempotencyKey,
    ApiPath(id_portfolio): ApiPath<Uuid>,
    ApiJson(request): ApiJson<CreatePortfolioOperationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let result = state
        .portfolio_operation_service
        .create_operation_idempotent(
            CreatePortfolioOperationInput {
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
            },
            idempotency_key,
        )
        .await
        .map_err(map_service_error)?;

    Ok(build_idempotent_write_response(result))
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
    IdempotencyKey(idempotency_key): IdempotencyKey,
    ApiPath((id_portfolio, id_portfolio_operation)): ApiPath<(Uuid, Uuid)>,
    ApiJson(request): ApiJson<CreateCorrectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let result = state
        .portfolio_operation_service
        .create_correction_idempotent(
            CreatePortfolioOperationCorrectionInput {
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
            },
            idempotency_key,
        )
        .await
        .map_err(map_service_error)?;

    Ok(build_idempotent_write_response(result))
}

/// Render an idempotent write outcome as an HTTP response.
///
/// First execution returns `HTTP 201 Created` with `Idempotency-Replayed: false`.
/// Exact replays return `HTTP 200 OK` with `Idempotency-Replayed: true` and
/// the SAME operation/refresh identity as the original write.
fn build_idempotent_write_response(
    result: IdempotentOperationWriteOutcome,
) -> axum::response::Response {
    let IdempotentOperationWriteOutcome { outcome, replayed } = result;
    let status = if replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let header_value = if replayed {
        HeaderValue::from_static("true")
    } else {
        HeaderValue::from_static("false")
    };
    let mut response =
        (status, Json(PortfolioOperationWriteEnvelope::from(outcome))).into_response();
    response.headers_mut().insert(
        axum::http::HeaderName::from_static(IDEMPOTENCY_REPLAYED_HEADER_LOWER),
        header_value,
    );
    response
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

impl From<PortfolioOperationView> for PortfolioOperationResponse {
    fn from(value: PortfolioOperationView) -> Self {
        let PortfolioOperationView {
            operation,
            asset,
            related_asset,
        } = value;
        Self {
            id_portfolio_operation: operation.id_portfolio_operation,
            id_portfolio: operation.id_portfolio,
            id_asset: operation.id_asset,
            id_related_asset: operation.id_related_asset,
            asset: asset.map(AssetIdentityResponse::from),
            related_asset: related_asset.map(AssetIdentityResponse::from),
            operation_type: operation.operation_type.as_str().to_string(),
            operation_status: operation.operation_status.as_str().to_string(),
            executed_at: format_datetime(operation.executed_at),
            effective_at: operation.effective_at.map(format_datetime),
            quantity: operation.quantity,
            related_quantity: operation.related_quantity,
            price_minor: operation.price_minor,
            gross_amount_minor: operation.gross_amount_minor,
            fees_minor: operation.fees_minor,
            taxes_minor: operation.taxes_minor,
            cash_amount_minor: operation.cash_amount_minor,
            currency: operation.currency,
            fx_rate_to_portfolio: operation.fx_rate_to_portfolio,
            external_provider: operation.external_provider,
            external_reference: operation.external_reference,
            id_corrected_operation: operation.id_corrected_operation,
            notes: operation.notes,
            metadata: operation.metadata,
            created_at: format_datetime(operation.created_at),
            updated_at: format_datetime(operation.updated_at),
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
            assets::AssetRepository,
            portfolio_operation_idempotency::PortfolioOperationIdempotencyRepository,
            portfolio_operations::PortfolioOperationRepository,
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
        let database_url = crate::test_support::require_disposable_test_database_url();
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    use crate::test_support::ensure_canonical_user_role;

    async fn create_user(pool: &PgPool, public_handle: &str) -> Uuid {
        ensure_canonical_user_role(pool).await;

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

    /// Tears down everything a test created under `id_user` plus any
    /// orphaned test-fixture assets.
    ///
    /// Order matters because foreign keys to `assets` are RESTRICT-keyed and
    /// dependent rows must be removed before the assets themselves:
    ///   1. derived per-portfolio tables under `id_user`
    ///   2. the user's portfolio_operations
    ///   3. the user's portfolios
    ///   4. the user itself
    ///   5. explicit assets the caller owns (legacy contract, kept intact)
    ///   6. defensive sweep — see below
    ///
    /// Defensive orphan sweep. After deleting the user, any equity-fixture
    /// asset (`asset_class='equity' AND exchange IS NULL`) the test created
    /// but did NOT pass to `asset_ids` (panic before cleanup, forgotten
    /// argument, etc.) becomes orphaned: it has no remaining references in
    /// any RESTRICT-keyed FK table. Mirrors the policy of
    /// `scripts/dev/clean-asset-catalog.ps1` so a leak healed by panic
    /// recovery is removed deterministically at the next clean teardown.
    ///
    /// Concurrency safety. A parallel test's fixture assets stay referenced
    /// by THAT test's still-live `portfolio_operations` until ITS own
    /// cleanup, so the sweep only removes assets whose creating test has
    /// already torn down its rows. Canonical seeded assets always have
    /// `exchange IS NOT NULL` and are never touched.
    async fn cleanup_user_tree(pool: &PgPool, id_user: Uuid, asset_ids: &[Uuid]) {
        // P3 audit FKs use ON DELETE RESTRICT for user/portfolio/operation,
        // so the idempotency rows must be cleaned up BEFORE the operations
        // they reference.
        sqlx::query(
            r#"
        DELETE FROM portfolio_operation_idempotency
        WHERE id_user = $1
        "#,
        )
        .bind(id_user)
        .execute(pool)
        .await
        .expect("idempotency records should be deleted");

        // Holding snapshots reference assets via RESTRICT — scrub them before
        // attempting any asset deletion. Same for the holdings read model.
        sqlx::query(
            r#"
        DELETE FROM portfolio_holding_snapshot_daily
        WHERE id_portfolio_snapshot_daily IN (
            SELECT id_portfolio_snapshot_daily
            FROM portfolio_snapshots_daily
            WHERE id_portfolio IN (
                SELECT id_portfolio
                FROM portfolios
                WHERE id_user = $1
            )
        )
        "#,
        )
        .bind(id_user)
        .execute(pool)
        .await
        .expect("holding snapshots should be deleted");

        sqlx::query(
            r#"
        DELETE FROM portfolio_snapshots_daily
        WHERE id_portfolio IN (
            SELECT id_portfolio
            FROM portfolios
            WHERE id_user = $1
        )
        "#,
        )
        .bind(id_user)
        .execute(pool)
        .await
        .expect("snapshots should be deleted");

        sqlx::query(
            r#"
        DELETE FROM rm_portfolio_holdings
        WHERE id_portfolio IN (
            SELECT id_portfolio
            FROM portfolios
            WHERE id_user = $1
        )
        "#,
        )
        .bind(id_user)
        .execute(pool)
        .await
        .expect("holdings read model should be deleted");

        sqlx::query(
            r#"
        DELETE FROM rm_portfolio_summary
        WHERE id_portfolio IN (
            SELECT id_portfolio
            FROM portfolios
            WHERE id_user = $1
        )
        "#,
        )
        .bind(id_user)
        .execute(pool)
        .await
        .expect("summary read model should be deleted");

        sqlx::query(
            r#"
        DELETE FROM portfolio_refresh_requests
        WHERE id_portfolio IN (
            SELECT id_portfolio
            FROM portfolios
            WHERE id_user = $1
        )
        "#,
        )
        .bind(id_user)
        .execute(pool)
        .await
        .expect("refresh requests should be deleted");

        // Posted portfolio_operations are intentionally immutable through the
        // `prevent_posted_operation_mutation` database trigger. Only operations
        // whose status permits deletion are removed here.
        sqlx::query(
            r#"
        DELETE FROM portfolio_operations
        WHERE id_portfolio IN (
            SELECT id_portfolio
            FROM portfolios
            WHERE id_user = $1
        )
          AND operation_status IN ('pending', 'cancelled')
        "#,
        )
        .bind(id_user)
        .execute(pool)
        .await
        .expect("deletable operations should be deleted");

        // These deletes may remain blocked by immutable posted operations.
        // The tests now run in disposable databases, so any remaining tree is
        // removed when the temporary database itself is dropped.
        let _ = sqlx::query("DELETE FROM portfolios WHERE id_user = $1")
            .bind(id_user)
            .execute(pool)
            .await;

        let _ = sqlx::query("DELETE FROM users WHERE id_user = $1")
            .bind(id_user)
            .execute(pool)
            .await;

        for id_asset in asset_ids {
            sqlx::query("DELETE FROM assets WHERE id_asset = $1")
                .bind(id_asset)
                .execute(pool)
                .await
                .expect("asset should be deleted");
        }

        // Intentionally no defensive global sweep here: a concurrent test may
        // have created an asset but not yet inserted the operation referencing it.
        // Exact UUID deletion remains race-safe.
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
            PortfolioOperationIdempotencyRepository::new(pool.clone()),
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

    /// Whether a POST URI targets a P3 idempotent endpoint. Used by the
    /// generic `post_json` helper to inject a random `Idempotency-Key` for
    /// the existing legacy test bodies; tests that exercise the idempotency
    /// contract itself use `post_json_with_headers` to control the header.
    fn requires_idempotency_key(uri: &str) -> bool {
        let path = uri.split('?').next().unwrap_or(uri);
        if path.contains("/operations/") {
            // /operations/{id}/corrections is idempotent; /post and /cancel are not.
            return path.ends_with("/corrections");
        }
        path.ends_with("/operations")
    }

    async fn post_json(
        pool: &PgPool,
        id_user: Uuid,
        handle: &str,
        uri: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let mut headers: Vec<(&'static str, String)> = Vec::new();
        if requires_idempotency_key(uri) {
            headers.push(("idempotency-key", Uuid::new_v4().to_string()));
        }
        post_json_with_headers(pool, id_user, handle, uri, body, &headers).await
    }

    async fn post_json_with_headers(
        pool: &PgPool,
        id_user: Uuid,
        handle: &str,
        uri: &str,
        body: Value,
        extra_headers: &[(&'static str, String)],
    ) -> (StatusCode, Value) {
        let (status, _, value) =
            post_json_full(pool, id_user, handle, uri, body, extra_headers).await;
        (status, value)
    }

    async fn post_json_full(
        pool: &PgPool,
        id_user: Uuid,
        handle: &str,
        uri: &str,
        body: Value,
        extra_headers: &[(&'static str, String)],
    ) -> (StatusCode, axum::http::HeaderMap, Value) {
        let app = crate::http::router(test_state(pool.clone()).await);
        let mut builder = Request::builder()
            .method("POST")
            .uri(uri)
            .header(
                AUTHORIZATION,
                format!("Bearer {}", build_access_token(id_user, handle)),
            )
            // Distinct from inline test content-type lines so the bulk
            // idempotency-key injection regex below does not touch this
            // shared helper.
            .header(axum::http::header::CONTENT_TYPE, "application/json");
        for (name, value) in extra_headers {
            builder = builder.header(*name, value.clone());
        }
        let response = app
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .expect("response should be built");
        let status = response.status();
        let headers = response.headers().clone();
        (status, headers, response_json(response).await)
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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
                    .header("idempotency-key", Uuid::new_v4().to_string())
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

    // ---------------------------------------------------------------------
    // P2 — Durable asset identity in operation responses
    // ---------------------------------------------------------------------

    /// Seeds an active equity with both a canonical `ticker` and a populated
    /// `symbol`. The existing `create_asset` helper only sets `symbol`, but the
    /// P2 identity contract exposes `ticker`, so P2 tests need a fixture that
    /// matches the production shape.
    async fn create_asset_with_ticker(pool: &PgPool, ticker: &str) -> Uuid {
        let id_asset = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO assets (id_asset, asset_class, status, name, native_currency, symbol, ticker)
            VALUES ($1, 'equity', 'active', $2, 'EUR', $3, $3)
            "#,
        )
        .bind(id_asset)
        .bind(format!("Asset {ticker}"))
        .bind(ticker)
        .execute(pool)
        .await
        .expect("ticker asset should be inserted");
        id_asset
    }

    async fn list_operations(
        pool: &PgPool,
        id_user: Uuid,
        handle: &str,
        id_portfolio: Uuid,
    ) -> (StatusCode, Value) {
        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let status = response.status();
        (status, response_json(response).await)
    }

    #[tokio::test]
    async fn p2_cash_operation_response_has_null_asset_refs() {
        let pool = test_pool().await;
        let handle = format!("p2c{}", &Uuid::new_v4().simple().to_string()[..12]);
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

        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::CREATED);
        assert!(body["operation"]["asset"].is_null());
        assert!(body["operation"]["related_asset"].is_null());
        assert!(body["operation"]["id_asset"].is_null());
    }

    #[tokio::test]
    async fn p2_asset_linked_operation_response_carries_identity() {
        let pool = test_pool().await;
        let handle = format!("p2a{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset_with_ticker(&pool, "AAPL").await;

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            buy_payload(id_asset),
        )
        .await;

        cleanup_user_tree(&pool, id_user, &[id_asset]).await;

        assert_eq!(status, StatusCode::CREATED);
        let asset = &body["operation"]["asset"];
        assert_eq!(asset["id_asset"], id_asset.to_string());
        assert_eq!(asset["ticker"], "AAPL");
        assert_eq!(asset["name"], "Asset AAPL");
        assert_eq!(asset["status"], "active");
        assert!(body["operation"]["related_asset"].is_null());
    }

    #[tokio::test]
    async fn p2_list_response_enriches_every_operation_with_identity() {
        let pool = test_pool().await;
        let handle = format!("p2l{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_aapl = create_asset_with_ticker(&pool, "AAPL").await;
        let id_msft = create_asset_with_ticker(&pool, "MSFT").await;

        insert_operation(&pool, id_portfolio, "pending", deposit_payload()).await;
        insert_operation(&pool, id_portfolio, "pending", buy_payload(id_aapl)).await;
        // Repeated reference to the same asset must still resolve consistently.
        insert_operation(&pool, id_portfolio, "pending", buy_payload(id_aapl)).await;
        insert_operation(&pool, id_portfolio, "pending", buy_payload(id_msft)).await;

        let (status, body) = list_operations(&pool, id_user, &handle, id_portfolio).await;

        cleanup_user_tree(&pool, id_user, &[id_aapl, id_msft]).await;

        assert_eq!(status, StatusCode::OK);
        let ops = body["operations"]
            .as_array()
            .expect("operations should be an array");
        assert_eq!(ops.len(), 4);

        let mut deposit_seen = false;
        let mut aapl_count = 0;
        let mut msft_seen = false;
        for op in ops {
            match op["operation_type"].as_str() {
                Some("deposit") => {
                    deposit_seen = true;
                    assert!(op["asset"].is_null());
                }
                Some("buy") => {
                    let id = op["id_asset"].as_str().unwrap();
                    if id == id_aapl.to_string() {
                        assert_eq!(op["asset"]["ticker"], "AAPL");
                        aapl_count += 1;
                    } else if id == id_msft.to_string() {
                        assert_eq!(op["asset"]["ticker"], "MSFT");
                        msft_seen = true;
                    }
                }
                _ => {}
            }
        }
        assert!(deposit_seen);
        assert_eq!(aapl_count, 2);
        assert!(msft_seen);
    }

    #[tokio::test]
    async fn p2_inactive_asset_referenced_by_existing_operation_stays_displayable() {
        let pool = test_pool().await;
        let handle = format!("p2i{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset_with_ticker(&pool, "OLDX").await;

        // Insert a historical operation pointing to the asset, then mark the
        // asset inactive. The response must still expose the compact identity.
        insert_operation(&pool, id_portfolio, "pending", buy_payload(id_asset)).await;
        set_asset_status(&pool, id_asset, "inactive").await;

        let (status, body) = list_operations(&pool, id_user, &handle, id_portfolio).await;

        cleanup_user_tree(&pool, id_user, &[id_asset]).await;

        assert_eq!(status, StatusCode::OK);
        let asset = &body["operations"][0]["asset"];
        assert_eq!(asset["id_asset"], id_asset.to_string());
        assert_eq!(asset["ticker"], "OLDX");
        assert_eq!(asset["status"], "inactive");
    }

    #[tokio::test]
    async fn p2_cross_user_operation_access_returns_404() {
        let pool = test_pool().await;
        let owner_handle = format!("p2o{}", &Uuid::new_v4().simple().to_string()[..12]);
        let intruder_handle = format!("p2x{}", &Uuid::new_v4().simple().to_string()[..12]);
        let owner = create_user(&pool, &owner_handle).await;
        let intruder = create_user(&pool, &intruder_handle).await;
        let id_portfolio = create_portfolio(&pool, owner, None).await;

        let (status, _) = list_operations(&pool, intruder, &intruder_handle, id_portfolio).await;

        cleanup_user_tree(&pool, owner, &[]).await;
        cleanup_user_tree(&pool, intruder, &[]).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn p2_list_endpoint_with_refresh_token_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("p2r{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations"))
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

    /// Source-level guard: the operation service must batch asset-identity
    /// lookups via `list_identities_by_ids` rather than fall back to per-row
    /// `find_by_id` calls inside the enrichment path. Static regression
    /// against re-introducing a backend N+1.
    #[test]
    fn p2_service_uses_batch_identity_lookup_not_per_row_find() {
        let service_source = include_str!("../services/portfolio_operations.rs");
        assert!(
            service_source.contains("list_identities_by_ids"),
            "service must use the batch identity lookup",
        );
        // Extract the body of `enrich_many` (used by all read paths).
        let after_enrich_many = service_source
            .split("pub(crate) async fn enrich_many")
            .nth(1)
            .expect("enrich_many should exist");
        // Stop at the next service-level helper definition.
        let enrich_body = after_enrich_many
            .split("pub(crate) async fn prefetch_identities")
            .next()
            .unwrap_or(after_enrich_many);
        assert!(
            !enrich_body.contains("find_by_id"),
            "enrichment must not call asset_repository.find_by_id per operation",
        );
        // The read path must reach the repository through `prefetch_identities`
        // — i.e. it must NOT call `list_identities_by_ids` more than once and
        // must NOT call it directly without going through the dedup helper.
        assert!(
            !enrich_body.contains("list_identities_by_ids"),
            "enrich_many must delegate to prefetch_identities (one dedup point), \
             not call list_identities_by_ids directly",
        );
        // And `prefetch_identities` itself must invoke the repository exactly
        // once — guarantees one batch lookup per call site, not per id.
        let prefetch_body = service_source
            .split("pub(crate) async fn prefetch_identities")
            .nth(1)
            .expect("prefetch_identities should exist")
            .split("\n    }\n")
            .next()
            .unwrap_or("");
        let calls = prefetch_body.matches("list_identities_by_ids").count();
        assert_eq!(
            calls, 1,
            "prefetch_identities must call list_identities_by_ids exactly once \
             (got {calls})",
        );
    }

    /// Atomicity guard: every write path must call `prefetch_identities`
    /// BEFORE its mutating repository call so a failing identity SELECT
    /// cannot turn a committed mutation into an HTTP 500. The check is
    /// structural — we scan each `pub async fn <write>()` body and require
    /// the prefetch token to appear before the mutation token.
    #[test]
    fn p2_writes_prefetch_identities_before_mutation() {
        let service_source = include_str!("../services/portfolio_operations.rs");

        // Signatures include the opening paren to disambiguate the legacy
        // write methods from the P3 `*_idempotent` variants which add a
        // replay path (enrich_many is legitimately called on replay, not on
        // a write — so the invariant only applies to the legacy methods).
        let cases: &[(&str, &str)] = &[
            (
                "pub async fn create_operation(",
                "create_with_optional_refresh",
            ),
            ("pub async fn update_operation(", ".update("),
            ("pub async fn cancel_operation(", ".set_status("),
            (
                "pub async fn post_operation(",
                "set_status_posted_with_refresh",
            ),
            (
                "pub async fn create_correction(",
                "create_with_optional_refresh",
            ),
        ];

        for (signature, mutation) in cases {
            let body = service_source
                .split(signature)
                .nth(1)
                .unwrap_or_else(|| panic!("{signature} must exist"));
            // Bound the search at the next pub async fn marker so we look at
            // this method only.
            let body = body.split("\n    pub async fn ").next().unwrap_or(body);
            let prefetch_pos = body
                .find("prefetch_identities")
                .unwrap_or_else(|| panic!("{signature} must call prefetch_identities"));
            let mutation_pos = body
                .find(mutation)
                .unwrap_or_else(|| panic!("{signature} must perform mutation {mutation}"));
            assert!(
                prefetch_pos < mutation_pos,
                "{signature}: prefetch_identities must appear BEFORE {mutation} \
                 to preserve write-response atomicity",
            );
            // No post-commit fallible asset lookup is allowed inside the
            // write body — only the in-memory `build_view` helper.
            let tail = &body[mutation_pos..];
            assert!(
                !tail.contains("list_identities_by_ids"),
                "{signature}: must not perform an asset SELECT after the mutation",
            );
            assert!(
                !tail.contains("enrich_many"),
                "{signature}: must not call enrich_many (read-path helper) after the mutation",
            );
        }
    }

    // ---------- Endpoint-level additive contract assertions ----------

    /// Helper: GET an operation by id and return (status, json).
    async fn get_operation(
        pool: &PgPool,
        id_user: Uuid,
        handle: &str,
        id_portfolio: Uuid,
        id_operation: Uuid,
    ) -> (StatusCode, Value) {
        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_operation}"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let status = response.status();
        (status, response_json(response).await)
    }

    fn assert_identity_matches(node: &Value, id: Uuid, ticker: &str) {
        assert_eq!(node["id_asset"], id.to_string(), "id_asset mismatch");
        assert_eq!(node["ticker"], ticker, "ticker mismatch");
        assert_eq!(node["name"], format!("Asset {ticker}"), "name mismatch");
        assert_eq!(node["status"], "active", "status mismatch");
    }

    #[tokio::test]
    async fn p2_get_endpoint_carries_asset_identity() {
        let pool = test_pool().await;
        let handle = format!("p2g{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset_with_ticker(&pool, "GETX").await;
        let id_op = insert_operation(&pool, id_portfolio, "pending", buy_payload(id_asset)).await;

        let (status, body) = get_operation(&pool, id_user, &handle, id_portfolio, id_op).await;
        cleanup_user_tree(&pool, id_user, &[id_asset]).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operation"]["id_asset"], id_asset.to_string());
        assert_identity_matches(&body["operation"]["asset"], id_asset, "GETX");
        assert!(body["operation"]["related_asset"].is_null());
    }

    #[tokio::test]
    async fn p2_update_response_carries_asset_identity_after_changing_id_asset() {
        let pool = test_pool().await;
        let handle = format!("p2u{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_old = create_asset_with_ticker(&pool, "UPDA").await;
        let id_new = create_asset_with_ticker(&pool, "UPDB").await;
        let id_op = insert_operation(&pool, id_portfolio, "pending", buy_payload(id_old)).await;

        let app = crate::http::router(test_state(pool.clone()).await);
        let payload = json!({ "id_asset": id_new });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/portfolios/{id_portfolio}/operations/{id_op}"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .header("idempotency-key", Uuid::new_v4().to_string())
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let status = response.status();
        let body = response_json(response).await;
        cleanup_user_tree(&pool, id_user, &[id_old, id_new]).await;

        assert_eq!(status, StatusCode::OK);
        // PATCH update merges id_asset with existing — both old and new are
        // valid candidates the service prefetched. Whichever the merge applied
        // must match the embedded asset identity.
        let returned_id = body["operation"]["id_asset"]
            .as_str()
            .expect("id_asset should be a string");
        let asset = &body["operation"]["asset"];
        assert_eq!(asset["id_asset"], returned_id);
        assert!(
            asset["ticker"] == "UPDA" || asset["ticker"] == "UPDB",
            "ticker must match the merged id_asset, got {}",
            asset["ticker"],
        );
    }

    #[tokio::test]
    async fn p2_cancel_response_preserves_asset_identity() {
        let pool = test_pool().await;
        let handle = format!("p2x{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset_with_ticker(&pool, "CXLA").await;
        let id_op = insert_operation(&pool, id_portfolio, "pending", buy_payload(id_asset)).await;

        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_op}/cancel"
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

        assert_eq!(body["operation"]["operation_status"], "cancelled");
        assert_identity_matches(&body["operation"]["asset"], id_asset, "CXLA");
    }

    #[tokio::test]
    async fn p2_post_pending_response_carries_asset_identity() {
        let pool = test_pool().await;
        let handle = format!("p2p{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await; // EUR
        let id_asset = create_asset_with_ticker(&pool, "PSTA").await;
        let id_op = insert_operation(&pool, id_portfolio, "pending", buy_payload(id_asset)).await;

        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations/{id_op}/post"),
            json!({}),
        )
        .await;

        cleanup_refresh_requests(&pool, id_portfolio).await;
        // Posted rows are immutable so we can't delete operations rows; only
        // clean the asset row that is still safe to delete (no posted row
        // links to it after refresh requests are cleared — but the operation
        // row does, so leave the asset alone too). The user-tree cleanup will
        // skip the posted operation rows, which is the same pattern as the
        // existing posted tests.
        sqlx::query("DELETE FROM portfolio_refresh_requests WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .ok();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operation"]["operation_status"], "posted");
        assert_identity_matches(&body["operation"]["asset"], id_asset, "PSTA");
    }

    #[tokio::test]
    async fn p2_correction_response_carries_asset_identity() {
        let pool = test_pool().await;
        let handle = format!("p2k{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_original = insert_operation(&pool, id_portfolio, "posted", deposit_payload()).await;
        let id_asset = create_asset_with_ticker(&pool, "CRRA").await;

        let payload = json!({
            "executed_at": "2026-06-06T10:00:00Z",
            "id_asset": id_asset,
            "quantity": "1.0000000000",
            "cash_amount_minor": 5000,
            "currency": "EUR",
            "metadata": { "reason": "asset_adjustment" }
        });
        let (status, body) = post_json(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations/{id_original}/corrections"),
            payload,
        )
        .await;

        // Best-effort cleanup; original posted row is immutable.
        sqlx::query("DELETE FROM portfolio_refresh_requests WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .ok();

        assert_eq!(status, StatusCode::CREATED);
        assert_identity_matches(&body["operation"]["asset"], id_asset, "CRRA");
    }

    #[tokio::test]
    async fn p2_corrections_list_endpoint_enriches_every_entry() {
        let pool = test_pool().await;
        let handle = format!("p2y{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset_with_ticker(&pool, "LCRR").await;

        let id_original =
            insert_operation(&pool, id_portfolio, "posted", buy_payload(id_asset)).await;
        // Two adjustment rows correcting the original.
        let mut adj = empty_adjustment_payload();
        adj["id_asset"] = json!(id_asset);
        adj["quantity"] = json!("1.0000000000");
        adj["cash_amount_minor"] = json!(1000);
        adj["id_corrected_operation"] = json!(id_original);
        insert_operation(&pool, id_portfolio, "pending", adj.clone()).await;
        insert_operation(&pool, id_portfolio, "pending", adj).await;

        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
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
        let status = response.status();
        let body = response_json(response).await;

        // Posted original is immutable — best-effort cleanup like other posted tests.
        sqlx::query("DELETE FROM portfolio_refresh_requests WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .ok();

        assert_eq!(status, StatusCode::OK);
        assert_identity_matches(&body["operation"]["asset"], id_asset, "LCRR");
        let corrections = body["corrections"]
            .as_array()
            .expect("corrections must be an array");
        assert_eq!(corrections.len(), 2);
        for c in corrections {
            assert_identity_matches(&c["asset"], id_asset, "LCRR");
        }
    }

    #[tokio::test]
    async fn p2_audit_timeline_enriches_primaries_and_corrections() {
        let pool = test_pool().await;
        let handle = format!("p2t{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset_with_ticker(&pool, "TMLA").await;

        let id_primary =
            insert_operation(&pool, id_portfolio, "posted", buy_payload(id_asset)).await;
        let mut adj = empty_adjustment_payload();
        adj["id_asset"] = json!(id_asset);
        adj["quantity"] = json!("1.0000000000");
        adj["cash_amount_minor"] = json!(2000);
        adj["id_corrected_operation"] = json!(id_primary);
        insert_operation(&pool, id_portfolio, "pending", adj).await;

        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
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
        let status = response.status();
        let body = response_json(response).await;

        sqlx::query("DELETE FROM portfolio_refresh_requests WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .ok();

        assert_eq!(status, StatusCode::OK);
        let items = body["items"].as_array().expect("items must be array");
        assert!(!items.is_empty());
        let first = &items[0];
        assert_identity_matches(&first["operation"]["asset"], id_asset, "TMLA");
        let corrections = first["corrections"].as_array().expect("corrections array");
        assert_eq!(corrections.len(), 1);
        assert_identity_matches(&corrections[0]["asset"], id_asset, "TMLA");
    }

    #[tokio::test]
    async fn p2_audit_endpoint_enriches_operation_and_corrections() {
        let pool = test_pool().await;
        let handle = format!("p2d{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset_with_ticker(&pool, "AUDA").await;

        let id_primary =
            insert_operation(&pool, id_portfolio, "posted", buy_payload(id_asset)).await;
        let mut adj = empty_adjustment_payload();
        adj["id_asset"] = json!(id_asset);
        adj["quantity"] = json!("1.0000000000");
        adj["cash_amount_minor"] = json!(1500);
        adj["id_corrected_operation"] = json!(id_primary);
        insert_operation(&pool, id_portfolio, "pending", adj).await;

        let app = crate::http::router(test_state(pool.clone()).await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/portfolios/{id_portfolio}/operations/{id_primary}/audit"
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
        sqlx::query("DELETE FROM portfolio_refresh_requests WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .ok();

        assert_eq!(status, StatusCode::OK);
        assert_identity_matches(&body["operation"]["asset"], id_asset, "AUDA");
        let corrections = body["corrections"].as_array().expect("corrections array");
        assert!(!corrections.is_empty());
        for c in corrections {
            assert_identity_matches(&c["asset"], id_asset, "AUDA");
        }
    }

    #[tokio::test]
    async fn p2_related_asset_is_enriched_on_an_operation_carrying_one() {
        // A spin_off operation carries both id_asset and id_related_asset.
        // The list endpoint must enrich both compact references.
        let pool = test_pool().await;
        let handle = format!("p2s{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_parent = create_asset_with_ticker(&pool, "SPNA").await;
        let id_child = create_asset_with_ticker(&pool, "SPNB").await;

        let payload = json!({
            "operation_type": "spin_off",
            "executed_at": "2026-06-05T10:00:00Z",
            "id_asset": id_parent,
            "id_related_asset": id_child,
            "quantity": "1.0000000000",
            "related_quantity": "0.5000000000",
            "currency": "EUR",
            "metadata": {}
        });
        insert_operation(&pool, id_portfolio, "pending", payload).await;

        let (status, body) = list_operations(&pool, id_user, &handle, id_portfolio).await;
        cleanup_user_tree(&pool, id_user, &[id_parent, id_child]).await;

        assert_eq!(status, StatusCode::OK);
        let op = &body["operations"][0];
        assert_identity_matches(&op["asset"], id_parent, "SPNA");
        assert_identity_matches(&op["related_asset"], id_child, "SPNB");
    }

    // ====================================================================
    // P3: Durable operation idempotency tests
    // ====================================================================

    /// Multi-connection pool needed by concurrent-race scenarios. The
    /// `test_pool` helper above uses `max_connections=1` for the legacy
    /// tests, which would serialize every query and turn race tests into
    /// sequential ones.
    async fn test_pool_concurrent() -> PgPool {
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
        PgPoolOptions::new()
            .max_connections(8)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    /// Delete every idempotency record for a portfolio so reruns are clean.
    async fn cleanup_idempotency(pool: &PgPool, id_portfolio: Uuid) {
        sqlx::query("DELETE FROM portfolio_operation_idempotency WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .execute(pool)
            .await
            .expect("idempotency rows should be deleted");
    }

    async fn count_operations(pool: &PgPool, id_portfolio: Uuid) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .fetch_one(pool)
            .await
            .expect("count should succeed")
    }

    async fn count_idempotency(pool: &PgPool, id_portfolio: Uuid) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operation_idempotency WHERE id_portfolio = $1",
        )
        .bind(id_portfolio)
        .fetch_one(pool)
        .await
        .expect("count should succeed")
    }

    fn header_value(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
        headers.get(name).map(|v| v.to_str().unwrap().to_string())
    }

    #[tokio::test]
    async fn missing_idempotency_key_is_rejected_with_400() {
        let pool = test_pool().await;
        let handle = format!("p3a{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let (status, body) = post_json_with_headers(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            posted_deposit_payload(),
            &[],
        )
        .await;

        let op_count = count_operations(&pool, id_portfolio).await;
        let refresh_count = count_all_refresh_requests(&pool, id_portfolio).await;
        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "missing_idempotency_key");
        assert_eq!(op_count, 0);
        assert_eq!(refresh_count, 0);
    }

    #[tokio::test]
    async fn malformed_idempotency_key_is_rejected_with_400() {
        let pool = test_pool().await;
        let handle = format!("p3b{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let (status, body) = post_json_with_headers(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            posted_deposit_payload(),
            &[("idempotency-key", "not-a-uuid".to_string())],
        )
        .await;

        let op_count = count_operations(&pool, id_portfolio).await;
        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_idempotency_key");
        assert_eq!(op_count, 0);
    }

    #[tokio::test]
    async fn first_posted_create_returns_201_and_replayed_false() {
        let pool = test_pool().await;
        let handle = format!("p3c{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();

        let (status, headers, body) = post_json_full(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            posted_deposit_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let pending = count_pending_refresh_requests(&pool, id_portfolio).await;
        let idemp = count_idempotency(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            header_value(&headers, "idempotency-replayed").as_deref(),
            Some("false")
        );
        assert_eq!(body["operation"]["operation_status"], "posted");
        assert!(body["refresh_request"]["id_portfolio_refresh_request"].is_string());
        assert_eq!(pending, 1);
        assert_eq!(idemp, 1);
    }

    #[tokio::test]
    async fn exact_replay_returns_same_ids_no_new_rows_and_200() {
        let pool = test_pool().await;
        let handle = format!("p3d{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();
        let uri = format!("/v1/portfolios/{id_portfolio}/operations");

        let (status1, headers1, body1) = post_json_full(
            &pool,
            id_user,
            &handle,
            &uri,
            posted_deposit_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let (status2, headers2, body2) = post_json_full(
            &pool,
            id_user,
            &handle,
            &uri,
            posted_deposit_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let op_count = count_operations(&pool, id_portfolio).await;
        let refresh_total = count_all_refresh_requests(&pool, id_portfolio).await;
        let idemp_count = count_idempotency(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        assert_eq!(status1, StatusCode::CREATED);
        assert_eq!(status2, StatusCode::OK);
        assert_eq!(
            header_value(&headers1, "idempotency-replayed").as_deref(),
            Some("false")
        );
        assert_eq!(
            header_value(&headers2, "idempotency-replayed").as_deref(),
            Some("true")
        );
        assert_eq!(
            body1["operation"]["id_portfolio_operation"],
            body2["operation"]["id_portfolio_operation"],
            "operation id must be stable across replay"
        );
        assert_eq!(
            body1["refresh_request"]["id_portfolio_refresh_request"],
            body2["refresh_request"]["id_portfolio_refresh_request"],
            "refresh-request id must be stable across replay"
        );
        assert_eq!(op_count, 1, "no new operation on replay");
        assert_eq!(refresh_total, 1, "no new refresh-request on replay");
        assert_eq!(idemp_count, 1);
    }

    #[tokio::test]
    async fn same_key_different_amount_returns_409_conflict() {
        let pool = test_pool().await;
        let handle = format!("p3e{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();
        let uri = format!("/v1/portfolios/{id_portfolio}/operations");

        let _ = post_json_with_headers(
            &pool,
            id_user,
            &handle,
            &uri,
            posted_deposit_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;

        // Same key, mutate the amount.
        let mut other = posted_deposit_payload();
        other["gross_amount_minor"] = json!(200000);
        other["cash_amount_minor"] = json!(200000);
        let (status, body) = post_json_with_headers(
            &pool,
            id_user,
            &handle,
            &uri,
            other,
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let op_count = count_operations(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "idempotency_key_conflict");
        assert_eq!(op_count, 1, "no extra operation must be inserted");
    }

    #[tokio::test]
    async fn same_uuid_isolated_per_user() {
        let pool = test_pool().await;
        let handle_a = format!("p3f{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("p3g{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user_a = create_user(&pool, &handle_a).await;
        let id_user_b = create_user(&pool, &handle_b).await;
        let id_portfolio_a = create_portfolio(&pool, id_user_a, None).await;
        let id_portfolio_b = create_portfolio(&pool, id_user_b, None).await;
        let key = Uuid::new_v4();

        let (status_a, _headers_a, _body_a) = post_json_full(
            &pool,
            id_user_a,
            &handle_a,
            &format!("/v1/portfolios/{id_portfolio_a}/operations"),
            posted_deposit_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;
        let (status_b, headers_b, _body_b) = post_json_full(
            &pool,
            id_user_b,
            &handle_b,
            &format!("/v1/portfolios/{id_portfolio_b}/operations"),
            posted_deposit_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;

        cleanup_refresh_requests(&pool, id_portfolio_a).await;
        cleanup_refresh_requests(&pool, id_portfolio_b).await;
        cleanup_idempotency(&pool, id_portfolio_a).await;
        cleanup_idempotency(&pool, id_portfolio_b).await;

        assert_eq!(status_a, StatusCode::CREATED);
        assert_eq!(
            status_b,
            StatusCode::CREATED,
            "second user's key must not collide"
        );
        assert_eq!(
            header_value(&headers_b, "idempotency-replayed").as_deref(),
            Some("false")
        );
    }

    #[tokio::test]
    async fn pre_write_validation_failure_does_not_consume_key() {
        let pool = test_pool().await;
        let handle = format!("p3h{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await; // EUR
        let key = Uuid::new_v4();
        let uri = format!("/v1/portfolios/{id_portfolio}/operations");

        let mut bad = posted_deposit_payload();
        bad["currency"] = json!("USD"); // cross-currency without FX → 422

        let (status_bad, body_bad) = post_json_with_headers(
            &pool,
            id_user,
            &handle,
            &uri,
            bad,
            &[("idempotency-key", key.to_string())],
        )
        .await;

        // Same key now used with a valid payload — must succeed.
        let (status_ok, _, _body_ok) = post_json_full(
            &pool,
            id_user,
            &handle,
            &uri,
            posted_deposit_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let idemp = count_idempotency(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        assert_eq!(status_bad, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body_bad["error"]["code"], "unsupported_cross_currency");
        assert_eq!(status_ok, StatusCode::CREATED);
        assert_eq!(idemp, 1, "exactly one idempotency record after success");
    }

    #[tokio::test]
    async fn refresh_token_authentication_is_rejected() {
        let pool = test_pool().await;
        let handle = format!("p3i{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();

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
                    .header("idempotency-key", key.to_string())
                    .body(Body::from(posted_deposit_payload().to_string()))
                    .unwrap(),
            )
            .await
            .expect("response should build");

        let status = response.status();
        cleanup_user_tree(&pool, id_user, &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn correction_replay_returns_same_adjustment_and_no_new_row() {
        let pool = test_pool().await;
        let handle = format!("p3j{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        // Seed a posted operation to correct (insert directly, bypass idempotency).
        let original =
            insert_operation(&pool, id_portfolio, "posted", posted_deposit_payload()).await;

        let key = Uuid::new_v4();
        let uri = format!("/v1/portfolios/{id_portfolio}/operations/{original}/corrections");

        let (status1, headers1, body1) = post_json_full(
            &pool,
            id_user,
            &handle,
            &uri,
            correction_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;
        let (status2, headers2, body2) = post_json_full(
            &pool,
            id_user,
            &handle,
            &uri,
            correction_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let adjustment_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation = $2",
        )
        .bind(id_portfolio)
        .bind(original)
        .fetch_one(&pool)
        .await
        .expect("count");

        cleanup_idempotency(&pool, id_portfolio).await;
        sqlx::query("DELETE FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation = $2")
            .bind(id_portfolio)
            .bind(original)
            .execute(&pool)
            .await
            .expect("delete corrections");

        assert_eq!(status1, StatusCode::CREATED);
        assert_eq!(status2, StatusCode::OK);
        assert_eq!(
            header_value(&headers1, "idempotency-replayed").as_deref(),
            Some("false")
        );
        assert_eq!(
            header_value(&headers2, "idempotency-replayed").as_deref(),
            Some("true")
        );
        assert_eq!(
            body1["operation"]["id_portfolio_operation"],
            body2["operation"]["id_portfolio_operation"]
        );
        assert_eq!(adjustment_count, 1, "no second adjustment must be created");
    }

    #[tokio::test]
    async fn concurrent_identical_creates_produce_exactly_one_operation() {
        // Uses a multi-connection pool so two transactions truly run in
        // parallel and race on the (id_user, idempotency_key) unique index.
        let pool = test_pool_concurrent().await;
        let handle = format!("p3k{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();
        let uri = format!("/v1/portfolios/{id_portfolio}/operations");

        let app = crate::http::router(test_state(pool.clone()).await);

        let build_req = || {
            Request::builder()
                .method("POST")
                .uri(&uri)
                .header(
                    AUTHORIZATION,
                    format!("Bearer {}", build_access_token(id_user, &handle)),
                )
                .header("content-type", "application/json")
                .header("idempotency-key", key.to_string())
                .body(Body::from(posted_deposit_payload().to_string()))
                .unwrap()
        };

        let app_a = app.clone();
        let app_b = app.clone();
        let (resp_a, resp_b) =
            tokio::join!(app_a.oneshot(build_req()), app_b.oneshot(build_req()),);

        let resp_a = resp_a.expect("response a");
        let resp_b = resp_b.expect("response b");
        assert!(resp_a.status().is_success());
        assert!(resp_b.status().is_success());

        let body_a: Value = serde_json::from_slice(
            &body::to_bytes(resp_a.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let body_b: Value = serde_json::from_slice(
            &body::to_bytes(resp_b.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();

        let op_count = count_operations(&pool, id_portfolio).await;
        let refresh_total = count_all_refresh_requests(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        assert_eq!(
            body_a["operation"]["id_portfolio_operation"],
            body_b["operation"]["id_portfolio_operation"],
            "both concurrent winners must point at the same operation"
        );
        assert_eq!(op_count, 1, "exactly one operation row must persist");
        assert_eq!(refresh_total, 1, "exactly one refresh request must persist");
    }

    #[tokio::test]
    async fn concurrent_different_payloads_one_succeeds_one_conflicts() {
        let pool = test_pool_concurrent().await;
        let handle = format!("p3l{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();
        let uri = format!("/v1/portfolios/{id_portfolio}/operations");

        let app = crate::http::router(test_state(pool.clone()).await);
        let mut other_payload = posted_deposit_payload();
        other_payload["gross_amount_minor"] = json!(777777);
        other_payload["cash_amount_minor"] = json!(777777);

        let build_req = |payload: Value| {
            Request::builder()
                .method("POST")
                .uri(&uri)
                .header(
                    AUTHORIZATION,
                    format!("Bearer {}", build_access_token(id_user, &handle)),
                )
                .header("content-type", "application/json")
                .header("idempotency-key", key.to_string())
                .body(Body::from(payload.to_string()))
                .unwrap()
        };

        let app_a = app.clone();
        let app_b = app.clone();
        let (resp_a, resp_b) = tokio::join!(
            app_a.oneshot(build_req(posted_deposit_payload())),
            app_b.oneshot(build_req(other_payload)),
        );

        let status_a = resp_a.unwrap().status();
        let status_b = resp_b.unwrap().status();

        let op_count = count_operations(&pool, id_portfolio).await;
        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        // Exactly one success, one conflict (which one wins depends on
        // PostgreSQL's lock arbitration — we assert the outcome shape).
        let success_count = [status_a, status_b]
            .iter()
            .filter(|s| s.is_success())
            .count();
        let conflict_count = [status_a, status_b]
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count();
        assert_eq!(success_count, 1, "exactly one create must succeed");
        assert_eq!(conflict_count, 1, "the loser must see 409");
        assert_eq!(op_count, 1, "exactly one operation row");
    }

    #[tokio::test]
    async fn external_provider_remains_independent_from_idempotency() {
        // Two requests using the same external_provider/reference still
        // collide on the unique external-provider index, regardless of
        // Idempotency-Key. P3 must NOT replace external-provider dedup.
        let pool = test_pool().await;
        let handle = format!("p3m{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let uri = format!("/v1/portfolios/{id_portfolio}/operations");

        // Unique external reference per run so prior tests' rows don't
        // collide on the unique external-provider index (posted operations
        // are immutable and therefore not cleaned up between runs).
        let unique_ref = format!("ref-{}", Uuid::new_v4().simple());
        let mut payload = posted_deposit_payload();
        payload["external_provider"] = json!("acme-broker");
        payload["external_reference"] = json!(unique_ref);

        let (status1, _) = post_json_with_headers(
            &pool,
            id_user,
            &handle,
            &uri,
            payload.clone(),
            &[("idempotency-key", Uuid::new_v4().to_string())],
        )
        .await;
        // Different idempotency key, SAME external provider/reference.
        let (status2, _) = post_json_with_headers(
            &pool,
            id_user,
            &handle,
            &uri,
            payload,
            &[("idempotency-key", Uuid::new_v4().to_string())],
        )
        .await;

        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        assert_eq!(status1, StatusCode::CREATED);
        assert!(
            !status2.is_success(),
            "second external-provider conflict must NOT succeed, got {status2}"
        );
    }

    // ====================================================================
    // P3 hardening — replay ordering, cross-portfolio, kind conflicts,
    // pending/correction concurrency, rollback, external-ref regression,
    // CORS preflight.
    // ====================================================================

    /// Helper: drive the full create path with a fixed key and return the
    /// raw status + response body.
    async fn create_with_key(
        pool: &PgPool,
        id_user: Uuid,
        handle: &str,
        id_portfolio: Uuid,
        key: Uuid,
        body: Value,
    ) -> (StatusCode, axum::http::HeaderMap, Value) {
        post_json_full(
            pool,
            id_user,
            handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            body,
            &[("idempotency-key", key.to_string())],
        )
        .await
    }

    #[tokio::test]
    async fn replay_after_primary_asset_becomes_inactive() {
        // P3 contract: an exact replay must NOT re-run asset-activity
        // validation. The original write committed when the asset was
        // active; a subsequent identical retry must still return the same
        // operation id, even if the asset has since been marked inactive.
        let pool = test_pool().await;
        let handle = format!("p3ria{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset(
            &pool,
            &format!("RIA{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;
        let key = Uuid::new_v4();

        let mut buy = buy_payload(id_asset);
        buy["operation_status"] = json!("posted");

        let (status1, _, body1) =
            create_with_key(&pool, id_user, &handle, id_portfolio, key, buy.clone()).await;
        assert_eq!(status1, StatusCode::CREATED);

        // Drift: mark the asset inactive. A non-idempotent fresh request
        // would now hit `inactive_asset_reference`. The replay must not.
        set_asset_status(&pool, id_asset, "inactive").await;

        let (status2, headers2, body2) =
            create_with_key(&pool, id_user, &handle, id_portfolio, key, buy).await;

        let op_count = count_operations(&pool, id_portfolio).await;
        // Posted operations cannot be deleted (DB trigger). The other posted
        // tests in this file follow the same pattern — clean up only the
        // deletable sub-rows and leave the immutable operation in place.
        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        assert_eq!(status2, StatusCode::OK);
        assert_eq!(
            header_value(&headers2, "idempotency-replayed").as_deref(),
            Some("true")
        );
        assert_eq!(
            body1["operation"]["id_portfolio_operation"],
            body2["operation"]["id_portfolio_operation"],
            "replay must return the same operation id even if the asset is inactive now"
        );
        assert_eq!(op_count, 1);
    }

    #[tokio::test]
    async fn correction_replay_after_referenced_asset_becomes_inactive() {
        let pool = test_pool().await;
        let handle = format!("p3rci{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let id_asset = create_asset(
            &pool,
            &format!("RCI{}", &Uuid::new_v4().simple().to_string()[..8]),
        )
        .await;

        // Seed a posted original.
        let original =
            insert_operation(&pool, id_portfolio, "posted", posted_deposit_payload()).await;

        let key = Uuid::new_v4();
        let uri = format!("/v1/portfolios/{id_portfolio}/operations/{original}/corrections");
        let mut payload = correction_payload();
        payload["id_asset"] = json!(id_asset);

        let (status1, _, body1) = post_json_full(
            &pool,
            id_user,
            &handle,
            &uri,
            payload.clone(),
            &[("idempotency-key", key.to_string())],
        )
        .await;
        assert_eq!(status1, StatusCode::CREATED);

        set_asset_status(&pool, id_asset, "inactive").await;

        let (status2, headers2, body2) = post_json_full(
            &pool,
            id_user,
            &handle,
            &uri,
            payload,
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let adj_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation = $2",
        )
        .bind(id_portfolio)
        .bind(original)
        .fetch_one(&pool)
        .await
        .unwrap();

        cleanup_idempotency(&pool, id_portfolio).await;
        sqlx::query("DELETE FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation = $2")
            .bind(id_portfolio)
            .bind(original)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(status2, StatusCode::OK);
        assert_eq!(
            header_value(&headers2, "idempotency-replayed").as_deref(),
            Some("true")
        );
        assert_eq!(
            body1["operation"]["id_portfolio_operation"],
            body2["operation"]["id_portfolio_operation"]
        );
        assert_eq!(
            adj_count, 1,
            "no second adjustment must be created on replay"
        );
    }

    #[tokio::test]
    async fn replay_after_refresh_request_advanced_returns_same_refresh_id() {
        // The refresh-request status MAY advance from `pending` to
        // `processing`/`completed` between the original write and the
        // replay. The IDs on the replay envelope must be stable; the
        // status field reflects the CURRENT state of the refresh request
        // (the replay envelope returns the live status, not the original
        // one — documented in kushim-api/README.md).
        let pool = test_pool().await;
        let handle = format!("p3rrc{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();

        let (status1, _, body1) = create_with_key(
            &pool,
            id_user,
            &handle,
            id_portfolio,
            key,
            posted_deposit_payload(),
        )
        .await;
        assert_eq!(status1, StatusCode::CREATED);
        let refresh_id = body1["refresh_request"]["id_portfolio_refresh_request"]
            .as_str()
            .unwrap()
            .to_string();

        // Simulate the worker advancing the request.
        sqlx::query(
            "UPDATE portfolio_refresh_requests SET status = 'completed', completed_at = now() WHERE id_portfolio_refresh_request = $1::uuid",
        )
        .bind(&refresh_id)
        .execute(&pool)
        .await
        .unwrap();

        let (status2, _, body2) = create_with_key(
            &pool,
            id_user,
            &handle,
            id_portfolio,
            key,
            posted_deposit_payload(),
        )
        .await;

        cleanup_idempotency(&pool, id_portfolio).await;
        sqlx::query("DELETE FROM portfolio_refresh_requests WHERE id_portfolio = $1")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(status2, StatusCode::OK);
        assert_eq!(
            body1["operation"]["id_portfolio_operation"],
            body2["operation"]["id_portfolio_operation"]
        );
        assert_eq!(
            body2["refresh_request"]["id_portfolio_refresh_request"]
                .as_str()
                .unwrap(),
            refresh_id,
            "refresh-request id MUST be stable across replay"
        );
        assert_eq!(
            body2["refresh_request"]["status"], "completed",
            "replay surfaces the CURRENT refresh-request status"
        );
    }

    #[tokio::test]
    async fn same_user_cross_portfolio_returns_409_not_404() {
        // Brief §2: the durable key scope is (id_user, idempotency_key).
        // Reusing a successful key on another OWNED portfolio is a
        // CONFLICT (409), not a 404 — and must not create anything.
        let pool = test_pool().await;
        let handle = format!("p3xpf{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let portfolio_a = create_portfolio(&pool, id_user, None).await;
        let portfolio_b = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();

        let (status_a, _, body_a) = create_with_key(
            &pool,
            id_user,
            &handle,
            portfolio_a,
            key,
            posted_deposit_payload(),
        )
        .await;
        assert_eq!(status_a, StatusCode::CREATED);
        let op_a = body_a["operation"]["id_portfolio_operation"].clone();

        let (status_b, _, body_b) = create_with_key(
            &pool,
            id_user,
            &handle,
            portfolio_b,
            key,
            posted_deposit_payload(),
        )
        .await;

        let ops_b = count_operations(&pool, portfolio_b).await;
        let refresh_b = count_all_refresh_requests(&pool, portfolio_b).await;
        let idemp_b = count_idempotency(&pool, portfolio_b).await;
        // The original record (against portfolio_a) is unchanged.
        let record_op: Option<Uuid> = sqlx::query_scalar(
            "SELECT id_portfolio_operation FROM portfolio_operation_idempotency WHERE id_user = $1 AND idempotency_key = $2",
        )
        .bind(id_user)
        .bind(key)
        .fetch_one(&pool)
        .await
        .unwrap();

        cleanup_refresh_requests(&pool, portfolio_a).await;
        cleanup_idempotency(&pool, portfolio_a).await;

        assert_eq!(
            status_b,
            StatusCode::CONFLICT,
            "same user + same key on a different portfolio must 409, not 404"
        );
        assert_eq!(body_b["error"]["code"], "idempotency_key_conflict");
        assert_eq!(ops_b, 0, "no operation must be inserted in portfolio_b");
        assert_eq!(refresh_b, 0, "no refresh-request in portfolio_b");
        assert_eq!(idemp_b, 0, "no idempotency record in portfolio_b");
        assert_eq!(
            json!(record_op),
            op_a,
            "original (portfolio_a) idempotency record must remain unchanged"
        );
    }

    #[tokio::test]
    async fn same_uuid_independent_for_two_different_users_even_with_overlap() {
        // Sanity: the cross-portfolio conflict above is per-user; another
        // user with the same UUID is independent. Already covered by an
        // earlier test, kept here as a sibling assertion.
        let pool = test_pool().await;
        let h_a = format!("p3oa{}", &Uuid::new_v4().simple().to_string()[..10]);
        let h_b = format!("p3ob{}", &Uuid::new_v4().simple().to_string()[..10]);
        let user_a = create_user(&pool, &h_a).await;
        let user_b = create_user(&pool, &h_b).await;
        let pf_a = create_portfolio(&pool, user_a, None).await;
        let pf_b = create_portfolio(&pool, user_b, None).await;
        let key = Uuid::new_v4();

        let (status_a, _, _) =
            create_with_key(&pool, user_a, &h_a, pf_a, key, posted_deposit_payload()).await;
        let (status_b, _, _) =
            create_with_key(&pool, user_b, &h_b, pf_b, key, posted_deposit_payload()).await;

        cleanup_refresh_requests(&pool, pf_a).await;
        cleanup_refresh_requests(&pool, pf_b).await;
        cleanup_idempotency(&pool, pf_a).await;
        cleanup_idempotency(&pool, pf_b).await;

        assert_eq!(status_a, StatusCode::CREATED);
        assert_eq!(status_b, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn same_key_create_then_correction_returns_409() {
        let pool = test_pool().await;
        let handle = format!("p3kcc{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();

        // First: a normal create.
        let (status1, _, _) = create_with_key(
            &pool,
            id_user,
            &handle,
            id_portfolio,
            key,
            posted_deposit_payload(),
        )
        .await;
        assert_eq!(status1, StatusCode::CREATED);

        // Seed a posted op to correct (separate, via direct insert).
        let original =
            insert_operation(&pool, id_portfolio, "posted", posted_deposit_payload()).await;
        let uri = format!("/v1/portfolios/{id_portfolio}/operations/{original}/corrections");
        let (status2, _, body2) = post_json_full(
            &pool,
            id_user,
            &handle,
            &uri,
            correction_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let adj_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation IS NOT NULL",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .unwrap();

        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        assert_eq!(status2, StatusCode::CONFLICT);
        assert_eq!(body2["error"]["code"], "idempotency_key_conflict");
        assert_eq!(adj_count, 0, "no adjustment must be created");
    }

    #[tokio::test]
    async fn same_key_correction_then_create_returns_409() {
        let pool = test_pool().await;
        let handle = format!("p3kcb{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let original =
            insert_operation(&pool, id_portfolio, "posted", posted_deposit_payload()).await;
        let key = Uuid::new_v4();

        let (status1, _, _) = post_json_full(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations/{original}/corrections"),
            correction_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;
        assert_eq!(status1, StatusCode::CREATED);

        // Now reuse the key on a plain create.
        let (status2, _, body2) = create_with_key(
            &pool,
            id_user,
            &handle,
            id_portfolio,
            key,
            posted_deposit_payload(),
        )
        .await;

        let extra_ops: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation IS NULL",
        )
        .bind(id_portfolio)
        .fetch_one(&pool)
        .await
        .unwrap();

        cleanup_idempotency(&pool, id_portfolio).await;
        sqlx::query("DELETE FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation IS NOT NULL")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(status2, StatusCode::CONFLICT);
        assert_eq!(body2["error"]["code"], "idempotency_key_conflict");
        // `original` was inserted directly (posted, immutable) — so the
        // primary-operations count is exactly the seeded original, no extra
        // create must have landed.
        assert_eq!(extra_ops, 1, "no extra primary operation must be inserted");
    }

    #[tokio::test]
    async fn correction_same_key_against_different_originals_returns_409() {
        let pool = test_pool().await;
        let handle = format!("p3kco{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let original_a =
            insert_operation(&pool, id_portfolio, "posted", posted_deposit_payload()).await;
        let original_b =
            insert_operation(&pool, id_portfolio, "posted", posted_deposit_payload()).await;
        let key = Uuid::new_v4();

        let (status1, _, _) = post_json_full(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations/{original_a}/corrections"),
            correction_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;
        assert_eq!(status1, StatusCode::CREATED);

        let (status2, _, body2) = post_json_full(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations/{original_b}/corrections"),
            correction_payload(),
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let adj_against_b: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation = $2",
        )
        .bind(id_portfolio)
        .bind(original_b)
        .fetch_one(&pool)
        .await
        .unwrap();

        cleanup_idempotency(&pool, id_portfolio).await;
        sqlx::query("DELETE FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation IS NOT NULL")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(status2, StatusCode::CONFLICT);
        assert_eq!(body2["error"]["code"], "idempotency_key_conflict");
        assert_eq!(
            adj_against_b, 0,
            "no adjustment must be created against original_b"
        );
    }

    #[tokio::test]
    async fn pending_create_replay_returns_same_op_and_no_refresh() {
        let pool = test_pool().await;
        let handle = format!("p3pr{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();

        // pending default — no operation_status field at all.
        let (status1, headers1, body1) = create_with_key(
            &pool,
            id_user,
            &handle,
            id_portfolio,
            key,
            deposit_payload(),
        )
        .await;
        let (status2, headers2, body2) = create_with_key(
            &pool,
            id_user,
            &handle,
            id_portfolio,
            key,
            deposit_payload(),
        )
        .await;

        let op_count = count_operations(&pool, id_portfolio).await;
        let refresh_count = count_all_refresh_requests(&pool, id_portfolio).await;
        let idemp_count = count_idempotency(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;
        cleanup_user_tree(&pool, id_user, &[]).await;

        assert_eq!(status1, StatusCode::CREATED);
        assert_eq!(status2, StatusCode::OK);
        assert_eq!(
            header_value(&headers1, "idempotency-replayed").as_deref(),
            Some("false")
        );
        assert_eq!(
            header_value(&headers2, "idempotency-replayed").as_deref(),
            Some("true")
        );
        assert_eq!(
            body1["operation"]["id_portfolio_operation"],
            body2["operation"]["id_portfolio_operation"]
        );
        assert!(body1["refresh_request"].is_null());
        assert!(body2["refresh_request"].is_null());
        assert_eq!(op_count, 1);
        assert_eq!(refresh_count, 0, "pending creates never enqueue refresh");
        assert_eq!(idemp_count, 1);
    }

    #[tokio::test]
    async fn concurrent_identical_corrections_produce_exactly_one_adjustment() {
        let pool = test_pool_concurrent().await;
        let handle = format!("p3cic{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let original =
            insert_operation(&pool, id_portfolio, "posted", posted_deposit_payload()).await;
        let key = Uuid::new_v4();
        let uri = format!("/v1/portfolios/{id_portfolio}/operations/{original}/corrections");
        let app = crate::http::router(test_state(pool.clone()).await);

        let build_req = || {
            Request::builder()
                .method("POST")
                .uri(&uri)
                .header(
                    AUTHORIZATION,
                    format!("Bearer {}", build_access_token(id_user, &handle)),
                )
                .header("content-type", "application/json")
                .header("idempotency-key", key.to_string())
                .body(Body::from(correction_payload().to_string()))
                .unwrap()
        };

        let (r1, r2, r3) = tokio::join!(
            app.clone().oneshot(build_req()),
            app.clone().oneshot(build_req()),
            app.clone().oneshot(build_req()),
        );
        let responses = [r1.unwrap(), r2.unwrap(), r3.unwrap()];
        for r in &responses {
            assert!(
                r.status().is_success(),
                "all three must succeed, got {:?}",
                r.status()
            );
        }
        let mut bodies: Vec<Value> = Vec::new();
        for r in responses {
            let bytes = body::to_bytes(r.into_body(), usize::MAX).await.unwrap();
            bodies.push(serde_json::from_slice::<Value>(&bytes).unwrap());
        }

        let adj_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation = $2",
        )
        .bind(id_portfolio)
        .bind(original)
        .fetch_one(&pool)
        .await
        .unwrap();
        let idemp_count = count_idempotency(&pool, id_portfolio).await;

        cleanup_idempotency(&pool, id_portfolio).await;
        sqlx::query("DELETE FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation IS NOT NULL")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .unwrap();

        let op_ids: Vec<&str> = bodies
            .iter()
            .map(|b| b["operation"]["id_portfolio_operation"].as_str().unwrap())
            .collect();
        assert!(
            op_ids.iter().all(|id| *id == op_ids[0]),
            "all responses point at the same adjustment"
        );
        assert_eq!(adj_count, 1, "exactly one adjustment must be created");
        assert_eq!(idemp_count, 1);
    }

    #[tokio::test]
    async fn concurrent_correction_conflict_one_succeeds_one_409() {
        let pool = test_pool_concurrent().await;
        let handle = format!("p3ccc{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let original =
            insert_operation(&pool, id_portfolio, "posted", posted_deposit_payload()).await;
        let key = Uuid::new_v4();
        let uri = format!("/v1/portfolios/{id_portfolio}/operations/{original}/corrections");
        let app = crate::http::router(test_state(pool.clone()).await);

        let mut payload_b = correction_payload();
        payload_b["cash_amount_minor"] = json!(424242);

        let build_req = |payload: Value| {
            Request::builder()
                .method("POST")
                .uri(&uri)
                .header(
                    AUTHORIZATION,
                    format!("Bearer {}", build_access_token(id_user, &handle)),
                )
                .header("content-type", "application/json")
                .header("idempotency-key", key.to_string())
                .body(Body::from(payload.to_string()))
                .unwrap()
        };

        let (ra, rb) = tokio::join!(
            app.clone().oneshot(build_req(correction_payload())),
            app.clone().oneshot(build_req(payload_b)),
        );
        let sa = ra.unwrap().status();
        let sb = rb.unwrap().status();

        let adj_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation = $2",
        )
        .bind(id_portfolio)
        .bind(original)
        .fetch_one(&pool)
        .await
        .unwrap();

        cleanup_idempotency(&pool, id_portfolio).await;
        sqlx::query("DELETE FROM portfolio_operations WHERE id_portfolio = $1 AND id_corrected_operation IS NOT NULL")
            .bind(id_portfolio)
            .execute(&pool)
            .await
            .unwrap();

        let success = [sa, sb].iter().filter(|s| s.is_success()).count();
        let conflict = [sa, sb]
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count();
        assert_eq!(success, 1);
        assert_eq!(conflict, 1);
        assert_eq!(adj_count, 1);
    }

    #[tokio::test]
    async fn rollback_after_claim_leaves_no_idempotency_record() {
        // Forces a deterministic transactional failure AFTER the
        // idempotency claim by referencing a non-existent
        // `id_corrected_operation` (FK fk_portfolio_operations_corrected_op
        // fails at COMMIT for an adjustment row). The idempotency claim,
        // operation insert and refresh enqueue must all roll back.
        let pool = test_pool().await;
        let handle = format!("p3rb{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;
        let key = Uuid::new_v4();

        // Adjustment + id_corrected_operation pointing at a UUID that does
        // not exist → FK violation deep inside the same transaction that
        // claimed the idempotency row.
        let payload = json!({
            "operation_type": "adjustment",
            "operation_status": "posted",
            "executed_at": "2026-06-05T10:00:00Z",
            "cash_amount_minor": 100,
            "currency": "EUR",
            "id_corrected_operation": Uuid::new_v4(),
            "metadata": {}
        });

        let (status, body) = post_json_with_headers(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            payload,
            &[("idempotency-key", key.to_string())],
        )
        .await;

        let op_count = count_operations(&pool, id_portfolio).await;
        let refresh_count = count_all_refresh_requests(&pool, id_portfolio).await;
        let idemp_count = count_idempotency(&pool, id_portfolio).await;

        assert!(
            !status.is_success(),
            "the FK violation must surface as a non-2xx, got {status}"
        );
        // Never leak a raw SQL/constraint name.
        let body_text = body.to_string();
        assert!(
            !body_text.contains("constraint"),
            "must not leak constraint name"
        );
        assert!(
            !body_text.contains("violates"),
            "must not leak raw PG error"
        );
        assert!(!body_text.contains("sqlx"), "must not leak driver name");

        assert_eq!(op_count, 0, "operation must roll back");
        assert_eq!(refresh_count, 0, "refresh-request must roll back");
        assert_eq!(idemp_count, 0, "idempotency record must roll back");

        // After rollback the key is still reusable with a corrected payload.
        let (status_ok, _, _) = create_with_key(
            &pool,
            id_user,
            &handle,
            id_portfolio,
            key,
            posted_deposit_payload(),
        )
        .await;
        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;
        assert_eq!(status_ok, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn external_reference_conflict_leaves_no_idempotency_record() {
        // First operation seeds (provider, reference). A SECOND operation
        // re-using the same external (provider, reference) under a NEW
        // idempotency key must:
        //   (a) fail (unique constraint on external_provider/_reference);
        //   (b) leave no idempotency record;
        //   (c) allow that same key to be reused with a corrected
        //       external_reference.
        let pool = test_pool().await;
        let handle = format!("p3xr{}", &Uuid::new_v4().simple().to_string()[..10]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = create_portfolio(&pool, id_user, None).await;

        let unique_ref = format!("ref-{}", Uuid::new_v4().simple());
        let mut seed = posted_deposit_payload();
        seed["external_provider"] = json!("acme-broker");
        seed["external_reference"] = json!(unique_ref);

        let (status_seed, _, _) = create_with_key(
            &pool,
            id_user,
            &handle,
            id_portfolio,
            Uuid::new_v4(),
            seed.clone(),
        )
        .await;
        assert_eq!(status_seed, StatusCode::CREATED);

        // Second attempt: NEW idempotency key, same external_reference →
        // unique-index conflict deep inside the transaction.
        let bad_key = Uuid::new_v4();
        let (status_bad, body_bad) = post_json_with_headers(
            &pool,
            id_user,
            &handle,
            &format!("/v1/portfolios/{id_portfolio}/operations"),
            seed,
            &[("idempotency-key", bad_key.to_string())],
        )
        .await;

        let idemp_for_bad: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_operation_idempotency WHERE id_user = $1 AND idempotency_key = $2",
        )
        .bind(id_user)
        .bind(bad_key)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            !status_bad.is_success(),
            "unique-constraint must surface as non-2xx"
        );
        assert!(
            !body_bad.to_string().contains("constraint"),
            "no raw PG details"
        );
        assert_eq!(
            idemp_for_bad, 0,
            "no idempotency record may remain for the failed bad-key attempt"
        );

        // The same key can be reused with a corrected external_reference.
        let mut fixed = posted_deposit_payload();
        fixed["external_provider"] = json!("acme-broker");
        fixed["external_reference"] = json!(format!("ref-{}", Uuid::new_v4().simple()));
        let (status_fix, _, _) =
            create_with_key(&pool, id_user, &handle, id_portfolio, bad_key, fixed).await;

        cleanup_refresh_requests(&pool, id_portfolio).await;
        cleanup_idempotency(&pool, id_portfolio).await;

        assert_eq!(
            status_fix,
            StatusCode::CREATED,
            "reused key with corrected external_reference must succeed"
        );
    }

    #[tokio::test]
    async fn cors_preflight_allows_idempotency_key() {
        // The CORS preflight must accept the Idempotency-Key header so the
        // browser actually sends the subsequent POST. The Expose-Headers
        // contract is asserted separately on a real response below
        // (`cors_real_response_exposes_idempotency_replayed`) — tower-http
        // only emits Expose-Headers on the actual cross-origin response,
        // not on the OPTIONS preflight.
        let pool = test_pool().await;
        let app =
            crate::http::router_with_cors(test_state(pool).await, Some("http://localhost:5173"));
        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/v1/portfolios/00000000-0000-0000-0000-000000000000/operations")
                    .header("origin", "http://localhost:5173")
                    .header("access-control-request-method", "POST")
                    .header(
                        "access-control-request-headers",
                        "authorization,content-type,idempotency-key",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("preflight should be built");

        let status = response.status();
        let allow_headers = response
            .headers()
            .get("access-control-allow-headers")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();

        assert!(status.is_success(), "preflight must succeed, got {status}");
        assert!(
            allow_headers.to_lowercase().contains("idempotency-key"),
            "Allow-Headers must include idempotency-key, got: {allow_headers}"
        );
    }

    #[tokio::test]
    async fn cors_real_response_exposes_idempotency_replayed() {
        // tower-http emits Access-Control-Expose-Headers only on actual
        // cross-origin responses (not on preflights). Hit a public health
        // endpoint from the configured origin and assert the header.
        let pool = test_pool().await;
        let app =
            crate::http::router_with_cors(test_state(pool).await, Some("http://localhost:5173"));
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .header("origin", "http://localhost:5173")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let expose_headers = response
            .headers()
            .get("access-control-expose-headers")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();

        assert!(
            expose_headers
                .to_lowercase()
                .contains("idempotency-replayed"),
            "Expose-Headers must include idempotency-replayed, got: {expose_headers}"
        );
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
