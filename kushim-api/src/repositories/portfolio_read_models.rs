use crate::domain::{
    asset::{AssetClass, AssetStatus},
    portfolio_read_model::{
        HoldingAssetIdentity, PortfolioHolding, PortfolioHoldingPositionStatus,
        PortfolioHoldingsFilters, PortfolioHoldingsSort, PortfolioSummary, PortfolioSummaryStatus,
    },
};
use sqlx::{PgPool, Row};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct PortfolioReadModelRepository {
    pool: PgPool,
}

#[derive(Debug, Error)]
pub enum PortfolioReadModelRepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("invalid read model row")]
    InvalidRow,
}

struct PortfolioSummaryRow {
    id_portfolio: Uuid,
    base_currency: String,
    total_value_minor: i64,
    cash_balance_minor: i64,
    total_invested_minor: i64,
    total_pnl_minor: i64,
    total_pnl_pct: Option<String>,
    portfolio_status: String,
    is_estimated: bool,
    as_of: OffsetDateTime,
    updated_at: OffsetDateTime,
}

struct PortfolioHoldingRow {
    id_portfolio: Uuid,
    id_asset: Uuid,
    base_currency: String,
    quantity: String,
    avg_cost_minor: Option<i64>,
    invested_base_minor: i64,
    market_value_minor: i64,
    pnl_base_minor: i64,
    pnl_pct: Option<String>,
    weight_pct: Option<String>,
    position_status: String,
    is_estimated: bool,
    as_of: OffsetDateTime,
    updated_at: OffsetDateTime,
    asset_name: String,
    asset_ticker: Option<String>,
    asset_isin: Option<String>,
    asset_exchange: Option<String>,
    asset_class: String,
    asset_status: String,
    native_currency: Option<String>,
}

impl TryFrom<PortfolioSummaryRow> for PortfolioSummary {
    type Error = PortfolioReadModelRepositoryError;

    fn try_from(value: PortfolioSummaryRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id_portfolio: value.id_portfolio,
            base_currency: value.base_currency.trim().to_string(),
            total_value_minor: value.total_value_minor,
            cash_balance_minor: value.cash_balance_minor,
            total_invested_minor: value.total_invested_minor,
            total_pnl_minor: value.total_pnl_minor,
            total_pnl_pct: value.total_pnl_pct,
            portfolio_status: PortfolioSummaryStatus::try_from(value.portfolio_status.as_str())
                .map_err(|_| PortfolioReadModelRepositoryError::InvalidRow)?,
            is_estimated: value.is_estimated,
            as_of: value.as_of,
            updated_at: value.updated_at,
        })
    }
}

impl TryFrom<PortfolioHoldingRow> for PortfolioHolding {
    type Error = PortfolioReadModelRepositoryError;

    fn try_from(value: PortfolioHoldingRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id_portfolio: value.id_portfolio,
            id_asset: value.id_asset,
            base_currency: value.base_currency.trim().to_string(),
            quantity: value.quantity,
            avg_cost_minor: value.avg_cost_minor,
            invested_base_minor: value.invested_base_minor,
            market_value_minor: value.market_value_minor,
            pnl_base_minor: value.pnl_base_minor,
            pnl_pct: value.pnl_pct,
            weight_pct: value.weight_pct,
            position_status: PortfolioHoldingPositionStatus::try_from(
                value.position_status.as_str(),
            )
            .map_err(|_| PortfolioReadModelRepositoryError::InvalidRow)?,
            is_estimated: value.is_estimated,
            as_of: value.as_of,
            updated_at: value.updated_at,
            asset: HoldingAssetIdentity {
                id_asset: value.id_asset,
                name: value.asset_name,
                ticker: value.asset_ticker,
                isin: value.asset_isin,
                exchange: value.asset_exchange,
                asset_class: AssetClass::try_from(value.asset_class.as_str())
                    .map_err(|_| PortfolioReadModelRepositoryError::InvalidRow)?,
                status: AssetStatus::try_from(value.asset_status.as_str())
                    .map_err(|_| PortfolioReadModelRepositoryError::InvalidRow)?,
                native_currency: value.native_currency,
            },
        })
    }
}

impl PortfolioReadModelRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_summary_by_portfolio(
        &self,
        id_portfolio: Uuid,
    ) -> Result<Option<PortfolioSummary>, PortfolioReadModelRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id_portfolio,
                base_currency,
                total_value_minor,
                cash_balance_minor,
                total_invested_minor,
                total_pnl_minor,
                total_pnl_pct::text AS total_pnl_pct,
                portfolio_status,
                is_estimated,
                as_of,
                updated_at
            FROM rm_portfolio_summary
            WHERE id_portfolio = $1
            "#,
        )
        .bind(id_portfolio)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| summary_from_row(&row)).transpose()
    }

    pub async fn list_holdings_page(
        &self,
        id_portfolio: Uuid,
        filters: &PortfolioHoldingsFilters,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PortfolioHolding>, PortfolioReadModelRepositoryError> {
        let search_pattern = filters
            .search
            .as_ref()
            .map(|value| format!("%{}%", value.to_lowercase()));

        let order_by = match filters
            .sort
            .clone()
            .unwrap_or(PortfolioHoldingsSort::WeightDesc)
        {
            PortfolioHoldingsSort::WeightDesc => {
                "h.weight_pct DESC NULLS LAST, h.market_value_minor DESC, a.name ASC"
            }
            PortfolioHoldingsSort::ValueDesc => {
                "h.market_value_minor DESC, h.weight_pct DESC NULLS LAST, a.name ASC"
            }
            PortfolioHoldingsSort::NameAsc => {
                "a.name ASC, a.ticker ASC NULLS LAST, a.exchange ASC NULLS LAST"
            }
        };

        let query = format!(
            r#"
            SELECT
                h.id_portfolio,
                h.id_asset,
                h.base_currency,
                h.quantity::text AS quantity,
                h.avg_cost_minor,
                h.invested_base_minor,
                h.market_value_minor,
                h.pnl_base_minor,
                h.pnl_pct::text AS pnl_pct,
                h.weight_pct::text AS weight_pct,
                h.position_status,
                h.is_estimated,
                h.as_of,
                h.updated_at,
                a.name AS asset_name,
                a.ticker AS asset_ticker,
                a.isin AS asset_isin,
                a.exchange AS asset_exchange,
                a.asset_class,
                a.status AS asset_status,
                a.native_currency
            FROM rm_portfolio_holdings h
            INNER JOIN assets a
                ON a.id_asset = h.id_asset
            WHERE h.id_portfolio = $1
              AND ($2::varchar IS NULL OR a.asset_class = $2)
              AND (
                    $3::varchar IS NULL
                    OR lower(a.name) LIKE $3
                    OR lower(COALESCE(a.ticker, '')) LIKE $3
                    OR lower(COALESCE(a.isin, '')) LIKE $3
              )
            ORDER BY {order_by}
            LIMIT $4
            OFFSET $5
            "#
        );

        let rows = sqlx::query(&query)
            .bind(id_portfolio)
            .bind(filters.asset_class.as_ref().map(AssetClass::as_str))
            .bind(search_pattern.as_deref())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter().map(|row| holding_from_row(&row)).collect()
    }
}

fn summary_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<PortfolioSummary, PortfolioReadModelRepositoryError> {
    PortfolioSummaryRow {
        id_portfolio: row.try_get("id_portfolio")?,
        base_currency: row.try_get("base_currency")?,
        total_value_minor: row.try_get("total_value_minor")?,
        cash_balance_minor: row.try_get("cash_balance_minor")?,
        total_invested_minor: row.try_get("total_invested_minor")?,
        total_pnl_minor: row.try_get("total_pnl_minor")?,
        total_pnl_pct: row.try_get("total_pnl_pct")?,
        portfolio_status: row.try_get("portfolio_status")?,
        is_estimated: row.try_get("is_estimated")?,
        as_of: row.try_get("as_of")?,
        updated_at: row.try_get("updated_at")?,
    }
    .try_into()
}

fn holding_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<PortfolioHolding, PortfolioReadModelRepositoryError> {
    PortfolioHoldingRow {
        id_portfolio: row.try_get("id_portfolio")?,
        id_asset: row.try_get("id_asset")?,
        base_currency: row.try_get("base_currency")?,
        quantity: row.try_get("quantity")?,
        avg_cost_minor: row.try_get("avg_cost_minor")?,
        invested_base_minor: row.try_get("invested_base_minor")?,
        market_value_minor: row.try_get("market_value_minor")?,
        pnl_base_minor: row.try_get("pnl_base_minor")?,
        pnl_pct: row.try_get("pnl_pct")?,
        weight_pct: row.try_get("weight_pct")?,
        position_status: row.try_get("position_status")?,
        is_estimated: row.try_get("is_estimated")?,
        as_of: row.try_get("as_of")?,
        updated_at: row.try_get("updated_at")?,
        asset_name: row.try_get("asset_name")?,
        asset_ticker: row.try_get("asset_ticker")?,
        asset_isin: row.try_get("asset_isin")?,
        asset_exchange: row.try_get("asset_exchange")?,
        asset_class: row.try_get("asset_class")?,
        asset_status: row.try_get("asset_status")?,
        native_currency: row.try_get("native_currency")?,
    }
    .try_into()
}
