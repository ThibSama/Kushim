use crate::{
    auth::AuthenticatedUser,
    domain::{
        asset::{Asset, AssetClass},
        portfolio_read_model::{
            HoldingMarketDataQuality, PortfolioHolding, PortfolioHoldingPositionStatus,
            PortfolioHoldingsSort, PortfolioSummary, PortfolioSummaryStatus,
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
    /// Aggregate valuation status derived from open holdings vs. matching
    /// `asset_market_data` rows. Stable enum codes: `complete`, `partial`,
    /// `unavailable`, `empty`. Distinct from `portfolio_status` which only
    /// reflects the lifecycle of the portfolio container.
    pub valuation_status: String,
    pub positions_total: i64,
    pub positions_valued: i64,
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

/// Per-holding valuation provenance. All values come from
/// `rm_portfolio_holdings` — the API never joins the live `asset_market_data`
/// cache at read time, so a P2 quote written after the rebuild cannot appear
/// beside a `market_value_minor` computed from P1.
///
/// The legacy `fetched_at` field has been **removed**: no actual fetch
/// timestamp is stored. The new `record_updated_at` reflects only the
/// wall-clock time at which the `asset_market_data` row was last written
/// (captured at rebuild time).
#[derive(Debug, Serialize)]
pub struct HoldingMarketDataResponse {
    /// True only when the holding was actually valued from a compatible
    /// market-data row. False for missing, unsupported_currency, and legacy
    /// rows.
    pub available: bool,
    /// Stable code — `market_data` | `invested_cost_fallback`. `None` for
    /// legacy rows persisted before the migration.
    pub valuation_source: Option<&'static str>,
    /// `available` | `unavailable`. No `stale` value is emitted.
    pub status: &'static str,
    /// Stable reason code when `status = unavailable`. One of:
    /// `market_data_missing`, `unsupported_market_data_currency`,
    /// `valuation_provenance_missing`.
    pub unavailable_reason: Option<&'static str>,
    /// Exact price used (or rejected, for unsupported_currency). Null
    /// otherwise.
    pub price_minor: Option<i64>,
    /// Currency of `price_minor`.
    pub currency: Option<String>,
    pub provider: Option<String>,
    /// RFC 3339 — market-quote timestamp reported by the provider
    /// (`asset_market_data.as_of`, captured at rebuild time).
    pub market_data_as_of: Option<String>,
    /// RFC 3339 — wall-clock time at which `kushim-market-data` last wrote
    /// the row (`asset_market_data.updated_at`, captured at rebuild time).
    /// This is intentionally **not** named `fetched_at` because no real fetch
    /// timestamp exists upstream.
    pub record_updated_at: Option<String>,
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
    pub market_data: HoldingMarketDataResponse,
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
            valuation_status: value.valuation_status.as_str().to_string(),
            positions_total: value.positions_total,
            positions_valued: value.positions_valued,
        }
    }
}

impl From<HoldingMarketDataQuality> for HoldingMarketDataResponse {
    fn from(value: HoldingMarketDataQuality) -> Self {
        Self {
            available: value.available,
            valuation_source: value.valuation_source,
            status: value.status,
            unavailable_reason: value.unavailable_reason,
            price_minor: value.price_minor,
            currency: value.currency,
            provider: value.provider,
            market_data_as_of: value.market_data_as_of.map(format_datetime),
            record_updated_at: value.record_updated_at.map(format_datetime),
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
            market_data: HoldingMarketDataResponse::from(value.market_data),
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

    /// Legacy fixture — inserts a holding WITHOUT valuation provenance, i.e.
    /// the same shape a row would have if it was written before the
    /// `003_holding_valuation_provenance` migration. The API must surface
    /// such rows as `valuation_provenance_missing`.
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

    /// Inserts a holding WITH persisted valuation provenance, mimicking what
    /// the worker writes after a rebuild. Used by the temporal-consistency
    /// and provenance-readout tests.
    #[allow(clippy::too_many_arguments)]
    async fn insert_holding_valued_by_market_data(
        pool: &PgPool,
        id_portfolio: Uuid,
        id_asset: Uuid,
        market_value_minor: i64,
        invested_base_minor: i64,
        market_data_price_minor: i64,
        market_data_currency: &str,
        market_data_provider: Option<&str>,
        market_data_as_of: &str,
        market_data_record_updated_at: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO rm_portfolio_holdings (
                id_portfolio, id_asset, base_currency,
                quantity, avg_cost_minor, invested_base_minor,
                market_value_minor, pnl_base_minor,
                pnl_pct, weight_pct, position_status,
                is_estimated, as_of,
                valuation_source, market_data_status,
                market_data_price_minor, market_data_currency,
                market_data_provider, market_data_as_of,
                market_data_record_updated_at
            )
            VALUES (
                $1, $2, 'EUR',
                '1.0000000000'::numeric, 1000, $3,
                $4, $5,
                '0.0000'::numeric, '100.0000'::numeric, 'open',
                false, now(),
                'market_data', 'available',
                $6, $7,
                $8, $9::timestamptz,
                $10::timestamptz
            )
            "#,
        )
        .bind(id_portfolio)
        .bind(id_asset)
        .bind(invested_base_minor)
        .bind(market_value_minor)
        .bind(market_value_minor - invested_base_minor)
        .bind(market_data_price_minor)
        .bind(market_data_currency)
        .bind(market_data_provider)
        .bind(market_data_as_of)
        .bind(market_data_record_updated_at)
        .execute(pool)
        .await
        .expect("holding with provenance should be inserted");
    }

    async fn insert_holding_invested_cost_missing(
        pool: &PgPool,
        id_portfolio: Uuid,
        id_asset: Uuid,
        invested_base_minor: i64,
    ) {
        sqlx::query(
            r#"
            INSERT INTO rm_portfolio_holdings (
                id_portfolio, id_asset, base_currency,
                quantity, avg_cost_minor, invested_base_minor,
                market_value_minor, pnl_base_minor,
                pnl_pct, weight_pct, position_status,
                is_estimated, as_of,
                valuation_source, market_data_status
            )
            VALUES (
                $1, $2, 'EUR',
                '1.0000000000'::numeric, 1000, $3,
                $3, 0,
                '0.0000'::numeric, NULL, 'open',
                true, now(),
                'invested_cost_fallback', 'missing'
            )
            "#,
        )
        .bind(id_portfolio)
        .bind(id_asset)
        .bind(invested_base_minor)
        .execute(pool)
        .await
        .expect("missing-md holding should be inserted");
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert_holding_invested_cost_unsupported_currency(
        pool: &PgPool,
        id_portfolio: Uuid,
        id_asset: Uuid,
        invested_base_minor: i64,
        md_price_minor: i64,
        md_currency: &str,
        md_provider: Option<&str>,
        md_as_of: &str,
        md_record_updated_at: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO rm_portfolio_holdings (
                id_portfolio, id_asset, base_currency,
                quantity, avg_cost_minor, invested_base_minor,
                market_value_minor, pnl_base_minor,
                pnl_pct, weight_pct, position_status,
                is_estimated, as_of,
                valuation_source, market_data_status,
                market_data_price_minor, market_data_currency,
                market_data_provider, market_data_as_of,
                market_data_record_updated_at
            )
            VALUES (
                $1, $2, 'EUR',
                '1.0000000000'::numeric, 1000, $3,
                $3, 0,
                '0.0000'::numeric, NULL, 'open',
                true, now(),
                'invested_cost_fallback', 'unsupported_currency',
                $4, $5,
                $6, $7::timestamptz,
                $8::timestamptz
            )
            "#,
        )
        .bind(id_portfolio)
        .bind(id_asset)
        .bind(invested_base_minor)
        .bind(md_price_minor)
        .bind(md_currency)
        .bind(md_provider)
        .bind(md_as_of)
        .bind(md_record_updated_at)
        .execute(pool)
        .await
        .expect("unsupported-currency holding should be inserted");
    }

    async fn update_market_data_row(
        pool: &PgPool,
        id_asset: Uuid,
        new_price_minor: i64,
        new_as_of: &str,
    ) {
        // Mutate an existing asset_market_data row in place to simulate a
        // P1 → P2 quote refresh without running another worker rebuild.
        sqlx::query(
            r#"
            UPDATE asset_market_data
            SET price_minor = $2,
                as_of = $3::timestamptz,
                updated_at = now()
            WHERE id_asset = $1
            "#,
        )
        .bind(id_asset)
        .bind(new_price_minor)
        .bind(new_as_of)
        .execute(pool)
        .await
        .expect("update should succeed");
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

    async fn insert_market_data(
        pool: &PgPool,
        id_asset: Uuid,
        provider: &str,
        price_minor: i64,
        currency: &str,
        as_of: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO asset_market_data (id_asset, price_minor, currency, data_source, as_of)
            VALUES ($1, $2, $3, $4, $5::timestamptz)
            ON CONFLICT (id_asset) DO UPDATE SET
                price_minor = EXCLUDED.price_minor,
                currency = EXCLUDED.currency,
                data_source = EXCLUDED.data_source,
                as_of = EXCLUDED.as_of,
                updated_at = now()
            "#,
        )
        .bind(id_asset)
        .bind(price_minor)
        .bind(currency)
        .bind(provider)
        .bind(as_of)
        .execute(pool)
        .await
        .expect("market data row should insert");
    }

    async fn cleanup_market_data(pool: &PgPool, asset_ids: &[Uuid]) {
        if asset_ids.is_empty() {
            return;
        }
        sqlx::query("DELETE FROM asset_market_data WHERE id_asset = ANY($1)")
            .bind(asset_ids)
            .execute(pool)
            .await
            .ok();
    }

    // ---------------------------------------------------------------
    // Persisted-provenance contract tests (Phase 8/9/10 of the
    // valuation-provenance pass).
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn holdings_with_legacy_provenance_are_reported_as_missing_until_rebuilt() {
        // Legacy fixture inserts a row with NULL valuation_source — exactly
        // the shape a row would have after the migration but before the
        // worker has rebuilt the read model.
        let pool = test_pool().await;
        let handle = format!("lgp{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Legacy Row").await;
        let asset = insert_asset(&pool, "Legacy", "LGCY", "US8000000001", "equity").await;
        insert_summary(&pool, portfolio_id, 50_000).await;
        insert_holding(
            &pool,
            HoldingFixture {
                id_portfolio: portfolio_id,
                id_asset: asset,
                quantity: "1.0000000000",
                invested_base_minor: 50_000,
                market_value_minor: 50_000,
                pnl_base_minor: 0,
                pnl_pct: "0.0000",
                weight_pct: "100.0000",
                position_status: "open",
                as_of: "2026-06-10T10:00:00Z",
            },
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        let response = app
            .clone()
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
        let md = &body["holdings"][0]["market_data"];
        assert_eq!(md["available"], false);
        assert!(md["valuation_source"].is_null());
        assert_eq!(md["status"], "unavailable");
        assert_eq!(md["unavailable_reason"], "valuation_provenance_missing");
        assert!(md["price_minor"].is_null());
        assert!(md["currency"].is_null());

        // Summary aggregate must not count legacy NULL provenance as valued.
        let summary = app
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
            .expect("summary response built");
        let summary_body = response_json(summary).await;
        assert_eq!(summary_body["summary"]["valuation_status"], "unavailable");
        assert_eq!(summary_body["summary"]["positions_total"], 1);
        assert_eq!(summary_body["summary"]["positions_valued"], 0);

        cleanup_tree(&pool, &[portfolio_id], &[asset], &[user_id]).await;
    }

    #[tokio::test]
    async fn holdings_with_persisted_market_data_provenance_expose_exact_inputs() {
        let pool = test_pool().await;
        let handle = format!("pmd{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Persisted MD").await;
        let asset = insert_asset(&pool, "Priced", "PRCD", "US8100000001", "equity").await;
        insert_summary(&pool, portfolio_id, 60_000).await;
        insert_holding_valued_by_market_data(
            &pool,
            portfolio_id,
            asset,
            60_000,
            50_000,
            60_000,
            "EUR",
            Some("test-static"),
            "2026-06-10T09:30:00Z",
            "2026-06-10T09:30:05Z",
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
            .expect("response built");
        let body = response_json(response).await;
        let md = &body["holdings"][0]["market_data"];
        assert_eq!(md["available"], true);
        assert_eq!(md["valuation_source"], "market_data");
        assert_eq!(md["status"], "available");
        assert!(md["unavailable_reason"].is_null());
        assert_eq!(md["price_minor"], 60_000);
        assert_eq!(md["currency"], "EUR");
        assert_eq!(md["provider"], "test-static");
        assert_rfc3339_string(&md["market_data_as_of"]);
        assert_rfc3339_string(&md["record_updated_at"]);
        // fetched_at must NOT exist any more
        assert!(md.get("fetched_at").is_none(), "fetched_at must be removed");

        cleanup_tree(&pool, &[portfolio_id], &[asset], &[user_id]).await;
    }

    #[tokio::test]
    async fn temporal_consistency_p2_quote_does_not_pollute_p1_holding_until_rebuild() {
        // THE architectural-fix regression test: prove the API never joins a
        // later `asset_market_data` quote (P2) onto a holding whose value was
        // calculated from an earlier quote (P1).
        let pool = test_pool().await;
        let handle = format!("p1p2{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "P1/P2 Temporal").await;
        let asset = insert_asset(&pool, "Temporal", "TMPR", "US8200000001", "equity").await;
        insert_summary(&pool, portfolio_id, 60_000).await;

        // 1) Worker rebuild with P1 — persist the holding using P1 inputs.
        insert_holding_valued_by_market_data(
            &pool,
            portfolio_id,
            asset,
            60_000, // market_value_minor computed from P1
            50_000, // invested
            60_000, // P1 price
            "EUR",
            Some("test-static"),
            "2026-06-10T09:00:00Z", // P1 market timestamp
            "2026-06-10T09:00:05Z", // P1 record updated_at
        )
        .await;
        // 2) Live asset_market_data row also at P1 (this is what the worker
        //    snapshotted into rm_portfolio_holdings).
        insert_market_data(
            &pool,
            asset,
            "test-static",
            60_000,
            "EUR",
            "2026-06-10T09:00:00Z",
        )
        .await;
        let app = crate::http::router(test_state(pool.clone()).await);

        // First read — both should describe P1 consistently.
        let response = app
            .clone()
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
            .unwrap();
        let body = response_json(response).await;
        let md = &body["holdings"][0]["market_data"];
        assert_eq!(md["price_minor"], 60_000, "must read P1 price");
        assert_eq!(body["holdings"][0]["market_value_minor"], 60_000);

        // 3) Mutate the live cache to P2 — DO NOT trigger any rebuild.
        update_market_data_row(&pool, asset, 99_999, "2026-06-10T12:00:00Z").await;

        // 4) The API must still describe the holding via P1 — neither value
        //    nor provenance must drift to P2.
        let response = app
            .clone()
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
            .unwrap();
        let body = response_json(response).await;
        let md = &body["holdings"][0]["market_data"];
        assert_eq!(
            md["price_minor"], 60_000,
            "API must still expose P1 — temporal coupling regressed"
        );
        assert_eq!(md["market_data_as_of"], "2026-06-10T09:00:00Z");
        assert_eq!(body["holdings"][0]["market_value_minor"], 60_000);

        // 5) Now simulate a worker rebuild capturing P2 inputs — replace the
        //    persisted provenance to mimic the same atomic write the real
        //    worker performs.
        sqlx::query("DELETE FROM rm_portfolio_holdings WHERE id_portfolio = $1")
            .bind(portfolio_id)
            .execute(&pool)
            .await
            .unwrap();
        insert_holding_valued_by_market_data(
            &pool,
            portfolio_id,
            asset,
            99_999,
            50_000,
            99_999,
            "EUR",
            Some("test-static"),
            "2026-06-10T12:00:00Z",
            "2026-06-10T12:00:05Z",
        )
        .await;

        // 6) Now the API must reflect P2 — value AND provenance moved
        //    together because the read model was rebuilt.
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
            .unwrap();
        let body = response_json(response).await;
        let md = &body["holdings"][0]["market_data"];
        assert_eq!(md["price_minor"], 99_999);
        assert_eq!(md["market_data_as_of"], "2026-06-10T12:00:00Z");
        assert_eq!(body["holdings"][0]["market_value_minor"], 99_999);

        cleanup_market_data(&pool, &[asset]).await;
        cleanup_tree(&pool, &[portfolio_id], &[asset], &[user_id]).await;
    }

    #[tokio::test]
    async fn summary_counts_only_persisted_market_data_status_available() {
        // Three holdings: one available, one missing, one unsupported_currency.
        // Expected: valuation_status="partial", positions_valued=1.
        let pool = test_pool().await;
        let handle = format!("svm{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Mixed Persisted").await;
        let a1 = insert_asset(&pool, "Avail", "AV01", "US8300000001", "equity").await;
        let a2 = insert_asset(&pool, "Missing", "MI01", "US8300000002", "equity").await;
        let a3 = insert_asset(&pool, "Unsup", "UN01", "US8300000003", "equity").await;
        insert_summary(&pool, portfolio_id, 200_000).await;
        insert_holding_valued_by_market_data(
            &pool,
            portfolio_id,
            a1,
            60_000,
            50_000,
            60_000,
            "EUR",
            Some("test-static"),
            "2026-06-10T09:00:00Z",
            "2026-06-10T09:00:05Z",
        )
        .await;
        insert_holding_invested_cost_missing(&pool, portfolio_id, a2, 40_000).await;
        insert_holding_invested_cost_unsupported_currency(
            &pool,
            portfolio_id,
            a3,
            30_000,
            70_000,
            "USD",
            Some("test-static"),
            "2026-06-10T09:00:00Z",
            "2026-06-10T09:00:05Z",
        )
        .await;
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
            .unwrap();
        let body = response_json(response).await;
        assert_eq!(body["summary"]["valuation_status"], "partial");
        assert_eq!(body["summary"]["positions_total"], 3);
        assert_eq!(body["summary"]["positions_valued"], 1);

        cleanup_tree(&pool, &[portfolio_id], &[a1, a2, a3], &[user_id]).await;
    }

    #[tokio::test]
    async fn holdings_unsupported_currency_persists_provenance_but_not_available() {
        let pool = test_pool().await;
        let handle = format!("hus{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Unsup Currency").await;
        let asset = insert_asset(&pool, "USD Only", "USDO", "US8400000001", "equity").await;
        insert_summary(&pool, portfolio_id, 50_000).await;
        insert_holding_invested_cost_unsupported_currency(
            &pool,
            portfolio_id,
            asset,
            30_000,
            70_000,
            "USD",
            Some("test-static"),
            "2026-06-10T09:00:00Z",
            "2026-06-10T09:00:05Z",
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
            .unwrap();
        let body = response_json(response).await;
        let md = &body["holdings"][0]["market_data"];
        assert_eq!(md["available"], false);
        assert_eq!(md["valuation_source"], "invested_cost_fallback");
        assert_eq!(md["status"], "unavailable");
        assert_eq!(md["unavailable_reason"], "unsupported_market_data_currency");
        // Incompatible provenance preserved
        assert_eq!(md["price_minor"], 70_000);
        assert_eq!(md["currency"], "USD");

        cleanup_tree(&pool, &[portfolio_id], &[asset], &[user_id]).await;
    }

    #[tokio::test]
    async fn summary_valuation_status_is_unavailable_when_only_incompatible_currency_rows_exist() {
        let pool = test_pool().await;
        let handle = format!("vui{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "All Incompatible").await;
        let asset_a =
            insert_asset(&pool, "USD Only Legacy", "USDL", "US7200000001", "equity").await;
        insert_summary(&pool, portfolio_id, 100_000).await;
        insert_holding(
            &pool,
            HoldingFixture {
                id_portfolio: portfolio_id,
                id_asset: asset_a,
                quantity: "1.0000000000",
                invested_base_minor: 50_000,
                market_value_minor: 50_000,
                pnl_base_minor: 0,
                pnl_pct: "0.0000",
                weight_pct: "100.0000",
                position_status: "open",
                as_of: "2026-06-10T10:00:00Z",
            },
        )
        .await;
        insert_market_data(
            &pool,
            asset_a,
            "test-static",
            55_000,
            "USD",
            "2026-06-10T09:30:00Z",
        )
        .await;
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

        assert_eq!(body["summary"]["valuation_status"], "unavailable");
        assert_eq!(body["summary"]["positions_total"], 1);
        assert_eq!(body["summary"]["positions_valued"], 0);

        cleanup_market_data(&pool, &[asset_a]).await;
        cleanup_tree(&pool, &[portfolio_id], &[asset_a], &[user_id]).await;
    }

    #[tokio::test]
    async fn summary_valuation_status_is_empty_when_no_open_positions_exist() {
        let pool = test_pool().await;
        let handle = format!("vse{}", &Uuid::new_v4().simple().to_string()[..12]);
        let user_id = create_user(&pool, &handle).await;
        let portfolio_id = create_portfolio(&pool, user_id, "Empty Val").await;
        insert_summary(&pool, portfolio_id, 0).await;
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

        assert_eq!(body["summary"]["valuation_status"], "empty");
        assert_eq!(body["summary"]["positions_total"], 0);
        assert_eq!(body["summary"]["positions_valued"], 0);

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
