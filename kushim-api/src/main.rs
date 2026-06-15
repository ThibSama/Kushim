use anyhow::Result;
use kushim_api::{
    auth::JwtValidator,
    config::Config,
    db, http,
    repositories::{
        assets::AssetRepository, portfolio_operations::PortfolioOperationRepository,
        portfolio_read_models::PortfolioReadModelRepository,
        portfolio_refresh_requests::PortfolioRefreshRequestRepository,
        portfolio_snapshots::PortfolioSnapshotRepository, portfolios::PortfolioRepository,
    },
    services::{
        assets::AssetService, portfolio_operations::PortfolioOperationService,
        portfolio_read_models::PortfolioReadModelService,
        portfolio_snapshots::PortfolioSnapshotService, portfolios::PortfolioService,
    },
    state::AppState,
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let config = Config::from_env()?;

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| config.rust_log.clone().into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_pool = db::create_pool(&config.database_url).await?;
    db::check_connectivity(&db_pool).await?;
    tracing::info!("PostgreSQL connection established");

    if config.redis_url.is_some() {
        tracing::info!("Redis configuration detected but not used in this implementation pass");
    } else {
        tracing::info!("Redis not configured");
    }

    let portfolio_repository = PortfolioRepository::new(db_pool.clone());
    let asset_service = AssetService::new(AssetRepository::new(db_pool.clone()));
    let portfolio_service = PortfolioService::new(portfolio_repository.clone());
    let portfolio_operation_service = PortfolioOperationService::new(
        AssetRepository::new(db_pool.clone()),
        portfolio_repository,
        PortfolioOperationRepository::new(db_pool.clone()),
        PortfolioRefreshRequestRepository::new(db_pool.clone()),
    );
    let portfolio_read_model_service = PortfolioReadModelService::new(
        PortfolioRepository::new(db_pool.clone()),
        PortfolioReadModelRepository::new(db_pool.clone()),
    );
    let portfolio_snapshot_service = PortfolioSnapshotService::new(
        PortfolioRepository::new(db_pool.clone()),
        PortfolioSnapshotRepository::new(db_pool.clone()),
    );

    let state = AppState {
        db_pool,
        jwt_validator: JwtValidator::new(&config.auth_jwt_secret, config.jwt_issuer.clone()),
        asset_service,
        portfolio_service,
        portfolio_operation_service,
        portfolio_read_model_service,
        portfolio_snapshot_service,
        service_name: "kushim-api",
        service_version: env!("CARGO_PKG_VERSION"),
        routes_version: "api-routes-v1",
        environment: config.environment.clone(),
    };

    let app = http::router_with_cors(state, config.cors_allowed_origins.as_deref());
    let addr = config.socket_addr()?;

    tracing::info!(
        routes = http::ROUTES_DESCRIPTION,
        routes_version = "api-routes-v1",
        "kushim-api routes mounted"
    );
    tracing::info!(%addr, environment = %config.environment, "starting kushim-api");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
