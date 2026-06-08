use time::Date;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CurrentPortfolioSummaryReadModel {
    pub id_portfolio: Uuid,
    pub base_currency: String,
    pub total_value_minor: i64,
    pub cash_balance_minor: i64,
    pub total_invested_minor: i64,
    pub total_pnl_minor: i64,
    pub total_pnl_pct: Option<String>,
    pub is_estimated: bool,
}

#[derive(Debug, Clone)]
pub struct CurrentPortfolioHoldingReadModel {
    pub id_asset: Uuid,
    pub base_currency: String,
    pub quantity: String,
    pub avg_cost_minor: Option<i64>,
    pub invested_base_minor: i64,
    pub market_value_minor: i64,
    pub pnl_base_minor: i64,
    pub pnl_pct: Option<String>,
    pub weight_pct: Option<String>,
    pub is_estimated: bool,
}

#[derive(Debug, Clone)]
pub struct PortfolioDailySnapshotWrite {
    pub id_portfolio: Uuid,
    pub snapshot_date: Date,
    pub base_currency: String,
    pub cash_balance_minor: i64,
    pub total_value_minor: i64,
    pub total_invested_minor: i64,
    pub total_pnl_minor: i64,
    pub total_pnl_pct: Option<String>,
    pub is_estimated: bool,
    pub source_type: &'static str,
}

#[derive(Debug, Clone)]
pub struct PortfolioHoldingSnapshotDailyWrite {
    pub id_asset: Uuid,
    pub base_currency: String,
    pub quantity: String,
    pub avg_cost_minor: Option<i64>,
    pub invested_minor: i64,
    pub market_value_minor: i64,
    pub pnl_minor: i64,
    pub pnl_pct: Option<String>,
    pub weight_pct: Option<String>,
    pub is_estimated: bool,
}
