use crate::{
    auth::{JwtValidationError, claims::AuthClaims},
    errors::ApiError,
    state::AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub claims: AuthClaims,
}

impl FromRequestParts<AppState> for AuthenticatedUser {
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
            .trim();

        if raw_token.is_empty() {
            return Err(ApiError::Unauthorized {
                code: "invalid_bearer_token",
                message: "authorization bearer token must not be blank",
            });
        }

        let claims =
            state
                .jwt_validator
                .decode_access_token(raw_token)
                .map_err(|error| match error {
                    JwtValidationError::TokenExpired => ApiError::Unauthorized {
                        code: "token_expired",
                        message: "access token has expired",
                    },
                    JwtValidationError::InvalidToken | JwtValidationError::InvalidTokenType => {
                        ApiError::Unauthorized {
                            code: "invalid_bearer_token",
                            message: "access token is invalid",
                        }
                    }
                })?;

        Ok(Self { claims })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        auth::{
            JwtValidator,
            claims::{AuthClaims, TokenType, UserRole},
        },
        http,
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
        body::Body,
        http::{Request, StatusCode, header::AUTHORIZATION},
    };
    use jsonwebtoken::{EncodingKey, Header, encode};
    use sqlx::PgPool;
    use time::{Duration, OffsetDateTime};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    fn test_state() -> AppState {
        let db_pool =
            PgPool::connect_lazy("postgresql://kushim:kushim_secret_dev@localhost:5432/kushim")
                .expect("lazy pool should be created");
        let portfolio_repository = PortfolioRepository::new(db_pool.clone());
        let asset_service = AssetService::new(AssetRepository::new(db_pool.clone()));
        let portfolio_service = PortfolioService::new(portfolio_repository.clone());
        let portfolio_operation_service = PortfolioOperationService::new(
            AssetRepository::new(db_pool.clone()),
            portfolio_repository.clone(),
            PortfolioOperationRepository::new(db_pool.clone()),
            PortfolioRefreshRequestRepository::new(db_pool.clone()),
            PortfolioOperationIdempotencyRepository::new(db_pool.clone()),
        );
        let portfolio_read_model_service = PortfolioReadModelService::new(
            portfolio_repository.clone(),
            PortfolioReadModelRepository::new(db_pool.clone()),
        );
        let portfolio_snapshot_service = PortfolioSnapshotService::new(
            portfolio_repository,
            PortfolioSnapshotRepository::new(db_pool.clone()),
        );

        AppState {
            db_pool,
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

    fn build_token(token_type: TokenType, exp_offset: i64) -> String {
        let now = OffsetDateTime::now_utc();
        let claims = AuthClaims {
            sub: Uuid::new_v4(),
            public_handle: "test_handle".to_string(),
            role: UserRole::User,
            token_type,
            jti: Uuid::new_v4(),
            iat: now.unix_timestamp(),
            exp: (now + Duration::seconds(exp_offset)).unix_timestamp(),
            iss: "kushim-auth".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret("dev_only_change_me_minimum_32_chars".as_bytes()),
        )
        .expect("token should be encoded")
    }

    #[tokio::test]
    async fn missing_bearer_is_rejected() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn malformed_bearer_is_rejected() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .header(AUTHORIZATION, "Basic abc")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn access_token_is_accepted() {
        let app = http::router(test_state());
        let token = build_token(TokenType::Access, 900);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn refresh_token_is_rejected() {
        let app = http::router(test_state());
        let token = build_token(TokenType::Refresh, 900);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
