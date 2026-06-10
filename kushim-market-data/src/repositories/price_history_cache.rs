use crate::domain::HistoricalQuote;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn insert_if_missing(
    pool: &PgPool,
    id_asset: Uuid,
    quote: &HistoricalQuote,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO asset_price_history_cache
            (id_asset, price_date, currency, close_minor, source)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (id_asset, price_date, currency, source) DO NOTHING
        "#,
    )
    .bind(id_asset)
    .bind(quote.price_date)
    .bind(&quote.currency)
    .bind(quote.close_minor)
    .bind(&quote.data_source)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
