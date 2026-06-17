use crate::{
    auth::AuthenticatedUser,
    domain::portfolio::{Portfolio, PortfolioVisibility},
    errors::ApiError,
    http::extractors::{ApiJson, ApiPath},
    services::portfolios::{CreatePortfolioInput, PortfolioServiceError},
    state::AppState,
};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePortfolioRequest {
    pub name: String,
    pub base_currency: String,
    pub visibility: Option<PortfolioVisibility>,
}

#[derive(Debug, Serialize)]
pub struct PortfolioResponse {
    pub id_portfolio: Uuid,
    pub name: String,
    pub base_currency: String,
    pub visibility: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreatePortfolioResponse {
    pub portfolio: PortfolioResponse,
}

#[derive(Debug, Serialize)]
pub struct ListPortfoliosResponse {
    pub portfolios: Vec<PortfolioResponse>,
}

#[derive(Debug, Serialize)]
pub struct GetPortfolioResponse {
    pub portfolio: PortfolioResponse,
}

pub async fn create_portfolio(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiJson(request): ApiJson<CreatePortfolioRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let portfolio = state
        .portfolio_service
        .create_portfolio(CreatePortfolioInput {
            id_user: authenticated.claims.sub,
            name: request.name,
            description: None,
            base_currency: request.base_currency,
            visibility: request.visibility.unwrap_or(PortfolioVisibility::Private),
        })
        .await
        .map_err(map_service_error)?;

    Ok((
        StatusCode::CREATED,
        Json(CreatePortfolioResponse {
            portfolio: portfolio.into(),
        }),
    ))
}

pub async fn list_portfolios(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
) -> Result<Json<ListPortfoliosResponse>, ApiError> {
    let portfolios = state
        .portfolio_service
        .list_portfolios(authenticated.claims.sub)
        .await
        .map_err(map_service_error)?;

    Ok(Json(ListPortfoliosResponse {
        portfolios: portfolios
            .into_iter()
            .map(PortfolioResponse::from)
            .collect(),
    }))
}

pub async fn get_portfolio(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath(id_portfolio): ApiPath<Uuid>,
) -> Result<Json<GetPortfolioResponse>, ApiError> {
    let portfolio = state
        .portfolio_service
        .get_portfolio(id_portfolio, authenticated.claims.sub)
        .await
        .map_err(map_service_error)?;

    Ok(Json(GetPortfolioResponse {
        portfolio: portfolio.into(),
    }))
}

impl From<Portfolio> for PortfolioResponse {
    fn from(value: Portfolio) -> Self {
        Self {
            id_portfolio: value.id_portfolio,
            name: value.name,
            base_currency: value.base_currency,
            visibility: value.visibility.as_str().to_string(),
            created_at: format_datetime(value.created_at),
            updated_at: format_datetime(value.updated_at),
        }
    }
}

fn format_datetime(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("OffsetDateTime should always be serializable as RFC3339")
}

fn map_service_error(error: PortfolioServiceError) -> ApiError {
    match error {
        PortfolioServiceError::Validation { code, message } => {
            ApiError::Validation { code, message }
        }
        PortfolioServiceError::UnprocessableEntity { code, message } => {
            ApiError::UnprocessableEntity { code, message }
        }
        PortfolioServiceError::NotFound => ApiError::NotFound {
            code: "portfolio_not_found",
            message: "portfolio was not found",
        },
        PortfolioServiceError::Internal => ApiError::Internal {
            code: "portfolio_service_failed",
            message: "failed to process portfolio request",
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        auth::{
            JwtValidator,
            claims::{AuthClaims, TokenType, UserRole},
        },
        repositories::{
            assets::AssetRepository,
            portfolio_operation_idempotency::PortfolioOperationIdempotencyRepository,
            portfolio_operations::PortfolioOperationRepository,
            portfolio_read_models::PortfolioReadModelRepository,
            portfolio_refresh_requests::PortfolioRefreshRequestRepository,
            portfolio_snapshots::PortfolioSnapshotRepository, portfolios::PortfolioRepository,
        },
        services::{
            assets::AssetService, portfolio_operations::PortfolioOperationService,
            portfolio_read_models::PortfolioReadModelService,
            portfolio_snapshots::PortfolioSnapshotService, portfolios::PortfolioService,
        },
        state::AppState,
    };
    use axum::{
        body::{self, Body},
        http::{Request, StatusCode, header::AUTHORIZATION},
    };
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::{Value, json};
    use sqlx::{PgPool, postgres::PgPoolOptions};
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    async fn test_pool() -> PgPool {
        let database_url = crate::test_support::require_disposable_test_database_url();
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    use crate::test_support::ensure_canonical_user_role;

    async fn create_user(pool: &PgPool, public_handle: &str) -> Uuid {
        ensure_canonical_user_role(pool).await;

        let id_user = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id_user, id_role, username, public_handle, password_hash)
            VALUES ($1, 1, $2, $3, $4)
            "#,
        )
        .bind(id_user)
        .bind(public_handle)
        .bind(public_handle)
        .bind("$argon2id$placeholder")
        .execute(pool)
        .await
        .expect("user should be inserted");

        id_user
    }

    async fn insert_portfolio(
        pool: &PgPool,
        id_user: Uuid,
        name: &str,
        base_currency: &str,
        visibility: &str,
        deleted_at: Option<OffsetDateTime>,
    ) -> Uuid {
        let id_portfolio = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO portfolios (
                id_portfolio,
                id_user,
                name,
                base_currency,
                visibility,
                deleted_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(id_portfolio)
        .bind(id_user)
        .bind(name)
        .bind(base_currency)
        .bind(visibility)
        .bind(deleted_at)
        .execute(pool)
        .await
        .expect("portfolio should be inserted");

        id_portfolio
    }

    async fn cleanup_user(pool: &PgPool, id_user: Uuid) {
        sqlx::query("DELETE FROM portfolios WHERE id_user = $1")
            .bind(id_user)
            .execute(pool)
            .await
            .expect("portfolios should be deleted");

        sqlx::query("DELETE FROM users WHERE id_user = $1")
            .bind(id_user)
            .execute(pool)
            .await
            .expect("user should be deleted");
    }

    fn build_token(id_user: Uuid, public_handle: &str) -> String {
        let now = OffsetDateTime::now_utc();
        let claims = AuthClaims {
            sub: id_user,
            public_handle: public_handle.to_string(),
            role: UserRole::User,
            token_type: TokenType::Access,
            jti: Uuid::new_v4(),
            iat: now.unix_timestamp(),
            exp: (now + Duration::seconds(900)).unix_timestamp(),
            iss: "kushim-auth".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret("dev_only_change_me_minimum_32_chars".as_bytes()),
        )
        .expect("token should be encoded")
    }

    async fn test_state(pool: PgPool) -> AppState {
        let portfolio_repository = PortfolioRepository::new(pool.clone());
        let asset_service = AssetService::new(AssetRepository::new(pool.clone()));
        let portfolio_service = PortfolioService::new(portfolio_repository.clone());
        let portfolio_operation_service = PortfolioOperationService::new(
            AssetRepository::new(pool.clone()),
            portfolio_repository.clone(),
            PortfolioOperationRepository::new(pool.clone()),
            PortfolioRefreshRequestRepository::new(pool.clone()),
            PortfolioOperationIdempotencyRepository::new(pool.clone()),
        );
        let portfolio_read_model_service = PortfolioReadModelService::new(
            portfolio_repository.clone(),
            PortfolioReadModelRepository::new(pool.clone()),
        );
        let portfolio_snapshot_service = PortfolioSnapshotService::new(
            portfolio_repository,
            PortfolioSnapshotRepository::new(pool.clone()),
        );

        AppState {
            db_pool: pool,
            jwt_validator: JwtValidator::new(
                "dev_only_change_me_minimum_32_chars",
                "kushim-auth".to_string(),
            ),
            asset_service,
            portfolio_service,
            portfolio_operation_service,
            portfolio_read_model_service,
            portfolio_snapshot_service,
            service_name: "kushim-api",
            service_version: "test",
            routes_version: "api-routes-v1",
            environment: "test".to_string(),
        }
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        serde_json::from_slice(&bytes).expect("body should be valid json")
    }

    fn assert_rfc3339_string(value: &Value) {
        let as_str = value.as_str().expect("date field should be a JSON string");
        OffsetDateTime::parse(as_str, &Rfc3339).expect("date field should be valid RFC3339");
    }

    #[tokio::test]
    async fn create_portfolio_with_valid_token() {
        let pool = test_pool().await;
        let handle = format!("pc{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/portfolios")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "My Portfolio",
                            "base_currency": "EUR",
                            "visibility": "private"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user(&pool, id_user).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["portfolio"]["name"], "My Portfolio");
        assert_eq!(body["portfolio"]["base_currency"], "EUR");
        assert_eq!(body["portfolio"]["visibility"], "private");
        assert_rfc3339_string(&body["portfolio"]["created_at"]);
        assert_rfc3339_string(&body["portfolio"]["updated_at"]);
    }

    #[tokio::test]
    async fn reject_create_without_token() {
        let pool = test_pool().await;
        let app = crate::http::router(test_state(pool).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/portfolios")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "My Portfolio",
                            "base_currency": "EUR"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn lowercase_base_currency_is_normalized_and_accepted() {
        // P1 contract: lowercase/whitespace input is trimmed and uppercased
        // against the canonical catalogue, then accepted on success.
        let pool = test_pool().await;
        let handle = format!("pcl{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/portfolios")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "My Portfolio",
                            "base_currency": " eur "
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user(&pool, id_user).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["portfolio"]["base_currency"], "EUR");
    }

    #[tokio::test]
    async fn reject_invalid_base_currency_format() {
        // Wrong-length codes (e.g. "EURO") are a schema-level error → 400.
        let pool = test_pool().await;
        let handle = format!("pci{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/portfolios")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "My Portfolio",
                            "base_currency": "EURO"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user(&pool, id_user).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_base_currency");
    }

    #[tokio::test]
    async fn reject_unsupported_base_currency_code() {
        // Three-letter code that is not part of the catalogue → 422
        // unsupported_currency, no portfolio persisted.
        let pool = test_pool().await;
        let handle = format!("pcz{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/portfolios")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "My Portfolio",
                            "base_currency": "ZZZ"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;

        // Ensure the rejected portfolio was NOT persisted.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM portfolios WHERE id_user = $1")
            .bind(id_user)
            .fetch_one(&pool)
            .await
            .expect("count should succeed");

        cleanup_user(&pool, id_user).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "unsupported_currency");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn default_visibility_is_private_if_omitted() {
        let pool = test_pool().await;
        let handle = format!("pd{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/portfolios")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "My Portfolio",
                            "base_currency": "EUR"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user(&pool, id_user).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["portfolio"]["visibility"], "private");
        assert_rfc3339_string(&body["portfolio"]["created_at"]);
        assert_rfc3339_string(&body["portfolio"]["updated_at"]);
    }

    #[tokio::test]
    async fn list_returns_only_current_users_portfolios() {
        let pool = test_pool().await;
        let handle_a = format!("pla{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("plb{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        insert_portfolio(&pool, user_a, "Owned One", "EUR", "private", None).await;
        insert_portfolio(&pool, user_a, "Owned Two", "USD", "unlisted", None).await;
        insert_portfolio(&pool, user_b, "Other User", "EUR", "private", None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/portfolios")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(user_a, &handle_a)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user(&pool, user_a).await;
        cleanup_user(&pool, user_b).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["portfolios"].as_array().unwrap().len(), 2);
        for portfolio in body["portfolios"].as_array().unwrap() {
            assert_rfc3339_string(&portfolio["created_at"]);
            assert_rfc3339_string(&portfolio["updated_at"]);
        }
        assert!(
            body["portfolios"]
                .as_array()
                .unwrap()
                .iter()
                .all(|portfolio| portfolio["name"] != "Other User")
        );
    }

    #[tokio::test]
    async fn get_returns_owned_portfolio() {
        let pool = test_pool().await;
        let handle = format!("pg{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let id_portfolio = insert_portfolio(&pool, id_user, "Owned", "EUR", "private", None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        cleanup_user(&pool, id_user).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["portfolio"]["id_portfolio"], id_portfolio.to_string());
        assert_rfc3339_string(&body["portfolio"]["created_at"]);
        assert_rfc3339_string(&body["portfolio"]["updated_at"]);
    }

    #[tokio::test]
    async fn get_returns_404_for_another_users_portfolio() {
        let pool = test_pool().await;
        let handle_a = format!("p4a{}", &Uuid::new_v4().simple().to_string()[..12]);
        let handle_b = format!("p4b{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_a = create_user(&pool, &handle_a).await;
        let user_b = create_user(&pool, &handle_b).await;
        let id_portfolio = insert_portfolio(&pool, user_b, "Hidden", "EUR", "private", None).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(user_a, &handle_a)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        cleanup_user(&pool, user_a).await;
        cleanup_user(&pool, user_b).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_invalid_uuid_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("piv{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/portfolios/not-a-uuid")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_path_parameters");

        cleanup_user(&pool, id_user).await;
    }

    #[tokio::test]
    async fn create_portfolio_invalid_json_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("pij{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/portfolios")
                    .method("POST")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":123,"base_currency":"EUR"}"#))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");

        cleanup_user(&pool, id_user).await;
    }

    #[tokio::test]
    async fn create_portfolio_malformed_json_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("pmj{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/portfolios")
                    .method("POST")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Broken""#))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let headers = response.headers().clone();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(body["error"]["code"], "invalid_json_body");

        cleanup_user(&pool, id_user).await;
    }

    #[tokio::test]
    async fn create_portfolio_empty_body_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("peb{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/portfolios")
                    .method("POST")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let headers = response.headers().clone();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(body["error"]["code"], "invalid_json_body");

        cleanup_user(&pool, id_user).await;
    }

    #[tokio::test]
    async fn create_portfolio_unknown_field_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("puf{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/portfolios")
                    .method("POST")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"name":"Unknown","base_currency":"EUR","surprise":true}"#,
                    ))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request_body");

        cleanup_user(&pool, id_user).await;
    }

    #[tokio::test]
    async fn create_portfolio_invalid_content_type_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("pct{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/portfolios")
                    .method("POST")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .header("content-type", "text/plain")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let headers = response.headers().clone();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(body["error"]["code"], "invalid_content_type");

        cleanup_user(&pool, id_user).await;
    }

    #[tokio::test]
    async fn soft_deleted_portfolio_is_not_returned() {
        let pool = test_pool().await;
        let handle = format!("ps{}", &Uuid::new_v4().simple().to_string()[..12]);
        let id_user = create_user(&pool, &handle).await;
        let deleted_at = OffsetDateTime::now_utc() + Duration::seconds(5);
        let id_portfolio = insert_portfolio(
            &pool,
            id_user,
            "Soft Deleted",
            "EUR",
            "private",
            Some(deleted_at),
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let get_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{id_portfolio}"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let list_response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/portfolios")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_token(id_user, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let list_body = response_json(list_response).await;
        cleanup_user(&pool, id_user).await;

        assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
        assert_eq!(list_body["portfolios"].as_array().unwrap().len(), 0);
    }
}
