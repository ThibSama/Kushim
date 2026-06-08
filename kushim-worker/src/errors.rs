use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("job failed: {0}")]
    Job(String),
    #[error("health server failed: {0}")]
    HealthServer(String),
}

impl IntoResponse for WorkerError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Config(_) => (
                StatusCode::BAD_REQUEST,
                "invalid_worker_configuration",
                "worker configuration is invalid",
            ),
            Self::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_unavailable",
                "worker database dependency is unavailable",
            ),
            Self::Redis(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_unavailable",
                "worker redis dependency is unavailable",
            ),
            Self::Io(_) | Self::Job(_) | Self::HealthServer(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "worker_error",
                "worker request failed",
            ),
        };

        (
            status,
            Json(json!({
                "error": {
                    "code": code,
                    "message": message,
                }
            })),
        )
            .into_response()
    }
}
