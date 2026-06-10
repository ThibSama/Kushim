use crate::auth::AuthenticatedUser;
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReferenceItem {
    pub value: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ReferenceListResponse {
    pub data: &'static [ReferenceItem],
}

static OPERATION_TYPES: &[ReferenceItem] = &[
    ReferenceItem {
        value: "buy",
        label: "Buy",
    },
    ReferenceItem {
        value: "sell",
        label: "Sell",
    },
    ReferenceItem {
        value: "deposit",
        label: "Deposit",
    },
    ReferenceItem {
        value: "withdrawal",
        label: "Withdrawal",
    },
    ReferenceItem {
        value: "dividend",
        label: "Dividend",
    },
    ReferenceItem {
        value: "interest",
        label: "Interest",
    },
    ReferenceItem {
        value: "fee",
        label: "Fee",
    },
    ReferenceItem {
        value: "tax",
        label: "Tax",
    },
    ReferenceItem {
        value: "split",
        label: "Split",
    },
    ReferenceItem {
        value: "spin_off",
        label: "Spin Off",
    },
    ReferenceItem {
        value: "symbol_change",
        label: "Symbol Change",
    },
    ReferenceItem {
        value: "transfer_in",
        label: "Transfer In",
    },
    ReferenceItem {
        value: "transfer_out",
        label: "Transfer Out",
    },
    ReferenceItem {
        value: "adjustment",
        label: "Adjustment",
    },
];

static OPERATION_STATUSES: &[ReferenceItem] = &[
    ReferenceItem {
        value: "pending",
        label: "Pending",
    },
    ReferenceItem {
        value: "posted",
        label: "Posted",
    },
    ReferenceItem {
        value: "cancelled",
        label: "Cancelled",
    },
];

static PORTFOLIO_VISIBILITIES: &[ReferenceItem] = &[
    ReferenceItem {
        value: "private",
        label: "Private",
    },
    ReferenceItem {
        value: "public",
        label: "Public",
    },
    ReferenceItem {
        value: "unlisted",
        label: "Unlisted",
    },
];

pub async fn list_operation_types(
    _authenticated: AuthenticatedUser,
) -> Json<ReferenceListResponse> {
    Json(ReferenceListResponse {
        data: OPERATION_TYPES,
    })
}

pub async fn list_operation_statuses(
    _authenticated: AuthenticatedUser,
) -> Json<ReferenceListResponse> {
    Json(ReferenceListResponse {
        data: OPERATION_STATUSES,
    })
}

pub async fn list_portfolio_visibilities(
    _authenticated: AuthenticatedUser,
) -> Json<ReferenceListResponse> {
    Json(ReferenceListResponse {
        data: PORTFOLIO_VISIBILITIES,
    })
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
            assets::AssetRepository, portfolio_operations::PortfolioOperationRepository,
            portfolio_read_models::PortfolioReadModelRepository,
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
    use serde_json::Value;
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

    fn build_access_token() -> String {
        let now = OffsetDateTime::now_utc();
        let claims = AuthClaims {
            sub: Uuid::new_v4(),
            public_handle: "test_handle".to_string(),
            role: UserRole::User,
            token_type: TokenType::Access,
            jti: Uuid::new_v4(),
            iat: now.unix_timestamp(),
            exp: (now + Duration::minutes(15)).unix_timestamp(),
            iss: "kushim-auth".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret("dev_only_change_me_minimum_32_chars".as_bytes()),
        )
        .expect("token should be encoded")
    }

    fn build_refresh_token() -> String {
        let now = OffsetDateTime::now_utc();
        let claims = AuthClaims {
            sub: Uuid::new_v4(),
            public_handle: "test_handle".to_string(),
            role: UserRole::User,
            token_type: TokenType::Refresh,
            jti: Uuid::new_v4(),
            iat: now.unix_timestamp(),
            exp: (now + Duration::minutes(15)).unix_timestamp(),
            iss: "kushim-auth".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret("dev_only_change_me_minimum_32_chars".as_bytes()),
        )
        .expect("token should be encoded")
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        serde_json::from_slice(&bytes).expect("response body should be valid JSON")
    }

    #[tokio::test]
    async fn operation_types_without_token_returns_401() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/reference/operation-types")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn operation_types_with_refresh_token_returns_401() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/reference/operation-types")
                    .header(AUTHORIZATION, format!("Bearer {}", build_refresh_token()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn operation_types_returns_expected_values() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/reference/operation-types")
                    .header(AUTHORIZATION, format!("Bearer {}", build_access_token()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let data = body["data"].as_array().expect("data should be an array");
        assert_eq!(data.len(), 14);
        assert_eq!(data[0]["value"], "buy");
        assert_eq!(data[0]["label"], "Buy");
        assert_eq!(data[13]["value"], "adjustment");
        assert_eq!(data[13]["label"], "Adjustment");
    }

    #[tokio::test]
    async fn operation_statuses_without_token_returns_401() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/reference/operation-statuses")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn operation_statuses_returns_expected_values() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/reference/operation-statuses")
                    .header(AUTHORIZATION, format!("Bearer {}", build_access_token()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let data = body["data"].as_array().expect("data should be an array");
        assert_eq!(data.len(), 3);
        assert_eq!(data[0]["value"], "pending");
        assert_eq!(data[1]["value"], "posted");
        assert_eq!(data[2]["value"], "cancelled");
    }

    #[tokio::test]
    async fn portfolio_visibilities_without_token_returns_401() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/reference/portfolio-visibilities")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn portfolio_visibilities_with_refresh_token_returns_401() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/reference/portfolio-visibilities")
                    .header(AUTHORIZATION, format!("Bearer {}", build_refresh_token()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn portfolio_visibilities_returns_expected_values() {
        let app = http::router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/reference/portfolio-visibilities")
                    .header(AUTHORIZATION, format!("Bearer {}", build_access_token()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let data = body["data"].as_array().expect("data should be an array");
        assert_eq!(data.len(), 3);
        assert_eq!(data[0]["value"], "private");
        assert_eq!(data[1]["value"], "public");
        assert_eq!(data[2]["value"], "unlisted");
    }
}
