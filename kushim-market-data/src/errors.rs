use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MarketDataError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("health server failed: {0}")]
    HealthServer(String),
    #[error("job error: {0}")]
    Job(String),
    #[error("provider error: {0}")]
    Provider(String),
}

impl IntoResponse for MarketDataError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Config(_) => (
                StatusCode::BAD_REQUEST,
                "invalid_configuration",
                "market-data service configuration is invalid",
            ),
            Self::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_unavailable",
                "market-data database dependency is unavailable",
            ),
            Self::Job(_) | Self::Provider(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "job_error",
                "market-data job failed",
            ),
            Self::Io(_) | Self::HealthServer(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "service_error",
                "market-data service request failed",
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
