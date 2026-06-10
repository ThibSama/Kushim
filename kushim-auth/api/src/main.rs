use anyhow::Result;
use kushim_auth_api::{
    config::Config,
    db, http,
    repositories::{
        recovery_phrases::RecoveryPhraseRepository, revoked_tokens::RevokedTokenRepository,
        roles::RoleRepository, users::UserRepository,
    },
    services::{
        auth::AuthService, password::PasswordService, rate_limit::RateLimitService,
        recovery::RecoveryService, token::TokenService,
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

    let rate_limiter = if config.rate_limit_enabled {
        let rate_limiter = RateLimitService::new(
            config
                .redis_url
                .as_deref()
                .expect("redis url should be validated when rate limiting is enabled"),
        )?;
        rate_limiter.check_health().await?;
        tracing::info!("Redis connection established for rate limiting");
        Some(rate_limiter)
    } else {
        tracing::info!("Rate limiting disabled");
        None
    };

    let auth_service = AuthService::new(
        RoleRepository::new(db_pool.clone()),
        UserRepository::new(db_pool.clone()),
        RecoveryPhraseRepository::new(db_pool.clone()),
        RevokedTokenRepository::new(db_pool.clone()),
        PasswordService::new(),
        RecoveryService::new(),
        TokenService::new(
            &config.auth_jwt_secret,
            config.jwt_issuer.clone(),
            config.access_token_ttl_seconds,
            config.refresh_token_ttl_seconds,
        ),
    );

    let state = AppState {
        db_pool,
        auth_service,
        rate_limiter,
        rate_limit_enabled: config.rate_limit_enabled,
        service_name: "kushim-auth",
        service_version: env!("CARGO_PKG_VERSION"),
        routes_version: "auth-routes-v1",
        environment: config.environment.clone(),
    };

    let app = http::router_with_cors(state, config.cors_allowed_origin.as_deref());
    let addr = config.socket_addr()?;

    tracing::info!(
        routes = "/health, /ready, /auth/signup, /auth/login, /auth/refresh, /auth/logout, /auth/me, /auth/recovery/setup, /auth/recovery/reset-password",
        routes_version = "auth-routes-v1",
        "kushim-auth routes mounted"
    );
    tracing::info!(%addr, environment = %config.environment, "starting kushim-auth-api");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
