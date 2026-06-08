use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BackfillPortfolioDefinition {
    pub id_portfolio: Uuid,
    pub base_currency: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct BackfillDateRange {
    pub date_from: Date,
    pub date_to: Date,
}
