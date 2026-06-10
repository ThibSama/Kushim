pub mod auth;
pub mod extractors;
pub mod health;
pub mod me;
pub mod middleware;

use crate::state::AppState;
use axum::{
    Router,
    http::{HeaderValue, Method},
    middleware::from_fn,
    routing::{get, post},
};
use tower_http::cors::CorsLayer;

pub fn router(state: AppState) -> Router {
    router_with_cors(state, None)
}

pub fn router_with_cors(state: AppState, cors_allowed_origin: Option<&str>) -> Router {
    let cors_layer = build_cors_layer(cors_allowed_origin);

    Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .nest(
            "/auth",
            Router::new()
                .route("/signup", post(auth::signup))
                .route("/login", post(auth::login))
                .route("/refresh", post(auth::refresh))
                .route("/logout", post(auth::logout))
                .route("/me", get(me::me))
                .route("/recovery/setup", post(auth::setup_recovery_phrase))
                .route("/recovery/reset-password", post(auth::reset_password))
                .layer(from_fn(middleware::auth_security_headers)),
        )
        .layer(cors_layer)
        .with_state(state)
}

fn build_cors_layer(allowed_origin: Option<&str>) -> CorsLayer {
    let Some(origin) = allowed_origin else {
        return CorsLayer::new();
    };

    let origin_value: HeaderValue = origin
        .parse()
        .expect("CORS_ALLOWED_ORIGIN must be a valid header value");

    CorsLayer::new()
        .allow_origin(origin_value)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ])
}
