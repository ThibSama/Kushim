use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("service unavailable")]
    ServiceUnavailable,
    #[error("unsupported media type")]
    UnsupportedMediaType {
        code: &'static str,
        message: &'static str,
    },
    #[error("validation failed")]
    Validation {
        code: &'static str,
        message: &'static str,
    },
    #[error("unauthorized")]
    Unauthorized {
        code: &'static str,
        message: &'static str,
    },
    #[error("resource not found")]
    NotFound {
        code: &'static str,
        message: &'static str,
    },
    #[error("conflict")]
    Conflict {
        code: &'static str,
        message: &'static str,
    },
    /// Returned for semantic-layer rejections that should map to HTTP 422
    /// (Unprocessable Entity). Used by the P1 currency contract for
    /// `unsupported_currency` and `unsupported_cross_currency` so client
    /// tooling can distinguish a syntactically valid payload from a payload
    /// that violates a business rule.
    #[error("unprocessable entity")]
    UnprocessableEntity {
        code: &'static str,
        message: &'static str,
    },
    #[error("internal server error")]
    Internal {
        code: &'static str,
        message: &'static str,
    },
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
                    error: ApiErrorPayload { code, message },
                }),
            )
                .into_response(),
            Self::UnsupportedMediaType { code, message } => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(ApiErrorBody {
                    error: ApiErrorPayload { code, message },
                }),
            )
                .into_response(),
            Self::Unauthorized { code, message } => (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorBody {
                    error: ApiErrorPayload { code, message },
                }),
            )
                .into_response(),
            Self::NotFound { code, message } => (
                StatusCode::NOT_FOUND,
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
            Self::UnprocessableEntity { code, message } => (
                StatusCode::UNPROCESSABLE_ENTITY,
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

#[cfg(test)]
mod tests {
    use super::ApiError;
    use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
    use serde_json::Value;

    #[tokio::test]
    async fn internal_error_is_normalized_and_generic() {
        let response = ApiError::Internal {
            code: "portfolio_service_failed",
            message: "failed to process portfolio request",
        }
        .into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        let json: Value = serde_json::from_slice(&body).expect("response body should be JSON");

        assert_eq!(json["error"]["code"], "portfolio_service_failed");
        assert_eq!(
            json["error"]["message"],
            "failed to process portfolio request"
        );

        let body_text = String::from_utf8(body.to_vec()).expect("body should be utf8");
        assert!(!body_text.contains("duplicate key value"));
        assert!(!body_text.contains("sqlx"));
        assert!(!body_text.contains("constraint"));
    }
}
