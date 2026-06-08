use crate::domain::asset::{AssetClass, AssetStatus};
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioSnapshotSourceType {
    DailyJob,
    Backfill,
    OnDemand,
}

impl PortfolioSnapshotSourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DailyJob => "daily_job",
            Self::Backfill => "backfill",
            Self::OnDemand => "on_demand",
        }
    }
}

impl TryFrom<&str> for PortfolioSnapshotSourceType {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "daily_job" => Ok(Self::DailyJob),
            "backfill" => Ok(Self::Backfill),
            "on_demand" => Ok(Self::OnDemand),
            _ => Err("unknown portfolio snapshot source type"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PortfolioDailySnapshot {
    pub id_portfolio_snapshot_daily: Uuid,
    pub id_portfolio: Uuid,
    pub snapshot_date: Date,
    pub base_currency: String,
    pub cash_balance_minor: i64,
    pub total_value_minor: i64,
    pub total_invested_minor: i64,
    pub total_pnl_minor: i64,
    pub total_pnl_pct: Option<String>,
    pub is_estimated: bool,
    pub source_type: PortfolioSnapshotSourceType,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioSnapshotsSort {
    Asc,
    Desc,
}

impl PortfolioSnapshotsSort {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

impl TryFrom<&str> for PortfolioSnapshotsSort {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "asc" => Ok(Self::Asc),
            "desc" => Ok(Self::Desc),
            _ => Err("unknown portfolio snapshots sort"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SnapshotDailyFilters {
    pub date_from: Option<Date>,
    pub date_to: Option<Date>,
    pub sort: Option<PortfolioSnapshotsSort>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalSnapshotHoldingsSort {
    WeightDesc,
    ValueDesc,
    NameAsc,
}

impl HistoricalSnapshotHoldingsSort {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WeightDesc => "weight_desc",
            Self::ValueDesc => "value_desc",
            Self::NameAsc => "name_asc",
        }
    }
}

impl TryFrom<&str> for HistoricalSnapshotHoldingsSort {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "weight_desc" => Ok(Self::WeightDesc),
            "value_desc" => Ok(Self::ValueDesc),
            "name_asc" => Ok(Self::NameAsc),
            _ => Err("unknown historical snapshot holdings sort"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HistoricalSnapshotHoldingFilters {
    pub sort: Option<HistoricalSnapshotHoldingsSort>,
    pub asset_class: Option<AssetClass>,
    pub search: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SnapshotHoldingAssetIdentity {
    pub id_asset: Uuid,
    pub name: String,
    pub ticker: Option<String>,
    pub isin: Option<String>,
    pub exchange: Option<String>,
    pub asset_class: AssetClass,
    pub status: AssetStatus,
    pub native_currency: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PortfolioDailySnapshotHolding {
    pub id_portfolio_holding_snapshot_daily: Uuid,
    pub id_portfolio_snapshot_daily: Uuid,
    pub id_asset: Uuid,
    pub base_currency: String,
    pub quantity: String,
    pub avg_cost_minor: Option<i64>,
    pub invested_minor: i64,
    pub market_value_minor: i64,
    pub pnl_minor: i64,
    pub pnl_pct: Option<String>,
    pub weight_pct: Option<String>,
    pub is_estimated: bool,
    pub created_at: OffsetDateTime,
    pub asset: SnapshotHoldingAssetIdentity,
}
