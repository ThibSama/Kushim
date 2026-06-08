use crate::{
    domain::asset::{AssetClass, AssetDetails, AssetSearchFilters, AssetStatus},
    repositories::assets::{AssetRepository, AssetRepositoryError},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone)]
pub struct AssetService {
    asset_repository: AssetRepository,
}

#[derive(Debug, Clone, Default)]
pub struct ListAssetsInput {
    pub search: Option<String>,
    pub asset_class: Option<AssetClass>,
    pub ticker: Option<String>,
    pub isin: Option<String>,
    pub exchange: Option<String>,
    pub status: Option<AssetStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AssetPaginationView {
    pub limit: i64,
    pub offset: i64,
    pub returned: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct ListAssetsView {
    pub assets: Vec<AssetDetails>,
    pub pagination: AssetPaginationView,
}

#[derive(Debug, Error)]
pub enum AssetServiceError {
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
    #[error("service failure")]
    Internal,
}

impl AssetService {
    pub fn new(asset_repository: AssetRepository) -> Self {
        Self { asset_repository }
    }

    pub async fn list_assets(
        &self,
        input: ListAssetsInput,
    ) -> Result<ListAssetsView, AssetServiceError> {
        let limit = validate_limit(input.limit)?;
        let offset = validate_offset(input.offset)?;

        let filters = AssetSearchFilters {
            search: normalize_optional_string(input.search),
            asset_class: input.asset_class,
            ticker: normalize_optional_string(input.ticker),
            isin: normalize_optional_string(input.isin).map(|value| value.to_uppercase()),
            exchange: normalize_optional_string(input.exchange),
            status: Some(input.status.unwrap_or(AssetStatus::Active)),
        };

        let assets = self
            .asset_repository
            .list_assets_page(&filters, limit + 1, offset)
            .await
            .map_err(map_repository_error)?;

        let has_more = assets.len() > limit as usize;
        let mut assets: Vec<_> = assets.into_iter().take(limit as usize).collect();

        for asset in &mut assets {
            asset.aliases.clear();
        }

        let returned = assets.len();

        Ok(ListAssetsView {
            assets,
            pagination: AssetPaginationView {
                limit,
                offset,
                returned,
                has_more,
            },
        })
    }

    pub async fn get_asset(&self, id_asset: Uuid) -> Result<AssetDetails, AssetServiceError> {
        let mut asset = self
            .asset_repository
            .find_by_id(id_asset)
            .await
            .map_err(map_repository_error)?
            .ok_or(AssetServiceError::NotFound {
                code: "asset_not_found",
                message: "asset was not found",
            })?;

        asset.aliases = self
            .asset_repository
            .list_aliases_for_asset(id_asset)
            .await
            .map_err(map_repository_error)?;

        Ok(asset)
    }
}

fn validate_limit(value: Option<i64>) -> Result<i64, AssetServiceError> {
    let limit = value.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(AssetServiceError::Validation {
            code: "invalid_limit",
            message: "limit must be between 1 and 100",
        });
    }

    Ok(limit)
}

fn validate_offset(value: Option<i64>) -> Result<i64, AssetServiceError> {
    let offset = value.unwrap_or(0);
    if offset < 0 {
        return Err(AssetServiceError::Validation {
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

fn map_repository_error(error: AssetRepositoryError) -> AssetServiceError {
    match error {
        AssetRepositoryError::Database(error) => {
            tracing::error!(error = %error, "asset repository database error");
            AssetServiceError::Internal
        }
        AssetRepositoryError::InvalidRow => {
            tracing::error!("asset repository returned an invalid row");
            AssetServiceError::Internal
        }
    }
}
