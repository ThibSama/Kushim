pub mod auth;
pub mod extractors;
pub mod health;
pub mod me;
pub mod middleware;

use crate::state::AppState;
use axum::{
    Router,
    middleware::from_fn,
    routing::{get, post},
};

pub fn router(state: AppState) -> Router {
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
        .with_state(state)
}
