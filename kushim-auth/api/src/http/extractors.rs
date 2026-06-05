use crate::{domain::token::TokenClaims, errors::ApiError, state::AppState};
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};

#[derive(Debug, Clone)]
pub struct AuthenticatedAccessToken {
    pub raw_token: String,
    pub claims: TokenClaims,
}

impl FromRequestParts<AppState> for AuthenticatedAccessToken {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header_value = parts
            .headers
            .get(AUTHORIZATION)
            .ok_or(ApiError::Unauthorized {
                code: "missing_bearer_token",
                message: "authorization bearer token is required",
            })?;

        let header_value = header_value.to_str().map_err(|_| ApiError::Unauthorized {
            code: "invalid_bearer_token",
            message: "authorization header must be valid ASCII",
        })?;

        let raw_token = header_value
            .strip_prefix("Bearer ")
            .ok_or(ApiError::Unauthorized {
                code: "invalid_bearer_token",
                message: "authorization header must use Bearer token format",
            })?
            .trim()
            .to_string();

        if raw_token.is_empty() {
            return Err(ApiError::Unauthorized {
                code: "invalid_bearer_token",
                message: "authorization bearer token must not be blank",
            });
        }

        let claims =
            state
                .auth_service
                .decode_access_token(&raw_token)
                .map_err(|error| match error {
                    crate::services::auth::AuthServiceError::TokenExpired => {
                        ApiError::Unauthorized {
                            code: "token_expired",
                            message: "access token has expired",
                        }
                    }
                    crate::services::auth::AuthServiceError::InvalidToken
                    | crate::services::auth::AuthServiceError::InvalidTokenType => {
                        ApiError::Unauthorized {
                            code: "invalid_bearer_token",
                            message: "access token is invalid",
                        }
                    }
                    _ => ApiError::Internal {
                        code: "token_decode_failed",
                        message: "failed to validate access token",
                    },
                })?;

        Ok(Self { raw_token, claims })
    }
}
