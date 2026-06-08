use crate::{
    domain::asset::AssetClass,
    domain::portfolio_snapshot::{
        HistoricalSnapshotHoldingFilters, HistoricalSnapshotHoldingsSort, PortfolioDailySnapshot,
        PortfolioDailySnapshotHolding, PortfolioSnapshotsSort, SnapshotDailyFilters,
    },
    repositories::{
        portfolio_snapshots::{PortfolioSnapshotRepository, PortfolioSnapshotRepositoryError},
        portfolios::{PortfolioRepository, PortfolioRepositoryError},
    },
};
use thiserror::Error;
use time::Date;
use uuid::Uuid;

#[derive(Clone)]
pub struct PortfolioSnapshotService {
    portfolio_repository: PortfolioRepository,
    portfolio_snapshot_repository: PortfolioSnapshotRepository,
}

#[derive(Debug, Clone, Default)]
pub struct ListPortfolioDailySnapshotsInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub date_from: Option<Date>,
    pub date_to: Option<Date>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<PortfolioSnapshotsSort>,
}

#[derive(Debug, Clone)]
pub struct PortfolioSnapshotsPaginationView {
    pub limit: i64,
    pub offset: i64,
    pub returned: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct PortfolioDailySnapshotsView {
    pub data_available: bool,
    pub snapshots: Vec<PortfolioDailySnapshot>,
    pub pagination: PortfolioSnapshotsPaginationView,
}

#[derive(Debug, Clone)]
pub struct GetPortfolioDailySnapshotHoldingsInput {
    pub id_user: Uuid,
    pub id_portfolio: Uuid,
    pub snapshot_date: Date,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<HistoricalSnapshotHoldingsSort>,
    pub asset_class: Option<AssetClass>,
    pub search: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PortfolioDailySnapshotHoldingsView {
    pub data_available: bool,
    pub snapshot: Option<PortfolioDailySnapshot>,
    pub holdings: Vec<PortfolioDailySnapshotHolding>,
    pub pagination: PortfolioSnapshotsPaginationView,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Error)]
pub enum PortfolioSnapshotServiceError {
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

impl PortfolioSnapshotService {
    pub fn new(
        portfolio_repository: PortfolioRepository,
        portfolio_snapshot_repository: PortfolioSnapshotRepository,
    ) -> Self {
        Self {
            portfolio_repository,
            portfolio_snapshot_repository,
        }
    }

    pub async fn list_daily_snapshots(
        &self,
        input: ListPortfolioDailySnapshotsInput,
    ) -> Result<PortfolioDailySnapshotsView, PortfolioSnapshotServiceError> {
        self.ensure_portfolio_owned(input.id_portfolio, input.id_user)
            .await?;

        validate_date_range(input.date_from, input.date_to)?;
        let limit = validate_limit(input.limit)?;
        let offset = validate_offset(input.offset)?;

        let filters = SnapshotDailyFilters {
            date_from: input.date_from,
            date_to: input.date_to,
            sort: Some(input.sort.unwrap_or(PortfolioSnapshotsSort::Asc)),
        };

        let snapshots = self
            .portfolio_snapshot_repository
            .list_daily_snapshots_page(input.id_portfolio, &filters, limit + 1, offset)
            .await
            .map_err(map_snapshot_repository_error)?;

        let has_more = snapshots.len() > limit as usize;
        let snapshots: Vec<_> = snapshots.into_iter().take(limit as usize).collect();
        let returned = snapshots.len();

        Ok(PortfolioDailySnapshotsView {
            data_available: !snapshots.is_empty(),
            snapshots,
            pagination: PortfolioSnapshotsPaginationView {
                limit,
                offset,
                returned,
                has_more,
            },
        })
    }

    pub async fn get_daily_snapshot_holdings(
        &self,
        input: GetPortfolioDailySnapshotHoldingsInput,
    ) -> Result<PortfolioDailySnapshotHoldingsView, PortfolioSnapshotServiceError> {
        self.ensure_portfolio_owned(input.id_portfolio, input.id_user)
            .await?;

        let limit = validate_holdings_limit(input.limit)?;
        let offset = validate_offset(input.offset)?;

        let snapshot = self
            .portfolio_snapshot_repository
            .find_daily_snapshot_by_portfolio_and_date(input.id_portfolio, input.snapshot_date)
            .await
            .map_err(map_snapshot_repository_error)?;

        let Some(snapshot) = snapshot else {
            return Ok(PortfolioDailySnapshotHoldingsView {
                data_available: false,
                snapshot: None,
                holdings: Vec::new(),
                pagination: PortfolioSnapshotsPaginationView {
                    limit,
                    offset,
                    returned: 0,
                    has_more: false,
                },
                reason: Some("snapshot_missing"),
            });
        };

        let filters = HistoricalSnapshotHoldingFilters {
            sort: Some(
                input
                    .sort
                    .unwrap_or(HistoricalSnapshotHoldingsSort::WeightDesc),
            ),
            asset_class: input.asset_class,
            search: input.search,
        };

        let holdings = self
            .portfolio_snapshot_repository
            .list_snapshot_holdings_page(
                snapshot.id_portfolio_snapshot_daily,
                &filters,
                limit + 1,
                offset,
            )
            .await
            .map_err(map_snapshot_repository_error)?;

        let has_more = holdings.len() > limit as usize;
        let holdings: Vec<_> = holdings.into_iter().take(limit as usize).collect();
        let returned = holdings.len();

        Ok(PortfolioDailySnapshotHoldingsView {
            data_available: true,
            snapshot: Some(snapshot),
            holdings,
            pagination: PortfolioSnapshotsPaginationView {
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
    ) -> Result<(), PortfolioSnapshotServiceError> {
        let portfolio = self
            .portfolio_repository
            .find_by_id_and_user(id_portfolio, id_user)
            .await
            .map_err(map_portfolio_repository_error)?;

        if portfolio.is_none() {
            return Err(PortfolioSnapshotServiceError::NotFound);
        }

        Ok(())
    }
}

fn validate_date_range(
    date_from: Option<Date>,
    date_to: Option<Date>,
) -> Result<(), PortfolioSnapshotServiceError> {
    if let (Some(date_from), Some(date_to)) = (date_from, date_to)
        && date_from > date_to
    {
        return Err(PortfolioSnapshotServiceError::Validation {
            code: "invalid_date_range",
            message: "date_from must be less than or equal to date_to",
        });
    }

    Ok(())
}

fn validate_limit(value: Option<i64>) -> Result<i64, PortfolioSnapshotServiceError> {
    let limit = value.unwrap_or(100);
    if !(1..=366).contains(&limit) {
        return Err(PortfolioSnapshotServiceError::Validation {
            code: "invalid_limit",
            message: "limit must be between 1 and 366",
        });
    }

    Ok(limit)
}

fn validate_offset(value: Option<i64>) -> Result<i64, PortfolioSnapshotServiceError> {
    let offset = value.unwrap_or(0);
    if offset < 0 {
        return Err(PortfolioSnapshotServiceError::Validation {
            code: "invalid_offset",
            message: "offset must be greater than or equal to 0",
        });
    }

    Ok(offset)
}

fn validate_holdings_limit(value: Option<i64>) -> Result<i64, PortfolioSnapshotServiceError> {
    let limit = value.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(PortfolioSnapshotServiceError::Validation {
            code: "invalid_limit",
            message: "limit must be between 1 and 100",
        });
    }

    Ok(limit)
}

fn map_portfolio_repository_error(
    error: PortfolioRepositoryError,
) -> PortfolioSnapshotServiceError {
    match error {
        PortfolioRepositoryError::Database(error) => {
            tracing::error!(error = %error, "portfolio repository database error");
            PortfolioSnapshotServiceError::Internal
        }
        PortfolioRepositoryError::InvalidRow => {
            tracing::error!("portfolio repository returned an invalid row");
            PortfolioSnapshotServiceError::Internal
        }
    }
}

fn map_snapshot_repository_error(
    error: PortfolioSnapshotRepositoryError,
) -> PortfolioSnapshotServiceError {
    match error {
        PortfolioSnapshotRepositoryError::Database(error) => {
            tracing::error!(error = %error, "portfolio snapshot repository database error");
            PortfolioSnapshotServiceError::Internal
        }
        PortfolioSnapshotRepositoryError::InvalidRow => {
            tracing::error!("portfolio snapshot repository returned an invalid row");
            PortfolioSnapshotServiceError::Internal
        }
    }
}
