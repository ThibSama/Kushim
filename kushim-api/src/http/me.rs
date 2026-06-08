use crate::{auth::AuthenticatedUser, errors::ApiError};
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id_user: Uuid,
    pub public_handle: String,
    pub role: String,
}

pub async fn me(authenticated: AuthenticatedUser) -> Result<Json<MeResponse>, ApiError> {
    Ok(Json(MeResponse {
        id_user: authenticated.claims.sub,
        public_handle: authenticated.claims.public_handle,
        role: authenticated.claims.role.as_str().to_string(),
    }))
}
