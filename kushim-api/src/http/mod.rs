pub mod assets;
pub mod extractors;
pub mod health;
pub mod me;
pub mod portfolio_operations;
pub mod portfolio_read_models;
pub mod portfolio_snapshots;
pub mod portfolios;
pub mod reference;

use crate::state::AppState;
use axum::{
    Router,
    body::to_bytes,
    http::{HeaderValue, Method, StatusCode},
    middleware,
    response::Response,
    routing::{get, post},
};
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;

pub const ROUTES_DESCRIPTION: &str = "/health, /ready, /v1/me, /v1/reference/operation-types, /v1/reference/operation-statuses, /v1/reference/portfolio-visibilities, /v1/assets, /v1/assets/{id_asset}, /v1/portfolios, /v1/portfolios/{id_portfolio}, /v1/portfolios/{id_portfolio}/summary, /v1/portfolios/{id_portfolio}/holdings, /v1/portfolios/{id_portfolio}/snapshots/daily, /v1/portfolios/{id_portfolio}/snapshots/daily/{snapshot_date}/holdings, /v1/portfolios/{id_portfolio}/operations, /v1/portfolios/{id_portfolio}/operations/audit, /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}, /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/cancel, /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/corrections, /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/post, /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/audit, /v1/portfolios/{id_portfolio}/refresh-requests/{id_refresh_request}";

pub fn router(state: AppState) -> Router {
    router_with_cors(state, None)
}

pub fn router_with_cors(state: AppState, cors_allowed_origins: Option<&str>) -> Router {
    let cors_layer = build_cors_layer(cors_allowed_origins);

    Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/v1/me", get(me::me))
        .route(
            "/v1/reference/operation-types",
            get(reference::list_operation_types),
        )
        .route(
            "/v1/reference/operation-statuses",
            get(reference::list_operation_statuses),
        )
        .route(
            "/v1/reference/portfolio-visibilities",
            get(reference::list_portfolio_visibilities),
        )
        .route("/v1/assets", get(assets::list_assets))
        .route("/v1/assets/{id_asset}", get(assets::get_asset))
        .route(
            "/v1/portfolios",
            post(portfolios::create_portfolio).get(portfolios::list_portfolios),
        )
        .route(
            "/v1/portfolios/{id_portfolio}",
            get(portfolios::get_portfolio),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/summary",
            get(portfolio_read_models::get_portfolio_summary),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/holdings",
            get(portfolio_read_models::get_portfolio_holdings),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/snapshots/daily",
            get(portfolio_snapshots::list_portfolio_daily_snapshots),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/snapshots/daily/{snapshot_date}/holdings",
            get(portfolio_snapshots::get_portfolio_daily_snapshot_holdings),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/operations",
            post(portfolio_operations::create_portfolio_operation)
                .get(portfolio_operations::list_portfolio_operations),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/operations/audit",
            get(portfolio_operations::get_portfolio_operations_audit_timeline),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}",
            get(portfolio_operations::get_portfolio_operation)
                .patch(portfolio_operations::update_portfolio_operation),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/cancel",
            post(portfolio_operations::cancel_portfolio_operation),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/corrections",
            post(portfolio_operations::create_portfolio_operation_correction)
                .get(portfolio_operations::get_portfolio_operation_corrections),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/post",
            post(portfolio_operations::post_portfolio_operation),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/audit",
            get(portfolio_operations::get_portfolio_operation_audit),
        )
        .route(
            "/v1/portfolios/{id_portfolio}/refresh-requests/{id_refresh_request}",
            get(portfolio_operations::get_portfolio_refresh_request),
        )
        .layer(middleware::map_response(
            normalize_plaintext_error_responses,
        ))
        .layer(cors_layer)
        .with_state(state)
}

fn build_cors_layer(allowed_origins: Option<&str>) -> CorsLayer {
    let Some(origins_str) = allowed_origins else {
        return CorsLayer::new();
    };

    let origins: Vec<HeaderValue> = origins_str
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|o| {
            o.parse()
                .expect("each CORS_ALLOWED_ORIGINS value must be a valid header value")
        })
        .collect();

    if origins.is_empty() {
        return CorsLayer::new();
    }

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ])
}

async fn normalize_plaintext_error_responses(response: Response) -> Response {
    let status = response.status();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

    let is_plaintext_error = matches!(
        status,
        StatusCode::BAD_REQUEST
            | StatusCode::UNPROCESSABLE_ENTITY
            | StatusCode::UNSUPPORTED_MEDIA_TYPE
    ) && content_type.starts_with("text/plain");

    let is_legacy_json_content_type_error =
        status == StatusCode::BAD_REQUEST && content_type.starts_with("application/json");

    if !is_plaintext_error && !is_legacy_json_content_type_error {
        return response;
    }

    let body = response.into_body();
    let body_bytes = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => return internal_normalization_error_response(),
    };

    if is_legacy_json_content_type_error {
        let body_json: Value = match serde_json::from_slice(&body_bytes) {
            Ok(value) => value,
            Err(_) => return response_from_bytes(status, &content_type, body_bytes),
        };

        if body_json["error"]["code"] == "missing_json_content_type" {
            return json_error_response(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "invalid_content_type",
                "request content-type must be application/json",
            );
        }

        return response_from_bytes(status, &content_type, body_bytes);
    }

    let body_text = String::from_utf8_lossy(&body_bytes);
    let (status, code, message) = if body_text.starts_with("Invalid URL:") {
        (
            StatusCode::BAD_REQUEST,
            "invalid_path_parameters",
            "path parameters are invalid",
        )
    } else if body_text.contains("Failed to deserialize query string") {
        (
            StatusCode::BAD_REQUEST,
            "invalid_query_parameters",
            "query parameters are invalid",
        )
    } else if body_text.contains("Content-Type")
        || body_text.contains("content-type")
        || status == StatusCode::UNSUPPORTED_MEDIA_TYPE
    {
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid_content_type",
            "request content-type must be application/json",
        )
    } else if body_text.contains("Failed to deserialize the JSON body into the target type") {
        (
            StatusCode::BAD_REQUEST,
            "invalid_request_body",
            "request body does not match the expected schema",
        )
    } else {
        (
            StatusCode::BAD_REQUEST,
            "invalid_json_body",
            "request body is invalid",
        )
    };

    json_error_response(status, code, message)
}

fn internal_normalization_error_response() -> Response {
    json_error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "failed to normalize error response",
    )
}

fn json_error_response(status: StatusCode, code: &str, message: &str) -> Response {
    Response::builder()
        .status(status)
        .header(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )
        .body(axum::body::Body::from(
            json!({
                "error": {
                    "code": code,
                    "message": message,
                }
            })
            .to_string(),
        ))
        .expect("response should be built")
}

fn response_from_bytes(
    status: StatusCode,
    content_type: &str,
    body_bytes: axum::body::Bytes,
) -> Response {
    Response::builder()
        .status(status)
        .header(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_str(content_type)
                .unwrap_or_else(|_| HeaderValue::from_static("application/json")),
        )
        .body(axum::body::Body::from(body_bytes))
        .expect("response should be built")
}
