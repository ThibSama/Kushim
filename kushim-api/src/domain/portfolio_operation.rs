use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Buy,
    Sell,
    Deposit,
    Withdrawal,
    Dividend,
    Interest,
    Fee,
    Tax,
    Split,
    SpinOff,
    SymbolChange,
    TransferIn,
    TransferOut,
    Adjustment,
}

impl OperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Deposit => "deposit",
            Self::Withdrawal => "withdrawal",
            Self::Dividend => "dividend",
            Self::Interest => "interest",
            Self::Fee => "fee",
            Self::Tax => "tax",
            Self::Split => "split",
            Self::SpinOff => "spin_off",
            Self::SymbolChange => "symbol_change",
            Self::TransferIn => "transfer_in",
            Self::TransferOut => "transfer_out",
            Self::Adjustment => "adjustment",
        }
    }
}

impl TryFrom<&str> for OperationType {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "buy" => Ok(Self::Buy),
            "sell" => Ok(Self::Sell),
            "deposit" => Ok(Self::Deposit),
            "withdrawal" => Ok(Self::Withdrawal),
            "dividend" => Ok(Self::Dividend),
            "interest" => Ok(Self::Interest),
            "fee" => Ok(Self::Fee),
            "tax" => Ok(Self::Tax),
            "split" => Ok(Self::Split),
            "spin_off" => Ok(Self::SpinOff),
            "symbol_change" => Ok(Self::SymbolChange),
            "transfer_in" => Ok(Self::TransferIn),
            "transfer_out" => Ok(Self::TransferOut),
            "adjustment" => Ok(Self::Adjustment),
            _ => Err("unknown operation type"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Pending,
    Posted,
    Cancelled,
}

impl OperationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Posted => "posted",
            Self::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for OperationStatus {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pending" => Ok(Self::Pending),
            "posted" => Ok(Self::Posted),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err("unknown operation status"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PortfolioOperation {
    pub id_portfolio_operation: Uuid,
    pub id_portfolio: Uuid,
    pub id_asset: Option<Uuid>,
    pub id_related_asset: Option<Uuid>,
    pub operation_type: OperationType,
    pub operation_status: OperationStatus,
    pub executed_at: OffsetDateTime,
    pub effective_at: Option<OffsetDateTime>,
    pub quantity: Option<String>,
    pub related_quantity: Option<String>,
    pub price_minor: Option<i64>,
    pub gross_amount_minor: Option<i64>,
    pub fees_minor: Option<i64>,
    pub taxes_minor: Option<i64>,
    pub cash_amount_minor: i64,
    pub currency: String,
    pub fx_rate_to_portfolio: Option<String>,
    pub external_provider: Option<String>,
    pub external_reference: Option<String>,
    pub id_corrected_operation: Option<Uuid>,
    pub notes: Option<String>,
    pub metadata: Value,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct NewPortfolioOperation {
    pub id_portfolio: Uuid,
    pub id_asset: Option<Uuid>,
    pub id_related_asset: Option<Uuid>,
    pub operation_type: OperationType,
    pub operation_status: OperationStatus,
    pub executed_at: OffsetDateTime,
    pub effective_at: Option<OffsetDateTime>,
    pub quantity: Option<String>,
    pub related_quantity: Option<String>,
    pub price_minor: Option<i64>,
    pub gross_amount_minor: Option<i64>,
    pub fees_minor: Option<i64>,
    pub taxes_minor: Option<i64>,
    pub cash_amount_minor: i64,
    pub currency: String,
    pub fx_rate_to_portfolio: Option<String>,
    pub external_provider: Option<String>,
    pub external_reference: Option<String>,
    pub id_corrected_operation: Option<Uuid>,
    pub notes: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Default)]
pub struct PortfolioOperationFilters {
    pub operation_status: Option<OperationStatus>,
    pub operation_type: Option<OperationType>,
    pub id_asset: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct UpdatePortfolioOperation {
    pub id_asset: Option<Uuid>,
    pub id_related_asset: Option<Uuid>,
    pub operation_type: OperationType,
    pub executed_at: OffsetDateTime,
    pub effective_at: Option<OffsetDateTime>,
    pub quantity: Option<String>,
    pub related_quantity: Option<String>,
    pub price_minor: Option<i64>,
    pub gross_amount_minor: Option<i64>,
    pub fees_minor: Option<i64>,
    pub taxes_minor: Option<i64>,
    pub cash_amount_minor: i64,
    pub currency: String,
    pub fx_rate_to_portfolio: Option<String>,
    pub external_provider: Option<String>,
    pub external_reference: Option<String>,
    pub id_corrected_operation: Option<Uuid>,
    pub notes: Option<String>,
    pub metadata: Value,
}
