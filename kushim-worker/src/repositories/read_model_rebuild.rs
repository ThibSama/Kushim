use crate::{
    domain::portfolio_state::{
        AssetMarketValue, PortfolioDefinition, PortfolioOperationEvent, RebuiltPortfolioHolding,
        RebuiltPortfolioState, RebuiltPortfolioSummary,
    },
    errors::WorkerError,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct ReadModelRebuildRepository {
    pool: PgPool,
}

impl ReadModelRebuildRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_portfolios_for_rebuild(
        &self,
        target_portfolio_id: Option<Uuid>,
    ) -> Result<Vec<PortfolioDefinition>, WorkerError> {
        let rows = sqlx::query(
            r#"
            SELECT id_portfolio, base_currency
            FROM portfolios
            WHERE deleted_at IS NULL
              AND ($1::uuid IS NULL OR id_portfolio = $1)
            ORDER BY created_at ASC, id_portfolio ASC
            "#,
        )
        .bind(target_portfolio_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(PortfolioDefinition {
                    id_portfolio: row.try_get("id_portfolio")?,
                    base_currency: trim_currency(row.try_get::<String, _>("base_currency")?),
                })
            })
            .collect()
    }

    pub async fn list_posted_operations_for_portfolio(
        &self,
        id_portfolio: Uuid,
    ) -> Result<Vec<PortfolioOperationEvent>, WorkerError> {
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
            ORDER BY executed_at ASC, created_at ASC, id_portfolio_operation ASC
            "#,
        )
        .bind(id_portfolio)
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

    pub async fn find_market_data_for_assets(
        &self,
        id_assets: &[Uuid],
    ) -> Result<HashMap<Uuid, AssetMarketValue>, WorkerError> {
        if id_assets.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT
                id_asset,
                price_minor,
                currency,
                data_source,
                as_of,
                updated_at
            FROM asset_market_data
            WHERE id_asset = ANY($1)
            "#,
        )
        .bind(id_assets)
        .fetch_all(&self.pool)
        .await?;

        let mut market_data = HashMap::new();
        for row in rows {
            let value = AssetMarketValue {
                id_asset: row.try_get("id_asset")?,
                price_minor: row.try_get("price_minor")?,
                currency: trim_currency(row.try_get::<String, _>("currency")?),
                data_source: row.try_get::<Option<String>, _>("data_source")?,
                as_of: row.try_get("as_of")?,
                record_updated_at: row.try_get("updated_at")?,
            };
            market_data.insert(value.id_asset, value);
        }

        Ok(market_data)
    }

    pub async fn replace_read_models_for_portfolio(
        &self,
        rebuilt: &RebuiltPortfolioState,
    ) -> Result<(), WorkerError> {
        let mut transaction = self.pool.begin().await?;
        self.delete_holdings_for_portfolio(&mut transaction, rebuilt.summary.id_portfolio)
            .await?;
        self.insert_holdings(&mut transaction, &rebuilt.holdings)
            .await?;
        self.upsert_summary(&mut transaction, &rebuilt.summary)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn delete_holdings_for_portfolio(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        id_portfolio: Uuid,
    ) -> Result<(), WorkerError> {
        sqlx::query(
            r#"
            DELETE FROM rm_portfolio_holdings
            WHERE id_portfolio = $1
            "#,
        )
        .bind(id_portfolio)
        .execute(transaction.as_mut())
        .await?;

        Ok(())
    }

    async fn insert_holdings(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        holdings: &[RebuiltPortfolioHolding],
    ) -> Result<(), WorkerError> {
        for holding in holdings {
            // Provenance is written in the SAME row insert as the financial
            // values — atomicity is enforced by `replace_read_models_for_portfolio`
            // wrapping the delete + insert + summary upsert in a single
            // transaction. No post-write second pass.
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
                    as_of,
                    valuation_source,
                    market_data_status,
                    market_data_price_minor,
                    market_data_currency,
                    market_data_provider,
                    market_data_as_of,
                    market_data_record_updated_at
                )
                VALUES (
                    $1,
                    $2,
                    $3,
                    $4::numeric,
                    $5,
                    $6,
                    $7,
                    $8,
                    $9::numeric,
                    $10::numeric,
                    $11,
                    $12,
                    $13,
                    $14,
                    $15,
                    $16,
                    $17,
                    $18,
                    $19,
                    $20
                )
                "#,
            )
            .bind(holding.id_portfolio)
            .bind(holding.id_asset)
            .bind(&holding.base_currency)
            .bind(&holding.quantity)
            .bind(holding.avg_cost_minor)
            .bind(holding.invested_base_minor)
            .bind(holding.market_value_minor)
            .bind(holding.pnl_base_minor)
            .bind(holding.pnl_pct.as_deref())
            .bind(holding.weight_pct.as_deref())
            .bind(holding.position_status)
            .bind(holding.is_estimated)
            .bind(holding.as_of)
            .bind(holding.valuation_provenance.valuation_source.as_str())
            .bind(holding.valuation_provenance.market_data_status.as_str())
            .bind(holding.valuation_provenance.market_data_price_minor)
            .bind(holding.valuation_provenance.market_data_currency.as_deref())
            .bind(holding.valuation_provenance.market_data_provider.as_deref())
            .bind(holding.valuation_provenance.market_data_as_of)
            .bind(holding.valuation_provenance.market_data_record_updated_at)
            .execute(transaction.as_mut())
            .await?;
        }

        Ok(())
    }

    async fn upsert_summary(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        summary: &RebuiltPortfolioSummary,
    ) -> Result<(), WorkerError> {
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
            VALUES ($1, $2, $3, $4, $5, $6, $7::numeric, $8, $9, $10)
            ON CONFLICT (id_portfolio) DO UPDATE
            SET
                base_currency = EXCLUDED.base_currency,
                total_value_minor = EXCLUDED.total_value_minor,
                cash_balance_minor = EXCLUDED.cash_balance_minor,
                total_invested_minor = EXCLUDED.total_invested_minor,
                total_pnl_minor = EXCLUDED.total_pnl_minor,
                total_pnl_pct = EXCLUDED.total_pnl_pct,
                portfolio_status = EXCLUDED.portfolio_status,
                is_estimated = EXCLUDED.is_estimated,
                as_of = EXCLUDED.as_of
            "#,
        )
        .bind(summary.id_portfolio)
        .bind(&summary.base_currency)
        .bind(summary.total_value_minor)
        .bind(summary.cash_balance_minor)
        .bind(summary.total_invested_minor)
        .bind(summary.total_pnl_minor)
        .bind(summary.total_pnl_pct.as_deref())
        .bind(summary.portfolio_status)
        .bind(summary.is_estimated)
        .bind(summary.as_of)
        .execute(transaction.as_mut())
        .await?;

        Ok(())
    }
}

fn trim_currency(value: String) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::ReadModelRebuildRepository;
    use sqlx::postgres::PgPoolOptions;
    use std::{fs, path::Path};

    #[tokio::test]
    async fn repository_can_be_constructed() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://postgres:postgres@localhost:5432/kushim")
            .expect("lazy pool should build");

        let repository = ReadModelRebuildRepository::new(pool);
        assert!(std::mem::size_of_val(&repository) > 0);
    }

    #[test]
    fn source_does_not_write_forbidden_tables() {
        let repository_source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("repositories")
            .join("read_model_rebuild.rs");
        let source = fs::read_to_string(&repository_source).expect("source should be readable");
        let content = source.split("#[cfg(test)]").next().unwrap_or(&source);

        let forbidden_patterns = [
            format!("{} {}", "INSERT INTO", "portfolio_snapshots_daily"),
            format!("{} {}", "UPDATE", "portfolio_snapshots_daily"),
            format!("{} {}", "DELETE FROM", "portfolio_snapshots_daily"),
            format!("{} {}", "INSERT INTO", "portfolio_holding_snapshot_daily"),
            format!("{} {}", "UPDATE", "portfolio_holding_snapshot_daily"),
            format!("{} {}", "DELETE FROM", "portfolio_holding_snapshot_daily"),
            format!("{} {}", "INSERT INTO", "asset_market_data"),
            format!("{} {}", "UPDATE", "asset_market_data"),
            format!("{} {}", "DELETE FROM", "asset_market_data"),
            format!("{} {}", "INSERT INTO", "asset_price_history_cache"),
            format!("{} {}", "UPDATE", "asset_price_history_cache"),
            format!("{} {}", "DELETE FROM", "asset_price_history_cache"),
            format!("{} {}", "UPDATE", "portfolio_operations"),
            format!("{} {}", "DELETE FROM", "portfolio_operations"),
        ];

        for pattern in forbidden_patterns {
            assert!(
                !content.contains(&pattern),
                "worker source must not contain forbidden write pattern: {pattern}"
            );
        }
    }
}
