use axum::{Json, Router, routing::post};
use serde::Serialize;
use std::{env, net::SocketAddr};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

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

    let app = Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/forgot-password", post(forgot_password));

    let port = env::var("PORT").unwrap_or_else(|_| "3002".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("PORT must produce a valid socket address");

    tracing::info!(%addr, "starting kushim-auth-api");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind auth api listener");
    axum::serve(listener, app).await.expect("serve auth api");
}

async fn login() -> Json<StubResponse> {
    Json(stub("/login"))
}

async fn register() -> Json<StubResponse> {
    Json(stub("/register"))
}

async fn forgot_password() -> Json<StubResponse> {
    Json(stub("/forgot-password"))
}

fn stub(route: &'static str) -> StubResponse {
    StubResponse {
        service: "kushim-auth-api",
        route,
        status: "stub",
    }
}
