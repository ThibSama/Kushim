use crate::domain::asset::{Asset, AssetClass, AssetStatus};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioSummaryStatus {
    Active,
    Empty,
    Archived,
}

impl PortfolioSummaryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Empty => "empty",
            Self::Archived => "archived",
        }
    }
}

impl TryFrom<&str> for PortfolioSummaryStatus {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "active" => Ok(Self::Active),
            "empty" => Ok(Self::Empty),
            "archived" => Ok(Self::Archived),
            _ => Err("unknown portfolio summary status"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioHoldingPositionStatus {
    Open,
    Closed,
}

impl PortfolioHoldingPositionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

impl TryFrom<&str> for PortfolioHoldingPositionStatus {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "open" => Ok(Self::Open),
            "closed" => Ok(Self::Closed),
            _ => Err("unknown portfolio holding status"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PortfolioSummary {
    pub id_portfolio: Uuid,
    pub base_currency: String,
    pub total_value_minor: i64,
    pub cash_balance_minor: i64,
    pub total_invested_minor: i64,
    pub total_pnl_minor: i64,
    pub total_pnl_pct: Option<String>,
    pub portfolio_status: PortfolioSummaryStatus,
    pub is_estimated: bool,
    pub as_of: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct HoldingAssetIdentity {
    pub id_asset: Uuid,
    pub name: String,
    pub ticker: Option<String>,
    pub isin: Option<String>,
    pub exchange: Option<String>,
    pub asset_class: AssetClass,
    pub status: AssetStatus,
    pub native_currency: Option<String>,
}

impl From<HoldingAssetIdentity> for Asset {
    fn from(value: HoldingAssetIdentity) -> Self {
        Self {
            id_asset: value.id_asset,
            name: value.name,
            ticker: value.ticker,
            isin: value.isin,
            exchange: value.exchange,
            symbol: None,
            network: None,
            asset_class: value.asset_class,
            status: value.status,
            native_currency: value.native_currency,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PortfolioHolding {
    pub id_portfolio: Uuid,
    pub id_asset: Uuid,
    pub base_currency: String,
    pub quantity: String,
    pub avg_cost_minor: Option<i64>,
    pub invested_base_minor: i64,
    pub market_value_minor: i64,
    pub pnl_base_minor: i64,
    pub pnl_pct: Option<String>,
    pub weight_pct: Option<String>,
    pub position_status: PortfolioHoldingPositionStatus,
    pub is_estimated: bool,
    pub as_of: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub asset: HoldingAssetIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioHoldingsSort {
    WeightDesc,
    ValueDesc,
    NameAsc,
}

impl PortfolioHoldingsSort {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WeightDesc => "weight_desc",
            Self::ValueDesc => "value_desc",
            Self::NameAsc => "name_asc",
        }
    }
}

impl TryFrom<&str> for PortfolioHoldingsSort {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "weight_desc" => Ok(Self::WeightDesc),
            "value_desc" => Ok(Self::ValueDesc),
            "name_asc" => Ok(Self::NameAsc),
            _ => Err("unknown holdings sort"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PortfolioHoldingsFilters {
    pub asset_class: Option<AssetClass>,
    pub search: Option<String>,
    pub sort: Option<PortfolioHoldingsSort>,
}
