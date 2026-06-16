// P3 idempotency header extractor.
//
// Reads the `Idempotency-Key` header from a request, validates it as a
// well-formed UUID, and surfaces normalized ApiError variants on failure
// (`missing_idempotency_key` / `invalid_idempotency_key`). The header name is
// matched case-insensitively because the underlying `HeaderMap` already
// canonicalizes header names.

use crate::errors::ApiError;
use axum::{extract::FromRequestParts, http::request::Parts};
use uuid::Uuid;

pub const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";
/// Lowercase form required by `axum::http::HeaderName::from_static`.
pub const IDEMPOTENCY_KEY_HEADER_LOWER: &str = "idempotency-key";

#[derive(Debug, Clone, Copy)]
pub struct IdempotencyKey(pub Uuid);

impl<S> FromRequestParts<S> for IdempotencyKey
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(IDEMPOTENCY_KEY_HEADER)
            .ok_or(ApiError::Validation {
                code: "missing_idempotency_key",
                message: "Idempotency-Key header is required for this endpoint",
            })?;

        let raw = header.to_str().map_err(|_| ApiError::Validation {
            code: "invalid_idempotency_key",
            message: "Idempotency-Key header must be a valid UUID",
        })?;

        let trimmed = raw.trim();
        let parsed = Uuid::parse_str(trimmed).map_err(|_| ApiError::Validation {
            code: "invalid_idempotency_key",
            message: "Idempotency-Key header must be a valid UUID",
        })?;

        Ok(IdempotencyKey(parsed))
    }
}
