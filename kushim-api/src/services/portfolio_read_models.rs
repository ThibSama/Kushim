use crate::{
    domain::{
        asset::AssetClass,
        portfolio_read_model::{
            PortfolioHolding, PortfolioHoldingsFilters, PortfolioHoldingsSort, PortfolioSummary,
            PortfolioValuationStatus,
        },
    },
    repositories::{
        portfolio_read_models::{PortfolioReadModelRepository, PortfolioReadModelRepositoryError},
        portfolios::{PortfolioRepository, PortfolioRepositoryError},
    },
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone)]
pub struct PortfolioReadModelService {
    portfolio_repository: PortfolioRepository,
    portfolio_read_model_repository: PortfolioReadModelRepository,
}

#[derive(Debug, Clone)]
pub struct GetPortfolioSummaryInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
}

#[derive(Debug, Clone, Default)]
pub struct ListPortfolioHoldingsInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<PortfolioHoldingsSort>,
    pub asset_class: Option<AssetClass>,
    pub search: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PortfolioHoldingsPaginationView {
    pub limit: i64,
    pub offset: i64,
    pub returned: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct PortfolioSummaryView {
    pub data_available: bool,
    pub summary: Option<PortfolioSummary>,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct PortfolioHoldingsView {
    pub data_available: bool,
    pub holdings: Vec<PortfolioHolding>,
    pub pagination: PortfolioHoldingsPaginationView,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Error)]
pub enum PortfolioReadModelServiceError {
    #[error("validation failed")]
    Validation {
        code: &'static str,
        message: &'static str,
    },
    #[error("portfolio not found")]
    NotFound,
    #[error("service failure")]
    Internal,
}

impl PortfolioReadModelService {
    pub fn new(
        portfolio_repository: PortfolioRepository,
        portfolio_read_model_repository: PortfolioReadModelRepository,
    ) -> Self {
        Self {
            portfolio_repository,
            portfolio_read_model_repository,
        }
    }

    pub async fn get_summary(
        &self,
        input: GetPortfolioSummaryInput,
    ) -> Result<PortfolioSummaryView, PortfolioReadModelServiceError> {
        self.ensure_portfolio_owned(input.id_portfolio, input.id_user)
            .await?;

        let summary = self
            .portfolio_read_model_repository
            .find_summary_by_portfolio(input.id_portfolio)
            .await
            .map_err(map_read_model_repository_error)?;

        Ok(match summary {
            Some(mut summary) => {
                // The summary row itself does not carry the market-data join.
                // We compute the breakdown via a dedicated read-only query so
                // the API can expose an objective `valuation_status` derived
                // strictly from `position_status = 'open'` and the presence of
                // a matching row in `asset_market_data`.
                let breakdown = self
                    .portfolio_read_model_repository
                    .valuation_breakdown(input.id_portfolio)
                    .await
                    .map_err(map_read_model_repository_error)?;
                summary.positions_total = breakdown.open_positions;
                summary.positions_valued = breakdown.valued_positions;
                summary.valuation_status = PortfolioValuationStatus::from_counts(
                    breakdown.open_positions,
                    breakdown.valued_positions,
                );
                PortfolioSummaryView {
                    data_available: true,
                    summary: Some(summary),
                    reason: None,
                }
            }
            None => PortfolioSummaryView {
                data_available: false,
                summary: None,
                reason: Some("read_model_missing"),
            },
        })
    }

    pub async fn list_holdings(
        &self,
        input: ListPortfolioHoldingsInput,
    ) -> Result<PortfolioHoldingsView, PortfolioReadModelServiceError> {
        self.ensure_portfolio_owned(input.id_portfolio, input.id_user)
            .await?;

        let limit = validate_limit(input.limit)?;
        let offset = validate_offset(input.offset)?;
        let filters = PortfolioHoldingsFilters {
            asset_class: input.asset_class,
            search: normalize_optional_string(input.search),
            sort: Some(input.sort.unwrap_or(PortfolioHoldingsSort::WeightDesc)),
        };

        let summary_exists = self
            .portfolio_read_model_repository
            .find_summary_by_portfolio(input.id_portfolio)
            .await
            .map_err(map_read_model_repository_error)?
            .is_some();

        if !summary_exists {
            return Ok(PortfolioHoldingsView {
                data_available: false,
                holdings: Vec::new(),
                pagination: PortfolioHoldingsPaginationView {
                    limit,
                    offset,
                    returned: 0,
                    has_more: false,
                },
                reason: Some("read_model_missing"),
            });
        }

        let holdings = self
            .portfolio_read_model_repository
            .list_holdings_page(input.id_portfolio, &filters, limit + 1, offset)
            .await
            .map_err(map_read_model_repository_error)?;

        let has_more = holdings.len() > limit as usize;
        let holdings: Vec<_> = holdings.into_iter().take(limit as usize).collect();
        let returned = holdings.len();

        Ok(PortfolioHoldingsView {
            data_available: true,
            holdings,
            pagination: PortfolioHoldingsPaginationView {
                limit,
                offset,
                returned,
                has_more,
            },
            reason: None,
        })
    }

    async fn ensure_portfolio_owned(
        &self,
        id_portfolio: Uuid,
        id_user: Uuid,
    ) -> Result<(), PortfolioReadModelServiceError> {
        let portfolio = self
            .portfolio_repository
            .find_by_id_and_user(id_portfolio, id_user)
            .await
            .map_err(map_portfolio_repository_error)?;

        if portfolio.is_none() {
            return Err(PortfolioReadModelServiceError::NotFound);
        }

        Ok(())
    }
}

fn validate_limit(value: Option<i64>) -> Result<i64, PortfolioReadModelServiceError> {
    let limit = value.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(PortfolioReadModelServiceError::Validation {
            code: "invalid_limit",
            message: "limit must be between 1 and 100",
        });
    }

    Ok(limit)
}

fn validate_offset(value: Option<i64>) -> Result<i64, PortfolioReadModelServiceError> {
    let offset = value.unwrap_or(0);
    if offset < 0 {
        return Err(PortfolioReadModelServiceError::Validation {
            code: "invalid_offset",
            message: "offset must be greater than or equal to 0",
        });
    }

    Ok(offset)
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

fn map_portfolio_repository_error(
    error: PortfolioRepositoryError,
) -> PortfolioReadModelServiceError {
    match error {
        PortfolioRepositoryError::Database(error) => {
            tracing::error!(error = %error, "portfolio repository database error");
            PortfolioReadModelServiceError::Internal
        }
        PortfolioRepositoryError::InvalidRow => {
            tracing::error!("portfolio repository returned an invalid row");
            PortfolioReadModelServiceError::Internal
        }
    }
}

fn map_read_model_repository_error(
    error: PortfolioReadModelRepositoryError,
) -> PortfolioReadModelServiceError {
    match error {
        PortfolioReadModelRepositoryError::Database(error) => {
            tracing::error!(error = %error, "portfolio read model repository database error");
            PortfolioReadModelServiceError::Internal
        }
        PortfolioReadModelRepositoryError::InvalidRow => {
            tracing::error!("portfolio read model repository returned an invalid row");
            PortfolioReadModelServiceError::Internal
        }
    }
}
