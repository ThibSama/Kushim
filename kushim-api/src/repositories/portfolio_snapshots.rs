use crate::domain::asset::{AssetClass, AssetStatus};
use crate::domain::portfolio_snapshot::{
    HistoricalSnapshotHoldingFilters, HistoricalSnapshotHoldingsSort, PortfolioDailySnapshot,
    PortfolioDailySnapshotHolding, PortfolioSnapshotSourceType, PortfolioSnapshotsSort,
    SnapshotDailyFilters, SnapshotHoldingAssetIdentity,
};
use sqlx::{PgPool, Row};
use thiserror::Error;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Clone)]
pub struct PortfolioSnapshotRepository {
    pool: PgPool,
}

#[derive(Debug, Error)]
pub enum PortfolioSnapshotRepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("invalid snapshot row")]
    InvalidRow,
}

struct PortfolioDailySnapshotRow {
    id_portfolio_snapshot_daily: Uuid,
    id_portfolio: Uuid,
    snapshot_date: Date,
    base_currency: String,
    cash_balance_minor: i64,
    total_value_minor: i64,
    total_invested_minor: i64,
    total_pnl_minor: i64,
    total_pnl_pct: Option<String>,
    is_estimated: bool,
    source_type: String,
    created_at: OffsetDateTime,
}

struct PortfolioDailySnapshotHoldingRow {
    id_portfolio_holding_snapshot_daily: Uuid,
    id_portfolio_snapshot_daily: Uuid,
    id_asset: Uuid,
    base_currency: String,
    quantity: String,
    avg_cost_minor: Option<i64>,
    invested_minor: i64,
    market_value_minor: i64,
    pnl_minor: i64,
    pnl_pct: Option<String>,
    weight_pct: Option<String>,
    is_estimated: bool,
    created_at: OffsetDateTime,
    asset_name: String,
    asset_ticker: Option<String>,
    asset_isin: Option<String>,
    asset_exchange: Option<String>,
    asset_class: String,
    asset_status: String,
    native_currency: Option<String>,
}

impl TryFrom<PortfolioDailySnapshotRow> for PortfolioDailySnapshot {
    type Error = PortfolioSnapshotRepositoryError;

    fn try_from(value: PortfolioDailySnapshotRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id_portfolio_snapshot_daily: value.id_portfolio_snapshot_daily,
            id_portfolio: value.id_portfolio,
            snapshot_date: value.snapshot_date,
            base_currency: value.base_currency.trim().to_string(),
            cash_balance_minor: value.cash_balance_minor,
            total_value_minor: value.total_value_minor,
            total_invested_minor: value.total_invested_minor,
            total_pnl_minor: value.total_pnl_minor,
            total_pnl_pct: value.total_pnl_pct,
            is_estimated: value.is_estimated,
            source_type: PortfolioSnapshotSourceType::try_from(value.source_type.as_str())
                .map_err(|_| PortfolioSnapshotRepositoryError::InvalidRow)?,
            created_at: value.created_at,
        })
    }
}

impl TryFrom<PortfolioDailySnapshotHoldingRow> for PortfolioDailySnapshotHolding {
    type Error = PortfolioSnapshotRepositoryError;

    fn try_from(value: PortfolioDailySnapshotHoldingRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id_portfolio_holding_snapshot_daily: value.id_portfolio_holding_snapshot_daily,
            id_portfolio_snapshot_daily: value.id_portfolio_snapshot_daily,
            id_asset: value.id_asset,
            base_currency: value.base_currency.trim().to_string(),
            quantity: value.quantity,
            avg_cost_minor: value.avg_cost_minor,
            invested_minor: value.invested_minor,
            market_value_minor: value.market_value_minor,
            pnl_minor: value.pnl_minor,
            pnl_pct: value.pnl_pct,
            weight_pct: value.weight_pct,
            is_estimated: value.is_estimated,
            created_at: value.created_at,
            asset: SnapshotHoldingAssetIdentity {
                id_asset: value.id_asset,
                name: value.asset_name,
                ticker: value.asset_ticker,
                isin: value.asset_isin,
                exchange: value.asset_exchange,
                asset_class: AssetClass::try_from(value.asset_class.as_str())
                    .map_err(|_| PortfolioSnapshotRepositoryError::InvalidRow)?,
                status: AssetStatus::try_from(value.asset_status.as_str())
                    .map_err(|_| PortfolioSnapshotRepositoryError::InvalidRow)?,
                native_currency: value.native_currency,
            },
        })
    }
}

impl PortfolioSnapshotRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_daily_snapshots_page(
        &self,
        id_portfolio: Uuid,
        filters: &SnapshotDailyFilters,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PortfolioDailySnapshot>, PortfolioSnapshotRepositoryError> {
        let order_by = match filters.sort.clone().unwrap_or(PortfolioSnapshotsSort::Asc) {
            PortfolioSnapshotsSort::Asc => "snapshot_date ASC, created_at ASC",
            PortfolioSnapshotsSort::Desc => "snapshot_date DESC, created_at DESC",
        };

        let query = format!(
            r#"
            SELECT
                id_portfolio_snapshot_daily,
                id_portfolio,
                snapshot_date,
                base_currency,
                cash_balance_minor,
                total_value_minor,
                total_invested_minor,
                total_pnl_minor,
                total_pnl_pct::text AS total_pnl_pct,
                is_estimated,
                source_type,
                created_at
            FROM portfolio_snapshots_daily
            WHERE id_portfolio = $1
              AND ($2::date IS NULL OR snapshot_date >= $2)
              AND ($3::date IS NULL OR snapshot_date <= $3)
            ORDER BY {order_by}
            LIMIT $4
            OFFSET $5
            "#
        );

        let rows = sqlx::query(&query)
            .bind(id_portfolio)
            .bind(filters.date_from)
            .bind(filters.date_to)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter()
            .map(|row| snapshot_from_row(&row))
            .collect()
    }

    pub async fn find_daily_snapshot_by_portfolio_and_date(
        &self,
        id_portfolio: Uuid,
        snapshot_date: Date,
    ) -> Result<Option<PortfolioDailySnapshot>, PortfolioSnapshotRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id_portfolio_snapshot_daily,
                id_portfolio,
                snapshot_date,
                base_currency,
                cash_balance_minor,
                total_value_minor,
                total_invested_minor,
                total_pnl_minor,
                total_pnl_pct::text AS total_pnl_pct,
                is_estimated,
                source_type,
                created_at
            FROM portfolio_snapshots_daily
            WHERE id_portfolio = $1
              AND snapshot_date = $2
            "#,
        )
        .bind(id_portfolio)
        .bind(snapshot_date)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| snapshot_from_row(&row)).transpose()
    }

    pub async fn list_snapshot_holdings_page(
        &self,
        id_portfolio_snapshot_daily: Uuid,
        filters: &HistoricalSnapshotHoldingFilters,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PortfolioDailySnapshotHolding>, PortfolioSnapshotRepositoryError> {
        let search_pattern = filters
            .search
            .as_ref()
            .map(|value| format!("%{}%", value.to_lowercase()));

        let order_by = match filters
            .sort
            .clone()
            .unwrap_or(HistoricalSnapshotHoldingsSort::WeightDesc)
        {
            HistoricalSnapshotHoldingsSort::WeightDesc => {
                "h.weight_pct DESC NULLS LAST, h.market_value_minor DESC, a.name ASC"
            }
            HistoricalSnapshotHoldingsSort::ValueDesc => {
                "h.market_value_minor DESC, h.weight_pct DESC NULLS LAST, a.name ASC"
            }
            HistoricalSnapshotHoldingsSort::NameAsc => {
                "a.name ASC, a.ticker ASC NULLS LAST, a.exchange ASC NULLS LAST"
            }
        };

        let query = format!(
            r#"
            SELECT
                h.id_portfolio_holding_snapshot_daily,
                h.id_portfolio_snapshot_daily,
                h.id_asset,
                h.base_currency,
                h.quantity::text AS quantity,
                h.avg_cost_minor,
                h.invested_minor,
                h.market_value_minor,
                h.pnl_minor,
                h.pnl_pct::text AS pnl_pct,
                h.weight_pct::text AS weight_pct,
                h.is_estimated,
                h.created_at,
                a.name AS asset_name,
                a.ticker AS asset_ticker,
                a.isin AS asset_isin,
                a.exchange AS asset_exchange,
                a.asset_class,
                a.status AS asset_status,
                a.native_currency
            FROM portfolio_holding_snapshot_daily h
            INNER JOIN assets a
                ON a.id_asset = h.id_asset
            WHERE h.id_portfolio_snapshot_daily = $1
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
            .bind(id_portfolio_snapshot_daily)
            .bind(filters.asset_class.as_ref().map(AssetClass::as_str))
            .bind(search_pattern.as_deref())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter()
            .map(|row| snapshot_holding_from_row(&row))
            .collect()
    }
}

fn snapshot_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<PortfolioDailySnapshot, PortfolioSnapshotRepositoryError> {
    PortfolioDailySnapshotRow {
        id_portfolio_snapshot_daily: row.try_get("id_portfolio_snapshot_daily")?,
        id_portfolio: row.try_get("id_portfolio")?,
        snapshot_date: row.try_get("snapshot_date")?,
        base_currency: row.try_get("base_currency")?,
        cash_balance_minor: row.try_get("cash_balance_minor")?,
        total_value_minor: row.try_get("total_value_minor")?,
        total_invested_minor: row.try_get("total_invested_minor")?,
        total_pnl_minor: row.try_get("total_pnl_minor")?,
        total_pnl_pct: row.try_get("total_pnl_pct")?,
        is_estimated: row.try_get("is_estimated")?,
        source_type: row.try_get("source_type")?,
        created_at: row.try_get("created_at")?,
    }
    .try_into()
}

fn snapshot_holding_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<PortfolioDailySnapshotHolding, PortfolioSnapshotRepositoryError> {
    PortfolioDailySnapshotHoldingRow {
        id_portfolio_holding_snapshot_daily: row.try_get("id_portfolio_holding_snapshot_daily")?,
        id_portfolio_snapshot_daily: row.try_get("id_portfolio_snapshot_daily")?,
        id_asset: row.try_get("id_asset")?,
        base_currency: row.try_get("base_currency")?,
        quantity: row.try_get("quantity")?,
        avg_cost_minor: row.try_get("avg_cost_minor")?,
        invested_minor: row.try_get("invested_minor")?,
        market_value_minor: row.try_get("market_value_minor")?,
        pnl_minor: row.try_get("pnl_minor")?,
        pnl_pct: row.try_get("pnl_pct")?,
        weight_pct: row.try_get("weight_pct")?,
        is_estimated: row.try_get("is_estimated")?,
        created_at: row.try_get("created_at")?,
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
