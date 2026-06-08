use crate::{
    domain::{
        portfolio_snapshot::{
            CurrentPortfolioHoldingReadModel, CurrentPortfolioSummaryReadModel,
            PortfolioDailySnapshotWrite, PortfolioHoldingSnapshotDailyWrite,
        },
        portfolio_state::PortfolioDefinition,
    },
    errors::WorkerError,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub struct SnapshotGenerationRepository {
    pool: PgPool,
}

impl SnapshotGenerationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_portfolios_for_snapshot_generation(
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

    pub async fn begin(&self) -> Result<Transaction<'_, Postgres>, WorkerError> {
        Ok(self.pool.begin().await?)
    }

    pub async fn find_current_summary_for_portfolio(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        id_portfolio: Uuid,
    ) -> Result<Option<CurrentPortfolioSummaryReadModel>, WorkerError> {
        let row = sqlx::query(
            r#"
            SELECT
                id_portfolio,
                base_currency,
                total_value_minor,
                cash_balance_minor,
                total_invested_minor,
                total_pnl_minor,
                total_pnl_pct::text AS total_pnl_pct,
                is_estimated
            FROM rm_portfolio_summary
            WHERE id_portfolio = $1
            "#,
        )
        .bind(id_portfolio)
        .fetch_optional(transaction.as_mut())
        .await?;

        row.map(|row| {
            Ok(CurrentPortfolioSummaryReadModel {
                id_portfolio: row.try_get("id_portfolio")?,
                base_currency: trim_currency(row.try_get::<String, _>("base_currency")?),
                total_value_minor: row.try_get("total_value_minor")?,
                cash_balance_minor: row.try_get("cash_balance_minor")?,
                total_invested_minor: row.try_get("total_invested_minor")?,
                total_pnl_minor: row.try_get("total_pnl_minor")?,
                total_pnl_pct: row.try_get("total_pnl_pct")?,
                is_estimated: row.try_get("is_estimated")?,
            })
        })
        .transpose()
    }

    pub async fn list_current_holdings_for_portfolio(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        id_portfolio: Uuid,
    ) -> Result<Vec<CurrentPortfolioHoldingReadModel>, WorkerError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id_asset,
                base_currency,
                quantity::text AS quantity,
                avg_cost_minor,
                invested_base_minor,
                market_value_minor,
                pnl_base_minor,
                pnl_pct::text AS pnl_pct,
                weight_pct::text AS weight_pct,
                is_estimated
            FROM rm_portfolio_holdings
            WHERE id_portfolio = $1
            ORDER BY weight_pct DESC NULLS LAST, market_value_minor DESC, id_asset ASC
            "#,
        )
        .bind(id_portfolio)
        .fetch_all(transaction.as_mut())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(CurrentPortfolioHoldingReadModel {
                    id_asset: row.try_get("id_asset")?,
                    base_currency: trim_currency(row.try_get::<String, _>("base_currency")?),
                    quantity: row.try_get("quantity")?,
                    avg_cost_minor: row.try_get("avg_cost_minor")?,
                    invested_base_minor: row.try_get("invested_base_minor")?,
                    market_value_minor: row.try_get("market_value_minor")?,
                    pnl_base_minor: row.try_get("pnl_base_minor")?,
                    pnl_pct: row.try_get("pnl_pct")?,
                    weight_pct: row.try_get("weight_pct")?,
                    is_estimated: row.try_get("is_estimated")?,
                })
            })
            .collect()
    }

    pub async fn upsert_daily_snapshot(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        snapshot: &PortfolioDailySnapshotWrite,
    ) -> Result<Uuid, WorkerError> {
        let row = sqlx::query(
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
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8::numeric, $9, $10)
            ON CONFLICT (id_portfolio, snapshot_date) DO UPDATE
            SET
                base_currency = EXCLUDED.base_currency,
                cash_balance_minor = EXCLUDED.cash_balance_minor,
                total_value_minor = EXCLUDED.total_value_minor,
                total_invested_minor = EXCLUDED.total_invested_minor,
                total_pnl_minor = EXCLUDED.total_pnl_minor,
                total_pnl_pct = EXCLUDED.total_pnl_pct,
                is_estimated = EXCLUDED.is_estimated,
                source_type = EXCLUDED.source_type
            RETURNING id_portfolio_snapshot_daily
            "#,
        )
        .bind(snapshot.id_portfolio)
        .bind(snapshot.snapshot_date)
        .bind(&snapshot.base_currency)
        .bind(snapshot.cash_balance_minor)
        .bind(snapshot.total_value_minor)
        .bind(snapshot.total_invested_minor)
        .bind(snapshot.total_pnl_minor)
        .bind(snapshot.total_pnl_pct.as_deref())
        .bind(snapshot.is_estimated)
        .bind(snapshot.source_type)
        .fetch_one(transaction.as_mut())
        .await?;

        Ok(row.try_get("id_portfolio_snapshot_daily")?)
    }

    pub async fn replace_holding_snapshots(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        id_portfolio_snapshot_daily: Uuid,
        holdings: &[PortfolioHoldingSnapshotDailyWrite],
    ) -> Result<(), WorkerError> {
        sqlx::query(
            r#"
            DELETE FROM portfolio_holding_snapshot_daily
            WHERE id_portfolio_snapshot_daily = $1
            "#,
        )
        .bind(id_portfolio_snapshot_daily)
        .execute(transaction.as_mut())
        .await?;

        for holding in holdings {
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
                    is_estimated
                )
                VALUES (
                    $1, $2, $3, $4::numeric, $5, $6, $7, $8, $9::numeric, $10::numeric, $11
                )
                "#,
            )
            .bind(id_portfolio_snapshot_daily)
            .bind(holding.id_asset)
            .bind(&holding.base_currency)
            .bind(&holding.quantity)
            .bind(holding.avg_cost_minor)
            .bind(holding.invested_minor)
            .bind(holding.market_value_minor)
            .bind(holding.pnl_minor)
            .bind(holding.pnl_pct.as_deref())
            .bind(holding.weight_pct.as_deref())
            .bind(holding.is_estimated)
            .execute(transaction.as_mut())
            .await?;
        }

        Ok(())
    }
}

fn trim_currency(value: String) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::SnapshotGenerationRepository;
    use sqlx::postgres::PgPoolOptions;
    use std::{fs, path::Path};

    #[tokio::test]
    async fn repository_can_be_constructed() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://postgres:postgres@localhost:5432/kushim")
            .expect("lazy pool should build");

        let repository = SnapshotGenerationRepository::new(pool);
        assert!(std::mem::size_of_val(&repository) > 0);
    }

    #[test]
    fn source_writes_only_snapshot_tables() {
        let repository_source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("repositories")
            .join("snapshot_generation.rs");
        let source = fs::read_to_string(&repository_source).expect("source should be readable");
        let content = source.split("#[cfg(test)]").next().unwrap_or(&source);

        let forbidden_patterns = [
            format!("{} {}", "INSERT INTO", "rm_portfolio_summary"),
            format!("{} {}", "UPDATE", "rm_portfolio_summary"),
            format!("{} {}", "DELETE FROM", "rm_portfolio_summary"),
            format!("{} {}", "INSERT INTO", "rm_portfolio_holdings"),
            format!("{} {}", "UPDATE", "rm_portfolio_holdings"),
            format!("{} {}", "DELETE FROM", "rm_portfolio_holdings"),
            format!("{} {}", "UPDATE", "portfolio_operations"),
            format!("{} {}", "DELETE FROM", "portfolio_operations"),
            format!("{} {}", "INSERT INTO", "asset_market_data"),
            format!("{} {}", "UPDATE", "asset_market_data"),
            format!("{} {}", "DELETE FROM", "asset_market_data"),
            format!("{} {}", "INSERT INTO", "asset_price_history_cache"),
            format!("{} {}", "UPDATE", "asset_price_history_cache"),
            format!("{} {}", "DELETE FROM", "asset_price_history_cache"),
            format!("{} {}", "UPDATE", "portfolios"),
            format!("{} {}", "DELETE FROM", "portfolios"),
            format!("{} {}", "UPDATE", "assets"),
            format!("{} {}", "DELETE FROM", "assets"),
        ];

        for pattern in forbidden_patterns {
            assert!(
                !content.contains(&pattern),
                "worker source must not contain forbidden write pattern: {pattern}"
            );
        }
    }
}
