use crate::{
    domain::asset::{AssetIdentity, AssetStatus},
    domain::currency::{self, CurrencyValidationError},
    domain::portfolio::Portfolio,
    domain::portfolio_operation::{
        NewPortfolioOperation, OperationStatus, OperationType, PortfolioOperation,
        PortfolioOperationFilters, UpdatePortfolioOperation,
    },
    domain::portfolio_refresh_request::PortfolioRefreshRequest,
    repositories::{
        assets::{AssetRepository, AssetRepositoryError},
        portfolio_operation_idempotency::{
            IdempotencyRecord, IdempotencyRepositoryError, IdempotencyRequestKind,
            PortfolioOperationIdempotencyRepository,
        },
        portfolio_operations::{
            IdempotencyWriteOutcome, PortfolioOperationRepository,
            PortfolioOperationRepositoryError,
        },
        portfolio_refresh_requests::{
            PortfolioRefreshRequestRepository, PortfolioRefreshRequestRepositoryError,
        },
        portfolios::{PortfolioRepository, PortfolioRepositoryError},
    },
    services::operation_fingerprint::build_fingerprint,
};
use bigdecimal::BigDecimal;
use serde_json::Value;
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

/// User-facing view of a portfolio operation: the ledger row plus the compact
/// identity of its primary and related assets, when present.
///
/// The ledger entity (`PortfolioOperation`) remains the immutable source of
/// truth. This view is built by the service layer using a single batch
/// asset-identity lookup, and is what every HTTP response now serializes.
#[derive(Debug, Clone)]
pub struct PortfolioOperationView {
    pub operation: PortfolioOperation,
    pub asset: Option<AssetIdentity>,
    pub related_asset: Option<AssetIdentity>,
}

#[derive(Clone)]
pub struct PortfolioOperationService {
    asset_repository: AssetRepository,
    portfolio_repository: PortfolioRepository,
    portfolio_operation_repository: PortfolioOperationRepository,
    portfolio_refresh_request_repository: PortfolioRefreshRequestRepository,
    idempotency_repository: PortfolioOperationIdempotencyRepository,
}

/// Result of a write that may have enqueued a portfolio refresh request.
/// `refresh_request` is `Some` exactly when the write produced a `posted`
/// operation (direct posted creation, posting a pending operation, or posted
/// correction creation). Pending creations carry `None`.
#[derive(Debug, Clone)]
pub struct OperationWriteOutcome {
    pub operation: PortfolioOperationView,
    pub refresh_request: Option<PortfolioRefreshRequest>,
}

/// P3 idempotent write outcome. `replayed` is `true` when the response was
/// served from a previously committed idempotency record (no new operation
/// was inserted). The HTTP layer surfaces this through the
/// `Idempotency-Replayed` response header.
#[derive(Debug, Clone)]
pub struct IdempotentOperationWriteOutcome {
    pub outcome: OperationWriteOutcome,
    pub replayed: bool,
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
    pub operation: PortfolioOperationView,
    pub corrections: Vec<PortfolioOperationView>,
}

#[derive(Debug, Clone)]
pub struct PortfolioOperationAuditView {
    pub operation: PortfolioOperationView,
    pub corrected_operation: Option<PortfolioOperationView>,
    pub corrections: Vec<PortfolioOperationView>,
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
    pub operation: PortfolioOperationView,
    pub corrections: Vec<PortfolioOperationView>,
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
    /// Semantic-layer rejection mapped to HTTP 422. Used by the P1 currency
    /// contract for `unsupported_currency` and `unsupported_cross_currency`.
    #[error("unprocessable entity")]
    UnprocessableEntity {
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
        idempotency_repository: PortfolioOperationIdempotencyRepository,
    ) -> Self {
        Self {
            asset_repository,
            portfolio_repository,
            portfolio_operation_repository,
            portfolio_refresh_request_repository,
            idempotency_repository,
        }
    }

    pub async fn create_operation(
        &self,
        input: CreatePortfolioOperationInput,
    ) -> Result<OperationWriteOutcome, PortfolioOperationServiceError> {
        let portfolio = self
            .assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        let operation_status = input
            .operation_status
            .clone()
            .unwrap_or(OperationStatus::Pending);
        if operation_status == OperationStatus::Cancelled {
            return Err(PortfolioOperationServiceError::Validation {
                code: "invalid_operation_status",
                message: "operation creation does not accept cancelled status",
            });
        }

        // Normalize the currency to its catalogue-canonical form before the
        // candidate is built, so the row persisted (and later replayed by the
        // worker) always uses the canonical uppercase code.
        let canonical_currency = validate_currency(&input.currency)?;

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
            currency: canonical_currency.to_string(),
            fx_rate_to_portfolio: normalize_optional_string(input.fx_rate_to_portfolio),
            external_provider: normalize_optional_string(input.external_provider),
            external_reference: normalize_optional_string(input.external_reference),
            id_corrected_operation: input.id_corrected_operation,
            notes: normalize_optional_string(input.notes),
            metadata: input.metadata.unwrap_or_else(default_metadata),
        };

        validate_operation_payload(&new_operation)?;
        self.validate_asset_references(&new_operation).await?;

        // P1 cross-currency contract: a direct posted creation with a
        // non-zero monetary leg in a foreign currency must carry a valid
        // positive `fx_rate_to_portfolio`. Reject BEFORE the insert so neither
        // the operation row nor the refresh request is created.
        if new_operation.operation_status == OperationStatus::Posted {
            validate_cross_currency_posting(
                &new_operation.currency,
                &portfolio.base_currency,
                new_operation.cash_amount_minor,
                new_operation.fx_rate_to_portfolio.as_deref(),
            )?;
        }

        // Atomicity invariant: resolve asset identities BEFORE the mutation.
        // If this read fails, the API returns an error without ever inserting
        // a row. After the commit only the in-memory `build_view` runs, so a
        // committed mutation always produces a 2xx response — no ambiguous
        // write that would invite a duplicate-action retry.
        let identities = self
            .prefetch_identities(
                new_operation
                    .id_asset
                    .into_iter()
                    .chain(new_operation.id_related_asset),
            )
            .await?;

        let (operation, refresh_request) = self
            .portfolio_operation_repository
            .create_with_optional_refresh(&new_operation)
            .await
            .map_err(map_operation_repository_error)?;

        Ok(OperationWriteOutcome {
            operation: build_view(operation, &identities),
            refresh_request,
        })
    }

    /// P3 idempotent variant of `create_operation`. Ordering invariant:
    ///
    ///   1. authenticate (handler) + ownership check;
    ///   2. canonicalize the request — pure transforms only (status default,
    ///      canonical currency, RFC3339 parse, normalized strings, default
    ///      metadata);
    ///   3. build the canonical fingerprint;
    ///   4. look up `(id_user, idempotency_key)` and replay on exact match
    ///      or 409 on any mismatch;
    ///   5. ONLY when no record exists: run mutable-state validations
    ///      (`validate_operation_payload`, asset activity, cross-currency,
    ///      identity prefetch) and then execute the transactional claim +
    ///      operation + refresh + finalize.
    ///
    /// This ordering guarantees an exact retry of a previously successful
    /// request returns the same operation/refresh identity even if the
    /// referenced asset has since been deactivated, the FX rate has gone
    /// stale, or the refresh request has moved to `processing`/`completed`.
    /// A replay never re-runs business validation that depends on mutable
    /// state. Concurrent identical requests collapse into ONE committed row:
    /// the loser sees `RaceLost`, re-reads the winner's record, and replays.
    pub async fn create_operation_idempotent(
        &self,
        input: CreatePortfolioOperationInput,
        idempotency_key: Uuid,
    ) -> Result<IdempotentOperationWriteOutcome, PortfolioOperationServiceError> {
        let portfolio = self
            .assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        // Step 2 — canonicalize. Pure transforms; no mutable-state checks.
        let candidate = self.build_create_candidate(&input)?;

        // Step 3 — fingerprint.
        let fingerprint = build_fingerprint(
            input.id_user,
            IdempotencyRequestKind::CreateOperation,
            None,
            &candidate,
        );

        // Step 4 — replay lookup. A successful prior request bypasses every
        // mutable-state validation below: if the asset was active when the
        // original write committed, the replay must still succeed even if
        // it is inactive today.
        if let Some(existing) = self
            .idempotency_repository
            .find_by_user_and_key(input.id_user, idempotency_key)
            .await
            .map_err(map_idempotency_repository_error)?
        {
            return self
                .replay_existing(
                    existing,
                    IdempotencyRequestKind::CreateOperation,
                    None,
                    &fingerprint,
                    input.id_portfolio,
                    input.id_user,
                )
                .await;
        }

        // Step 5 — fresh request. Mutable-state validations run ONLY here.
        // A failure rejects BEFORE the transactional claim, so the key is
        // not consumed and a corrected retry may reuse the same UUID.
        validate_operation_payload(&candidate)?;
        self.validate_asset_references(&candidate).await?;
        if candidate.operation_status == OperationStatus::Posted {
            validate_cross_currency_posting(
                &candidate.currency,
                &portfolio.base_currency,
                candidate.cash_amount_minor,
                candidate.fx_rate_to_portfolio.as_deref(),
            )?;
        }

        // Identities must be resolved BEFORE the transaction starts, same as
        // the non-idempotent path: a SELECT failure after the commit would
        // turn a committed mutation into an HTTP 500 and invite a duplicate.
        let identities = self
            .prefetch_identities(
                candidate
                    .id_asset
                    .into_iter()
                    .chain(candidate.id_related_asset),
            )
            .await?;

        let write_outcome = self
            .portfolio_operation_repository
            .create_with_optional_refresh_and_idempotency(
                &candidate,
                input.id_user,
                idempotency_key,
                IdempotencyRequestKind::CreateOperation,
                None,
                &fingerprint,
            )
            .await
            .map_err(map_operation_repository_error)?;

        match write_outcome {
            IdempotencyWriteOutcome::Created {
                operation,
                refresh_request,
            } => Ok(IdempotentOperationWriteOutcome {
                outcome: OperationWriteOutcome {
                    operation: build_view(operation, &identities),
                    refresh_request,
                },
                replayed: false,
            }),
            IdempotencyWriteOutcome::RaceLost => {
                // Another transaction committed first. Re-read and replay.
                let existing = self
                    .idempotency_repository
                    .find_by_user_and_key(input.id_user, idempotency_key)
                    .await
                    .map_err(map_idempotency_repository_error)?
                    .ok_or(PortfolioOperationServiceError::Internal)?;
                self.replay_existing(
                    existing,
                    IdempotencyRequestKind::CreateOperation,
                    None,
                    &fingerprint,
                    input.id_portfolio,
                    input.id_user,
                )
                .await
            }
        }
    }

    /// P3 idempotent variant of `create_correction`. Same ordering as
    /// `create_operation_idempotent`; the only additions are loading the
    /// original operation (needed for the canonical currency fallback and
    /// for the fingerprint's `id_corrected_operation` field) BEFORE the
    /// lookup, and deferring the "original must be posted" check to the
    /// fresh-request path so a replay still works if the original somehow
    /// changed state (it can't via the API — posted is DB-immutable — but
    /// the contract is symmetric with `create_operation_idempotent`).
    pub async fn create_correction_idempotent(
        &self,
        input: CreatePortfolioOperationCorrectionInput,
        idempotency_key: Uuid,
    ) -> Result<IdempotentOperationWriteOutcome, PortfolioOperationServiceError> {
        let portfolio = self
            .assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        // Loading the original is mandatory for canonicalization (default
        // currency fallback and `id_corrected_operation` in the fingerprint).
        let original_operation = self
            .portfolio_operation_repository
            .find_by_id_and_portfolio(input.id_portfolio_operation, input.id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })?;

        // Step 2 — canonicalize (pure transforms; no posted/asset checks).
        let candidate = self.build_correction_candidate(&original_operation, &input)?;

        // Step 3 — fingerprint.
        let fingerprint = build_fingerprint(
            input.id_user,
            IdempotencyRequestKind::CreateCorrection,
            Some(original_operation.id_portfolio_operation),
            &candidate,
        );

        // Step 4 — replay lookup BEFORE any mutable-state check.
        if let Some(existing) = self
            .idempotency_repository
            .find_by_user_and_key(input.id_user, idempotency_key)
            .await
            .map_err(map_idempotency_repository_error)?
        {
            return self
                .replay_existing(
                    existing,
                    IdempotencyRequestKind::CreateCorrection,
                    Some(original_operation.id_portfolio_operation),
                    &fingerprint,
                    input.id_portfolio,
                    input.id_user,
                )
                .await;
        }

        // Step 5 — fresh request. Posted-original requirement + payload +
        // asset activity + cross-currency. Failures here do not consume the
        // idempotency key.
        require_correctable_original(&original_operation)?;
        validate_correction_payload(&candidate)?;
        validate_operation_payload(&candidate)?;
        self.validate_asset_references(&candidate).await?;
        if candidate.operation_status == OperationStatus::Posted {
            validate_cross_currency_posting(
                &candidate.currency,
                &portfolio.base_currency,
                candidate.cash_amount_minor,
                candidate.fx_rate_to_portfolio.as_deref(),
            )?;
        }

        let identities = self
            .prefetch_identities(
                candidate
                    .id_asset
                    .into_iter()
                    .chain(candidate.id_related_asset),
            )
            .await?;

        let write_outcome = self
            .portfolio_operation_repository
            .create_with_optional_refresh_and_idempotency(
                &candidate,
                input.id_user,
                idempotency_key,
                IdempotencyRequestKind::CreateCorrection,
                Some(original_operation.id_portfolio_operation),
                &fingerprint,
            )
            .await
            .map_err(map_operation_repository_error)?;

        match write_outcome {
            IdempotencyWriteOutcome::Created {
                operation,
                refresh_request,
            } => Ok(IdempotentOperationWriteOutcome {
                outcome: OperationWriteOutcome {
                    operation: build_view(operation, &identities),
                    refresh_request,
                },
                replayed: false,
            }),
            IdempotencyWriteOutcome::RaceLost => {
                let existing = self
                    .idempotency_repository
                    .find_by_user_and_key(input.id_user, idempotency_key)
                    .await
                    .map_err(map_idempotency_repository_error)?
                    .ok_or(PortfolioOperationServiceError::Internal)?;
                self.replay_existing(
                    existing,
                    IdempotencyRequestKind::CreateCorrection,
                    Some(original_operation.id_portfolio_operation),
                    &fingerprint,
                    input.id_portfolio,
                    input.id_user,
                )
                .await
            }
        }
    }

    /// Build the `NewPortfolioOperation` that would have been persisted by
    /// `create_operation`, including all defaults/normalization, so the
    /// idempotency fingerprint reflects what the row would look like once
    /// committed.
    ///
    /// Pure transforms only: status default, canonical currency lookup,
    /// RFC3339 parsing, normalized strings, default metadata. No mutable-
    /// state checks (asset activity, cross-currency, etc.) — those belong
    /// to the post-lookup fresh-request path so an exact replay never
    /// fails because of unrelated drift since the original write committed.
    /// The status-cancelled and unsupported-currency rejections stay here
    /// because they reject the REQUEST shape, never an in-DB state.
    fn build_create_candidate(
        &self,
        input: &CreatePortfolioOperationInput,
    ) -> Result<NewPortfolioOperation, PortfolioOperationServiceError> {
        let operation_status = input
            .operation_status
            .clone()
            .unwrap_or(OperationStatus::Pending);
        if operation_status == OperationStatus::Cancelled {
            return Err(PortfolioOperationServiceError::Validation {
                code: "invalid_operation_status",
                message: "operation creation does not accept cancelled status",
            });
        }
        let canonical_currency = validate_currency(&input.currency)?;
        Ok(NewPortfolioOperation {
            id_portfolio: input.id_portfolio,
            id_asset: input.id_asset,
            id_related_asset: input.id_related_asset,
            operation_type: input.operation_type.clone(),
            operation_status,
            executed_at: parse_datetime(&input.executed_at)?,
            effective_at: parse_optional_datetime(input.effective_at.as_deref())?,
            quantity: normalize_optional_string(input.quantity.clone()),
            related_quantity: normalize_optional_string(input.related_quantity.clone()),
            price_minor: input.price_minor,
            gross_amount_minor: input.gross_amount_minor,
            fees_minor: input.fees_minor,
            taxes_minor: input.taxes_minor,
            cash_amount_minor: input.cash_amount_minor.unwrap_or(0),
            currency: canonical_currency.to_string(),
            fx_rate_to_portfolio: normalize_optional_string(input.fx_rate_to_portfolio.clone()),
            external_provider: normalize_optional_string(input.external_provider.clone()),
            external_reference: normalize_optional_string(input.external_reference.clone()),
            id_corrected_operation: input.id_corrected_operation,
            notes: normalize_optional_string(input.notes.clone()),
            metadata: input.metadata.clone().unwrap_or_else(default_metadata),
        })
    }

    /// Pure correction-candidate builder. Mirrors `build_create_candidate`:
    /// it never inspects the original's mutable state (status, asset
    /// activity, etc.) — those checks live in
    /// `require_correctable_original` and run only on the fresh-request
    /// path so a replay survives any drift.
    fn build_correction_candidate(
        &self,
        original_operation: &PortfolioOperation,
        input: &CreatePortfolioOperationCorrectionInput,
    ) -> Result<NewPortfolioOperation, PortfolioOperationServiceError> {
        let operation_status = input
            .operation_status
            .clone()
            .unwrap_or(OperationStatus::Pending);
        if operation_status == OperationStatus::Cancelled {
            return Err(PortfolioOperationServiceError::Validation {
                code: "invalid_operation_status",
                message: "correction creation does not accept cancelled status",
            });
        }

        Ok(NewPortfolioOperation {
            id_portfolio: input.id_portfolio,
            id_asset: input.id_asset,
            id_related_asset: input.id_related_asset,
            operation_type: OperationType::Adjustment,
            operation_status,
            executed_at: parse_datetime(&input.executed_at)?,
            effective_at: parse_optional_datetime(input.effective_at.as_deref())?,
            quantity: normalize_optional_string(input.quantity.clone()),
            related_quantity: normalize_optional_string(input.related_quantity.clone()),
            price_minor: input.price_minor,
            gross_amount_minor: input.gross_amount_minor,
            fees_minor: input.fees_minor,
            taxes_minor: input.taxes_minor,
            cash_amount_minor: input.cash_amount_minor.unwrap_or(0),
            currency: match &input.currency {
                Some(value) => validate_currency(value)?.to_string(),
                None => original_operation.currency.clone(),
            },
            fx_rate_to_portfolio: normalize_optional_string(input.fx_rate_to_portfolio.clone()),
            external_provider: normalize_optional_string(input.external_provider.clone()),
            external_reference: normalize_optional_string(input.external_reference.clone()),
            id_corrected_operation: Some(original_operation.id_portfolio_operation),
            notes: normalize_optional_string(input.notes.clone()),
            metadata: input.metadata.clone().unwrap_or_else(default_metadata),
        })
    }

    /// Replay a previously committed idempotency record. Validates that the
    /// new request matches the original (kind, correction link, fingerprint)
    /// before loading and returning the original operation/refresh identity.
    async fn replay_existing(
        &self,
        record: IdempotencyRecord,
        request_kind: IdempotencyRequestKind,
        id_corrected_operation: Option<Uuid>,
        new_fingerprint: &serde_json::Value,
        id_portfolio: Uuid,
        id_user: Uuid,
    ) -> Result<IdempotentOperationWriteOutcome, PortfolioOperationServiceError> {
        // User scoping is enforced at lookup time, but a row whose recorded
        // portfolio differs from the request must still be a conflict — same
        // key cannot be reused across portfolios for a different write.
        if record.request_kind != request_kind
            || record.id_corrected_operation != id_corrected_operation
            || record.id_portfolio != id_portfolio
            || record.id_user != id_user
            || &record.request_fingerprint != new_fingerprint
        {
            return Err(PortfolioOperationServiceError::Conflict {
                code: "idempotency_key_conflict",
                message: "Idempotency-Key was reused with a different request",
            });
        }

        let id_operation = record
            .id_portfolio_operation
            .ok_or(PortfolioOperationServiceError::Internal)?;

        let operation = self
            .portfolio_operation_repository
            .find_by_id_and_portfolio(id_operation, record.id_portfolio)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or_else(|| {
                tracing::error!(
                    %id_operation,
                    id_portfolio = %record.id_portfolio,
                    "idempotency replay: recorded operation no longer exists"
                );
                PortfolioOperationServiceError::Internal
            })?;

        let refresh = match record.id_portfolio_refresh_request {
            Some(id_refresh) => Some(
                self.portfolio_refresh_request_repository
                    .find_by_id_and_portfolio(id_refresh, record.id_portfolio)
                    .await
                    .map_err(map_refresh_repository_error)?
                    .ok_or_else(|| {
                        tracing::error!(
                            %id_refresh,
                            id_portfolio = %record.id_portfolio,
                            "idempotency replay: recorded refresh request no longer exists"
                        );
                        PortfolioOperationServiceError::Internal
                    })?,
            ),
            None => None,
        };

        let mut views = self.enrich_many(vec![operation]).await?;
        let view = views.remove(0);

        Ok(IdempotentOperationWriteOutcome {
            outcome: OperationWriteOutcome {
                operation: view,
                refresh_request: refresh,
            },
            replayed: true,
        })
    }

    pub async fn list_operations(
        &self,
        input: ListPortfolioOperationsInput,
    ) -> Result<Vec<PortfolioOperationView>, PortfolioOperationServiceError> {
        self.assert_owned_portfolio(input.id_portfolio, input.id_user)
            .await?;

        let operations = self
            .portfolio_operation_repository
            .list_by_portfolio(
                input.id_portfolio,
                &PortfolioOperationFilters {
                    operation_status: input.operation_status,
                    operation_type: input.operation_type,
                    id_asset: input.id_asset,
                },
            )
            .await
            .map_err(map_operation_repository_error)?;

        self.enrich_many(operations).await
    }

    pub async fn get_operation(
        &self,
        id_user: Uuid,
        id_portfolio: Uuid,
        id_portfolio_operation: Uuid,
    ) -> Result<PortfolioOperationView, PortfolioOperationServiceError> {
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

        let mut views = self.enrich_many(vec![operation]).await?;
        Ok(views.remove(0))
    }

    pub async fn update_operation(
        &self,
        input: UpdatePortfolioOperationInput,
    ) -> Result<PortfolioOperationView, PortfolioOperationServiceError> {
        let _portfolio = self
            .assert_owned_portfolio(input.id_portfolio, input.id_user)
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
                Some(value) => validate_currency(&value)?.to_string(),
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

        // Pre-mutation identity prefetch — see invariant on `create_operation`.
        let identities = self
            .prefetch_identities(
                candidate
                    .id_asset
                    .into_iter()
                    .chain(candidate.id_related_asset),
            )
            .await?;

        let updated = self
            .portfolio_operation_repository
            .update(input.id_portfolio_operation, input.id_portfolio, &update)
            .await
            .map_err(map_operation_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "operation_not_found",
                message: "portfolio operation was not found",
            })?;

        Ok(build_view(updated, &identities))
    }

    pub async fn cancel_operation(
        &self,
        input: CancelPortfolioOperationInput,
    ) -> Result<PortfolioOperationView, PortfolioOperationServiceError> {
        let _portfolio = self
            .assert_owned_portfolio(input.id_portfolio, input.id_user)
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

        // Pre-mutation identity prefetch — see invariant on `create_operation`.
        // Cancel cannot change `id_asset` / `id_related_asset`, so the ids on
        // `existing` are exactly the ids of the row after the mutation.
        let identities = self
            .prefetch_identities(operation_asset_ids(&existing))
            .await?;

        let updated = match existing.operation_status {
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
                })?,
            OperationStatus::Cancelled => existing,
            OperationStatus::Posted => {
                return Err(PortfolioOperationServiceError::Conflict {
                    code: "posted_operation_immutable",
                    message: "posted portfolio operations cannot be cancelled",
                });
            }
        };

        Ok(build_view(updated, &identities))
    }

    pub async fn create_correction(
        &self,
        input: CreatePortfolioOperationCorrectionInput,
    ) -> Result<OperationWriteOutcome, PortfolioOperationServiceError> {
        let portfolio = self
            .assert_owned_portfolio(input.id_portfolio, input.id_user)
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

        let operation_status = input
            .operation_status
            .clone()
            .unwrap_or(OperationStatus::Pending);
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
            currency: match input.currency {
                Some(value) => validate_currency(&value)?.to_string(),
                None => original_operation.currency.clone(),
            },
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

        // P1 cross-currency contract for posted corrections.
        if new_operation.operation_status == OperationStatus::Posted {
            validate_cross_currency_posting(
                &new_operation.currency,
                &portfolio.base_currency,
                new_operation.cash_amount_minor,
                new_operation.fx_rate_to_portfolio.as_deref(),
            )?;
        }

        // Pre-mutation identity prefetch — see invariant on `create_operation`.
        let identities = self
            .prefetch_identities(
                new_operation
                    .id_asset
                    .into_iter()
                    .chain(new_operation.id_related_asset),
            )
            .await?;

        let (operation, refresh_request) = self
            .portfolio_operation_repository
            .create_with_optional_refresh(&new_operation)
            .await
            .map_err(map_operation_repository_error)?;

        Ok(OperationWriteOutcome {
            operation: build_view(operation, &identities),
            refresh_request,
        })
    }

    pub async fn post_operation(
        &self,
        input: PostPortfolioOperationInput,
    ) -> Result<OperationWriteOutcome, PortfolioOperationServiceError> {
        let portfolio = self
            .assert_owned_portfolio(input.id_portfolio, input.id_user)
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

        // P1 cross-currency contract: transitioning a pending row to posted
        // must satisfy the FX contract atomically — if it does not, the row
        // stays pending and no refresh request is enqueued.
        validate_cross_currency_posting(
            &candidate.currency,
            &portfolio.base_currency,
            candidate.cash_amount_minor,
            candidate.fx_rate_to_portfolio.as_deref(),
        )?;

        // Pre-mutation identity prefetch — see invariant on `create_operation`.
        // Posting cannot change `id_asset` / `id_related_asset`, so the ids on
        // `existing` are exactly the ids of the row after the status flip.
        let identities = self
            .prefetch_identities(operation_asset_ids(&existing))
            .await?;

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
            operation: build_view(operation, &identities),
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

        let bundle: Vec<PortfolioOperation> =
            std::iter::once(operation).chain(corrections).collect();
        let enriched = self.enrich_many(bundle).await?;
        let mut iter = enriched.into_iter();
        let operation = iter
            .next()
            .expect("primary operation must be retained after enrichment");
        let corrections = iter.collect();

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

        let mut bundle: Vec<PortfolioOperation> = Vec::with_capacity(2 + corrections.len());
        bundle.push(operation);
        let has_corrected = corrected_operation.is_some();
        if let Some(corrected) = corrected_operation {
            bundle.push(corrected);
        }
        bundle.extend(corrections);
        let bundle = self.enrich_many(bundle).await?;
        let mut iter = bundle.into_iter();
        let operation = iter
            .next()
            .expect("primary operation must be retained after enrichment");
        let corrected_operation = if has_corrected { iter.next() } else { None };
        let corrections = iter.collect();

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

        // Batch-enrich primary operations and corrections in a SINGLE asset
        // identity lookup, then reassemble the timeline items. This keeps the
        // query count bounded (one operation list, one corrections list, one
        // asset identity lookup) regardless of page size.
        let returned = primary_operations.len();
        let mut flat: Vec<PortfolioOperation> = Vec::with_capacity(
            primary_operations.len()
                + corrections_by_original
                    .values()
                    .map(Vec::len)
                    .sum::<usize>(),
        );
        let mut shape: Vec<(Uuid, usize)> = Vec::with_capacity(primary_operations.len());
        for primary in &primary_operations {
            let corrections = corrections_by_original
                .remove(&primary.id_portfolio_operation)
                .unwrap_or_default();
            shape.push((primary.id_portfolio_operation, corrections.len()));
            flat.push(primary.clone());
            flat.extend(corrections);
        }

        let enriched = self.enrich_many(flat).await?;
        let mut iter = enriched.into_iter();
        let items = shape
            .into_iter()
            .map(|(_id, count)| {
                let operation = iter
                    .next()
                    .expect("primary operation must be present after enrichment");
                let corrections = (&mut iter).take(count).collect();
                PortfolioOperationAuditTimelineItemView {
                    operation,
                    corrections,
                }
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

    /// Batch-enrich a collection of operations with compact asset identities
    /// in a single database round trip, regardless of how many operations the
    /// slice contains. Distinct primary AND related asset ids are deduplicated
    /// before the lookup; an empty input skips the database entirely.
    ///
    /// Operations that reference an asset id which no longer resolves (corrupt
    /// or legacy row) keep `id_asset` / `id_related_asset` on the underlying
    /// operation but receive `asset = None` / `related_asset = None` so the
    /// frontend can apply its defensive fallback without crashing the list.
    ///
    /// **Read-path only.** This helper performs the asset SELECT *after*
    /// it has the operations in hand. Write paths must never call it, or a
    /// transient SELECT failure could turn a committed mutation into an HTTP
    /// 500 (an "ambiguous write" that invites a duplicate retry). Writes use
    /// `prefetch_identities` + `build_view` instead — see those helpers.
    pub(crate) async fn enrich_many(
        &self,
        operations: Vec<PortfolioOperation>,
    ) -> Result<Vec<PortfolioOperationView>, PortfolioOperationServiceError> {
        let by_id = self
            .prefetch_identities(operations.iter().flat_map(operation_asset_ids))
            .await?;

        Ok(operations
            .into_iter()
            .map(|op| build_view(op, &by_id))
            .collect())
    }

    /// Resolve the compact identity for a known set of asset ids in a single
    /// batch SELECT, deduplicating before the round trip and returning an
    /// empty map if no id is supplied (no database call). Used by all
    /// **write** paths *before* mutating the database, so the post-mutation
    /// response can be assembled purely in-memory.
    ///
    /// This is the structural invariant that makes write responses
    /// non-ambiguous: if the asset SELECT fails, it fails *before* the
    /// `INSERT/UPDATE`. After the commit, no fallible lookup can run, so a
    /// successful commit always produces a `2xx` response.
    pub(crate) async fn prefetch_identities(
        &self,
        ids: impl IntoIterator<Item = Uuid>,
    ) -> Result<HashMap<Uuid, AssetIdentity>, PortfolioOperationServiceError> {
        let mut dedup: Vec<Uuid> = Vec::new();
        for id in ids {
            if !dedup.contains(&id) {
                dedup.push(id);
            }
        }

        let identities = self
            .asset_repository
            .list_identities_by_ids(&dedup)
            .await
            .map_err(map_asset_repository_error)?;
        Ok(identities
            .into_iter()
            .map(|identity| (identity.id_asset, identity))
            .collect())
    }

    /// Loads the portfolio owned by `id_user` or returns `NotFound` (mapped
    /// to 404 by the HTTP layer, preserving cross-user isolation). The
    /// returned `Portfolio` is reused by callers that need `base_currency`
    /// for cross-currency validation without performing a second query.
    async fn assert_owned_portfolio(
        &self,
        id_portfolio: Uuid,
        id_user: Uuid,
    ) -> Result<Portfolio, PortfolioOperationServiceError> {
        self.portfolio_repository
            .find_by_id_and_user(id_portfolio, id_user)
            .await
            .map_err(map_portfolio_repository_error)?
            .ok_or(PortfolioOperationServiceError::NotFound {
                code: "portfolio_not_found",
                message: "portfolio was not found",
            })
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

/// Yields the asset ids referenced by an operation (primary then related),
/// skipping `None`. Stable iteration order keeps the deduplicated batch list
/// deterministic for tests.
fn operation_asset_ids(op: &PortfolioOperation) -> impl Iterator<Item = Uuid> {
    op.id_asset.into_iter().chain(op.id_related_asset)
}

/// In-memory assembly of a `PortfolioOperationView` from a `PortfolioOperation`
/// and a previously-fetched identity map. Pure function — no I/O, never fails.
/// Write paths use this *after* a successful commit so the response can be
/// built without any further fallible database access.
fn build_view(
    operation: PortfolioOperation,
    identities: &HashMap<Uuid, AssetIdentity>,
) -> PortfolioOperationView {
    PortfolioOperationView {
        asset: operation
            .id_asset
            .and_then(|id| identities.get(&id).cloned()),
        related_asset: operation
            .id_related_asset
            .and_then(|id| identities.get(&id).cloned()),
        operation,
    }
}

fn validate_operation_payload(
    operation: &NewPortfolioOperation,
) -> Result<(), PortfolioOperationServiceError> {
    // The candidate was built from a string that already went through
    // `validate_currency` at the constructor (so it is canonical and part of
    // the catalogue). Calling `validate_currency` again here is defensive: it
    // ensures unit tests that construct a candidate directly can still rely
    // on payload validation rejecting bad currency codes.
    let _ = validate_currency(&operation.currency)?;
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

/// Validates `value` against the canonical currency catalogue. Format
/// failures stay on the existing `invalid_currency`/400 path; unknown
/// three-letter codes map to `unsupported_currency`/422 so the frontend can
/// distinguish a schema error from a value not part of the catalogue.
///
/// Returns the canonical uppercase code on success so callers can use the
/// catalogue-canonical form for cross-currency comparison.
fn validate_currency(value: &str) -> Result<&'static str, PortfolioOperationServiceError> {
    match currency::normalize_and_validate(value) {
        Ok(canonical) => Ok(canonical),
        Err(CurrencyValidationError::Empty | CurrencyValidationError::InvalidFormat) => {
            Err(PortfolioOperationServiceError::Validation {
                code: "invalid_currency",
                message: "currency must be exactly 3 uppercase letters",
            })
        }
        Err(CurrencyValidationError::Unsupported) => {
            Err(PortfolioOperationServiceError::UnprocessableEntity {
                code: "unsupported_currency",
                message: "currency is not part of the supported currency catalogue",
            })
        }
    }
}

/// Enforces the cross-currency posting contract. When the operation about to
/// be posted has a non-zero monetary leg (`cash_amount_minor != 0`) AND its
/// currency differs from the portfolio's base currency, a positive
/// `fx_rate_to_portfolio` must already be present on the row.
///
/// Same-currency operations never require an FX rate. Zero-cash corporate
/// actions (split, spin_off, symbol_change with `cash_amount_minor == 0`) never
/// require an FX rate either — the worker has no monetary amount to convert,
/// so the rule is grounded in the replay contract rather than the type name.
///
/// `fx_rate_to_portfolio` is otherwise validated as a positive decimal by
/// `validate_optional_positive_decimal`; that check still runs separately.
fn validate_cross_currency_posting(
    operation_currency: &str,
    portfolio_base_currency: &str,
    cash_amount_minor: i64,
    fx_rate_to_portfolio: Option<&str>,
) -> Result<(), PortfolioOperationServiceError> {
    if operation_currency == portfolio_base_currency {
        return Ok(());
    }

    if cash_amount_minor == 0 {
        return Ok(());
    }

    let has_positive_rate = fx_rate_to_portfolio
        .map(|value| {
            BigDecimal::from_str(value)
                .map(|parsed| parsed.sign() == bigdecimal::num_bigint::Sign::Plus)
                .unwrap_or(false)
        })
        .unwrap_or(false);

    if !has_positive_rate {
        return Err(PortfolioOperationServiceError::UnprocessableEntity {
            code: "unsupported_cross_currency",
            message: "fx_rate_to_portfolio is required when operation currency differs from the portfolio base currency",
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

/// Mutable-state requirement for the correction create path: the original
/// operation must be `posted`. Pulled out of `build_correction_candidate`
/// so it only runs on the fresh-request path; replays of a successful
/// correction must never re-check this (the original was posted at write
/// time, that is enough).
fn require_correctable_original(
    original: &PortfolioOperation,
) -> Result<(), PortfolioOperationServiceError> {
    match original.operation_status {
        OperationStatus::Posted => Ok(()),
        OperationStatus::Pending => Err(PortfolioOperationServiceError::Conflict {
            code: "correction_requires_posted_operation",
            message: "only posted portfolio operations can be corrected",
        }),
        OperationStatus::Cancelled => Err(PortfolioOperationServiceError::Conflict {
            code: "correction_requires_posted_operation",
            message: "cancelled portfolio operations cannot be corrected",
        }),
    }
}

fn map_idempotency_repository_error(
    error: IdempotencyRepositoryError,
) -> PortfolioOperationServiceError {
    match error {
        IdempotencyRepositoryError::Database(error) => {
            tracing::error!(error = %error, "operation idempotency repository database error");
            PortfolioOperationServiceError::Internal
        }
        IdempotencyRepositoryError::InvalidRow => {
            tracing::error!("operation idempotency repository returned an invalid row");
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
