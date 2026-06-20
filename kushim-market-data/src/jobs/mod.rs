pub mod fill_missing_fx_history_cache;
pub mod fill_missing_price_history_cache;
pub mod noop;
pub mod refresh_current_market_data;

use crate::{errors::MarketDataError, state::AppState};

pub trait Job: Send + Sync {
    fn name(&self) -> &'static str;
    fn run(
        &self,
        state: &AppState,
    ) -> impl std::future::Future<Output = Result<(), MarketDataError>> + Send;
}
