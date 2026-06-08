use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetClass {
    Equity,
    Etf,
    Fund,
    Bond,
    Crypto,
    Commodity,
    Cash,
    Forex,
    Index,
    RealEstate,
    PrivateEquity,
    Derivative,
    Other,
}

impl AssetClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Equity => "equity",
            Self::Etf => "etf",
            Self::Fund => "fund",
            Self::Bond => "bond",
            Self::Crypto => "crypto",
            Self::Commodity => "commodity",
            Self::Cash => "cash",
            Self::Forex => "forex",
            Self::Index => "index",
            Self::RealEstate => "real_estate",
            Self::PrivateEquity => "private_equity",
            Self::Derivative => "derivative",
            Self::Other => "other",
        }
    }
}

impl TryFrom<&str> for AssetClass {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "equity" => Ok(Self::Equity),
            "etf" => Ok(Self::Etf),
            "fund" => Ok(Self::Fund),
            "bond" => Ok(Self::Bond),
            "crypto" => Ok(Self::Crypto),
            "commodity" => Ok(Self::Commodity),
            "cash" => Ok(Self::Cash),
            "forex" => Ok(Self::Forex),
            "index" => Ok(Self::Index),
            "real_estate" => Ok(Self::RealEstate),
            "private_equity" => Ok(Self::PrivateEquity),
            "derivative" => Ok(Self::Derivative),
            "other" => Ok(Self::Other),
            _ => Err("unknown asset class"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetStatus {
    Active,
    Inactive,
    Delisted,
    Merged,
}

impl AssetStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Delisted => "delisted",
            Self::Merged => "merged",
        }
    }
}

impl TryFrom<&str> for AssetStatus {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "delisted" => Ok(Self::Delisted),
            "merged" => Ok(Self::Merged),
            _ => Err("unknown asset status"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub id_asset: Uuid,
    pub name: String,
    pub ticker: Option<String>,
    pub isin: Option<String>,
    pub exchange: Option<String>,
    pub symbol: Option<String>,
    pub network: Option<String>,
    pub asset_class: AssetClass,
    pub status: AssetStatus,
    pub native_currency: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct AssetValidationInfo {
    pub status: AssetStatus,
}

#[derive(Debug, Clone)]
pub struct AssetMetadata {
    pub country: Option<String>,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
    pub description: Option<String>,
    pub provider: Option<String>,
    pub provider_asset_id: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub last_synced_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct AssetMarketData {
    pub price_minor: i64,
    pub currency: String,
    pub market_cap_minor: Option<i64>,
    pub volume_24h_minor: Option<i64>,
    pub change_24h_pct: Option<String>,
    pub change_7d_pct: Option<String>,
    pub change_30d_pct: Option<String>,
    pub data_source: Option<String>,
    pub source_asset_id: Option<String>,
    pub as_of: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct AssetAlias {
    pub alias: String,
    pub alias_type: Option<String>,
    pub source: Option<String>,
    pub valid_from: Option<Date>,
    pub valid_to: Option<Date>,
}

#[derive(Debug, Clone)]
pub struct AssetDetails {
    pub asset: Asset,
    pub metadata: Option<AssetMetadata>,
    pub market_data: Option<AssetMarketData>,
    pub aliases: Vec<AssetAlias>,
}

#[derive(Debug, Clone, Default)]
pub struct AssetSearchFilters {
    pub search: Option<String>,
    pub asset_class: Option<AssetClass>,
    pub ticker: Option<String>,
    pub isin: Option<String>,
    pub exchange: Option<String>,
    pub status: Option<AssetStatus>,
}
