use crate::{
    domain::{
        portfolio_backfill::BackfillPortfolioDefinition,
        portfolio_state::{AssetMarketValue, PortfolioOperationEvent},
    },
    errors::WorkerError,
};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use time::{Date, PrimitiveDateTime};
use uuid::Uuid;

#[derive(Clone)]
pub struct BackfillSnapshotsRepository {
    pool: PgPool,
}

impl BackfillSnapshotsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_target_portfolio(
        &self,
        id_portfolio: Uuid,
    ) -> Result<Option<BackfillPortfolioDefinition>, WorkerError> {
        let row = sqlx::query(
            r#"
            SELECT id_portfolio, base_currency, created_at
            FROM portfolios
            WHERE id_portfolio = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(id_portfolio)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(BackfillPortfolioDefinition {
                id_portfolio: row.try_get("id_portfolio")?,
                base_currency: trim_currency(row.try_get::<String, _>("base_currency")?),
                created_at: row.try_get("created_at")?,
            })
        })
        .transpose()
    }

    pub async fn list_posted_operations_through_date(
        &self,
        id_portfolio: Uuid,
        snapshot_date: Date,
    ) -> Result<Vec<PortfolioOperationEvent>, WorkerError> {
        let next_day = snapshot_date.next_day().ok_or_else(|| {
            WorkerError::Job(format!(
                "snapshot date {} exceeds supported date bounds for backfill",
                snapshot_date
            ))
        })?;
        let next_day_start = PrimitiveDateTime::new(next_day, time::Time::MIDNIGHT).assume_utc();

        let rows = sqlx::query(
            r#"
            SELECT
                id_portfolio_operation,
                id_asset,
                id_related_asset,
                operation_type,
                quantity::text AS quantity,
                related_quantity::text AS related_quantity,
                cash_amount_minor,
                currency,
                fx_rate_to_portfolio::text AS fx_rate_to_portfolio
            FROM portfolio_operations
            WHERE id_portfolio = $1
              AND operation_status = 'posted'
              AND executed_at < $2
            ORDER BY executed_at ASC, created_at ASC, id_portfolio_operation ASC
            "#,
        )
        .bind(id_portfolio)
        .bind(next_day_start)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(PortfolioOperationEvent {
                    id_portfolio_operation: row.try_get("id_portfolio_operation")?,
                    id_asset: row.try_get("id_asset")?,
                    id_related_asset: row.try_get("id_related_asset")?,
                    operation_type: row
                        .try_get::<String, _>("operation_type")?
                        .as_str()
                        .try_into()?,
                    quantity: row.try_get("quantity")?,
                    related_quantity: row.try_get("related_quantity")?,
                    cash_amount_minor: row.try_get("cash_amount_minor")?,
                    currency: trim_currency(row.try_get::<String, _>("currency")?),
                    fx_rate_to_portfolio: row.try_get("fx_rate_to_portfolio")?,
                })
            })
            .collect()
    }

    pub async fn find_historical_prices_for_assets(
        &self,
        id_assets: &[Uuid],
        price_date: Date,
        currency: &str,
    ) -> Result<HashMap<Uuid, AssetMarketValue>, WorkerError> {
        if id_assets.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (id_asset)
                id_asset,
                close_minor,
                currency
            FROM asset_price_history_cache
            WHERE id_asset = ANY($1)
              AND price_date = $2
              AND currency = $3
            ORDER BY
                id_asset ASC,
                CASE WHEN source = 'default' THEN 0 ELSE 1 END ASC,
                fetched_at DESC,
                created_at DESC,
                id_asset_price_history_cache DESC
            "#,
        )
        .bind(id_assets)
        .bind(price_date)
        .bind(currency)
        .fetch_all(&self.pool)
        .await?;

        let mut prices = HashMap::new();
        for row in rows {
            let value = AssetMarketValue {
                id_asset: row.try_get("id_asset")?,
                price_minor: row.try_get("close_minor")?,
                currency: trim_currency(row.try_get::<String, _>("currency")?),
            };
            prices.insert(value.id_asset, value);
        }

        Ok(prices)
    }
}

fn trim_currency(value: String) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::BackfillSnapshotsRepository;
    use sqlx::postgres::PgPoolOptions;
    use std::{fs, path::Path};

    #[tokio::test]
    async fn repository_can_be_constructed() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://postgres:postgres@localhost:5432/kushim")
            .expect("lazy pool should build");

        let repository = BackfillSnapshotsRepository::new(pool);
        assert!(std::mem::size_of_val(&repository) > 0);
    }

    #[test]
    fn source_reads_history_and_does_not_write_forbidden_tables() {
        let repository_source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("repositories")
            .join("backfill_snapshots.rs");
        let source = fs::read_to_string(&repository_source).expect("source should be readable");
        let content = source.split("#[cfg(test)]").next().unwrap_or(&source);

        let forbidden_patterns = [
            format!("{} {}", "INSERT INTO", "rm_portfolio_summary"),
            format!("{} {}", "UPDATE", "rm_portfolio_summary"),
            format!("{} {}", "DELETE FROM", "rm_portfolio_summary"),
            format!("{} {}", "INSERT INTO", "rm_portfolio_holdings"),
            format!("{} {}", "UPDATE", "rm_portfolio_holdings"),
            format!("{} {}", "DELETE FROM", "rm_portfolio_holdings"),
            format!("{} {}", "INSERT INTO", "portfolio_snapshots_daily"),
            format!("{} {}", "UPDATE", "portfolio_snapshots_daily"),
            format!("{} {}", "DELETE FROM", "portfolio_snapshots_daily"),
            format!("{} {}", "INSERT INTO", "portfolio_holding_snapshot_daily"),
            format!("{} {}", "UPDATE", "portfolio_holding_snapshot_daily"),
            format!("{} {}", "DELETE FROM", "portfolio_holding_snapshot_daily"),
            format!("{} {}", "UPDATE", "portfolio_operations"),
            format!("{} {}", "DELETE FROM", "portfolio_operations"),
            format!("{} {}", "INSERT INTO", "asset_market_data"),
            format!("{} {}", "UPDATE", "asset_market_data"),
            format!("{} {}", "DELETE FROM", "asset_market_data"),
            format!("{} {}", "INSERT INTO", "asset_price_history_cache"),
            format!("{} {}", "UPDATE", "asset_price_history_cache"),
            format!("{} {}", "DELETE FROM", "asset_price_history_cache"),
        ];

        for pattern in forbidden_patterns {
            assert!(
                !content.contains(&pattern),
                "worker source must not contain forbidden write pattern: {pattern}"
            );
        }

        assert!(
            content.contains("FROM asset_price_history_cache"),
            "backfill repository should read asset_price_history_cache"
        );
    }
}
