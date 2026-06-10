use crate::{errors::ApiError, services::handoff::HandoffError, state::AppState};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateHandoffRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct CreateHandoffResponse {
    pub handoff_code: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeHandoffRequest {
    pub handoff_code: String,
}

#[derive(Debug, Serialize)]
pub struct ExchangeHandoffResponse {
    pub access_token: String,
    pub refresh_token: String,
}

pub async fn create_handoff(
    State(state): State<AppState>,
    authenticated: crate::http::extractors::AuthenticatedAccessToken,
    Json(request): Json<CreateHandoffRequest>,
) -> Result<(StatusCode, Json<CreateHandoffResponse>), ApiError> {
    let handoff = state
        .handoff_service
        .as_ref()
        .ok_or(ApiError::ServiceUnavailable)?;

    if request.refresh_token.trim().is_empty() {
        return Err(ApiError::Validation {
            code: "blank_refresh_token",
            message: "refresh_token must not be blank".to_string(),
        });
    }

    let code = handoff
        .create_code(&authenticated.raw_token, &request.refresh_token)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "handoff code creation failed");
            ApiError::ServiceUnavailable
        })?;

    tracing::info!(
        event = "handoff_created",
        user_id = %authenticated.claims.sub,
        "handoff code created"
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateHandoffResponse { handoff_code: code }),
    ))
}

pub async fn exchange_handoff(
    State(state): State<AppState>,
    Json(request): Json<ExchangeHandoffRequest>,
) -> Result<Json<ExchangeHandoffResponse>, ApiError> {
    let handoff = state
        .handoff_service
        .as_ref()
        .ok_or(ApiError::ServiceUnavailable)?;

    if request.handoff_code.trim().is_empty() {
        return Err(ApiError::Validation {
            code: "blank_handoff_code",
            message: "handoff_code must not be blank".to_string(),
        });
    }

    let (access_token, refresh_token) = handoff
        .exchange_code(&request.handoff_code)
        .await
        .map_err(|error| match error {
            HandoffError::InvalidCode => ApiError::Unauthorized {
                code: "invalid_handoff_code",
                message: "handoff code is invalid or expired",
            },
            HandoffError::RedisUnavailable => {
                tracing::error!(error = %error, "handoff code exchange failed");
                ApiError::ServiceUnavailable
            }
        })?;

    tracing::info!(event = "handoff_exchanged", "handoff code exchanged");

    Ok(Json(ExchangeHandoffResponse {
        access_token,
        refresh_token,
    }))
}
