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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioValuationStatus {
    /// Every open position has a market-data row in `asset_market_data`.
    Complete,
    /// At least one open position has market data and at least one does not.
    Partial,
    /// At least one open position exists, none of them have market data.
    Unavailable,
    /// The portfolio currently has no open positions.
    Empty,
}

impl PortfolioValuationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
            Self::Empty => "empty",
        }
    }

    /// Derive the aggregate valuation status from objective counts collected at
    /// the database layer. `open_positions` is the number of open holdings for
    /// the portfolio; `valued_positions` is the subset of those that join to a
    /// row in `asset_market_data`. This mapping uses no time-based heuristics.
    pub fn from_counts(open_positions: i64, valued_positions: i64) -> Self {
        if open_positions <= 0 {
            return Self::Empty;
        }
        if valued_positions <= 0 {
            return Self::Unavailable;
        }
        if valued_positions >= open_positions {
            Self::Complete
        } else {
            Self::Partial
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
    pub valuation_status: PortfolioValuationStatus,
    pub positions_total: i64,
    pub positions_valued: i64,
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

/// Per-holding valuation provenance. **All fields come from
/// `rm_portfolio_holdings`** — `kushim-api` no longer joins
/// `asset_market_data` at read time. This guarantees the displayed provenance
/// always describes the exact market-data version the worker used to compute
/// `market_value_minor`. Any newer quote written into `asset_market_data`
/// after the rebuild remains invisible until the next worker rebuild.
///
/// Combinations follow the database CHECK constraint
/// `chk_rm_portfolio_holdings_provenance_combination`:
///
/// | Source                     | Status                 | Numeric fields |
/// | -------------------------- | ---------------------- | -------------- |
/// | `market_data`              | `available`            | populated      |
/// | `invested_cost_fallback`   | `missing`              | all NULL       |
/// | `invested_cost_fallback`   | `unsupported_currency` | populated      |
/// | `None` (legacy row)        | `None`                 | all NULL       |
#[derive(Debug, Clone)]
pub struct HoldingMarketDataQuality {
    /// True only when the holding's `market_value_minor` was actually
    /// computed from a compatible live market-data row.
    pub available: bool,
    /// Stable source code: `market_data` | `invested_cost_fallback`. `None`
    /// for legacy rows persisted before the migration — surfaced to the API
    /// as `unavailable_reason = "valuation_provenance_missing"`.
    pub valuation_source: Option<&'static str>,
    /// Stable status: `available` (in the `market_data` case) or
    /// `unavailable` (for missing, unsupported-currency, or legacy rows).
    pub status: &'static str,
    /// Reason code when `status = unavailable`. Currently one of:
    /// `market_data_missing`, `unsupported_market_data_currency`,
    /// `valuation_provenance_missing`. None when `status = available`.
    pub unavailable_reason: Option<&'static str>,
    /// Exact price the worker consumed (or rejected, in the
    /// unsupported-currency case). None when no market-data row existed at
    /// rebuild time.
    pub price_minor: Option<i64>,
    /// Currency of that price.
    pub currency: Option<String>,
    /// Provider identifier (e.g. `"test-static"`, `"finnhub"`). Optional even
    /// when the row exists because the upstream `asset_market_data.data_source`
    /// is nullable.
    pub provider: Option<String>,
    /// Market-quote timestamp reported by the provider
    /// (`asset_market_data.as_of` captured at rebuild time).
    pub market_data_as_of: Option<OffsetDateTime>,
    /// Wall-clock time at which the `asset_market_data` row was last written
    /// (`asset_market_data.updated_at` captured at rebuild time). This is NOT
    /// a fetch timestamp — see
    /// `documentation/architecture/market-data-quality-contract.md`.
    pub record_updated_at: Option<OffsetDateTime>,
}

impl HoldingMarketDataQuality {
    /// `valuation_source = market_data`, `market_data_status = available`.
    pub fn available(
        provider: Option<String>,
        price_minor: i64,
        currency: String,
        market_data_as_of: OffsetDateTime,
        record_updated_at: OffsetDateTime,
    ) -> Self {
        Self {
            available: true,
            valuation_source: Some("market_data"),
            status: "available",
            unavailable_reason: None,
            price_minor: Some(price_minor),
            currency: Some(currency),
            provider,
            market_data_as_of: Some(market_data_as_of),
            record_updated_at: Some(record_updated_at),
        }
    }

    /// `valuation_source = invested_cost_fallback`,
    /// `market_data_status = missing` — no market-data row existed.
    pub fn missing() -> Self {
        Self {
            available: false,
            valuation_source: Some("invested_cost_fallback"),
            status: "unavailable",
            unavailable_reason: Some("market_data_missing"),
            price_minor: None,
            currency: None,
            provider: None,
            market_data_as_of: None,
            record_updated_at: None,
        }
    }

    /// `valuation_source = invested_cost_fallback`,
    /// `market_data_status = unsupported_currency` — a row existed but its
    /// currency does not match the holding's base; provenance is preserved
    /// for transparency.
    pub fn unsupported_currency(
        provider: Option<String>,
        price_minor: i64,
        currency: String,
        market_data_as_of: OffsetDateTime,
        record_updated_at: OffsetDateTime,
    ) -> Self {
        Self {
            available: false,
            valuation_source: Some("invested_cost_fallback"),
            status: "unavailable",
            unavailable_reason: Some("unsupported_market_data_currency"),
            price_minor: Some(price_minor),
            currency: Some(currency),
            provider,
            market_data_as_of: Some(market_data_as_of),
            record_updated_at: Some(record_updated_at),
        }
    }

    /// Legacy `rm_portfolio_holdings` row created before the migration. No
    /// provenance was ever persisted; the worker must rebuild the read model
    /// before the API can return accurate provenance.
    pub fn legacy_provenance_missing() -> Self {
        Self {
            available: false,
            valuation_source: None,
            status: "unavailable",
            unavailable_reason: Some("valuation_provenance_missing"),
            price_minor: None,
            currency: None,
            provider: None,
            market_data_as_of: None,
            record_updated_at: None,
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
    pub market_data: HoldingMarketDataQuality,
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
