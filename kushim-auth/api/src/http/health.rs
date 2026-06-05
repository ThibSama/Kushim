use crate::{errors::ApiError, state::AppState};
use axum::{Json, extract::State};
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
    pub environment: String,
    pub routes_version: &'static str,
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: state.service_name,
        version: state.service_version,
        environment: state.environment.clone(),
        routes_version: state.routes_version,
    })
}

pub async fn ready(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db_pool)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "readiness check failed");
            ApiError::ServiceUnavailable
        })?;

    if let Some(rate_limiter) = &state.rate_limiter {
        rate_limiter.check_health().await.map_err(|error| {
            tracing::error!(error = %error, "redis readiness check failed");
            ApiError::ServiceUnavailable
        })?;
    }

    Ok(Json(HealthResponse {
        status: "ok",
        service: state.service_name,
        version: state.service_version,
        environment: state.environment.clone(),
        routes_version: state.routes_version,
    }))
}
