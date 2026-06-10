pub mod mock;

use crate::domain::{ActiveAsset, CurrentQuote, HistoricalQuote};
use time::Date;

pub trait MarketDataProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn get_quote(&self, asset: &ActiveAsset) -> Option<CurrentQuote>;
    fn get_historical_quote(&self, asset: &ActiveAsset, date: Date) -> Option<HistoricalQuote>;
}
