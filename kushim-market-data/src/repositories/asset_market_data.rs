use crate::domain::CurrentQuote;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn upsert_current(
    pool: &PgPool,
    id_asset: Uuid,
    quote: &CurrentQuote,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO asset_market_data (id_asset, price_minor, currency, data_source, as_of)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (id_asset) DO UPDATE SET
            price_minor = EXCLUDED.price_minor,
            currency = EXCLUDED.currency,
            data_source = EXCLUDED.data_source,
            as_of = EXCLUDED.as_of,
            updated_at = now()
        "#,
    )
    .bind(id_asset)
    .bind(quote.price_minor)
    .bind(&quote.currency)
    .bind(&quote.data_source)
    .bind(quote.as_of)
    .execute(pool)
    .await?;

    Ok(())
}
