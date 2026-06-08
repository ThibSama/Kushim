use crate::{
    domain::portfolio_state::{
        AssetMarketValue, PortfolioDefinition, PortfolioOperationEvent, PortfolioState,
        RebuiltPortfolioState,
    },
    errors::WorkerError,
};
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn rebuild_portfolio_state(
    portfolio: PortfolioDefinition,
    operations: &[PortfolioOperationEvent],
    market_data: &HashMap<Uuid, AssetMarketValue>,
    as_of: OffsetDateTime,
) -> Result<RebuiltPortfolioState, WorkerError> {
    let mut state = PortfolioState::new(portfolio);
    for operation in operations {
        state.apply(operation)?;
    }
    state.finalize(market_data, as_of)
}
