pub mod fx_rate;

use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ActiveAsset {
    pub id_asset: Uuid,
    pub symbol: Option<String>,
    pub ticker: Option<String>,
    pub native_currency: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CurrentQuote {
    pub price_minor: i64,
    pub currency: String,
    pub data_source: String,
    pub as_of: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct HistoricalQuote {
    pub close_minor: i64,
    pub currency: String,
    pub data_source: String,
    pub price_date: Date,
}
