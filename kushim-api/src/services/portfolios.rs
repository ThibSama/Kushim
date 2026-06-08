use crate::{
    domain::portfolio::{NewPortfolio, Portfolio, PortfolioVisibility},
    repositories::portfolios::{PortfolioRepository, PortfolioRepositoryError},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone)]
pub struct PortfolioService {
    repository: PortfolioRepository,
}

#[derive(Debug, Clone)]
pub struct CreatePortfolioInput {
    pub id_user: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub base_currency: String,
    pub visibility: PortfolioVisibility,
}

#[derive(Debug, Error)]
pub enum PortfolioServiceError {
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

impl PortfolioService {
    pub fn new(repository: PortfolioRepository) -> Self {
        Self { repository }
    }

    pub async fn create_portfolio(
        &self,
        input: CreatePortfolioInput,
    ) -> Result<Portfolio, PortfolioServiceError> {
        validate_name(&input.name)?;
        validate_base_currency(&input.base_currency)?;

        let portfolio = self
            .repository
            .create(&NewPortfolio {
                id_user: input.id_user,
                name: input.name.trim().to_string(),
                description: input.description.map(|value| value.trim().to_string()),
                base_currency: input.base_currency,
                visibility: input.visibility,
            })
            .await
            .map_err(map_repository_error)?;

        Ok(portfolio)
    }

    pub async fn list_portfolios(
        &self,
        id_user: Uuid,
    ) -> Result<Vec<Portfolio>, PortfolioServiceError> {
        self.repository
            .list_by_user(id_user)
            .await
            .map_err(map_repository_error)
    }

    pub async fn get_portfolio(
        &self,
        id_portfolio: Uuid,
        id_user: Uuid,
    ) -> Result<Portfolio, PortfolioServiceError> {
        self.repository
            .find_by_id_and_user(id_portfolio, id_user)
            .await
            .map_err(map_repository_error)?
            .ok_or(PortfolioServiceError::NotFound)
    }
}

fn validate_name(name: &str) -> Result<(), PortfolioServiceError> {
    if name.trim().is_empty() {
        return Err(PortfolioServiceError::Validation {
            code: "invalid_portfolio_name",
            message: "portfolio name must not be blank",
        });
    }

    if name.chars().count() > 50 {
        return Err(PortfolioServiceError::Validation {
            code: "invalid_portfolio_name",
            message: "portfolio name must be at most 50 characters",
        });
    }

    Ok(())
}

fn validate_base_currency(base_currency: &str) -> Result<(), PortfolioServiceError> {
    let is_valid = base_currency.len() == 3
        && base_currency
            .chars()
            .all(|character| character.is_ascii_uppercase());

    if !is_valid {
        return Err(PortfolioServiceError::Validation {
            code: "invalid_base_currency",
            message: "base_currency must be exactly 3 uppercase letters",
        });
    }

    Ok(())
}

fn map_repository_error(error: PortfolioRepositoryError) -> PortfolioServiceError {
    match error {
        PortfolioRepositoryError::Database(error) => {
            tracing::error!(error = %error, "portfolio repository database error");
            PortfolioServiceError::Internal
        }
        PortfolioRepositoryError::InvalidRow => {
            tracing::error!("portfolio repository returned an invalid row");
            PortfolioServiceError::Internal
        }
    }
}
