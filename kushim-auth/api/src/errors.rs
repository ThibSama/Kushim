use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("service unavailable")]
    ServiceUnavailable,
    #[error("validation failed")]
    Validation { code: &'static str, message: String },
    #[error("too many requests")]
    RateLimited { retry_after_seconds: u64 },
    #[error("unauthorized")]
    Unauthorized {
        code: &'static str,
        message: &'static str,
    },
    #[error("forbidden")]
    Forbidden {
        code: &'static str,
        message: &'static str,
    },
    #[error("conflict")]
    Conflict {
        code: &'static str,
        message: &'static str,
    },
    #[error("internal server error")]
    Internal {
        code: &'static str,
        message: &'static str,
    },
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("resource conflict: {0}")]
    Conflict(&'static str),
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

#[derive(Serialize)]
struct ApiErrorBody<'a> {
    error: ApiErrorPayload<'a>,
}

#[derive(Serialize)]
struct ApiErrorPayload<'a> {
    code: &'a str,
    message: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::ServiceUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiErrorBody {
                    error: ApiErrorPayload {
                        code: "service_unavailable",
                        message: "service dependency check failed",
                    },
                }),
            )
                .into_response(),
            Self::Validation { code, message } => (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorBody {
                    error: ApiErrorPayload {
                        code,
                        message: &message,
                    },
                }),
            )
                .into_response(),
            Self::RateLimited {
                retry_after_seconds,
            } => {
                let mut response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(ApiErrorBody {
                        error: ApiErrorPayload {
                            code: "rate_limited",
                            message: "too many attempts, please try again later",
                        },
                    }),
                )
                    .into_response();

                if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
                    response.headers_mut().insert(header::RETRY_AFTER, value);
                }

                response
            }
            Self::Unauthorized { code, message } => (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorBody {
                    error: ApiErrorPayload { code, message },
                }),
            )
                .into_response(),
            Self::Forbidden { code, message } => (
                StatusCode::FORBIDDEN,
                Json(ApiErrorBody {
                    error: ApiErrorPayload { code, message },
                }),
            )
                .into_response(),
            Self::Conflict { code, message } => (
                StatusCode::CONFLICT,
                Json(ApiErrorBody {
                    error: ApiErrorPayload { code, message },
                }),
            )
                .into_response(),
            Self::Internal { code, message } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorBody {
                    error: ApiErrorPayload { code, message },
                }),
            )
                .into_response(),
        }
    }
}
