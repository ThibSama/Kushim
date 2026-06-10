use crate::domain::ActiveAsset;
use sqlx::PgPool;

pub async fn list_active_assets(pool: &PgPool) -> Result<Vec<ActiveAsset>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ActiveAssetRow>(
        r#"
        SELECT id_asset, symbol, ticker, native_currency
        FROM assets
        WHERE status = 'active'
        ORDER BY name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

#[derive(sqlx::FromRow)]
struct ActiveAssetRow {
    id_asset: uuid::Uuid,
    symbol: Option<String>,
    ticker: Option<String>,
    native_currency: Option<String>,
}

impl From<ActiveAssetRow> for ActiveAsset {
    fn from(row: ActiveAssetRow) -> Self {
        Self {
            id_asset: row.id_asset,
            symbol: row.symbol,
            ticker: row.ticker,
            native_currency: row.native_currency,
        }
    }
}
