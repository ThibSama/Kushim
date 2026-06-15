use crate::{
    domain::asset::AssetStatus,
    domain::portfolio_operation::{
        NewPortfolioOperation, OperationStatus, OperationType, PortfolioOperation,
        PortfolioOperationFilters, UpdatePortfolioOperation,
    },
    domain::portfolio_refresh_request::PortfolioRefreshRequest,
    repositories::{
        assets::{AssetRepository, AssetRepositoryError},
        portfolio_operations::{PortfolioOperationRepository, PortfolioOperationRepositoryError},
        portfolio_refresh_requests::{
            PortfolioRefreshRequestRepository, PortfolioRefreshRequestRepositoryError,
        },
        portfolios::{PortfolioRepository, PortfolioRepositoryError},
    },
};
use bigdecimal::BigDecimal;
use serde_json::Value;
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[derive(Clone)]
pub struct PortfolioOperationService {
    asset_repository: AssetRepository,
    portfolio_repository: PortfolioRepository,
    portfolio_operation_repository: PortfolioOperationRepository,
    portfolio_refresh_request_repository: PortfolioRefreshRequestRepository,
}

/// Result of a write that may have enqueued a portfolio refresh request.
/// `refresh_request` is `Some` exactly when the write produced a `posted`
/// operation (direct posted creation, posting a pending operation, or posted
/// correction creation). Pending creations carry `None`.
#[derive(Debug, Clone)]
pub struct OperationWriteOutcome {
    pub operation: PortfolioOperation,
    pub refresh_request: Option<PortfolioRefreshRequest>,
}

#[derive(Debug, Clone)]
pub struct CreatePortfolioOperationInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
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

#[derive(Debug, Clone, Default)]
pub struct ListPortfolioOperationsInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub operation_status: Option<OperationStatus>,
    pub operation_type: Option<OperationType>,
    pub id_asset: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct UpdatePortfolioOperationInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub id_portfolio_operation: Uuid,
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

#[derive(Debug, Clone)]
pub struct CancelPortfolioOperationInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub id_portfolio_operation: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreatePortfolioOperationCorrectionInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub id_portfolio_operation: Uuid,
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

#[derive(Debug, Clone)]
pub struct PostPortfolioOperationInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub id_portfolio_operation: Uuid,
}

#[derive(Debug, Clone)]
pub struct PortfolioOperationCorrectionsView {
    pub operation: PortfolioOperation,
    pub corrections: Vec<PortfolioOperation>,
}

#[derive(Debug, Clone)]
pub struct PortfolioOperationAuditView {
    pub operation: PortfolioOperation,
    pub corrected_operation: Option<PortfolioOperation>,
    pub corrections: Vec<PortfolioOperation>,
}

#[derive(Debug, Clone)]
pub struct PortfolioOperationAuditTimelineInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub operation_status: Option<OperationStatus>,
    pub operation_type: Option<OperationType>,
}

#[derive(Debug, Clone)]
pub struct PortfolioOperationAuditTimelineItemView {
    pub operation: PortfolioOperation,
    pub corrections: Vec<PortfolioOperation>,
}

#[derive(Debug, Clone)]
pub struct PortfolioOperationAuditTimelinePaginationView {
    pub limit: i64,
    pub offset: i64,
    pub returned: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct PortfolioOperationAuditTimelineView {
    pub items: Vec<PortfolioOperationAuditTimelineItemView>,
    pub pagination: PortfolioOperationAuditTimelinePaginationView,
}

#[derive(Debug, Error)]
pub enum PortfolioOperationServiceError {
    #[error("validation failed")]
    Validation {
        code: &'static str,
        message: &'static str,
    },
    #[error("resource not found")]
    NotFound {
        code: &'static str,
        message: &'static str,
    },
    #[error("conflict")]
    Conflict {
        code: &'static str,
        message: &'static str,
    },
    #[error("service failure")]
    Internal,
}

impl PortfolioOperationService {
    pub fn new(
        asset_repository: AssetRepository,
        portfolio_repository: PortfolioRepository,
        portfolio_operation_repository: PortfolioOperationRepository,
        portfolio_refresh_request_repository: PortfolioRefreshRequestRepository,
    ) -> Self {
        Self {
            asset_repository,
            portfolio_repository,
            portfolio_operation_repository,
            portfolio_refresh_request_repository,
        }
    }

    pub async fn create_operation(
        &self,
        input: CreatePortfolioOperationInput,
    ) -> Result<OperationWriteOutcome, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        let operation_status = input.operation_status.unwrap_or(OperationStatus::Pending);
        if operation_status == OperationStatus::Cancelled {
            return Err(PortfolioOperationServiceError::Validation {
                code: "invalid_operation_status",
                message: "operation creation does not accept cancelled status",
            });
        }

        let new_operation = NewPortfolioOperation {
            id_portfolio: input.id_portfolio,
            id_asset: input.id_asset,
            id_related_asset: input.id_related_asset,
            operation_type: input.operation_type,
            operation_status,
            executed_at: parse_datetime(&input.executed_at)?,
            effective_at: parse_optional_datetime(input.effective_at.as_deref())?,
            quantity: normalize_optional_string(input.quantity),
            related_quantity: normalize_optional_string(input.related_quantity),
            price_minor: input.price_minor,
            gross_amount_minor: input.gross_amount_minor,
            fees_minor: input.fees_minor,
            taxes_minor: input.taxes_minor,
            cash_amount_minor: input.cash_amount_minor.unwrap_or(0),
            currency: normalize_required_string(&input.currency),
            fx_rate_to_portfolio: normalize_optional_string(input.fx_rate_to_portfolio),
            external_provider: normalize_optional_string(input.external_provider),
            external_reference: normalize_optional_string(input.external_reference),
            id_corrected_operation: input.id_corrected_operation,
            notes: normalize_optional_string(input.notes),
            metadata: input.metadata.unwrap_or_else(default_metadata),
        };

        validate_operation_payload(&new_operation)?;
        self.validate_asset_references(&new_operation).await?;

        let (operation, refresh_request) = self
            .portfolio_operation_repository
            .create_with_optional_refresh(&new_operation)
            .await
            .map_err(map_operation_repository_error)?;

        Ok(OperationWriteOutcome {
            operation,
            refresh_request,
        })
    }

    pub async fn list_operations(
        &self,
        input: ListPortfolioOperationsInput,
    ) -> Result<Vec<PortfolioOperation>, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        self.portfolio_operation_repository
            .list_by_portfolio(
                input.id_portfolio,
                &PortfolioOperationFilters {
                    operation_status: input.operation_status,
                    operation_type: input.operation_type,
                    id_asset: input.id_asset,
                },
            )
            .await
            .map_err(map_operation_repository_error)
    }

    pub async fn get_operation(
        &self,
        id_user: Uuid,
        id_portfolio: Uuid,
        id_portfolio_operation: Uuid,
    ) -> Result<PortfolioOperation, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(id_portfolio, id_user).await?;

        self.portfolio_operation_repository
            .find_by_id_and_portfolio(id_portfolio_operation, id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })
    }

    pub async fn update_operation(
        &self,
        input: UpdatePortfolioOperationInput,
    ) -> Result<PortfolioOperation, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        let existing = self
            .portfolio_operation_repository
            .find_by_id_and_portfolio(input.id_portfolio_operation, input.id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })?;

        match existing.operation_status {
            OperationStatus::Pending => {}
            OperationStatus::Posted => {
                return Err(PortfolioOperationServiceError::Conflict {
                    code: "posted_operation_immutable",
                    message: "posted portfolio operations cannot be updated",
                });
            }
            OperationStatus::Cancelled => {
                return Err(PortfolioOperationServiceError::Conflict {
                    code: "cancelled_operation_immutable",
                    message: "cancelled portfolio operations cannot be updated",
                });
            }
        }

        let update = UpdatePortfolioOperation {
            id_asset: input.id_asset.or(existing.id_asset),
            id_related_asset: input.id_related_asset.or(existing.id_related_asset),
            operation_type: input.operation_type.unwrap_or(existing.operation_type),
            executed_at: match input.executed_at.as_deref() {
                Some(value) => parse_datetime(value)?,
                None => existing.executed_at,
            },
            effective_at: match input.effective_at {
                Some(value) => parse_optional_datetime(value.as_deref())?,
                None => existing.effective_at,
            },
            quantity: match input.quantity {
                Some(value) => normalize_optional_string(value),
                None => existing.quantity,
            },
            related_quantity: match input.related_quantity {
                Some(value) => normalize_optional_string(value),
                None => existing.related_quantity,
            },
            price_minor: match input.price_minor {
                Some(value) => value,
                None => existing.price_minor,
            },
            gross_amount_minor: match input.gross_amount_minor {
                Some(value) => value,
                None => existing.gross_amount_minor,
            },
            fees_minor: match input.fees_minor {
                Some(value) => value,
                None => existing.fees_minor,
            },
            taxes_minor: match input.taxes_minor {
                Some(value) => value,
                None => existing.taxes_minor,
            },
            cash_amount_minor: input
                .cash_amount_minor
                .unwrap_or(existing.cash_amount_minor),
            currency: match input.currency {
                Some(value) => normalize_required_string(&value),
                None => existing.currency,
            },
            fx_rate_to_portfolio: match input.fx_rate_to_portfolio {
                Some(value) => normalize_optional_string(value),
                None => existing.fx_rate_to_portfolio,
            },
            external_provider: match input.external_provider {
                Some(value) => normalize_optional_string(value),
                None => existing.external_provider,
            },
            external_reference: match input.external_reference {
                Some(value) => normalize_optional_string(value),
                None => existing.external_reference,
            },
            id_corrected_operation: match input.id_corrected_operation {
                Some(value) => value,
                None => existing.id_corrected_operation,
            },
            notes: match input.notes {
                Some(value) => normalize_optional_string(value),
                None => existing.notes,
            },
            metadata: input.metadata.unwrap_or(existing.metadata),
        };

        let candidate = NewPortfolioOperation {
            id_portfolio: input.id_portfolio,
            id_asset: update.id_asset,
            id_related_asset: update.id_related_asset,
            operation_type: update.operation_type.clone(),
            operation_status: OperationStatus::Pending,
            executed_at: update.executed_at,
            effective_at: update.effective_at,
            quantity: update.quantity.clone(),
            related_quantity: update.related_quantity.clone(),
            price_minor: update.price_minor,
            gross_amount_minor: update.gross_amount_minor,
            fees_minor: update.fees_minor,
            taxes_minor: update.taxes_minor,
            cash_amount_minor: update.cash_amount_minor,
            currency: update.currency.clone(),
            fx_rate_to_portfolio: update.fx_rate_to_portfolio.clone(),
            external_provider: update.external_provider.clone(),
            external_reference: update.external_reference.clone(),
            id_corrected_operation: update.id_corrected_operation,
            notes: update.notes.clone(),
            metadata: update.metadata.clone(),
        };

        validate_operation_payload(&candidate)?;
        self.validate_asset_references(&candidate).await?;

        self.portfolio_operation_repository
            .update(input.id_portfolio_operation, input.id_portfolio, &update)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })
    }

    pub async fn cancel_operation(
        &self,
        input: CancelPortfolioOperationInput,
    ) -> Result<PortfolioOperation, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        let existing = self
            .portfolio_operation_repository
            .find_by_id_and_portfolio(input.id_portfolio_operation, input.id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })?;

        match existing.operation_status {
            OperationStatus::Pending => self
                .portfolio_operation_repository
                .set_status(
                    input.id_portfolio_operation,
                    input.id_portfolio,
                    OperationStatus::Cancelled,
                )
                .await
                .map_err(map_operation_repository_error)?
                .ok_or(PortfolioOperationServiceError::NotFound {
                    code: "operation_not_found",
                    message: "portfolio operation was not found",
                }),
            OperationStatus::Cancelled => Ok(existing),
            OperationStatus::Posted => Err(PortfolioOperationServiceError::Conflict {
                code: "posted_operation_immutable",
                message: "posted portfolio operations cannot be cancelled",
            }),
        }
    }

    pub async fn create_correction(
        &self,
        input: CreatePortfolioOperationCorrectionInput,
    ) -> Result<OperationWriteOutcome, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        let original_operation = self
            .portfolio_operation_repository
            .find_by_id_and_portfolio(input.id_portfolio_operation, input.id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })?;

        match original_operation.operation_status {
            OperationStatus::Posted => {}
            OperationStatus::Pending => {
                return Err(PortfolioOperationServiceError::Conflict {
                    code: "correction_requires_posted_operation",
                    message: "only posted portfolio operations can be corrected",
                });
            }
            OperationStatus::Cancelled => {
                return Err(PortfolioOperationServiceError::Conflict {
                    code: "correction_requires_posted_operation",
                    message: "cancelled portfolio operations cannot be corrected",
                });
            }
        }

        let operation_status = input.operation_status.unwrap_or(OperationStatus::Pending);
        if operation_status == OperationStatus::Cancelled {
            return Err(PortfolioOperationServiceError::Validation {
                code: "invalid_operation_status",
                message: "correction creation does not accept cancelled status",
            });
        }

        let new_operation = NewPortfolioOperation {
            id_portfolio: input.id_portfolio,
            id_asset: input.id_asset,
            id_related_asset: input.id_related_asset,
            operation_type: OperationType::Adjustment,
            operation_status,
            executed_at: parse_datetime(&input.executed_at)?,
            effective_at: parse_optional_datetime(input.effective_at.as_deref())?,
            quantity: normalize_optional_string(input.quantity),
            related_quantity: normalize_optional_string(input.related_quantity),
            price_minor: input.price_minor,
            gross_amount_minor: input.gross_amount_minor,
            fees_minor: input.fees_minor,
            taxes_minor: input.taxes_minor,
            cash_amount_minor: input.cash_amount_minor.unwrap_or(0),
            currency: input
                .currency
                .map(|value| normalize_required_string(&value))
                .unwrap_or_else(|| original_operation.currency.clone()),
            fx_rate_to_portfolio: normalize_optional_string(input.fx_rate_to_portfolio),
            external_provider: normalize_optional_string(input.external_provider),
            external_reference: normalize_optional_string(input.external_reference),
            id_corrected_operation: Some(original_operation.id_portfolio_operation),
            notes: normalize_optional_string(input.notes),
            metadata: input.metadata.unwrap_or_else(default_metadata),
        };

        validate_correction_payload(&new_operation)?;
        validate_operation_payload(&new_operation)?;
        self.validate_asset_references(&new_operation).await?;

        let (operation, refresh_request) = self
            .portfolio_operation_repository
            .create_with_optional_refresh(&new_operation)
            .await
            .map_err(map_operation_repository_error)?;

        Ok(OperationWriteOutcome {
            operation,
            refresh_request,
        })
    }

    pub async fn post_operation(
        &self,
        input: PostPortfolioOperationInput,
    ) -> Result<OperationWriteOutcome, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        let existing = self
            .portfolio_operation_repository
            .find_by_id_and_portfolio(input.id_portfolio_operation, input.id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })?;

        match existing.operation_status {
            OperationStatus::Pending => {}
            OperationStatus::Posted => {
                return Err(PortfolioOperationServiceError::Conflict {
                    code: "operation_already_posted",
                    message: "portfolio operation is already posted",
                });
            }
            OperationStatus::Cancelled => {
                return Err(PortfolioOperationServiceError::Conflict {
                    code: "cancelled_operation_cannot_be_posted",
                    message: "cancelled portfolio operations cannot be posted",
                });
            }
        }

        let candidate = candidate_from_existing_operation(&existing, OperationStatus::Pending);
        if candidate.operation_type == OperationType::Adjustment {
            validate_correction_payload(&candidate)?;
        }
        validate_operation_payload(&candidate)?;
        self.validate_asset_references(&candidate).await?;

        let (operation, refresh_request) = self
            .portfolio_operation_repository
            .set_status_posted_with_refresh(input.id_portfolio_operation, input.id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })?;

        Ok(OperationWriteOutcome {
            operation,
            refresh_request: Some(refresh_request),
        })
    }

    /// Look up a refresh request for the authenticated user, enforcing both
    /// portfolio ownership and that the request belongs to the portfolio.
    pub async fn get_refresh_request(
        &self,
        id_user: Uuid,
        id_portfolio: Uuid,
        id_refresh_request: Uuid,
    ) -> Result<PortfolioRefreshRequest, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(id_portfolio, id_user).await?;

        self.portfolio_refresh_request_repository
            .find_by_id_and_portfolio(id_refresh_request, id_portfolio)
            .await
            .map_err(map_refresh_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "refresh_request_not_found",
                message: "portfolio refresh request was not found",
            })
    }

    pub async fn get_corrections(
        &self,
        id_user: Uuid,
        id_portfolio: Uuid,
        id_portfolio_operation: Uuid,
    ) -> Result<PortfolioOperationCorrectionsView, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(id_portfolio, id_user).await?;

        let operation = self
            .portfolio_operation_repository
            .find_by_id_and_portfolio(id_portfolio_operation, id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })?;

        let corrections = self
            .portfolio_operation_repository
            .list_corrections_for_operation(id_portfolio, id_portfolio_operation)
            .await
            .map_err(map_operation_repository_error)?;

        Ok(PortfolioOperationCorrectionsView {
            operation,
            corrections,
        })
    }

    pub async fn get_audit(
        &self,
        id_user: Uuid,
        id_portfolio: Uuid,
        id_portfolio_operation: Uuid,
    ) -> Result<PortfolioOperationAuditView, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(id_portfolio, id_user).await?;

        let operation = self
            .portfolio_operation_repository
            .find_by_id_and_portfolio(id_portfolio_operation, id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })?;

        let corrected_operation = match operation.id_corrected_operation {
            Some(id_corrected_operation) => self
                .portfolio_operation_repository
                .find_by_id_and_portfolio(id_corrected_operation, id_portfolio)
                .await
                .map_err(map_operation_repository_error)?,
            None => None,
        };

        let corrections = match operation.id_corrected_operation {
            Some(id_corrected_operation) => self
                .portfolio_operation_repository
                .list_corrections_for_operation(id_portfolio, id_corrected_operation)
                .await
                .map_err(map_operation_repository_error)?,
            None => self
                .portfolio_operation_repository
                .list_corrections_for_operation(id_portfolio, id_portfolio_operation)
                .await
                .map_err(map_operation_repository_error)?,
        };

        Ok(PortfolioOperationAuditView {
            operation,
            corrected_operation,
            corrections,
        })
    }

    pub async fn get_audit_timeline(
        &self,
        input: PortfolioOperationAuditTimelineInput,
    ) -> Result<PortfolioOperationAuditTimelineView, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        let limit = validate_limit(input.limit)?;
        let offset = validate_offset(input.offset)?;

        let primary_operations = self
            .portfolio_operation_repository
            .list_primary_operations_page(
                input.id_portfolio,
                &PortfolioOperationFilters {
                    operation_status: input.operation_status,
                    operation_type: input.operation_type,
                    id_asset: None,
                },
                limit + 1,
                offset,
            )
            .await
            .map_err(map_operation_repository_error)?;

        let has_more = primary_operations.len() > limit as usize;
        let primary_operations: Vec<_> = primary_operations
            .into_iter()
            .take(limit as usize)
            .collect();

        let primary_ids: Vec<_> = primary_operations
            .iter()
            .map(|operation| operation.id_portfolio_operation)
            .collect();

        let corrections = self
            .portfolio_operation_repository
            .list_corrections_for_operations(input.id_portfolio, &primary_ids)
            .await
            .map_err(map_operation_repository_error)?;

        let mut corrections_by_original: HashMap<Uuid, Vec<PortfolioOperation>> = HashMap::new();
        for correction in corrections {
            if let Some(id_corrected_operation) = correction.id_corrected_operation {
                corrections_by_original
                    .entry(id_corrected_operation)
                    .or_default()
                    .push(correction);
            }
        }

        let returned = primary_operations.len();
        let items = primary_operations
            .into_iter()
            .map(|operation| PortfolioOperationAuditTimelineItemView {
                corrections: corrections_by_original
                    .remove(&operation.id_portfolio_operation)
                    .unwrap_or_default(),
                operation,
            })
            .collect();

        Ok(PortfolioOperationAuditTimelineView {
            items,
            pagination: PortfolioOperationAuditTimelinePaginationView {
                limit,
                offset,
                returned,
                has_more,
            },
        })
    }

    async fn assert_owned_portfolio(
        &self,
        id_portfolio: Uuid,
        id_user: Uuid,
    ) -> Result<(), PortfolioOperationServiceError> {
        let portfolio = self
            .portfolio_repository
            .find_by_id_and_user(id_portfolio, id_user)
            .await
            .map_err(map_portfolio_repository_error)?;

        if portfolio.is_none() {
            return Err(PortfolioOperationServiceError::NotFound {
                code: "portfolio_not_found",
                message: "portfolio was not found",
            });
        }

        Ok(())
    }

    async fn validate_asset_references(
        &self,
        operation: &NewPortfolioOperation,
    ) -> Result<(), PortfolioOperationServiceError> {
        self.validate_asset_reference(operation.id_asset, false)
            .await?;
        self.validate_asset_reference(operation.id_related_asset, true)
            .await?;

        match operation.operation_type {
            OperationType::SpinOff | OperationType::SymbolChange | OperationType::Adjustment => {
                if matches!(
                    (operation.id_asset, operation.id_related_asset),
                    (Some(id_asset), Some(id_related_asset)) if id_asset == id_related_asset
                ) {
                    return Err(PortfolioOperationServiceError::Validation {
                        code: "same_asset_relation",
                        message: "id_asset and id_related_asset must be different for this operation type",
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn validate_asset_reference(
        &self,
        id_asset: Option<Uuid>,
        is_related: bool,
    ) -> Result<(), PortfolioOperationServiceError> {
        let Some(id_asset) = id_asset else {
            return Ok(());
        };

        let asset = self
            .asset_repository
            .find_validation_info(id_asset)
            .await
            .map_err(map_asset_repository_error)?
            .ok_or(PortfolioOperationServiceError::Validation {
                code: if is_related {
                    "invalid_related_asset_reference"
                } else {
                    "invalid_asset_reference"
                },
                message: if is_related {
                    "id_related_asset must reference an existing asset"
                } else {
                    "id_asset must reference an existing asset"
                },
            })?;

        if asset.status != AssetStatus::Active {
            return Err(PortfolioOperationServiceError::Validation {
                code: if is_related {
                    "inactive_related_asset_reference"
                } else {
                    "inactive_asset_reference"
                },
                message: if is_related {
                    "id_related_asset must reference an active asset"
                } else {
                    "id_asset must reference an active asset"
                },
            });
        }

        Ok(())
    }
}

fn validate_operation_payload(
    operation: &NewPortfolioOperation,
) -> Result<(), PortfolioOperationServiceError> {
    validate_currency(&operation.currency)?;
    validate_non_negative_i64("price_minor", operation.price_minor)?;
    validate_non_negative_i64("gross_amount_minor", operation.gross_amount_minor)?;
    validate_non_negative_i64("fees_minor", operation.fees_minor)?;
    validate_non_negative_i64("taxes_minor", operation.taxes_minor)?;

    if operation.cash_amount_minor < 0 {
        return Err(PortfolioOperationServiceError::Validation {
            code: "invalid_cash_amount_minor",
            message: "cash_amount_minor must be greater than or equal to 0",
        });
    }

    validate_optional_positive_decimal("quantity", operation.quantity.as_deref())?;
    validate_optional_positive_decimal("related_quantity", operation.related_quantity.as_deref())?;
    validate_optional_positive_decimal(
        "fx_rate_to_portfolio",
        operation.fx_rate_to_portfolio.as_deref(),
    )?;

    if operation.metadata.is_null() || !operation.metadata.is_object() {
        return Err(PortfolioOperationServiceError::Validation {
            code: "invalid_metadata",
            message: "metadata must be a JSON object",
        });
    }

    if operation.id_corrected_operation.is_some()
        && operation.operation_type != OperationType::Adjustment
    {
        return Err(PortfolioOperationServiceError::Validation {
            code: "invalid_corrected_operation",
            message: "id_corrected_operation is only allowed for adjustment operations",
        });
    }

    let provider_present = operation.external_provider.is_some();
    let reference_present = operation.external_reference.is_some();
    if provider_present != reference_present {
        return Err(PortfolioOperationServiceError::Validation {
            code: "invalid_external_reference",
            message: "external_provider and external_reference must both be provided together",
        });
    }

    match operation.operation_type {
        OperationType::Buy | OperationType::Sell => {
            require_some_uuid("id_asset", operation.id_asset)?;
            require_some_decimal("quantity", operation.quantity.as_deref())?;
            require_some_i64("price_minor", operation.price_minor)?;
            require_positive_i64("gross_amount_minor", operation.gross_amount_minor)?;
            require_positive_cash(operation.cash_amount_minor)?;
        }
        OperationType::Deposit
        | OperationType::Withdrawal
        | OperationType::Interest
        | OperationType::Fee
        | OperationType::Tax => {
            require_none_uuid("id_asset", operation.id_asset)?;
            require_positive_i64("gross_amount_minor", operation.gross_amount_minor)?;
            require_positive_cash(operation.cash_amount_minor)?;
        }
        OperationType::Dividend => {
            require_some_uuid("id_asset", operation.id_asset)?;
            require_positive_i64("gross_amount_minor", operation.gross_amount_minor)?;
            require_positive_cash(operation.cash_amount_minor)?;
        }
        OperationType::Split => {
            require_some_uuid("id_asset", operation.id_asset)?;
            require_some_decimal("quantity", operation.quantity.as_deref())?;
            if operation.cash_amount_minor != 0 {
                return Err(PortfolioOperationServiceError::Validation {
                    code: "invalid_cash_amount_minor",
                    message: "split operations must use cash_amount_minor = 0",
                });
            }
            require_none_i64("gross_amount_minor", operation.gross_amount_minor)?;
        }
        OperationType::SpinOff => {
            require_some_uuid("id_asset", operation.id_asset)?;
            require_some_uuid("id_related_asset", operation.id_related_asset)?;
            require_some_decimal("quantity", operation.quantity.as_deref())?;
            require_some_decimal("related_quantity", operation.related_quantity.as_deref())?;
            if operation.cash_amount_minor != 0 {
                return Err(PortfolioOperationServiceError::Validation {
                    code: "invalid_cash_amount_minor",
                    message: "spin_off operations must use cash_amount_minor = 0",
                });
            }
            require_none_i64("gross_amount_minor", operation.gross_amount_minor)?;
        }
        OperationType::SymbolChange => {
            require_some_uuid("id_asset", operation.id_asset)?;
            require_some_uuid("id_related_asset", operation.id_related_asset)?;
            require_some_decimal("quantity", operation.quantity.as_deref())?;
            if operation.cash_amount_minor != 0 {
                return Err(PortfolioOperationServiceError::Validation {
                    code: "invalid_cash_amount_minor",
                    message: "symbol_change operations must use cash_amount_minor = 0",
                });
            }
            require_none_i64("gross_amount_minor", operation.gross_amount_minor)?;
        }
        OperationType::TransferIn | OperationType::TransferOut | OperationType::Adjustment => {}
    }

    Ok(())
}

fn validate_limit(value: Option<i64>) -> Result<i64, PortfolioOperationServiceError> {
    let limit = value.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(PortfolioOperationServiceError::Validation {
            code: "invalid_limit",
            message: "limit must be between 1 and 100",
        });
    }

    Ok(limit)
}

fn validate_offset(value: Option<i64>) -> Result<i64, PortfolioOperationServiceError> {
    let offset = value.unwrap_or(0);
    if offset < 0 {
        return Err(PortfolioOperationServiceError::Validation {
            code: "invalid_offset",
            message: "offset must be greater than or equal to 0",
        });
    }

    Ok(offset)
}

fn validate_correction_payload(
    operation: &NewPortfolioOperation,
) -> Result<(), PortfolioOperationServiceError> {
    let has_meaningful_adjustment = operation.quantity.is_some()
        || operation.related_quantity.is_some()
        || operation.price_minor.is_some()
        || operation.gross_amount_minor.is_some()
        || operation.fees_minor.is_some()
        || operation.taxes_minor.is_some()
        || operation.cash_amount_minor > 0
        || operation.id_asset.is_some()
        || operation.id_related_asset.is_some()
        || !operation
            .metadata
            .as_object()
            .is_some_and(|object| object.is_empty());

    if !has_meaningful_adjustment {
        return Err(PortfolioOperationServiceError::Validation {
            code: "empty_correction",
            message: "correction must contain at least one meaningful adjustment field",
        });
    }

    Ok(())
}

fn candidate_from_existing_operation(
    operation: &PortfolioOperation,
    operation_status: OperationStatus,
) -> NewPortfolioOperation {
    NewPortfolioOperation {
        id_portfolio: operation.id_portfolio,
        id_asset: operation.id_asset,
        id_related_asset: operation.id_related_asset,
        operation_type: operation.operation_type.clone(),
        operation_status,
        executed_at: operation.executed_at,
        effective_at: operation.effective_at,
        quantity: operation.quantity.clone(),
        related_quantity: operation.related_quantity.clone(),
        price_minor: operation.price_minor,
        gross_amount_minor: operation.gross_amount_minor,
        fees_minor: operation.fees_minor,
        taxes_minor: operation.taxes_minor,
        cash_amount_minor: operation.cash_amount_minor,
        currency: operation.currency.clone(),
        fx_rate_to_portfolio: operation.fx_rate_to_portfolio.clone(),
        external_provider: operation.external_provider.clone(),
        external_reference: operation.external_reference.clone(),
        id_corrected_operation: operation.id_corrected_operation,
        notes: operation.notes.clone(),
        metadata: operation.metadata.clone(),
    }
}

fn parse_datetime(value: &str) -> Result<OffsetDateTime, PortfolioOperationServiceError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| PortfolioOperationServiceError::Validation {
        code: "invalid_datetime",
        message: "datetime fields must be valid RFC3339 strings",
    })
}

fn parse_optional_datetime(
    value: Option<&str>,
) -> Result<Option<OffsetDateTime>, PortfolioOperationServiceError> {
    value.map(parse_datetime).transpose()
}

fn validate_currency(value: &str) -> Result<(), PortfolioOperationServiceError> {
    let is_valid = value.len() == 3
        && value
            .chars()
            .all(|character| character.is_ascii_uppercase());
    if !is_valid {
        return Err(PortfolioOperationServiceError::Validation {
            code: "invalid_currency",
            message: "currency must be exactly 3 uppercase letters",
        });
    }

    Ok(())
}

fn validate_non_negative_i64(
    field: &'static str,
    value: Option<i64>,
) -> Result<(), PortfolioOperationServiceError> {
    if matches!(value, Some(number) if number < 0) {
        return Err(PortfolioOperationServiceError::Validation {
            code: invalid_minor_code(field),
            message: invalid_minor_message(field),
        });
    }

    Ok(())
}

fn validate_optional_positive_decimal(
    field: &'static str,
    value: Option<&str>,
) -> Result<(), PortfolioOperationServiceError> {
    if let Some(value) = value {
        let parsed = BigDecimal::from_str(value).map_err(|_| {
            PortfolioOperationServiceError::Validation {
                code: invalid_decimal_code(field),
                message: invalid_decimal_message(field),
            }
        })?;

        if parsed <= 0 {
            return Err(PortfolioOperationServiceError::Validation {
                code: invalid_decimal_code(field),
                message: invalid_decimal_message(field),
            });
        }
    }

    Ok(())
}

fn require_some_uuid(
    field: &'static str,
    value: Option<Uuid>,
) -> Result<(), PortfolioOperationServiceError> {
    if value.is_none() {
        return Err(PortfolioOperationServiceError::Validation {
            code: missing_field_code(field),
            message: missing_field_message(field),
        });
    }

    Ok(())
}

fn require_none_uuid(
    field: &'static str,
    value: Option<Uuid>,
) -> Result<(), PortfolioOperationServiceError> {
    if value.is_some() {
        return Err(PortfolioOperationServiceError::Validation {
            code: forbidden_field_code(field),
            message: forbidden_field_message(field),
        });
    }

    Ok(())
}

fn require_some_decimal(
    field: &'static str,
    value: Option<&str>,
) -> Result<(), PortfolioOperationServiceError> {
    if value.is_none() {
        return Err(PortfolioOperationServiceError::Validation {
            code: missing_field_code(field),
            message: missing_field_message(field),
        });
    }

    validate_optional_positive_decimal(field, value)
}

fn require_some_i64(
    field: &'static str,
    value: Option<i64>,
) -> Result<(), PortfolioOperationServiceError> {
    if value.is_none() {
        return Err(PortfolioOperationServiceError::Validation {
            code: missing_field_code(field),
            message: missing_field_message(field),
        });
    }

    validate_non_negative_i64(field, value)
}

fn require_none_i64(
    field: &'static str,
    value: Option<i64>,
) -> Result<(), PortfolioOperationServiceError> {
    if value.is_some() {
        return Err(PortfolioOperationServiceError::Validation {
            code: forbidden_field_code(field),
            message: forbidden_field_message(field),
        });
    }

    Ok(())
}

fn require_positive_i64(
    field: &'static str,
    value: Option<i64>,
) -> Result<(), PortfolioOperationServiceError> {
    match value {
        Some(number) if number > 0 => Ok(()),
        Some(_) => Err(PortfolioOperationServiceError::Validation {
            code: invalid_minor_code(field),
            message: positive_minor_message(field),
        }),
        None => Err(PortfolioOperationServiceError::Validation {
            code: missing_field_code(field),
            message: missing_field_message(field),
        }),
    }
}

fn require_positive_cash(value: i64) -> Result<(), PortfolioOperationServiceError> {
    if value <= 0 {
        return Err(PortfolioOperationServiceError::Validation {
            code: "invalid_cash_amount_minor",
            message: "cash_amount_minor must be greater than 0 for this operation type",
        });
    }

    Ok(())
}

fn normalize_required_string(value: &str) -> String {
    value.trim().to_string()
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn default_metadata() -> Value {
    Value::Object(Default::default())
}

fn missing_field_code(field: &'static str) -> &'static str {
    match field {
        "id_asset" => "missing_id_asset",
        "id_related_asset" => "missing_id_related_asset",
        "quantity" => "missing_quantity",
        "related_quantity" => "missing_related_quantity",
        "price_minor" => "missing_price_minor",
        "gross_amount_minor" => "missing_gross_amount_minor",
        _ => "missing_required_field",
    }
}

fn missing_field_message(field: &'static str) -> &'static str {
    match field {
        "id_asset" => "id_asset is required for this operation type",
        "id_related_asset" => "id_related_asset is required for this operation type",
        "quantity" => "quantity is required for this operation type",
        "related_quantity" => "related_quantity is required for this operation type",
        "price_minor" => "price_minor is required for this operation type",
        "gross_amount_minor" => "gross_amount_minor is required for this operation type",
        _ => "a required field is missing",
    }
}

fn forbidden_field_code(field: &'static str) -> &'static str {
    match field {
        "id_asset" => "invalid_id_asset",
        "gross_amount_minor" => "invalid_gross_amount_minor",
        _ => "forbidden_field",
    }
}

fn forbidden_field_message(field: &'static str) -> &'static str {
    match field {
        "id_asset" => "id_asset must be null for this operation type",
        "gross_amount_minor" => "gross_amount_minor must be null for this operation type",
        _ => "field is not allowed for this operation type",
    }
}

fn invalid_minor_code(field: &'static str) -> &'static str {
    match field {
        "price_minor" => "invalid_price_minor",
        "gross_amount_minor" => "invalid_gross_amount_minor",
        "fees_minor" => "invalid_fees_minor",
        "taxes_minor" => "invalid_taxes_minor",
        _ => "invalid_minor_field",
    }
}

fn invalid_minor_message(field: &'static str) -> &'static str {
    match field {
        "price_minor" => "price_minor must be greater than or equal to 0",
        "gross_amount_minor" => "gross_amount_minor must be greater than or equal to 0",
        "fees_minor" => "fees_minor must be greater than or equal to 0",
        "taxes_minor" => "taxes_minor must be greater than or equal to 0",
        _ => "minor monetary fields must be greater than or equal to 0",
    }
}

fn positive_minor_message(field: &'static str) -> &'static str {
    match field {
        "gross_amount_minor" => "gross_amount_minor must be greater than 0 for this operation type",
        _ => "field must be greater than 0",
    }
}

fn invalid_decimal_code(field: &'static str) -> &'static str {
    match field {
        "quantity" => "invalid_quantity",
        "related_quantity" => "invalid_related_quantity",
        "fx_rate_to_portfolio" => "invalid_fx_rate_to_portfolio",
        _ => "invalid_decimal_field",
    }
}

fn invalid_decimal_message(field: &'static str) -> &'static str {
    match field {
        "quantity" => "quantity must be a decimal greater than 0",
        "related_quantity" => "related_quantity must be a decimal greater than 0",
        "fx_rate_to_portfolio" => "fx_rate_to_portfolio must be a decimal greater than 0",
        _ => "decimal field must be greater than 0",
    }
}

fn map_operation_repository_error(
    error: PortfolioOperationRepositoryError,
) -> PortfolioOperationServiceError {
    match error {
        PortfolioOperationRepositoryError::Database(error) => {
            tracing::error!(error = %error, "portfolio operation repository database error");
            PortfolioOperationServiceError::Internal
        }
        PortfolioOperationRepositoryError::InvalidRow => {
            tracing::error!("portfolio operation repository returned an invalid row");
            PortfolioOperationServiceError::Internal
        }
    }
}

fn map_portfolio_repository_error(
    error: PortfolioRepositoryError,
) -> PortfolioOperationServiceError {
    match error {
        PortfolioRepositoryError::Database(error) => {
            tracing::error!(error = %error, "portfolio repository database error");
            PortfolioOperationServiceError::Internal
        }
        PortfolioRepositoryError::InvalidRow => {
            tracing::error!("portfolio repository returned an invalid row");
            PortfolioOperationServiceError::Internal
        }
    }
}

fn map_refresh_repository_error(
    error: PortfolioRefreshRequestRepositoryError,
) -> PortfolioOperationServiceError {
    match error {
        PortfolioRefreshRequestRepositoryError::Database(error) => {
            tracing::error!(error = %error, "portfolio refresh request repository database error");
            PortfolioOperationServiceError::Internal
        }
        PortfolioRefreshRequestRepositoryError::InvalidRow => {
            tracing::error!("portfolio refresh request repository returned an invalid row");
            PortfolioOperationServiceError::Internal
        }
    }
}

fn map_asset_repository_error(error: AssetRepositoryError) -> PortfolioOperationServiceError {
    match error {
        AssetRepositoryError::Database(error) => {
            tracing::error!(error = %error, "asset repository database error");
            PortfolioOperationServiceError::Internal
        }
        AssetRepositoryError::InvalidRow => {
            tracing::error!("asset repository returned an invalid row");
            PortfolioOperationServiceError::Internal
        }
    }
}
