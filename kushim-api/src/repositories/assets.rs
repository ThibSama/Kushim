use crate::domain::asset::{
    Asset, AssetAlias, AssetClass, AssetDetails, AssetIdentity, AssetMarketData, AssetMetadata,
    AssetSearchFilters, AssetStatus, AssetValidationInfo,
};
use sqlx::{PgPool, Row};
use thiserror::Error;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Clone)]
pub struct AssetRepository {
    pool: PgPool,
}

#[derive(Debug, Error)]
pub enum AssetRepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("invalid asset row")]
    InvalidRow,
}

struct AssetRow {
    id_asset: Uuid,
    asset_class: String,
    status: String,
    name: String,
    native_currency: Option<String>,
    isin: Option<String>,
    ticker: Option<String>,
    exchange: Option<String>,
    symbol: Option<String>,
    network: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    country: Option<String>,
    website_url: Option<String>,
    logo_url: Option<String>,
    description: Option<String>,
    provider: Option<String>,
    provider_asset_id: Option<String>,
    last_synced_at: Option<OffsetDateTime>,
    sector: Option<String>,
    industry: Option<String>,
    price_minor: Option<i64>,
    market_data_currency: Option<String>,
    market_cap_minor: Option<i64>,
    volume_24h_minor: Option<i64>,
    change_24h_pct: Option<String>,
    change_7d_pct: Option<String>,
    change_30d_pct: Option<String>,
    data_source: Option<String>,
    source_asset_id: Option<String>,
    as_of: Option<OffsetDateTime>,
}

impl TryFrom<AssetRow> for AssetDetails {
    type Error = AssetRepositoryError;

    fn try_from(value: AssetRow) -> Result<Self, Self::Error> {
        let asset_class = AssetClass::try_from(value.asset_class.as_str())
            .map_err(|_| AssetRepositoryError::InvalidRow)?;
        let status = AssetStatus::try_from(value.status.as_str())
            .map_err(|_| AssetRepositoryError::InvalidRow)?;

        let metadata = if value.country.is_some()
            || value.website_url.is_some()
            || value.logo_url.is_some()
            || value.description.is_some()
            || value.provider.is_some()
            || value.provider_asset_id.is_some()
            || value.last_synced_at.is_some()
            || value.sector.is_some()
            || value.industry.is_some()
        {
            Some(AssetMetadata {
                country: value.country,
                website_url: value.website_url,
                logo_url: value.logo_url,
                description: value.description,
                provider: value.provider,
                provider_asset_id: value.provider_asset_id,
                sector: value.sector,
                industry: value.industry,
                last_synced_at: value.last_synced_at,
            })
        } else {
            None
        };

        let market_data = match (value.price_minor, value.market_data_currency, value.as_of) {
            (Some(price_minor), Some(currency), Some(as_of)) => Some(AssetMarketData {
                price_minor,
                currency,
                market_cap_minor: value.market_cap_minor,
                volume_24h_minor: value.volume_24h_minor,
                change_24h_pct: value.change_24h_pct,
                change_7d_pct: value.change_7d_pct,
                change_30d_pct: value.change_30d_pct,
                data_source: value.data_source,
                source_asset_id: value.source_asset_id,
                as_of,
            }),
            (None, None, None) => None,
            _ => return Err(AssetRepositoryError::InvalidRow),
        };

        Ok(AssetDetails {
            asset: Asset {
                id_asset: value.id_asset,
                name: value.name,
                ticker: value.ticker,
                isin: value.isin,
                exchange: value.exchange,
                symbol: value.symbol,
                network: value.network,
                asset_class,
                status,
                native_currency: value.native_currency,
                created_at: value.created_at,
                updated_at: value.updated_at,
            },
            metadata,
            market_data,
            aliases: Vec::new(),
        })
    }
}

impl AssetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_assets_page(
        &self,
        filters: &AssetSearchFilters,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AssetDetails>, AssetRepositoryError> {
        let search_pattern = filters
            .search
            .as_ref()
            .map(|value| format!("%{}%", value.to_lowercase()));

        let rows = sqlx::query(
            r#"
            SELECT
                a.id_asset,
                a.asset_class,
                a.status,
                a.name,
                a.native_currency,
                a.isin,
                a.ticker,
                a.exchange,
                a.symbol,
                a.network,
                a.created_at,
                a.updated_at,
                am.country,
                am.website_url,
                am.logo_url,
                am.description,
                am.provider,
                am.provider_asset_id,
                am.last_synced_at,
                s.label AS sector,
                i.label AS industry,
                amd.price_minor,
                amd.currency AS market_data_currency,
                amd.market_cap_minor,
                amd.volume_24h_minor,
                amd.change_24h_pct::text AS change_24h_pct,
                amd.change_7d_pct::text AS change_7d_pct,
                amd.change_30d_pct::text AS change_30d_pct,
                amd.data_source,
                amd.source_asset_id,
                amd.as_of
            FROM assets a
            LEFT JOIN asset_metadata am
                ON am.id_asset = a.id_asset
            LEFT JOIN industries i
                ON i.id_industry = am.id_industry
            LEFT JOIN sectors s
                ON s.id_sector = i.id_sector
            LEFT JOIN asset_market_data amd
                ON amd.id_asset = a.id_asset
            WHERE ($1::varchar IS NULL OR a.status = $1)
              AND ($2::varchar IS NULL OR a.asset_class = $2)
              AND ($3::varchar IS NULL OR lower(a.ticker) = lower($3))
              AND ($4::varchar IS NULL OR upper(a.isin) = upper($4))
              AND ($5::varchar IS NULL OR lower(a.exchange) = lower($5))
              AND (
                    $6::varchar IS NULL
                    OR lower(a.name) LIKE $6
                    OR lower(COALESCE(a.ticker, '')) LIKE $6
                    OR lower(COALESCE(a.isin, '')) LIKE $6
                    OR EXISTS (
                        SELECT 1
                        FROM asset_aliases aa
                        WHERE aa.id_asset = a.id_asset
                          AND lower(aa.alias) LIKE $6
                    )
              )
              -- Catalogue discoverability policy: equities are exchange-listed
              -- by definition (their natural identity is (ticker, exchange) per
              -- 002_seed_canonical_assets.sql). An equity row with no exchange
              -- cannot be uniquely resolved by a user — every integration-test
              -- fixture matches this shape, and no canonical row does. Hiding
              -- them here keeps direct-by-id lookup (`find_by_id`) unfiltered
              -- so historical operations remain resolvable. Crypto/ETF/bond/
              -- forex/etc. asset classes are NOT subject to this rule because
              -- their natural identity is symbol/network/ISIN, not exchange.
              AND NOT (a.asset_class = 'equity' AND a.exchange IS NULL)
            ORDER BY a.name ASC, a.ticker ASC NULLS LAST, a.exchange ASC NULLS LAST
            LIMIT $7
            OFFSET $8
            "#,
        )
        .bind(filters.status.as_ref().map(AssetStatus::as_str))
        .bind(filters.asset_class.as_ref().map(AssetClass::as_str))
        .bind(filters.ticker.as_deref())
        .bind(filters.isin.as_deref())
        .bind(filters.exchange.as_deref())
        .bind(search_pattern.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|row| asset_from_row(&row)).collect()
    }

    pub async fn find_by_id(
        &self,
        id_asset: Uuid,
    ) -> Result<Option<AssetDetails>, AssetRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                a.id_asset,
                a.asset_class,
                a.status,
                a.name,
                a.native_currency,
                a.isin,
                a.ticker,
                a.exchange,
                a.symbol,
                a.network,
                a.created_at,
                a.updated_at,
                am.country,
                am.website_url,
                am.logo_url,
                am.description,
                am.provider,
                am.provider_asset_id,
                am.last_synced_at,
                s.label AS sector,
                i.label AS industry,
                amd.price_minor,
                amd.currency AS market_data_currency,
                amd.market_cap_minor,
                amd.volume_24h_minor,
                amd.change_24h_pct::text AS change_24h_pct,
                amd.change_7d_pct::text AS change_7d_pct,
                amd.change_30d_pct::text AS change_30d_pct,
                amd.data_source,
                amd.source_asset_id,
                amd.as_of
            FROM assets a
            LEFT JOIN asset_metadata am
                ON am.id_asset = a.id_asset
            LEFT JOIN industries i
                ON i.id_industry = am.id_industry
            LEFT JOIN sectors s
                ON s.id_sector = i.id_sector
            LEFT JOIN asset_market_data amd
                ON amd.id_asset = a.id_asset
            WHERE a.id_asset = $1
            "#,
        )
        .bind(id_asset)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| asset_from_row(&row)).transpose()
    }

    pub async fn find_validation_info(
        &self,
        id_asset: Uuid,
    ) -> Result<Option<AssetValidationInfo>, AssetRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id_asset,
                status
            FROM assets
            WHERE id_asset = $1
            "#,
        )
        .bind(id_asset)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(AssetValidationInfo {
                status: AssetStatus::try_from(row.try_get::<String, _>("status")?.as_str())
                    .map_err(|_| AssetRepositoryError::InvalidRow)?,
            })
        })
        .transpose()
    }

    /// Batch lookup of compact asset identities by id.
    ///
    /// Used by the portfolio-operation enrichment path (P2) to resolve the
    /// `asset` / `related_asset` references for an operation list (or a single
    /// operation) in **one** database round trip, regardless of how many
    /// operations are returned. Duplicate ids in the input are deduplicated by
    /// PostgreSQL's `= ANY` operator; an empty input short-circuits without
    /// touching the database.
    ///
    /// Returns identities for whatever ids resolve — missing rows are not an
    /// error. The status column is preserved so callers can keep historical
    /// references displayable even when the asset is no longer active.
    pub async fn list_identities_by_ids(
        &self,
        ids: &[Uuid],
    ) -> Result<Vec<AssetIdentity>, AssetRepositoryError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT
                id_asset,
                name,
                ticker,
                status
            FROM assets
            WHERE id_asset = ANY($1)
            "#,
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let status_str: String = row.try_get("status")?;
                Ok(AssetIdentity {
                    id_asset: row.try_get("id_asset")?,
                    name: row.try_get("name")?,
                    ticker: row.try_get("ticker")?,
                    status: AssetStatus::try_from(status_str.as_str())
                        .map_err(|_| AssetRepositoryError::InvalidRow)?,
                })
            })
            .collect()
    }

    pub async fn list_aliases_for_asset(
        &self,
        id_asset: Uuid,
    ) -> Result<Vec<AssetAlias>, AssetRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                alias,
                alias_type,
                source,
                valid_from,
                valid_to
            FROM asset_aliases
            WHERE id_asset = $1
            ORDER BY alias ASC, valid_from ASC NULLS FIRST, valid_to ASC NULLS FIRST
            "#,
        )
        .bind(id_asset)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(AssetAlias {
                    alias: row.try_get("alias")?,
                    alias_type: row.try_get("alias_type")?,
                    source: row.try_get("source")?,
                    valid_from: row.try_get::<Option<Date>, _>("valid_from")?,
                    valid_to: row.try_get::<Option<Date>, _>("valid_to")?,
                })
            })
            .collect()
    }
}

fn asset_from_row(row: &sqlx::postgres::PgRow) -> Result<AssetDetails, AssetRepositoryError> {
    AssetRow {
        id_asset: row.try_get("id_asset")?,
        asset_class: row.try_get("asset_class")?,
        status: row.try_get("status")?,
        name: row.try_get("name")?,
        native_currency: row.try_get("native_currency")?,
        isin: row.try_get("isin")?,
        ticker: row.try_get("ticker")?,
        exchange: row.try_get("exchange")?,
        symbol: row.try_get("symbol")?,
        network: row.try_get("network")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        country: row.try_get("country")?,
        website_url: row.try_get("website_url")?,
        logo_url: row.try_get("logo_url")?,
        description: row.try_get("description")?,
        provider: row.try_get("provider")?,
        provider_asset_id: row.try_get("provider_asset_id")?,
        last_synced_at: row.try_get("last_synced_at")?,
        sector: row.try_get("sector")?,
        industry: row.try_get("industry")?,
        price_minor: row.try_get("price_minor")?,
        market_data_currency: row.try_get("market_data_currency")?,
        market_cap_minor: row.try_get("market_cap_minor")?,
        volume_24h_minor: row.try_get("volume_24h_minor")?,
        change_24h_pct: row.try_get("change_24h_pct")?,
        change_7d_pct: row.try_get("change_7d_pct")?,
        change_30d_pct: row.try_get("change_30d_pct")?,
        data_source: row.try_get("data_source")?,
        source_asset_id: row.try_get("source_asset_id")?,
        as_of: row.try_get("as_of")?,
    }
    .try_into()
}
