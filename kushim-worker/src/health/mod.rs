use crate::{config::HealthConfig, db, errors::WorkerError, state::AppState};
use axum::{Json, Router, extract::State, routing::get};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

pub async fn spawn_health_server(
    config: HealthConfig,
    state: AppState,
    cancellation_token: CancellationToken,
) -> Result<tokio::task::JoinHandle<Result<(), WorkerError>>, WorkerError> {
    let listener = tokio::net::TcpListener::bind((config.host, config.port)).await?;
    let router = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .with_state(state);

    let handle = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                cancellation_token.cancelled().await;
            })
            .await
            .map_err(|error| WorkerError::HealthServer(error.to_string()))
    });

    Ok(handle)
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "kushim-worker",
    }))
}

async fn ready(State(state): State<AppState>) -> Result<Json<Value>, WorkerError> {
    db::check_readiness(&state.pg_pool).await?;

    Ok(Json(json!({
        "status": "ok",
        "service": "kushim-worker",
        "worker_name": state.worker_name,
    })))
}

#[cfg(test)]
mod tests {
    use super::{health, ready};
    use crate::{db, state::AppState};
    use axum::{Json, extract::State};
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn health_returns_ok() {
        let Json(body) = health().await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "kushim-worker");
    }

    #[tokio::test]
    async fn ready_checks_database_when_database_url_is_available() {
        let database_url = {
            let _guard = crate::test_utils::lock_env();
            match std::env::var("DATABASE_URL") {
                Ok(value) => value,
                Err(_) => return,
            }
        };
        let pool = db::connect_and_check(&database_url)
            .await
            .expect("database should be reachable");
        let state = AppState {
            pg_pool: pool,
            worker_name: "test-worker".to_string(),
        };

        let Json(body) = ready(State(state)).await.expect("ready should succeed");
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test]
    async fn lazy_pool_can_be_built_for_unit_state() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://postgres:postgres@localhost:5432/kushim")
            .expect("lazy pool should build");
        let state = AppState {
            pg_pool: pool,
            worker_name: "test".to_string(),
        };
        assert_eq!(state.worker_name, "test");
    }
}
