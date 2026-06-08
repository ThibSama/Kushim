use crate::domain::portfolio::{NewPortfolio, Portfolio, PortfolioVisibility};
use sqlx::{PgPool, Row};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct PortfolioRepository {
    pool: PgPool,
}

#[derive(Debug, Error)]
pub enum PortfolioRepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("invalid portfolio row")]
    InvalidRow,
}

struct PortfolioRow {
    id_portfolio: Uuid,
    id_user: Uuid,
    name: String,
    description: Option<String>,
    base_currency: String,
    visibility: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<PortfolioRow> for Portfolio {
    type Error = PortfolioRepositoryError;

    fn try_from(value: PortfolioRow) -> Result<Self, Self::Error> {
        let visibility = PortfolioVisibility::try_from(value.visibility.as_str())
            .map_err(|_| PortfolioRepositoryError::InvalidRow)?;

        Ok(Self {
            id_portfolio: value.id_portfolio,
            id_user: value.id_user,
            name: value.name,
            description: value.description,
            base_currency: value.base_currency.trim().to_string(),
            visibility,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl PortfolioRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        input: &NewPortfolio,
    ) -> Result<Portfolio, PortfolioRepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO portfolios (id_user, name, description, base_currency, visibility)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id_portfolio,
                id_user,
                name,
                description,
                base_currency,
                visibility,
                created_at,
                updated_at
            "#,
        )
        .bind(input.id_user)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.base_currency)
        .bind(input.visibility.as_str())
        .fetch_one(&self.pool)
        .await?;

        portfolio_from_row(&row)
    }

    pub async fn list_by_user(
        &self,
        id_user: Uuid,
    ) -> Result<Vec<Portfolio>, PortfolioRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id_portfolio,
                id_user,
                name,
                description,
                base_currency,
                visibility,
                created_at,
                updated_at
            FROM portfolios
            WHERE id_user = $1
              AND deleted_at IS NULL
            ORDER BY created_at DESC, id_portfolio DESC
            "#,
        )
        .bind(id_user)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| portfolio_from_row(&row))
            .collect()
    }

    pub async fn find_by_id_and_user(
        &self,
        id_portfolio: Uuid,
        id_user: Uuid,
    ) -> Result<Option<Portfolio>, PortfolioRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id_portfolio,
                id_user,
                name,
                description,
                base_currency,
                visibility,
                created_at,
                updated_at
            FROM portfolios
            WHERE id_portfolio = $1
              AND id_user = $2
              AND deleted_at IS NULL
            "#,
        )
        .bind(id_portfolio)
        .bind(id_user)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| portfolio_from_row(&row)).transpose()
    }
}

fn portfolio_from_row(row: &sqlx::postgres::PgRow) -> Result<Portfolio, PortfolioRepositoryError> {
    PortfolioRow {
        id_portfolio: row.try_get("id_portfolio")?,
        id_user: row.try_get("id_user")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        base_currency: row.try_get("base_currency")?,
        visibility: row.try_get("visibility")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    }
    .try_into()
}
