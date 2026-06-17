use crate::{
    auth::AuthenticatedUser,
    domain::{
        asset::AssetClass,
        portfolio_snapshot::{
            HistoricalSnapshotHoldingsSort, PortfolioDailySnapshot, PortfolioDailySnapshotHolding,
            PortfolioSnapshotSourceType, PortfolioSnapshotsSort, SnapshotHoldingAssetIdentity,
        },
    },
    errors::ApiError,
    http::extractors::{ApiPath, ApiQuery},
    services::portfolio_snapshots::{
        GetPortfolioDailySnapshotHoldingsInput, ListPortfolioDailySnapshotsInput,
        PortfolioDailySnapshotHoldingsView, PortfolioDailySnapshotsView,
        PortfolioSnapshotServiceError,
    },
    state::AppState,
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct ListPortfolioDailySnapshotsQuery {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListHistoricalSnapshotHoldingsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<String>,
    pub asset_class: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PortfolioDailySnapshotResponse {
    pub id_portfolio_snapshot_daily: Uuid,
    pub id_portfolio: Uuid,
    pub snapshot_date: String,
    pub base_currency: String,
    pub cash_balance_minor: i64,
    pub total_value_minor: i64,
    pub total_invested_minor: i64,
    pub total_pnl_minor: i64,
    pub total_pnl_pct: Option<String>,
    pub is_estimated: bool,
    pub source_type: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct PortfolioSnapshotsPaginationResponse {
    pub limit: i64,
    pub offset: i64,
    pub returned: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct PortfolioDailySnapshotsEnvelope {
    pub data_available: bool,
    pub snapshots: Vec<PortfolioDailySnapshotResponse>,
    pub pagination: PortfolioSnapshotsPaginationResponse,
}

#[derive(Debug, Serialize)]
pub struct HistoricalSnapshotHoldingAssetResponse {
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
pub struct PortfolioDailySnapshotHoldingResponse {
    pub id_portfolio_holding_snapshot_daily: Uuid,
    pub id_portfolio_snapshot_daily: Uuid,
    pub id_asset: Uuid,
    pub asset: HistoricalSnapshotHoldingAssetResponse,
    pub base_currency: String,
    pub quantity: String,
    pub avg_cost_minor: Option<i64>,
    pub invested_minor: i64,
    pub market_value_minor: i64,
    pub pnl_minor: i64,
    pub pnl_pct: Option<String>,
    pub weight_pct: Option<String>,
    pub is_estimated: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct HistoricalSnapshotHoldingsEnvelope {
    pub data_available: bool,
    pub snapshot: Option<PortfolioDailySnapshotResponse>,
    pub holdings: Vec<PortfolioDailySnapshotHoldingResponse>,
    pub reason: Option<&'static str>,
    pub pagination: PortfolioSnapshotsPaginationResponse,
}

pub async fn list_portfolio_daily_snapshots(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath(id_portfolio): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<ListPortfolioDailySnapshotsQuery>,
) -> Result<Json<PortfolioDailySnapshotsEnvelope>, ApiError> {
    let view = state
        .portfolio_snapshot_service
        .list_daily_snapshots(ListPortfolioDailySnapshotsInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            date_from: parse_date(query.date_from.as_deref(), "date_from")?,
            date_to: parse_date(query.date_to.as_deref(), "date_to")?,
            limit: query.limit,
            offset: query.offset,
            sort: parse_sort(query.sort.as_deref())?,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(PortfolioDailySnapshotsEnvelope::from(view)))
}

pub async fn get_portfolio_daily_snapshot_holdings(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    ApiPath((id_portfolio, snapshot_date_raw)): ApiPath<(Uuid, String)>,
    ApiQuery(query): ApiQuery<ListHistoricalSnapshotHoldingsQuery>,
) -> Result<Json<HistoricalSnapshotHoldingsEnvelope>, ApiError> {
    let snapshot_date = Date::parse(
        &snapshot_date_raw,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|_| ApiError::Validation {
        code: "invalid_snapshot_date",
        message: "snapshot_date must be a valid ISO date (YYYY-MM-DD)",
    })?;

    let view = state
        .portfolio_snapshot_service
        .get_daily_snapshot_holdings(GetPortfolioDailySnapshotHoldingsInput {
            id_user: authenticated.claims.sub,
            id_portfolio,
            snapshot_date,
            limit: query.limit,
            offset: query.offset,
            sort: parse_snapshot_holdings_sort(query.sort.as_deref())?,
            asset_class: parse_asset_class(query.asset_class.as_deref())?,
            search: query.search,
        })
        .await
        .map_err(map_service_error)?;

    Ok(Json(HistoricalSnapshotHoldingsEnvelope::from(view)))
}

impl From<PortfolioDailySnapshot> for PortfolioDailySnapshotResponse {
    fn from(value: PortfolioDailySnapshot) -> Self {
        Self {
            id_portfolio_snapshot_daily: value.id_portfolio_snapshot_daily,
            id_portfolio: value.id_portfolio,
            snapshot_date: value.snapshot_date.to_string(),
            base_currency: value.base_currency,
            cash_balance_minor: value.cash_balance_minor,
            total_value_minor: value.total_value_minor,
            total_invested_minor: value.total_invested_minor,
            total_pnl_minor: value.total_pnl_minor,
            total_pnl_pct: value.total_pnl_pct,
            is_estimated: value.is_estimated,
            source_type: match value.source_type {
                PortfolioSnapshotSourceType::DailyJob => "daily_job",
                PortfolioSnapshotSourceType::Backfill => "backfill",
                PortfolioSnapshotSourceType::OnDemand => "on_demand",
            }
            .to_string(),
            created_at: format_datetime(value.created_at),
        }
    }
}

impl From<PortfolioDailySnapshotsView> for PortfolioDailySnapshotsEnvelope {
    fn from(value: PortfolioDailySnapshotsView) -> Self {
        Self {
            data_available: value.data_available,
            snapshots: value
                .snapshots
                .into_iter()
                .map(PortfolioDailySnapshotResponse::from)
                .collect(),
            pagination: PortfolioSnapshotsPaginationResponse {
                limit: value.pagination.limit,
                offset: value.pagination.offset,
                returned: value.pagination.returned,
                has_more: value.pagination.has_more,
            },
        }
    }
}

impl From<SnapshotHoldingAssetIdentity> for HistoricalSnapshotHoldingAssetResponse {
    fn from(value: SnapshotHoldingAssetIdentity) -> Self {
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

impl From<PortfolioDailySnapshotHolding> for PortfolioDailySnapshotHoldingResponse {
    fn from(value: PortfolioDailySnapshotHolding) -> Self {
        Self {
            id_portfolio_holding_snapshot_daily: value.id_portfolio_holding_snapshot_daily,
            id_portfolio_snapshot_daily: value.id_portfolio_snapshot_daily,
            id_asset: value.id_asset,
            asset: HistoricalSnapshotHoldingAssetResponse::from(value.asset),
            base_currency: value.base_currency,
            quantity: value.quantity,
            avg_cost_minor: value.avg_cost_minor,
            invested_minor: value.invested_minor,
            market_value_minor: value.market_value_minor,
            pnl_minor: value.pnl_minor,
            pnl_pct: value.pnl_pct,
            weight_pct: value.weight_pct,
            is_estimated: value.is_estimated,
            created_at: format_datetime(value.created_at),
        }
    }
}

impl From<PortfolioDailySnapshotHoldingsView> for HistoricalSnapshotHoldingsEnvelope {
    fn from(value: PortfolioDailySnapshotHoldingsView) -> Self {
        Self {
            data_available: value.data_available,
            snapshot: value.snapshot.map(PortfolioDailySnapshotResponse::from),
            holdings: value
                .holdings
                .into_iter()
                .map(PortfolioDailySnapshotHoldingResponse::from)
                .collect(),
            reason: value.reason,
            pagination: PortfolioSnapshotsPaginationResponse {
                limit: value.pagination.limit,
                offset: value.pagination.offset,
                returned: value.pagination.returned,
                has_more: value.pagination.has_more,
            },
        }
    }
}

fn parse_date(value: Option<&str>, field: &'static str) -> Result<Option<Date>, ApiError> {
    match value {
        None => Ok(None),
        Some(value) => Date::parse(
            value,
            &time::macros::format_description!("[year]-[month]-[day]"),
        )
        .map(Some)
        .map_err(|_| ApiError::Validation {
            code: if field == "date_from" {
                "invalid_date_from"
            } else {
                "invalid_date_to"
            },
            message: if field == "date_from" {
                "date_from must be a valid ISO date (YYYY-MM-DD)"
            } else {
                "date_to must be a valid ISO date (YYYY-MM-DD)"
            },
        }),
    }
}

fn parse_sort(value: Option<&str>) -> Result<Option<PortfolioSnapshotsSort>, ApiError> {
    match value {
        None => Ok(None),
        Some(value) => PortfolioSnapshotsSort::try_from(value)
            .map(Some)
            .map_err(|_| ApiError::Validation {
                code: "invalid_sort",
                message: "sort must be one of asc or desc",
            }),
    }
}

fn parse_snapshot_holdings_sort(
    value: Option<&str>,
) -> Result<Option<HistoricalSnapshotHoldingsSort>, ApiError> {
    match value {
        None => Ok(None),
        Some(value) => HistoricalSnapshotHoldingsSort::try_from(value)
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

fn map_service_error(error: PortfolioSnapshotServiceError) -> ApiError {
    match error {
        PortfolioSnapshotServiceError::Validation { code, message } => {
            ApiError::Validation { code, message }
        }
        PortfolioSnapshotServiceError::NotFound => ApiError::NotFound {
            code: "portfolio_not_found",
            message: "portfolio was not found",
        },
        PortfolioSnapshotServiceError::Internal => ApiError::Internal {
            code: "portfolio_snapshot_service_failed",
            message: "failed to process portfolio snapshots request",
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

    async fn insert_snapshot(
        pool: &PgPool,
        id_portfolio: Uuid,
        snapshot_date: &str,
        total_value_minor: i64,
        total_pnl_pct: &str,
        source_type: &str,
    ) -> Uuid {
        sqlx::query(
            r#"
            INSERT INTO portfolio_snapshots_daily (
                id_portfolio,
                snapshot_date,
                base_currency,
                cash_balance_minor,
                total_value_minor,
                total_invested_minor,
                total_pnl_minor,
                total_pnl_pct,
                is_estimated,
                source_type
            )
            VALUES ($1, $2::date, 'EUR', 1000, $3, 100000, 23456, $4::numeric, false, $5)
            RETURNING id_portfolio_snapshot_daily
            "#,
        )
        .bind(id_portfolio)
        .bind(snapshot_date)
        .bind(total_value_minor)
        .bind(total_pnl_pct)
        .bind(source_type)
        .fetch_one(pool)
        .await
        .expect("snapshot should be inserted")
        .try_get("id_portfolio_snapshot_daily")
        .expect("snapshot id should be returned")
    }

    async fn insert_asset(
        pool: &PgPool,
        name: &str,
        ticker: &str,
        _isin: &str,
        asset_class: &str,
    ) -> Uuid {
        let suffix = &Uuid::new_v4().simple().to_string()[..8];
        let unique_ticker = format!("{ticker}{suffix}");
        let unique_isin_middle = Uuid::new_v4().simple().to_string()[..9].to_uppercase();
        let unique_isin = format!("US{unique_isin_middle}1");
        sqlx::query(
            r#"
            INSERT INTO assets (asset_class, status, name, native_currency, isin, ticker, exchange, symbol)
            VALUES ($1, 'active', $2, 'USD', $3, $4, 'NYSE', $4)
            RETURNING id_asset
            "#,
        )
        .bind(asset_class)
        .bind(name)
        .bind(unique_isin)
        .bind(unique_ticker)
        .fetch_one(pool)
        .await
        .expect("asset should be inserted")
        .try_get("id_asset")
        .expect("id_asset should be returned")
    }

    struct SnapshotHoldingFixture<'a> {
        id_portfolio_snapshot_daily: Uuid,
        id_asset: Uuid,
        quantity: &'a str,
        avg_cost_minor: Option<i64>,
        invested_minor: i64,
        market_value_minor: i64,
        pnl_minor: i64,
        pnl_pct: &'a str,
        weight_pct: &'a str,
        created_at: &'a str,
    }

    async fn insert_snapshot_holding(pool: &PgPool, input: SnapshotHoldingFixture<'_>) -> Uuid {
        sqlx::query(
            r#"
            INSERT INTO portfolio_holding_snapshot_daily (
                id_portfolio_snapshot_daily,
                id_asset,
                base_currency,
                quantity,
                avg_cost_minor,
                invested_minor,
                market_value_minor,
                pnl_minor,
                pnl_pct,
                weight_pct,
                is_estimated,
                created_at
            )
            VALUES ($1, $2, 'EUR', $3::numeric, $4, $5, $6, $7, $8::numeric, $9::numeric, false, $10::timestamptz)
            RETURNING id_portfolio_holding_snapshot_daily
            "#,
        )
        .bind(input.id_portfolio_snapshot_daily)
        .bind(input.id_asset)
        .bind(input.quantity)
        .bind(input.avg_cost_minor)
        .bind(input.invested_minor)
        .bind(input.market_value_minor)
        .bind(input.pnl_minor)
        .bind(input.pnl_pct)
        .bind(input.weight_pct)
        .bind(input.created_at)
        .fetch_one(pool)
        .await
        .expect("snapshot holding should be inserted")
        .try_get("id_portfolio_holding_snapshot_daily")
        .expect("snapshot holding id should be returned")
    }

    async fn cleanup_tree(
        pool: &PgPool,
        snapshot_ids: &[Uuid],
        portfolio_ids: &[Uuid],
        asset_ids: &[Uuid],
        user_ids: &[Uuid],
    ) {
        if !snapshot_ids.is_empty() {
            sqlx::query(
                "DELETE FROM portfolio_holding_snapshot_daily WHERE id_portfolio_snapshot_daily = ANY($1)",
            )
            .bind(snapshot_ids)
            .execute(pool)
            .await
            .expect("snapshot holdings should be deleted");
            sqlx::query(
                "DELETE FROM portfolio_snapshots_daily WHERE id_portfolio_snapshot_daily = ANY($1)",
            )
            .bind(snapshot_ids)
            .execute(pool)
            .await
            .expect("snapshots should be deleted");
        }

        if !portfolio_ids.is_empty() {
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
    async fn get_snapshots_with_existing_snapshots_returns_data_available_true() {
        let pool = test_pool().await;
        let handle = format!("psa{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Snapshots Portfolio").await;
        let snapshot_id = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-05",
            123456,
            "23.4500",
            "daily_job",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/snapshots/daily"))
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
        assert_eq!(body["snapshots"][0]["snapshot_date"], "2026-06-05");
        assert_rfc3339_string(&body["snapshots"][0]["created_at"]);

        cleanup_tree(&pool, &[snapshot_id], &[portfolio_id], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_snapshots_when_none_exist_returns_data_available_false() {
        let pool = test_pool().await;
        let handle = format!("psm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "No Snapshots").await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/snapshots/daily"))
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
        assert!(body["snapshots"].as_array().unwrap().is_empty());

        cleanup_tree(&pool, &[], &[portfolio_id], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_snapshots_auth_cross_user_and_soft_delete_are_enforced() {
        let pool = test_pool().await;
        let owner = format!("pso{}", &Uuid::new_v4().simple().to_string()[..12]);
        let other = format!("psx{}", &Uuid::new_v4().simple().to_string()[..12]);
        let owner_id = create_user(&pool, &owner).await;
        let other_id = create_user(&pool, &other).await;
        let portfolio_id = create_portfolio(&pool, owner_id, "Secure Snapshots").await;
        let snapshot_id = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-05",
            100,
            "5.0000",
            "daily_job",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let no_token = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/portfolios/{portfolio_id}/snapshots/daily"))
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
                    .uri(format!("/v1/portfolios/{portfolio_id}/snapshots/daily"))
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
                    .uri(format!("/v1/portfolios/{portfolio_id}/snapshots/daily"))
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
                    .uri(format!("/v1/portfolios/{portfolio_id}/snapshots/daily"))
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

        cleanup_tree(
            &pool,
            &[snapshot_id],
            &[portfolio_id],
            &[],
            &[owner_id, other_id],
        )
        .await;
    }

    #[tokio::test]
    async fn get_snapshots_filters_date_bounds_and_sort() {
        let pool = test_pool().await;
        let handle = format!("psf{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Filtered Snapshots").await;
        let snapshot_a = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-03",
            100,
            "1.0000",
            "daily_job",
        )
        .await;
        let snapshot_b =
            insert_snapshot(&pool, portfolio_id, "2026-06-04", 200, "2.0000", "backfill").await;
        let snapshot_c = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-05",
            300,
            "3.0000",
            "on_demand",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let from_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily?date_from=2026-06-04"
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
        let from_body = response_json(from_response).await;
        assert_eq!(from_body["snapshots"].as_array().unwrap().len(), 2);

        let to_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily?date_to=2026-06-04"
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
        let to_body = response_json(to_response).await;
        assert_eq!(to_body["snapshots"].as_array().unwrap().len(), 2);

        let range_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily?date_from=2026-06-04&date_to=2026-06-05"
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
        let range_body = response_json(range_response).await;
        assert_eq!(range_body["snapshots"].as_array().unwrap().len(), 2);
        assert_eq!(range_body["snapshots"][0]["snapshot_date"], "2026-06-04");

        let desc_response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily?sort=desc"
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
        let desc_body = response_json(desc_response).await;
        assert_eq!(desc_body["snapshots"][0]["snapshot_date"], "2026-06-05");
        assert_eq!(desc_body["snapshots"][2]["source_type"], "daily_job");

        cleanup_tree(
            &pool,
            &[snapshot_a, snapshot_b, snapshot_c],
            &[portfolio_id],
            &[],
            &[user_id],
        )
        .await;
    }

    #[tokio::test]
    async fn get_snapshots_pagination_and_validation_work() {
        let pool = test_pool().await;
        let handle = format!("psp{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Paged Snapshots").await;
        let snapshot_a = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-03",
            100,
            "1.0000",
            "daily_job",
        )
        .await;
        let snapshot_b = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-04",
            200,
            "2.0000",
            "daily_job",
        )
        .await;
        let snapshot_c = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-05",
            300,
            "3.0000",
            "daily_job",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let first_page = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily?limit=2"
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
        let first_body = response_json(first_page).await;
        assert_eq!(first_body["pagination"]["limit"], 2);
        assert_eq!(first_body["pagination"]["has_more"], true);
        assert_eq!(first_body["snapshots"][0]["snapshot_date"], "2026-06-03");

        let second_page = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily?limit=2&offset=2"
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
        assert_eq!(second_body["pagination"]["offset"], 2);
        assert_eq!(second_body["pagination"]["has_more"], false);
        assert_eq!(second_body["snapshots"][0]["snapshot_date"], "2026-06-05");

        for (uri, expected_code) in [
            (
                format!("/v1/portfolios/{portfolio_id}/snapshots/daily?date_from=2026-13-01"),
                "invalid_date_from",
            ),
            (
                format!("/v1/portfolios/{portfolio_id}/snapshots/daily?date_to=2026-02-30"),
                "invalid_date_to",
            ),
            (
                format!(
                    "/v1/portfolios/{portfolio_id}/snapshots/daily?date_from=2026-06-06&date_to=2026-06-05"
                ),
                "invalid_date_range",
            ),
            (
                format!("/v1/portfolios/{portfolio_id}/snapshots/daily?limit=367"),
                "invalid_limit",
            ),
            (
                format!("/v1/portfolios/{portfolio_id}/snapshots/daily?offset=-1"),
                "invalid_offset",
            ),
            (
                format!("/v1/portfolios/{portfolio_id}/snapshots/daily?sort=boom"),
                "invalid_sort",
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
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(body["error"]["code"], expected_code);
        }

        cleanup_tree(
            &pool,
            &[snapshot_a, snapshot_b, snapshot_c],
            &[portfolio_id],
            &[],
            &[user_id],
        )
        .await;
    }

    #[tokio::test]
    async fn get_snapshots_handles_sql_injection_like_query_values_cleanly() {
        let pool = test_pool().await;
        let handle = format!("psi{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Injection Snapshots").await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily?date_from=';drop"
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

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_date_from");

        cleanup_tree(&pool, &[], &[portfolio_id], &[], &[user_id]).await;
    }

    #[tokio::test]
    async fn get_historical_holdings_with_snapshot_and_holdings_returns_data_available_true() {
        let pool = test_pool().await;
        let handle = format!("psh{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Snapshot Holdings").await;
        let snapshot_id = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-05",
            123456,
            "23.4500",
            "daily_job",
        )
        .await;
        let asset_a =
            insert_asset(&pool, "Historical Alpha", "HALA", "US3000000001", "equity").await;
        let asset_b = insert_asset(&pool, "Historical Beta", "HBET", "US3000000002", "etf").await;
        let _holding_a = insert_snapshot_holding(
            &pool,
            SnapshotHoldingFixture {
                id_portfolio_snapshot_daily: snapshot_id,
                id_asset: asset_a,
                quantity: "10.5000000000",
                avg_cost_minor: Some(1000),
                invested_minor: 100000,
                market_value_minor: 300000,
                pnl_minor: 200000,
                pnl_pct: "20.5000",
                weight_pct: "60.0000",
                created_at: "2026-06-05T23:59:10Z",
            },
        )
        .await;
        let _holding_b = insert_snapshot_holding(
            &pool,
            SnapshotHoldingFixture {
                id_portfolio_snapshot_daily: snapshot_id,
                id_asset: asset_b,
                quantity: "4.0000000000",
                avg_cost_minor: Some(2000),
                invested_minor: 90000,
                market_value_minor: 200000,
                pnl_minor: 110000,
                pnl_pct: "12.0000",
                weight_pct: "40.0000",
                created_at: "2026-06-05T23:59:11Z",
            },
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings"
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

        let body = response_json(response).await;
        assert_eq!(body["data_available"], true);
        assert_eq!(body["snapshot"]["snapshot_date"], "2026-06-05");
        assert_eq!(body["holdings"].as_array().unwrap().len(), 2);
        assert_eq!(body["holdings"][0]["asset"]["name"], "Historical Alpha");
        assert_rfc3339_string(&body["snapshot"]["created_at"]);
        assert_rfc3339_string(&body["holdings"][0]["created_at"]);

        cleanup_tree(
            &pool,
            &[snapshot_id],
            &[portfolio_id],
            &[asset_a, asset_b],
            &[user_id],
        )
        .await;
    }

    #[tokio::test]
    async fn get_historical_holdings_snapshot_missing_empty_and_auth_rules_work() {
        let pool = test_pool().await;
        let owner = format!("psu{}", &Uuid::new_v4().simple().to_string()[..12]);
        let other = format!("psv{}", &Uuid::new_v4().simple().to_string()[..12]);
        let owner_id = create_user(&pool, &owner).await;
        let other_id = create_user(&pool, &other).await;
        let portfolio_id = create_portfolio(&pool, owner_id, "Missing Snapshot Holdings").await;
        let snapshot_id = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-05",
            100,
            "5.0000",
            "daily_job",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let missing_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-06/holdings"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(owner_id, &owner)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let missing_body = response_json(missing_response).await;
        assert_eq!(missing_body["data_available"], false);
        assert!(missing_body["snapshot"].is_null());
        assert_eq!(missing_body["reason"], "snapshot_missing");
        assert!(missing_body["holdings"].as_array().unwrap().is_empty());

        let existing_empty = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(owner_id, &owner)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let existing_empty_body = response_json(existing_empty).await;
        assert_eq!(existing_empty_body["data_available"], true);
        assert!(
            existing_empty_body["holdings"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let no_token = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings"
                    ))
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
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings"
                    ))
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
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings"
                    ))
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
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings"
                    ))
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

        let invalid_date = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-13-01/holdings"
                    ))
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", build_access_token(owner_id, &owner)),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response should be built");
        let invalid_date_status = invalid_date.status();
        let invalid_date_body = response_json(invalid_date).await;
        assert_eq!(invalid_date_status, StatusCode::BAD_REQUEST);
        assert_eq!(invalid_date_body["error"]["code"], "invalid_snapshot_date");

        cleanup_tree(
            &pool,
            &[snapshot_id],
            &[portfolio_id],
            &[],
            &[owner_id, other_id],
        )
        .await;
    }

    #[tokio::test]
    async fn get_historical_holdings_pagination_filters_sort_and_validation_work() {
        let pool = test_pool().await;
        let handle = format!("psw{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Filtered Snapshot Holdings").await;
        let snapshot_id = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-05",
            999,
            "9.0000",
            "daily_job",
        )
        .await;
        let asset_a = insert_asset(
            &pool,
            "Snapshot Alpha Search",
            "SAS1",
            "US4000000001",
            "equity",
        )
        .await;
        let asset_b =
            insert_asset(&pool, "Snapshot Beta Search", "SBS1", "US4000000002", "etf").await;
        let asset_c = insert_asset(
            &pool,
            "Snapshot Gamma Search",
            "SGS1",
            "US4000000003",
            "equity",
        )
        .await;
        insert_snapshot_holding(
            &pool,
            SnapshotHoldingFixture {
                id_portfolio_snapshot_daily: snapshot_id,
                id_asset: asset_a,
                quantity: "1.0000000000",
                avg_cost_minor: Some(1000),
                invested_minor: 100,
                market_value_minor: 700,
                pnl_minor: 600,
                pnl_pct: "10.0000",
                weight_pct: "70.0000",
                created_at: "2026-06-05T23:59:10Z",
            },
        )
        .await;
        insert_snapshot_holding(
            &pool,
            SnapshotHoldingFixture {
                id_portfolio_snapshot_daily: snapshot_id,
                id_asset: asset_b,
                quantity: "2.0000000000",
                avg_cost_minor: Some(2000),
                invested_minor: 200,
                market_value_minor: 500,
                pnl_minor: 300,
                pnl_pct: "5.0000",
                weight_pct: "20.0000",
                created_at: "2026-06-05T23:59:11Z",
            },
        )
        .await;
        insert_snapshot_holding(
            &pool,
            SnapshotHoldingFixture {
                id_portfolio_snapshot_daily: snapshot_id,
                id_asset: asset_c,
                quantity: "3.0000000000",
                avg_cost_minor: Some(3000),
                invested_minor: 300,
                market_value_minor: 200,
                pnl_minor: 50,
                pnl_pct: "1.5000",
                weight_pct: "10.0000",
                created_at: "2026-06-05T23:59:12Z",
            },
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let first_page = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?limit=2"
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
        let first_body = response_json(first_page).await;
        assert_eq!(first_body["pagination"]["limit"], 2);
        assert_eq!(first_body["pagination"]["has_more"], true);
        assert_eq!(
            first_body["holdings"][0]["asset"]["name"],
            "Snapshot Alpha Search"
        );

        let second_page = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?limit=2&offset=2"
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
        assert_eq!(second_body["pagination"]["offset"], 2);
        assert_eq!(second_body["pagination"]["has_more"], false);
        assert_eq!(
            second_body["holdings"][0]["asset"]["name"],
            "Snapshot Gamma Search"
        );

        let value_sorted = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?sort=value_desc"
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
            "Snapshot Alpha Search"
        );

        let name_sorted = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?sort=name_asc"
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
        let name_body = response_json(name_sorted).await;
        assert_eq!(
            name_body["holdings"][0]["asset"]["name"],
            "Snapshot Alpha Search"
        );

        let filtered = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?asset_class=etf&search=Beta"
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
            "Snapshot Beta Search"
        );

        for (uri, expected_code) in [
            (
                format!(
                    "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?limit=101"
                ),
                "invalid_limit",
            ),
            (
                format!(
                    "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?offset=-1"
                ),
                "invalid_offset",
            ),
            (
                format!(
                    "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?sort=boom"
                ),
                "invalid_sort",
            ),
            (
                format!(
                    "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?asset_class=boom"
                ),
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
            assert_eq!(body["error"]["code"], expected_code);
        }

        cleanup_tree(
            &pool,
            &[snapshot_id],
            &[portfolio_id],
            &[asset_a, asset_b, asset_c],
            &[user_id],
        )
        .await;
    }

    #[tokio::test]
    async fn get_historical_holdings_handles_sql_injection_like_query_values_cleanly() {
        let pool = test_pool().await;
        let handle = format!("psy{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Injection Snapshot Holdings").await;
        let snapshot_id = insert_snapshot(
            &pool,
            portfolio_id,
            "2026-06-05",
            123,
            "1.0000",
            "daily_job",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/portfolios/{portfolio_id}/snapshots/daily/2026-06-05/holdings?asset_class=';drop"
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

        let status = response.status();
        let body = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_asset_class");

        cleanup_tree(&pool, &[snapshot_id], &[portfolio_id], &[], &[user_id]).await;
    }

    #[test]
    fn portfolio_snapshots_module_is_read_only_and_does_not_touch_worker_tables() {
        let repository_source = include_str!("../repositories/portfolio_snapshots.rs");
        let service_source = include_str!("../services/portfolio_snapshots.rs");

        for forbidden in [
            "INSERT INTO portfolio_snapshots_daily",
            "UPDATE portfolio_snapshots_daily",
            "DELETE FROM portfolio_snapshots_daily",
            "INSERT INTO portfolio_holding_snapshot_daily",
            "UPDATE portfolio_holding_snapshot_daily",
            "DELETE FROM portfolio_holding_snapshot_daily",
            "INSERT INTO rm_portfolio_summary",
            "UPDATE rm_portfolio_summary",
            "DELETE FROM rm_portfolio_summary",
            "INSERT INTO rm_portfolio_holdings",
            "UPDATE rm_portfolio_holdings",
            "DELETE FROM rm_portfolio_holdings",
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
