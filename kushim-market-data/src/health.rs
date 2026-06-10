use axum::{Json, Router, extract::State, routing::get};
use serde_json::json;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

use crate::db;
use crate::state::AppState;

pub async fn spawn_health_server(
    state: AppState,
    addr: SocketAddr,
    cancel: CancellationToken,
) -> Result<(), crate::errors::MarketDataError> {
    let router = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(crate::errors::MarketDataError::Io)?;

    tracing::info!("health server listening on {addr}");

    axum::serve(listener, router)
        .with_graceful_shutdown(cancel.cancelled_owned())
        .await
        .map_err(|e| crate::errors::MarketDataError::HealthServer(e.to_string()))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "kushim-market-data",
    }))
}

async fn ready(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, crate::errors::MarketDataError> {
    db::check_readiness(&state.pg_pool).await?;
    Ok(Json(json!({
        "status": "ready",
        "service": "kushim-market-data",
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_router(pool: sqlx::PgPool) -> Router {
        let state = AppState { pg_pool: pool };
        Router::new()
            .route("/health", get(health))
            .route("/ready", get(ready))
            .with_state(state)
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/fake").unwrap();
        let app = test_router(pool);

        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["service"], "kushim-market-data");
    }

    #[tokio::test]
    async fn ready_fails_without_db() {
        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/fake").unwrap();
        let app = test_router(pool);

        let response = app
            .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
