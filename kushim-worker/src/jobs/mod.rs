pub mod backfill_daily_snapshots;
pub mod generate_daily_snapshots;
pub mod noop;
pub mod rebuild_current_read_models;
pub mod refresh_current_portfolio_state;

use crate::{errors::WorkerError, state::AppState};
use async_trait::async_trait;

#[async_trait]
pub trait Job: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run(&self, state: &AppState) -> Result<(), WorkerError>;
}
