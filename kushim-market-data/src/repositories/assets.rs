use crate::domain::ActiveAsset;
use sqlx::{PgPool, Row};

pub async fn list_active_assets(pool: &PgPool) -> Result<Vec<ActiveAsset>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT id_asset, symbol, ticker, native_currency
        FROM assets
        WHERE status = 'active'
        ORDER BY name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ActiveAsset {
            id_asset: row.get("id_asset"),
            symbol: row.get("symbol"),
            ticker: row.get("ticker"),
            native_currency: row.get("native_currency"),
        })
        .collect())
}
