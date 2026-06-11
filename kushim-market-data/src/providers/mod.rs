pub mod finnhub;
pub mod mock;

use crate::domain::{ActiveAsset, CurrentQuote, HistoricalQuote};
use crate::errors::MarketDataError;
use std::future::Future;
use time::Date;

pub trait MarketDataProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn get_quote(
        &self,
        asset: &ActiveAsset,
    ) -> impl Future<Output = Result<Option<CurrentQuote>, MarketDataError>> + Send;
    fn get_historical_quote(
        &self,
        asset: &ActiveAsset,
        date: Date,
    ) -> impl Future<Output = Result<Option<HistoricalQuote>, MarketDataError>> + Send;
}
