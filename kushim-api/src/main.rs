use axum::{Json, Router, routing::get};
use serde::Serialize;
use std::{env, net::SocketAddr};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppConfig {
    database_url: String,
    redis_url: String,
}

#[derive(Serialize)]
struct StubResponse {
    service: &'static str,
    route: &'static str,
    status: &'static str,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig {
        database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://kushim:kushim@localhost:5432/kushim".to_string()
        }),
        redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
    };

    tracing::info!(
        database_configured = !config.database_url.is_empty(),
        redis_configured = !config.redis_url.is_empty(),
        "loaded service config"
    );

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/portfolios", get(portfolios))
        .route("/v1/transactions", get(transactions))
        .route("/v1/assets", get(assets));

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("PORT must produce a valid socket address");

    tracing::info!(%addr, "starting kushim-api");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind api listener");
    axum::serve(listener, app).await.expect("serve api");
}

async fn health() -> Json<StubResponse> {
    Json(stub("/health"))
}

async fn portfolios() -> Json<StubResponse> {
    Json(stub("/v1/portfolios"))
}

async fn transactions() -> Json<StubResponse> {
    Json(stub("/v1/transactions"))
}

async fn assets() -> Json<StubResponse> {
    Json(stub("/v1/assets"))
}

fn stub(route: &'static str) -> StubResponse {
    StubResponse {
        service: "kushim-api",
        route,
        status: "stub",
    }
}
