use crate::{
    auth::AuthenticatedUser,
    domain::asset::{
        AssetAlias, AssetClass, AssetDetails, AssetMarketData, AssetMetadata, AssetStatus,
    },
    errors::ApiError,
    http::extractors::{ApiPath, ApiQuery},
    services::assets::{AssetServiceError, ListAssetsInput, ListAssetsView},
    state::AppState,
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct ListAssetsQuery {
    pub search: Option<String>,
    pub asset_class: Option<String>,
    pub ticker: Option<String>,
    pub isin: Option<String>,
    pub exchange: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AssetResponse {
    pub id_asset: Uuid,
    pub name: String,
    pub ticker: Option<String>,
    pub isin: Option<String>,
    pub exchange: Option<String>,
    pub symbol: Option<String>,
    pub network: Option<String>,
    pub asset_class: String,
    pub status: String,
    pub native_currency: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Option<AssetMetadataResponse>,
    pub market_data: Option<AssetMarketDataResponse>,
    pub aliases: Option<Vec<AssetAliasResponse>>,
}

#[derive(Debug, Serialize)]
pub struct AssetMetadataResponse {
    pub country: Option<String>,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
    pub description: Option<String>,
    pub provider: Option<String>,
    pub provider_asset_id: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AssetMarketDataResponse {
    pub price_minor: i64,
    pub currency: String,
    pub market_cap_minor: Option<i64>,
    pub volume_24h_minor: Option<i64>,
    pub change_24h_pct: Option<String>,
    pub change_7d_pct: Option<String>,
    pub change_30d_pct: Option<String>,
    pub data_source: Option<String>,
    pub source_asset_id: Option<String>,
    pub as_of: String,
}

#[derive(Debug, Serialize)]
pub struct AssetAliasResponse {
    pub alias: String,
    pub alias_type: Option<String>,
    pub source: Option<String>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AssetEnvelope {
    pub asset: AssetResponse,
}

#[derive(Debug, Serialize)]
pub struct AssetPaginationResponse {
    pub limit: i64,
    pub offset: i64,
    pub returned: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct AssetListResponse {
    pub assets: Vec<AssetResponse>,
    pub pagination: AssetPaginationResponse,
}

pub async fn list_assets(
    State(state): State<AppState>,
    _authenticated: AuthenticatedUser,
    ApiQuery(query): ApiQuery<ListAssetsQuery>,
) -> Result<Json<AssetListResponse>, ApiError> {
    let view = state
        .asset_service
        .list_assets(ListAssetsInput {
            search: query.search,
            asset_class: parse_asset_class(query.asset_class.as_deref())?,
            ticker: query.ticker,
            isin: query.isin,
            exchange: query.exchange,
            status: parse_asset_status(query.status.as_deref())?,
            limit: query.limit,
            offset: query.offset,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(AssetListResponse::from(view)))
}

pub async fn get_asset(
    State(state): State<AppState>,
    _authenticated: AuthenticatedUser,
    ApiPath(id_asset): ApiPath<String>,
) -> Result<Json<AssetEnvelope>, ApiError> {
    let id_asset = Uuid::parse_str(&id_asset).map_err(|_| ApiError::Validation {
        code: "invalid_asset_id",
        message: "asset id must be a valid UUID",
    })?;

    let asset = state
        .asset_service
        .get_asset(id_asset)
        .await
        .map_err(map_service_error)?;

    Ok(Json(AssetEnvelope {
        asset: AssetResponse::from_detail(asset, true),
    }))
}

impl AssetResponse {
    fn from_detail(value: AssetDetails, include_aliases: bool) -> Self {
        Self {
            id_asset: value.asset.id_asset,
            name: value.asset.name,
            ticker: value.asset.ticker,
            isin: value.asset.isin,
            exchange: value.asset.exchange,
            symbol: value.asset.symbol,
            network: value.asset.network,
            asset_class: value.asset.asset_class.as_str().to_string(),
            status: value.asset.status.as_str().to_string(),
            native_currency: value.asset.native_currency,
            created_at: format_datetime(value.asset.created_at),
            updated_at: format_datetime(value.asset.updated_at),
            metadata: value.metadata.map(AssetMetadataResponse::from),
            market_data: value.market_data.map(AssetMarketDataResponse::from),
            aliases: include_aliases.then(|| {
                value
                    .aliases
                    .into_iter()
                    .map(AssetAliasResponse::from)
                    .collect()
            }),
        }
    }
}

impl From<AssetMetadata> for AssetMetadataResponse {
    fn from(value: AssetMetadata) -> Self {
        Self {
            country: value.country,
            website_url: value.website_url,
            logo_url: value.logo_url,
            description: value.description,
            provider: value.provider,
            provider_asset_id: value.provider_asset_id,
            sector: value.sector,
            industry: value.industry,
            last_synced_at: value.last_synced_at.map(format_datetime),
        }
    }
}

impl From<AssetMarketData> for AssetMarketDataResponse {
    fn from(value: AssetMarketData) -> Self {
        Self {
            price_minor: value.price_minor,
            currency: value.currency,
            market_cap_minor: value.market_cap_minor,
            volume_24h_minor: value.volume_24h_minor,
            change_24h_pct: value.change_24h_pct,
            change_7d_pct: value.change_7d_pct,
            change_30d_pct: value.change_30d_pct,
            data_source: value.data_source,
            source_asset_id: value.source_asset_id,
            as_of: format_datetime(value.as_of),
        }
    }
}

impl From<AssetAlias> for AssetAliasResponse {
    fn from(value: AssetAlias) -> Self {
        Self {
            alias: value.alias,
            alias_type: value.alias_type,
            source: value.source,
            valid_from: value.valid_from.map(format_date),
            valid_to: value.valid_to.map(format_date),
        }
    }
}

impl From<ListAssetsView> for AssetListResponse {
    fn from(value: ListAssetsView) -> Self {
        Self {
            assets: value
                .assets
                .into_iter()
                .map(|asset| AssetResponse::from_detail(asset, false))
                .collect(),
            pagination: AssetPaginationResponse {
                limit: value.pagination.limit,
                offset: value.pagination.offset,
                returned: value.pagination.returned,
                has_more: value.pagination.has_more,
            },
        }
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

fn parse_asset_status(value: Option<&str>) -> Result<Option<AssetStatus>, ApiError> {
    match value {
        None => Ok(None),
        Some(value) => AssetStatus::try_from(value)
            .map(Some)
            .map_err(|_| ApiError::Validation {
                code: "invalid_asset_status",
                message: "status must be one of active, inactive, delisted, merged",
            }),
    }
}

fn format_datetime(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("OffsetDateTime should always be serializable as RFC3339")
}

fn format_date(value: Date) -> String {
    value.to_string()
}

fn map_service_error(error: AssetServiceError) -> ApiError {
    match error {
        AssetServiceError::Validation { code, message } => ApiError::Validation { code, message },
        AssetServiceError::NotFound { code, message } => ApiError::NotFound { code, message },
        AssetServiceError::Internal => ApiError::Internal {
            code: "asset_service_failed",
            message: "failed to process asset request",
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

    async fn ensure_role(pool: &PgPool, id_role: i16, label: &str) {
        // `ON CONFLICT (label) DO NOTHING` is the race-safe shape under cargo's
        // parallel test runner: two tests that both call ensure_role at the
        // same time would otherwise pass the per-row uniqueness checks
        // independently and the second commit would fail on `uq_roles_label`
        // (CI runs only `001_init.sql`, so the roles table is empty on first
        // invocation). Conflict-on-label keeps the existing row when any other
        // test won the race.
        sqlx::query(
            r#"
            INSERT INTO roles (id_role, label)
            VALUES ($1, $2)
            ON CONFLICT (label) DO NOTHING
            "#,
        )
        .bind(id_role)
        .bind(label)
        .execute(pool)
        .await
        .expect("role should be upserted");
    }

    async fn create_user(pool: &PgPool, public_handle: &str) -> Uuid {
        ensure_role(pool, 1, "user").await;

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

    async fn create_sector(pool: &PgPool, label: &str) -> i16 {
        let id_sector: i32 =
            sqlx::query_scalar("SELECT COALESCE(MAX(id_sector), 0) + 1 FROM sectors")
                .fetch_one(pool)
                .await
                .expect("sector id should be generated");

        sqlx::query(
            r#"
            INSERT INTO sectors (id_sector, label)
            VALUES ($1, $2)
            "#,
        )
        .bind(id_sector as i16)
        .bind(label)
        .execute(pool)
        .await
        .expect("sector should be inserted");

        id_sector as i16
    }

    async fn create_industry(pool: &PgPool, id_sector: i16, label: &str) -> i16 {
        let id_industry: i32 =
            sqlx::query_scalar("SELECT COALESCE(MAX(id_industry), 0) + 1 FROM industries")
                .fetch_one(pool)
                .await
                .expect("industry id should be generated");

        sqlx::query(
            r#"
            INSERT INTO industries (id_industry, id_sector, label)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(id_industry as i16)
        .bind(id_sector)
        .bind(label)
        .execute(pool)
        .await
        .expect("industry should be inserted");

        id_industry as i16
    }

    async fn insert_asset(
        pool: &PgPool,
        name: &str,
        ticker: &str,
        isin: &str,
        exchange: &str,
        asset_class: &str,
        status: &str,
    ) -> Uuid {
        sqlx::query(
            r#"
            INSERT INTO assets (
                asset_class,
                status,
                name,
                native_currency,
                isin,
                ticker,
                exchange,
                symbol
            )
            VALUES ($1, $2, $3, 'USD', $4, $5, $6, $7)
            RETURNING id_asset
            "#,
        )
        .bind(asset_class)
        .bind(status)
        .bind(name)
        .bind(isin)
        .bind(ticker)
        .bind(exchange)
        .bind(ticker)
        .fetch_one(pool)
        .await
        .expect("asset should be inserted")
        .try_get("id_asset")
        .expect("id_asset should be returned")
    }

    async fn insert_asset_metadata(
        pool: &PgPool,
        id_asset: Uuid,
        id_industry: i16,
        provider_asset_id: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO asset_metadata (
                id_asset,
                id_industry,
                country,
                website_url,
                logo_url,
                description,
                provider,
                provider_asset_id,
                last_synced_at
            )
            VALUES ($1, $2, 'USA', 'https://example.com', 'https://example.com/logo.png', 'Test asset', 'fixture', $3, now())
            "#,
        )
        .bind(id_asset)
        .bind(id_industry)
        .bind(provider_asset_id)
        .execute(pool)
        .await
        .expect("asset metadata should be inserted");
    }

    async fn insert_asset_market_data(pool: &PgPool, id_asset: Uuid) {
        sqlx::query(
            r#"
            INSERT INTO asset_market_data (
                id_asset,
                price_minor,
                currency,
                market_cap_minor,
                volume_24h_minor,
                change_24h_pct,
                change_7d_pct,
                change_30d_pct,
                data_source,
                source_asset_id,
                as_of
            )
            VALUES ($1, 12345, 'USD', 999999, 4444, 1.5000, 2.2500, 3.7500, 'fixture', 'asset-source', now())
            "#,
        )
        .bind(id_asset)
        .execute(pool)
        .await
        .expect("asset market data should be inserted");
    }

    async fn insert_asset_alias(pool: &PgPool, id_asset: Uuid, alias: &str) {
        sqlx::query(
            r#"
            INSERT INTO asset_aliases (id_asset, alias, alias_type, source, valid_from, valid_to)
            VALUES ($1, $2, 'ticker', 'fixture', DATE '2026-01-01', DATE '2026-12-31')
            "#,
        )
        .bind(id_asset)
        .bind(alias)
        .execute(pool)
        .await
        .expect("asset alias should be inserted");
    }

    async fn cleanup_assets(
        pool: &PgPool,
        asset_ids: &[Uuid],
        industry_ids: &[i16],
        sector_ids: &[i16],
        user_ids: &[Uuid],
    ) {
        if !asset_ids.is_empty() {
            sqlx::query("DELETE FROM asset_aliases WHERE id_asset = ANY($1)")
                .bind(asset_ids)
                .execute(pool)
                .await
                .expect("asset aliases should be deleted");

            sqlx::query("DELETE FROM asset_metadata WHERE id_asset = ANY($1)")
                .bind(asset_ids)
                .execute(pool)
                .await
                .expect("asset metadata should be deleted");

            sqlx::query("DELETE FROM asset_market_data WHERE id_asset = ANY($1)")
                .bind(asset_ids)
                .execute(pool)
                .await
                .expect("asset market data should be deleted");

            sqlx::query("DELETE FROM assets WHERE id_asset = ANY($1)")
                .bind(asset_ids)
                .execute(pool)
                .await
                .expect("assets should be deleted");
        }

        if !industry_ids.is_empty() {
            sqlx::query("DELETE FROM industries WHERE id_industry = ANY($1)")
                .bind(industry_ids)
                .execute(pool)
                .await
                .expect("industries should be deleted");
        }

        if !sector_ids.is_empty() {
            sqlx::query("DELETE FROM sectors WHERE id_sector = ANY($1)")
                .bind(sector_ids)
                .execute(pool)
                .await
                .expect("sectors should be deleted");
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
        let portfolio_service = PortfolioService::new(portfolio_repository.clone());
        let portfolio_operation_service = PortfolioOperationService::new(
            AssetRepository::new(pool.clone()),
            portfolio_repository.clone(),
            PortfolioOperationRepository::new(pool.clone()),
            PortfolioRefreshRequestRepository::new(pool.clone()),
        );
        let asset_service = AssetService::new(AssetRepository::new(pool.clone()));
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
            portfolio_service,
            portfolio_operation_service,
            asset_service,
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
    async fn list_assets_without_token_returns_401() {
        let pool = test_pool().await;
        let app = crate::http::router(test_state(pool).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/assets")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_assets_with_refresh_token_returns_401() {
        let pool = test_pool().await;
        let handle = format!("asr{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/assets")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_refresh_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_assets(&pool, &[], &[], &[], &[user_id]).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_asset_without_token_returns_401() {
        let pool = test_pool().await;
        let app = crate::http::router(test_state(pool).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/assets/{}", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_asset_with_refresh_token_returns_401() {
        let pool = test_pool().await;
        let handle = format!("agr{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let asset_suffix = Uuid::new_v4().simple().to_string();
        let ticker = format!("AR{}", &asset_suffix[..4]).to_uppercase();
        let isin = format!("US{}", &asset_suffix[..10]).to_uppercase();
        let asset_id = insert_asset(
            &pool,
            "Asset Refresh",
            &ticker,
            &isin,
            "NASDAQ",
            "equity",
            "active",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/assets/{asset_id}"))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_refresh_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        cleanup_assets(&pool, &[asset_id], &[], &[], &[user_id]).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_assets_returns_paginated_response_and_default_limit() {
        let pool = test_pool().await;
        let handle = format!("ald{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let asset_id = insert_asset(
            &pool,
            "Asset Default",
            "ADF",
            "US0000000002",
            "NYSE",
            "equity",
            "active",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/assets")
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
        assert_eq!(body["pagination"]["limit"], 50);
        assert!(body["pagination"]["returned"].as_u64().unwrap() >= 1);
        assert_rfc3339_string(&body["assets"][0]["created_at"]);
        assert_rfc3339_string(&body["assets"][0]["updated_at"]);
        cleanup_assets(&pool, &[asset_id], &[], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn list_assets_explicit_limit_offset_and_has_more_work() {
        let pool = test_pool().await;
        let handle = format!("ale{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let asset_a = insert_asset(
            &pool,
            "Page Asset Alpha",
            "AAA",
            "US0000000003",
            "NYSE",
            "equity",
            "active",
        )
        .await;
        let asset_b = insert_asset(
            &pool,
            "Page Asset Beta",
            "BBB",
            "US0000000004",
            "NYSE",
            "equity",
            "active",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let first_page = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/assets?search=Page%20Asset&limit=1")
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
        assert_eq!(first_body["pagination"]["returned"], 1);
        assert_eq!(first_body["pagination"]["has_more"], true);
        assert_eq!(first_body["assets"][0]["name"], "Page Asset Alpha");

        let second_page = app
            .oneshot(
                Request::builder()
                    .uri("/v1/assets?search=Page%20Asset&limit=1&offset=1")
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
        assert_eq!(second_body["assets"][0]["name"], "Page Asset Beta");

        cleanup_assets(&pool, &[asset_a, asset_b], &[], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn list_assets_searches_by_name_ticker_isin_and_filters() {
        let pool = test_pool().await;
        let handle = format!("als{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let asset_equity = insert_asset(
            &pool,
            "Gamma Search Asset",
            "GSA",
            "US0000000005",
            "NASDAQ",
            "equity",
            "active",
        )
        .await;
        let asset_inactive = insert_asset(
            &pool,
            "Delta Search Asset",
            "DSA",
            "US0000000006",
            "NASDAQ",
            "fund",
            "inactive",
        )
        .await;
        insert_asset_alias(&pool, asset_equity, "Gamma Alias").await;
        let app = crate::http::router(test_state(pool.clone()).await);

        for (uri, expected_name) in [
            ("/v1/assets?search=Gamma%20Search", "Gamma Search Asset"),
            ("/v1/assets?search=GSA", "Gamma Search Asset"),
            ("/v1/assets?search=US0000000005", "Gamma Search Asset"),
            ("/v1/assets?search=Gamma%20Alias", "Gamma Search Asset"),
            (
                "/v1/assets?asset_class=equity&search=Gamma%20Search",
                "Gamma Search Asset",
            ),
            (
                "/v1/assets?status=inactive&search=Delta%20Search",
                "Delta Search Asset",
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
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
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["assets"].as_array().unwrap().len(), 1);
            assert_eq!(body["assets"][0]["name"], expected_name);
        }

        cleanup_assets(&pool, &[asset_equity, asset_inactive], &[], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn list_assets_invalid_filters_return_400() {
        let pool = test_pool().await;
        let handle = format!("ali{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        for (uri, code) in [
            ("/v1/assets?asset_class=invalid", "invalid_asset_class"),
            ("/v1/assets?status=invalid", "invalid_asset_status"),
            ("/v1/assets?limit=101", "invalid_limit"),
            ("/v1/assets?offset=-1", "invalid_offset"),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
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
            assert_eq!(body["error"]["code"], code);
        }

        cleanup_assets(&pool, &[], &[], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_existing_asset_includes_metadata_market_data_and_aliases() {
        let pool = test_pool().await;
        let handle = format!("agd{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let asset_suffix = Uuid::new_v4().simple().to_string();
        let detail_ticker = format!("DT{}", &asset_suffix[..4]).to_uppercase();
        let detail_isin = format!("US{}", &asset_suffix[..10]).to_uppercase();
        let asset_id = insert_asset(
            &pool,
            "Detail Asset",
            &detail_ticker,
            &detail_isin,
            "NASDAQ",
            "equity",
            "active",
        )
        .await;
        let sector_id = create_sector(&pool, "Technology Fixture").await;
        let industry_id = create_industry(&pool, sector_id, "Software Fixture").await;
        insert_asset_metadata(&pool, asset_id, industry_id, "provider-detail").await;
        insert_asset_market_data(&pool, asset_id).await;
        insert_asset_alias(&pool, asset_id, "Detail Alias").await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/assets/{asset_id}"))
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
        assert_eq!(body["asset"]["id_asset"], asset_id.to_string());
        assert_eq!(body["asset"]["metadata"]["sector"], "Technology Fixture");
        assert_eq!(body["asset"]["metadata"]["industry"], "Software Fixture");
        assert_eq!(body["asset"]["market_data"]["price_minor"], 12345);
        assert_eq!(body["asset"]["aliases"][0]["alias"], "Detail Alias");
        assert_rfc3339_string(&body["asset"]["created_at"]);
        assert_rfc3339_string(&body["asset"]["updated_at"]);
        assert_rfc3339_string(&body["asset"]["market_data"]["as_of"]);
        assert_eq!(body["asset"]["aliases"][0]["valid_from"], "2026-01-01");

        cleanup_assets(&pool, &[asset_id], &[industry_id], &[sector_id], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_missing_asset_returns_404_and_invalid_uuid_returns_400() {
        let pool = test_pool().await;
        let handle = format!("agm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let missing = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/assets/{}", Uuid::new_v4()))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        let invalid = app
            .oneshot(
                Request::builder()
                    .uri("/v1/assets/not-a-uuid")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let status = invalid.status();
        let body = response_json(invalid).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_asset_id");

        cleanup_assets(&pool, &[], &[], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_asset_without_optional_data_does_not_crash() {
        let pool = test_pool().await;
        let handle = format!("agn{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let asset_id = insert_asset(
            &pool,
            "No Optional Asset",
            "NOA",
            "US0000000009",
            "NYSE",
            "equity",
            "active",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/assets/{asset_id}"))
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
        assert_eq!(body["asset"]["id_asset"], asset_id.to_string());
        assert!(body["asset"]["metadata"].is_null());
        assert!(body["asset"]["market_data"].is_null());
        assert!(body["asset"]["aliases"].as_array().unwrap().is_empty());

        cleanup_assets(&pool, &[asset_id], &[], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn list_assets_handles_sql_injection_like_search_and_extra_query_params() {
        let pool = test_pool().await;
        let handle = format!("agi{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let asset_id = insert_asset(
            &pool,
            "Injection Asset",
            "IJA",
            "US0000000008",
            "NYSE",
            "equity",
            "active",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/assets?search=';DROP%20TABLE%20assets;--&unexpected=1")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(user_id, &handle)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");

        assert_eq!(response.status(), StatusCode::OK);
        cleanup_assets(&pool, &[asset_id], &[], &[], &[user_id]).await;
    }

    #[test]
    fn assets_repository_is_read_only_and_does_not_touch_worker_tables() {
        let repository_source = include_str!("../repositories/assets.rs");
        let service_source = include_str!("../services/assets.rs");

        for forbidden in [
            "INSERT INTO assets",
            "UPDATE assets",
            "DELETE FROM assets",
            "INSERT INTO asset_metadata",
            "UPDATE asset_metadata",
            "DELETE FROM asset_metadata",
            "INSERT INTO asset_market_data",
            "UPDATE asset_market_data",
            "DELETE FROM asset_market_data",
            "INSERT INTO asset_aliases",
            "UPDATE asset_aliases",
            "DELETE FROM asset_aliases",
            "asset_price_history_cache",
            "rm_portfolio_summary",
            "rm_portfolio_holdings",
            "portfolio_snapshots_daily",
            "portfolio_holding_snapshot_daily",
        ] {
            assert!(!repository_source.contains(forbidden));
            assert!(!service_source.contains(forbidden));
        }
    }
}
