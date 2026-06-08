use crate::{
    auth::JwtValidator,
    services::{
        assets::AssetService, portfolio_operations::PortfolioOperationService,
        portfolio_read_models::PortfolioReadModelService,
        portfolio_snapshots::PortfolioSnapshotService, portfolios::PortfolioService,
    },
};
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub jwt_validator: JwtValidator,
    pub asset_service: AssetService,
    pub portfolio_service: PortfolioService,
    pub portfolio_operation_service: PortfolioOperationService,
    pub portfolio_read_model_service: PortfolioReadModelService,
    pub portfolio_snapshot_service: PortfolioSnapshotService,
    pub service_name: &'static str,
    pub service_version: &'static str,
    pub routes_version: &'static str,
    pub environment: String,
}
