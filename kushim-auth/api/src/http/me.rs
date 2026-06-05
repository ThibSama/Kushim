use crate::{
    errors::ApiError,
    http::{
        auth::UserResponse, auth::map_auth_service_error, extractors::AuthenticatedAccessToken,
    },
    state::AppState,
};
use axum::{Json, extract::State};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MeResponse {
    pub user: UserResponse,
}

pub async fn me(
    State(state): State<AppState>,
    authenticated: AuthenticatedAccessToken,
) -> Result<Json<MeResponse>, ApiError> {
    let _ = &authenticated.claims;
    let response = state
        .auth_service
        .me(&authenticated.raw_token)
        .await
        .map_err(map_auth_service_error)?;

    Ok(Json(response))
}
