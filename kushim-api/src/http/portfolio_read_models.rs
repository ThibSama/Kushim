use crate::{
    auth::AuthenticatedUser,
    domain::{
        asset::{Asset, AssetClass},
        portfolio_read_model::{
            PortfolioHolding, PortfolioHoldingPositionStatus, PortfolioHoldingsSort,
            PortfolioSummary, PortfolioSummaryStatus,
        },
    },
    errors::ApiError,
    http::extractors::{ApiPath, ApiQuery},
    services::portfolio_read_models::{
        GetPortfolioSummaryInput, ListPortfolioHoldingsInput, PortfolioHoldingsView,
        PortfolioReadModelServiceError, PortfolioSummaryView,
    },
    state::AppState,
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct ListPortfolioHoldingsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<String>,
    pub asset_class: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PortfolioSummaryResponse {
    pub id_portfolio: Uuid,
    pub base_currency: String,
    pub total_value_minor: i64,
    pub cash_balance_minor: i64,
    pub total_invested_minor: i64,
    pub total_pnl_minor: i64,
    pub total_pnl_pct: Option<String>,
    pub portfolio_status: String,
    pub is_estimated: bool,
    pub as_of: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct PortfolioSummaryEnvelope {
    pub data_available: bool,
    pub summary: Option<PortfolioSummaryResponse>,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct HoldingAssetResponse {
    pub id_asset: Uuid,
    pub name: String,
    pub ticker: Option<String>,
    pub isin: Option<String>,
    pub exchange: Option<String>,
    pub asset_class: String,
    pub status: String,
    pub native_currency: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PortfolioHoldingResponse {
    pub id_asset: Uuid,
    pub asset: HoldingAssetResponse,
    pub base_currency: String,
    pub quantity: String,
    pub avg_cost_minor: Option<i64>,
    pub invested_base_minor: i64,
    pub market_value_minor: i64,
    pub pnl_base_minor: i64,
    pub pnl_pct: Option<String>,
    pub weight_pct: Option<String>,
    pub position_status: String,
    pub is_estimated: bool,
    pub as_of: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct PortfolioHoldingsPaginationResponse {
    pub limit: i64,
    pub offset: i64,
    pub returned: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct PortfolioHoldingsEnvelope {
    pub data_available: bool,
    pub holdings: Vec<PortfolioHoldingResponse>,
    pub pagination: PortfolioHoldingsPaginationResponse,
    pub reason: Option<&'static str>,
}

pub async fn get_portfolio_summary(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath(id_portfolio): ApiPath<Uuid>,
) -> Result<Json<PortfolioSummaryEnvelope>, ApiError> {
    let view = state
        .portfolio_read_model_service
        .get_summary(GetPortfolioSummaryInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioSummaryEnvelope::from(view)))
}

pub async fn get_portfolio_holdings(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath(id_portfolio): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<ListPortfolioHoldingsQuery>,
) -> Result<Json<PortfolioHoldingsEnvelope>, ApiError> {
    let view = state
        .portfolio_read_model_service
        .list_holdings(ListPortfolioHoldingsInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            limit: query.limit,
            offset: query.offset,
            sort: parse_holdings_sort(query.sort.as_deref())?,
            asset_class: parse_asset_class(query.asset_class.as_deref())?,
            search: query.search,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioHoldingsEnvelope::from(view)))
}

impl From<PortfolioSummary> for PortfolioSummaryResponse {
    fn from(value: PortfolioSummary) -> Self {
        Self {
            id_portfolio: value.id_portfolio,
            base_currency: value.base_currency,
            total_value_minor: value.total_value_minor,
            cash_balance_minor: value.cash_balance_minor,
            total_invested_minor: value.total_invested_minor,
            total_pnl_minor: value.total_pnl_minor,
            total_pnl_pct: value.total_pnl_pct,
            portfolio_status: match value.portfolio_status {
                PortfolioSummaryStatus::Active => "active",
                PortfolioSummaryStatus::Empty => "empty",
                PortfolioSummaryStatus::Archived => "archived",
            }
            .to_string(),
            is_estimated: value.is_estimated,
            as_of: format_datetime(value.as_of),
            updated_at: format_datetime(value.updated_at),
        }
    }
}

impl From<Asset> for HoldingAssetResponse {
    fn from(value: Asset) -> Self {
        Self {
            id_asset: value.id_asset,
            name: value.name,
            ticker: value.ticker,
            isin: value.isin,
            exchange: value.exchange,
            asset_class: value.asset_class.as_str().to_string(),
            status: value.status.as_str().to_string(),
            native_currency: value.native_currency,
        }
    }
}

impl From<PortfolioHolding> for PortfolioHoldingResponse {
    fn from(value: PortfolioHolding) -> Self {
        Self {
            id_asset: value.id_asset,
            asset: Asset::from(value.asset).into(),
            base_currency: value.base_currency,
            quantity: value.quantity,
            avg_cost_minor: value.avg_cost_minor,
            invested_base_minor: value.invested_base_minor,
            market_value_minor: value.market_value_minor,
            pnl_base_minor: value.pnl_base_minor,
            pnl_pct: value.pnl_pct,
            weight_pct: value.weight_pct,
            position_status: match value.position_status {
                PortfolioHoldingPositionStatus::Open => "open",
                PortfolioHoldingPositionStatus::Closed => "closed",
            }
            .to_string(),
            is_estimated: value.is_estimated,
            as_of: format_datetime(value.as_of),
            updated_at: format_datetime(value.updated_at),
        }
    }
}

impl From<PortfolioSummaryView> for PortfolioSummaryEnvelope {
    fn from(value: PortfolioSummaryView) -> Self {
        Self {
            data_available: value.data_available,
            summary: value.summary.map(PortfolioSummaryResponse::from),
            reason: value.reason,
        }
    }
}

impl From<PortfolioHoldingsView> for PortfolioHoldingsEnvelope {
    fn from(value: PortfolioHoldingsView) -> Self {
        Self {
            data_available: value.data_available,
            holdings: value
                .holdings
                .into_iter()
                .map(PortfolioHoldingResponse::from)
                .collect(),
            pagination: PortfolioHoldingsPaginationResponse {
                limit: value.pagination.limit,
                offset: value.pagination.offset,
                returned: value.pagination.returned,
                has_more: value.pagination.has_more,
            },
            reason: value.reason,
        }
    }
}

fn parse_holdings_sort(value: Option<&str>) -> Result<Option<PortfolioHoldingsSort>, ApiError> {
    match value {
        None => Ok(None),
        Some(value) => PortfolioHoldingsSort::try_from(value)
            .map(Some)
            .map_err(|_| ApiError::Validation {
                code: "invalid_sort",
                message: "sort must be one of weight_desc, value_desc, name_asc",
            }),
    }
}

fn parse_asset_class(value: Option<&str>) -> Result<Option<AssetClass>, ApiError> {
    match value {
        None => Ok(None),
        Some(value) => AssetClass::try_from(value)
            .map(Some)
            .map_err(|_| ApiError::Validation {
                code: "invalid_asset_class",
                message: "asset_class must be a supported asset class",
            }),
    }
}

fn format_datetime(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("OffsetDateTime should always be serializable as RFC3339")
}

fn map_service_error(error: PortfolioReadModelServiceError) -> ApiError {
    match error {
        PortfolioReadModelServiceError::Validation { code, message } => {
            ApiError::Validation { code, message }
        }
        PortfolioReadModelServiceError::NotFound => ApiError::NotFound {
            code: "portfolio_not_found",
            message: "portfolio was not found",
        },
        PortfolioReadModelServiceError::Internal => ApiError::Internal {
            code: "portfolio_read_model_service_failed",
            message: "failed to process portfolio dashboard request",
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
            assets::AssetRepository, portfolio_operations::PortfolioOperationRepository,
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
    use serde_json::Value;
    use sqlx::{PgPool, Row, postgres::PgPoolOptions};
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    async fn test_pool() -> PgPool {
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
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

    async fn create_portfolio(pool: &PgPool, id_user: Uuid, name: &str) -> Uuid {
        sqlx::query(
            r#"
            INSERT INTO portfolios (id_user, name, base_currency, visibility)
            VALUES ($1, $2, 'EUR', 'private')
            RETURNING id_portfolio
            "#,
        )
        .bind(id_user)
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("portfolio should be inserted")
        .try_get("id_portfolio")
        .expect("id_portfolio should be returned")
    }

    async fn soft_delete_portfolio(pool: &PgPool, id_portfolio: Uuid) {
        sqlx::query(
            r#"
            UPDATE portfolios
            SET deleted_at = created_at + interval '1 second'
            WHERE id_portfolio = $1
            "#,
        )
        .bind(id_portfolio)
        .execute(pool)
        .await
        .expect("portfolio should be soft deleted");
    }

    async fn insert_asset(
        pool: &PgPool,
        name: &str,
        ticker: &str,
        isin: &str,
        asset_class: &str,
    ) -> Uuid {
        sqlx::query(
            r#"
            INSERT INTO assets (asset_class, status, name, native_currency, isin, ticker, exchange, symbol)
            VALUES ($1, 'active', $2, 'USD', $3, $4, 'NYSE', $4)
            RETURNING id_asset
            "#,
        )
        .bind(asset_class)
        .bind(name)
        .bind(isin)
        .bind(ticker)
        .fetch_one(pool)
        .await
        .expect("asset should be inserted")
        .try_get("id_asset")
        .expect("id_asset should be returned")
    }

    async fn insert_summary(pool: &PgPool, id_portfolio: Uuid, total_value_minor: i64) {
        sqlx::query(
            r#"
            INSERT INTO rm_portfolio_summary (
                id_portfolio,
                base_currency,
                total_value_minor,
                cash_balance_minor,
                total_invested_minor,
                total_pnl_minor,
                total_pnl_pct,
                portfolio_status,
                is_estimated,
                as_of
            )
            VALUES ($1, 'EUR', $2, 1000, 5000, 250, 5.0000, 'active', false, now())
            "#,
        )
        .bind(id_portfolio)
        .bind(total_value_minor)
        .execute(pool)
        .await
        .expect("summary should be inserted");
    }

    struct HoldingFixture<'a> {
        id_portfolio: Uuid,
        id_asset: Uuid,
        quantity: &'a str,
        invested_base_minor: i64,
        market_value_minor: i64,
        pnl_base_minor: i64,
        pnl_pct: &'a str,
        weight_pct: &'a str,
        position_status: &'a str,
        as_of: &'a str,
    }

    async fn insert_holding(pool: &PgPool, input: HoldingFixture<'_>) {
        sqlx::query(
            r#"
            INSERT INTO rm_portfolio_holdings (
                id_portfolio,
                id_asset,
                base_currency,
                quantity,
                avg_cost_minor,
                invested_base_minor,
                market_value_minor,
                pnl_base_minor,
                pnl_pct,
                weight_pct,
                position_status,
                is_estimated,
                as_of
            )
            VALUES ($1, $2, 'EUR', $3::numeric, 1000, $4, $5, $6, $7::numeric, $8::numeric, $9, false, $10::timestamptz)
            "#,
        )
        .bind(input.id_portfolio)
        .bind(input.id_asset)
        .bind(input.quantity)
        .bind(input.invested_base_minor)
        .bind(input.market_value_minor)
        .bind(input.pnl_base_minor)
        .bind(input.pnl_pct)
        .bind(input.weight_pct)
        .bind(input.position_status)
        .bind(input.as_of)
        .execute(pool)
        .await
        .expect("holding should be inserted");
    }

    async fn cleanup_tree(
        pool: &PgPool,
        portfolio_ids: &[Uuid],
        asset_ids: &[Uuid],
        user_ids: &[Uuid],
    ) {
        if !portfolio_ids.is_empty() {
            sqlx::query("DELETE FROM rm_portfolio_holdings WHERE id_portfolio = ANY($1)")
                .bind(portfolio_ids)
                .execute(pool)
                .await
                .expect("holdings should be deleted");
            sqlx::query("DELETE FROM rm_portfolio_summary WHERE id_portfolio = ANY($1)")
                .bind(portfolio_ids)
                .execute(pool)
                .await
                .expect("summaries should be deleted");
            sqlx::query("DELETE FROM portfolios WHERE id_portfolio = ANY($1)")
                .bind(portfolio_ids)
                .execute(pool)
                .await
                .expect("portfolios should be deleted");
        }

        if !asset_ids.is_empty() {
            sqlx::query("DELETE FROM assets WHERE id_asset = ANY($1)")
                .bind(asset_ids)
                .execute(pool)
                .await
                .expect("assets should be deleted");
        }

        if !user_ids.is_empty() {
            sqlx::query("DELETE FROM users WHERE id_user = ANY($1)")
                .bind(user_ids)
                .execute(pool)
                .await
                .expect("users should be deleted");
        }
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

    fn build_token(id_user: Uuid, token_type: TokenType, public_handle: &str) -> String {
        let now = OffsetDateTime::now_utc();
        let claims = AuthClaims {
            sub: id_user,
            public_handle: public_handle.to_string(),
            role: UserRole::User,
            token_type,
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

    fn build_access_token(id_user: Uuid, public_handle: &str) -> String {
        build_token(id_user, TokenType::Access, public_handle)
    }

    fn build_refresh_token(id_user: Uuid, public_handle: &str) -> String {
        build_token(id_user, TokenType::Refresh, public_handle)
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        serde_json::from_slice(&bytes).expect("response body should be valid JSON")
    }

    fn assert_rfc3339_string(value: &Value) {
        let value = value.as_str().expect("value should be a string");
        OffsetDateTime::parse(value, &Rfc3339).expect("value should be valid RFC3339");
    }

    #[tokio::test]
    async fn get_summary_with_existing_read_model_returns_data_available_true() {
        let pool = test_pool().await;
        let handle = format!("prs{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Summary Portfolio").await;
        insert_summary(&pool, portfolio_id, 123456).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/summary"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["data_available"], true);
        assert_eq!(body["summary"]["total_value_minor"], 123456);
        assert_rfc3339_string(&body["summary"]["as_of"]);
        assert_rfc3339_string(&body["summary"]["updated_at"]);

        cleanup_tree(&pool, &[portfolio_id], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_summary_missing_returns_data_available_false() {
        let pool = test_pool().await;
        let handle = format!("prm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Missing Summary").await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/summary"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["data_available"], false);
        assert!(body["summary"].is_null());
        assert_eq!(body["reason"], "read_model_missing");

        cleanup_tree(&pool, &[portfolio_id], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_summary_auth_cross_user_and_soft_delete_are_enforced() {
        let pool = test_pool().await;
        let owner = format!("pro{}", &Uuid::new_v4().simple().to_string()[..12]);
        let other = format!("prx{}", &Uuid::new_v4().simple().to_string()[..12]);
        let owner_id = create_user(&pool, &owner).await;
        let other_id = create_user(&pool, &other).await;
        let portfolio_id = create_portfolio(&pool, owner_id, "Owned Summary").await;
        insert_summary(&pool, portfolio_id, 100).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let no_token = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/summary"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        assert_eq!(no_token.status(), StatusCode::UNAUTHORIZED);

        let refresh = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/summary"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_refresh_token(owner_id, &owner)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        assert_eq!(refresh.status(), StatusCode::UNAUTHORIZED);

        let cross_user = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/summary"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(other_id, &other)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        assert_eq!(cross_user.status(), StatusCode::NOT_FOUND);

        soft_delete_portfolio(&pool, portfolio_id).await;
        let soft_deleted = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/summary"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(owner_id, &owner)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        assert_eq!(soft_deleted.status(), StatusCode::NOT_FOUND);

        cleanup_tree(&pool, &[portfolio_id], &[], &[owner_id, other_id]).await;
    }

    #[tokio::test]
    async fn get_holdings_with_existing_rows_returns_data_available_true() {
        let pool = test_pool().await;
        let handle = format!("phh{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Holdings Portfolio").await;
        let asset_a = insert_asset(&pool, "Holding Alpha", "HALP", "US1000000001", "equity").await;
        let asset_b = insert_asset(&pool, "Holding Beta", "HBET", "US1000000002", "etf").await;
        insert_summary(&pool, portfolio_id, 500000).await;
        insert_holding(
            &pool,
            HoldingFixture {
                id_portfolio: portfolio_id,
                id_asset: asset_a,
                quantity: "10.5000000000",
                invested_base_minor: 100000,
                market_value_minor: 300000,
                pnl_base_minor: 200000,
                pnl_pct: "20.5000",
                weight_pct: "60.0000",
                position_status: "open",
                as_of: "2026-06-05T14:30:00Z",
            },
        )
        .await;
        insert_holding(
            &pool,
            HoldingFixture {
                id_portfolio: portfolio_id,
                id_asset: asset_b,
                quantity: "4.0000000000",
                invested_base_minor: 90000,
                market_value_minor: 200000,
                pnl_base_minor: 110000,
                pnl_pct: "12.0000",
                weight_pct: "40.0000",
                position_status: "open",
                as_of: "2026-06-05T14:31:00Z",
            },
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/holdings"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["data_available"], true);
        assert_eq!(body["holdings"].as_array().unwrap().len(), 2);
        assert_eq!(body["holdings"][0]["asset"]["name"], "Holding Alpha");
        assert_rfc3339_string(&body["holdings"][0]["as_of"]);
        assert_rfc3339_string(&body["holdings"][0]["updated_at"]);

        cleanup_tree(&pool, &[portfolio_id], &[asset_a, asset_b], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_holdings_missing_summary_returns_data_available_false() {
        let pool = test_pool().await;
        let handle = format!("phm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Missing Holdings").await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/holdings"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["data_available"], false);
        assert!(body["holdings"].as_array().unwrap().is_empty());
        assert_eq!(body["reason"], "read_model_missing");

        cleanup_tree(&pool, &[portfolio_id], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_holdings_with_empty_read_model_returns_documented_empty_behavior() {
        let pool = test_pool().await;
        let handle = format!("phe{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Empty Holdings").await;
        insert_summary(&pool, portfolio_id, 0).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/holdings"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let body = response_json(response).await;
        assert_eq!(body["data_available"], true);
        assert!(body["holdings"].as_array().unwrap().is_empty());

        cleanup_tree(&pool, &[portfolio_id], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_holdings_pagination_filters_and_sort_work() {
        let pool = test_pool().await;
        let handle = format!("php{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Paged Holdings").await;
        let asset_a = insert_asset(
            &pool,
            "Alpha Holding Search",
            "AHS1",
            "US2000000001",
            "equity",
        )
        .await;
        let asset_b =
            insert_asset(&pool, "Beta Holding Search", "BHS1", "US2000000002", "etf").await;
        insert_summary(&pool, portfolio_id, 1000).await;
        insert_holding(
            &pool,
            HoldingFixture {
                id_portfolio: portfolio_id,
                id_asset: asset_a,
                quantity: "1.0000000000",
                invested_base_minor: 100,
                market_value_minor: 700,
                pnl_base_minor: 600,
                pnl_pct: "10.0000",
                weight_pct: "70.0000",
                position_status: "open",
                as_of: "2026-06-05T14:30:00Z",
            },
        )
        .await;
        insert_holding(
            &pool,
            HoldingFixture {
                id_portfolio: portfolio_id,
                id_asset: asset_b,
                quantity: "2.0000000000",
                invested_base_minor: 200,
                market_value_minor: 300,
                pnl_base_minor: 100,
                pnl_pct: "5.0000",
                weight_pct: "30.0000",
                position_status: "open",
                as_of: "2026-06-05T14:31:00Z",
            },
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let first_page = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/holdings?limit=1"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let first_body = response_json(first_page).await;
        assert_eq!(first_body["pagination"]["limit"], 1);
        assert_eq!(first_body["pagination"]["has_more"], true);
        assert_eq!(
            first_body["holdings"][0]["asset"]["name"],
            "Alpha Holding Search"
        );

        let second_page = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/holdings?limit=1&offset=1"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let second_body = response_json(second_page).await;
        assert_eq!(second_body["pagination"]["offset"], 1);
        assert_eq!(
            second_body["holdings"][0]["asset"]["name"],
            "Beta Holding Search"
        );

        let value_sorted = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/holdings?sort=value_desc"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let value_body = response_json(value_sorted).await;
        assert_eq!(
            value_body["holdings"][0]["asset"]["name"],
            "Alpha Holding Search"
        );

        let filtered = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/holdings?asset_class=etf&search=Beta"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let filtered_body = response_json(filtered).await;
        assert_eq!(filtered_body["holdings"].as_array().unwrap().len(), 1);
        assert_eq!(
            filtered_body["holdings"][0]["asset"]["name"],
            "Beta Holding Search"
        );

        cleanup_tree(&pool, &[portfolio_id], &[asset_a, asset_b], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_holdings_invalid_query_and_security_rules_are_enforced() {
        let pool = test_pool().await;
        let owner = format!("pho{}", &Uuid::new_v4().simple().to_string()[..12]);
        let other = format!("phx{}", &Uuid::new_v4().simple().to_string()[..12]);
        let owner_id = create_user(&pool, &owner).await;
        let other_id = create_user(&pool, &other).await;
        let portfolio_id = create_portfolio(&pool, owner_id, "Secure Holdings").await;
        insert_summary(&pool, portfolio_id, 50).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        for (uri, expected_code) in [
            (
                format!("/v1/portfolios/{portfolio_id}/holdings?limit=101"),
                "invalid_limit",
            ),
            (
                format!("/v1/portfolios/{portfolio_id}/holdings?offset=-1"),
                "invalid_offset",
            ),
            (
                format!("/v1/portfolios/{portfolio_id}/holdings?sort=boom"),
                "invalid_sort",
            ),
            (
                format!("/v1/portfolios/{portfolio_id}/holdings?asset_class=boom"),
                "invalid_asset_class",
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .header(
                            AUTHORIZATION,
                            format!("Bearer {}", build_access_token(owner_id, &owner)),
                        )
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .expect("response should be built");
            let status = response.status();
            let body = response_json(response).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(body["error"]["code"], expected_code);
        }

        let no_token = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/holdings"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        assert_eq!(no_token.status(), StatusCode::UNAUTHORIZED);

        let refresh = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/holdings"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_refresh_token(owner_id, &owner)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        assert_eq!(refresh.status(), StatusCode::UNAUTHORIZED);

        let cross_user = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/holdings"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(other_id, &other)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        assert_eq!(cross_user.status(), StatusCode::NOT_FOUND);

        soft_delete_portfolio(&pool, portfolio_id).await;
        let soft_deleted = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/holdings"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(owner_id, &owner)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        assert_eq!(soft_deleted.status(), StatusCode::NOT_FOUND);

        cleanup_tree(&pool, &[portfolio_id], &[], &[owner_id, other_id]).await;
    }

    #[tokio::test]
    async fn get_holdings_non_numeric_limit_returns_normalized_error() {
        let pool = test_pool().await;
        let handle = format!("phq{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Bad Query Holdings").await;
        insert_summary(&pool, portfolio_id, 50).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/holdings?limit=abc"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_query_parameters");

        cleanup_tree(&pool, &[portfolio_id], &[], &[user_id]).await;
    }

    #[test]
    fn portfolio_read_models_module_is_read_only_and_does_not_touch_worker_tables() {
        let repository_source = include_str!("../repositories/portfolio_read_models.rs");
        let service_source = include_str!("../services/portfolio_read_models.rs");

        for forbidden in [
            "INSERT INTO rm_portfolio_summary",
            "UPDATE rm_portfolio_summary",
            "DELETE FROM rm_portfolio_summary",
            "INSERT INTO rm_portfolio_holdings",
            "UPDATE rm_portfolio_holdings",
            "DELETE FROM rm_portfolio_holdings",
            "INSERT INTO portfolio_snapshots_daily",
            "UPDATE portfolio_snapshots_daily",
            "DELETE FROM portfolio_snapshots_daily",
            "INSERT INTO portfolio_holding_snapshot_daily",
            "UPDATE portfolio_holding_snapshot_daily",
            "DELETE FROM portfolio_holding_snapshot_daily",
            "INSERT INTO asset_market_data",
            "UPDATE asset_market_data",
            "DELETE FROM asset_market_data",
            "asset_price_history_cache",
            "INSERT INTO portfolio_operations",
            "UPDATE portfolio_operations",
            "DELETE FROM portfolio_operations",
        ] {
            assert!(!repository_source.contains(forbidden));
            assert!(!service_source.contains(forbidden));
        }
    }
}
