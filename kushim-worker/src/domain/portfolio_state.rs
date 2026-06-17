use crate::errors::WorkerError;
use std::collections::{HashMap, HashSet};
use time::OffsetDateTime;
use uuid::Uuid;

const QUANTITY_SCALE: u32 = 10;
const RATE_SCALE: u32 = 10;
const PCT_SCALE: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl TryFrom<&str> for OperationType {
    type Error = WorkerError;

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
            _ => Err(WorkerError::Job(format!(
                "unknown portfolio operation type `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PortfolioOperationEvent {
    pub id_portfolio_operation: Uuid,
    pub id_asset: Option<Uuid>,
    pub id_related_asset: Option<Uuid>,
    pub operation_type: OperationType,
    pub quantity: Option<String>,
    pub related_quantity: Option<String>,
    pub cash_amount_minor: i64,
    pub currency: String,
    pub fx_rate_to_portfolio: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AssetMarketValue {
    pub id_asset: Uuid,
    pub price_minor: i64,
    pub currency: String,
}

#[derive(Debug, Clone)]
pub struct PortfolioDefinition {
    pub id_portfolio: Uuid,
    pub base_currency: String,
}

#[derive(Debug, Clone)]
pub struct RebuiltPortfolioState {
    pub summary: RebuiltPortfolioSummary,
    pub holdings: Vec<RebuiltPortfolioHolding>,
}

#[derive(Debug, Clone)]
pub struct RebuiltPortfolioSummary {
    pub id_portfolio: Uuid,
    pub base_currency: String,
    pub total_value_minor: i64,
    pub cash_balance_minor: i64,
    pub total_invested_minor: i64,
    pub total_pnl_minor: i64,
    pub total_pnl_pct: Option<String>,
    pub portfolio_status: &'static str,
    pub is_estimated: bool,
    pub as_of: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct RebuiltPortfolioHolding {
    pub id_portfolio: Uuid,
    pub id_asset: Uuid,
    pub base_currency: String,
    pub quantity: String,
    pub avg_cost_minor: Option<i64>,
    pub invested_base_minor: i64,
    pub market_value_minor: i64,
    pub pnl_base_minor: i64,
    pub pnl_pct: Option<String>,
    /// Holdings-only allocation: this holding's share of the sum of all
    /// holdings' `market_value_minor`, excluding cash. Always in `[0, 100]`
    /// (matches `chk_rm_portfolio_holdings_weight_pct_range`) and the non-null
    /// values across a portfolio sum to 100. NULL when the holding is valued
    /// at zero (no compatible market data and zero invested cost). The
    /// frontend Dashboard allocation chart uses the same denominator.
    pub weight_pct: Option<String>,
    pub position_status: &'static str,
    pub is_estimated: bool,
    pub as_of: OffsetDateTime,
}

#[derive(Debug, Clone)]
struct PositionState {
    quantity_scaled: i128,
    invested_base_minor: i128,
}

#[derive(Debug, Clone)]
pub struct PortfolioState {
    portfolio: PortfolioDefinition,
    cash_balance_minor: i128,
    total_invested_minor: i128,
    positions: HashMap<Uuid, PositionState>,
    estimated_assets: HashSet<Uuid>,
    is_estimated: bool,
}

impl PortfolioState {
    pub fn new(portfolio: PortfolioDefinition) -> Self {
        Self {
            portfolio,
            cash_balance_minor: 0,
            total_invested_minor: 0,
            positions: HashMap::new(),
            estimated_assets: HashSet::new(),
            is_estimated: false,
        }
    }

    pub fn apply(&mut self, operation: &PortfolioOperationEvent) -> Result<(), WorkerError> {
        let converted_cash = self.convert_amount_to_base(
            operation.cash_amount_minor,
            &operation.currency,
            operation.fx_rate_to_portfolio.as_deref(),
            operation.id_portfolio_operation,
        )?;

        match operation.operation_type {
            OperationType::Deposit => {
                self.cash_balance_minor += converted_cash;
                self.total_invested_minor += converted_cash;
            }
            OperationType::Withdrawal => {
                self.cash_balance_minor -= converted_cash;
                self.total_invested_minor = (self.total_invested_minor - converted_cash).max(0);
            }
            OperationType::Buy => {
                let id_asset = require_asset_id(operation)?;
                let quantity = require_quantity(operation)?;
                self.increase_position(id_asset, quantity, converted_cash);
                self.cash_balance_minor -= converted_cash;
            }
            OperationType::Sell => {
                let id_asset = require_asset_id(operation)?;
                let quantity = require_quantity(operation)?;
                self.reduce_position(id_asset, quantity, operation.id_portfolio_operation)?;
                self.cash_balance_minor += converted_cash;
            }
            OperationType::Dividend | OperationType::Interest => {
                self.cash_balance_minor += converted_cash;
            }
            OperationType::Fee | OperationType::Tax => {
                self.cash_balance_minor -= converted_cash;
            }
            OperationType::TransferIn => {
                self.cash_balance_minor += converted_cash;
                self.total_invested_minor += converted_cash;
                if let (Some(id_asset), Some(quantity)) = (
                    operation.id_asset,
                    parse_optional_quantity(operation.quantity.as_deref())?,
                ) {
                    self.increase_position(id_asset, quantity, converted_cash);
                }
            }
            OperationType::TransferOut => {
                self.cash_balance_minor -= converted_cash;
                self.total_invested_minor = (self.total_invested_minor - converted_cash).max(0);
                if let (Some(id_asset), Some(quantity)) = (
                    operation.id_asset,
                    parse_optional_quantity(operation.quantity.as_deref())?,
                ) {
                    self.reduce_position(id_asset, quantity, operation.id_portfolio_operation)?;
                }
            }
            OperationType::Split => {
                let id_asset = require_asset_id(operation)?;
                let quantity = require_quantity(operation)?;
                self.increase_position(id_asset, quantity, 0);
                self.is_estimated = true;
            }
            OperationType::SpinOff => {
                let id_related_asset = require_related_asset_id(operation)?;
                let related_quantity = require_related_quantity(operation)?;
                self.increase_position(id_related_asset, related_quantity, 0);
                self.is_estimated = true;
            }
            OperationType::SymbolChange => {
                let id_asset = require_asset_id(operation)?;
                let id_related_asset = require_related_asset_id(operation)?;
                let quantity = require_quantity(operation)?;
                let cost_reduction =
                    self.reduce_position(id_asset, quantity, operation.id_portfolio_operation)?;
                self.increase_position(id_related_asset, quantity, cost_reduction);
            }
            OperationType::Adjustment => {
                if converted_cash > 0 {
                    self.cash_balance_minor += converted_cash;
                }

                if let (Some(id_asset), Some(quantity)) = (
                    operation.id_asset,
                    parse_optional_quantity(operation.quantity.as_deref())?,
                ) {
                    self.increase_position(id_asset, quantity, converted_cash);
                }

                if let (Some(id_related_asset), Some(quantity)) = (
                    operation.id_related_asset,
                    parse_optional_quantity(operation.related_quantity.as_deref())?,
                ) {
                    let _ = self.reduce_position(
                        id_related_asset,
                        quantity,
                        operation.id_portfolio_operation,
                    )?;
                }
            }
        }

        Ok(())
    }

    pub fn finalize(
        mut self,
        market_data: &HashMap<Uuid, AssetMarketValue>,
        as_of: OffsetDateTime,
    ) -> Result<RebuiltPortfolioState, WorkerError> {
        self.prune_zero_positions();

        let mut holdings = Vec::new();
        let mut total_market_value_minor = 0_i128;
        let mut holding_rows = Vec::new();

        for (id_asset, position) in &self.positions {
            if position.quantity_scaled <= 0 {
                continue;
            }

            // `quantity_scaled` is the position's quantity multiplied by
            // `10^QUANTITY_SCALE` (i128), while `invested_base_minor` is a raw
            // minor-unit total. Dividing the two directly truncates to zero for
            // any realistic position because the denominator is ~10^10 larger
            // than the numerator. Scale the numerator by `10^QUANTITY_SCALE`
            // before dividing — the mirror of `multiply_price_by_quantity` —
            // to recover minor units per share. Uses the existing helper so
            // the multiplication happens in i128 before rounding.
            let avg_cost_minor = if position.quantity_scaled > 0 {
                Some(divide_multiply_round(
                    position.invested_base_minor,
                    ten_pow(QUANTITY_SCALE),
                    position.quantity_scaled,
                ))
            } else {
                None
            };

            let mut market_value_minor = 0_i128;
            let mut holding_is_estimated = self.estimated_assets.contains(id_asset);

            if let Some(market_value) = market_data.get(id_asset) {
                if market_value.currency.trim() == self.portfolio.base_currency {
                    market_value_minor = multiply_price_by_quantity(
                        i128::from(market_value.price_minor),
                        position.quantity_scaled,
                    );
                } else {
                    holding_is_estimated = true;
                }
            } else {
                holding_is_estimated = true;
            }

            // Deterministic fallback when no compatible live price is available:
            // value the holding at the invested base cost from the trusted ledger
            // and mark it as estimated. Without this, a portfolio whose holdings
            // are all in a foreign currency or whose market data is missing would
            // have its holdings silently valued at zero — and a recent buy whose
            // cash leg was already deducted would make `total_value_minor` go
            // negative and abort the entire rebuild.
            if holding_is_estimated && market_value_minor == 0 {
                market_value_minor = position.invested_base_minor;
            }

            if holding_is_estimated {
                self.is_estimated = true;
            }

            total_market_value_minor += market_value_minor;
            let pnl_base_minor = market_value_minor - position.invested_base_minor;
            let pnl_pct = if position.invested_base_minor > 0 {
                Some(format_scaled(
                    percentage_scaled(pnl_base_minor, position.invested_base_minor, PCT_SCALE),
                    PCT_SCALE,
                ))
            } else {
                None
            };

            holding_rows.push((
                *id_asset,
                RebuiltPortfolioHolding {
                    id_portfolio: self.portfolio.id_portfolio,
                    id_asset: *id_asset,
                    base_currency: self.portfolio.base_currency.clone(),
                    quantity: format_scaled(position.quantity_scaled, QUANTITY_SCALE),
                    avg_cost_minor: avg_cost_minor.map(to_i64_safely).transpose()?,
                    invested_base_minor: to_i64_safely(position.invested_base_minor)?,
                    market_value_minor: to_i64_safely(market_value_minor)?,
                    pnl_base_minor: to_i64_safely(pnl_base_minor)?,
                    pnl_pct,
                    weight_pct: None,
                    position_status: "open",
                    is_estimated: holding_is_estimated,
                    as_of,
                },
            ));
        }

        let total_value_minor = self.cash_balance_minor + total_market_value_minor;
        if total_value_minor < 0 {
            return Err(WorkerError::Job(format!(
                "portfolio {} produced a negative total_value_minor during rebuild",
                self.portfolio.id_portfolio
            )));
        }

        // Weights are computed against the sum of holding values rather than
        // total_value, so they always lie in [0, 100] (the storage constraint)
        // and sum to 100% across holdings even when cash is negative — which
        // can happen after a foreign-currency buy with no fx_rate is replayed
        // with the invested-cost fallback.
        for (_, mut holding) in holding_rows {
            if total_market_value_minor > 0 && i128::from(holding.market_value_minor) > 0 {
                let weight = percentage_scaled(
                    i128::from(holding.market_value_minor),
                    total_market_value_minor,
                    PCT_SCALE,
                );
                holding.weight_pct = Some(format_scaled(weight, PCT_SCALE));
            }
            holdings.push(holding);
        }

        let total_pnl_minor = total_value_minor - self.total_invested_minor;
        let total_pnl_pct = if self.total_invested_minor > 0 {
            Some(format_scaled(
                percentage_scaled(total_pnl_minor, self.total_invested_minor, PCT_SCALE),
                PCT_SCALE,
            ))
        } else {
            None
        };

        let portfolio_status = if holdings.is_empty() && self.cash_balance_minor == 0 {
            "empty"
        } else {
            "active"
        };

        Ok(RebuiltPortfolioState {
            summary: RebuiltPortfolioSummary {
                id_portfolio: self.portfolio.id_portfolio,
                base_currency: self.portfolio.base_currency,
                total_value_minor: to_i64_safely(total_value_minor)?,
                cash_balance_minor: to_i64_safely(self.cash_balance_minor)?,
                total_invested_minor: to_i64_safely(self.total_invested_minor.max(0))?,
                total_pnl_minor: to_i64_safely(total_pnl_minor)?,
                total_pnl_pct,
                portfolio_status,
                is_estimated: self.is_estimated,
                as_of,
            },
            holdings,
        })
    }

    fn increase_position(&mut self, id_asset: Uuid, quantity_scaled: i128, invested_delta: i128) {
        let entry = self.positions.entry(id_asset).or_insert(PositionState {
            quantity_scaled: 0,
            invested_base_minor: 0,
        });

        entry.quantity_scaled += quantity_scaled;
        entry.invested_base_minor += invested_delta.max(0);
    }

    fn reduce_position(
        &mut self,
        id_asset: Uuid,
        quantity_scaled: i128,
        operation_id: Uuid,
    ) -> Result<i128, WorkerError> {
        let entry = self.positions.get_mut(&id_asset).ok_or_else(|| {
            WorkerError::Job(format!(
                "operation {operation_id} references asset {id_asset} with no open position"
            ))
        })?;

        if entry.quantity_scaled < quantity_scaled {
            return Err(WorkerError::Job(format!(
                "operation {operation_id} would make asset {id_asset} quantity negative"
            )));
        }

        let invested_reduction = if entry.quantity_scaled == quantity_scaled {
            entry.invested_base_minor
        } else if entry.quantity_scaled > 0 {
            divide_multiply_round(
                entry.invested_base_minor,
                quantity_scaled,
                entry.quantity_scaled,
            )
        } else {
            0
        };

        entry.quantity_scaled -= quantity_scaled;
        entry.invested_base_minor = (entry.invested_base_minor - invested_reduction).max(0);
        Ok(invested_reduction)
    }

    fn prune_zero_positions(&mut self) {
        self.positions
            .retain(|_, position| position.quantity_scaled > 0);
    }

    fn convert_amount_to_base(
        &mut self,
        amount_minor: i64,
        currency: &str,
        fx_rate_to_portfolio: Option<&str>,
        operation_id: Uuid,
    ) -> Result<i128, WorkerError> {
        if amount_minor == 0 {
            return Ok(0);
        }

        if currency.trim() == self.portfolio.base_currency {
            return Ok(i128::from(amount_minor));
        }

        let Some(rate) = fx_rate_to_portfolio else {
            self.is_estimated = true;
            return Ok(0);
        };

        let rate_scaled = parse_scaled(rate, RATE_SCALE).map_err(|error| {
            WorkerError::Job(format!(
                "operation {operation_id} has an invalid fx_rate_to_portfolio: {error}"
            ))
        })?;

        Ok(multiply_and_round(
            i128::from(amount_minor),
            rate_scaled,
            RATE_SCALE,
        ))
    }
}

fn require_asset_id(operation: &PortfolioOperationEvent) -> Result<Uuid, WorkerError> {
    operation.id_asset.ok_or_else(|| {
        WorkerError::Job(format!(
            "operation {} requires id_asset for replay",
            operation.id_portfolio_operation
        ))
    })
}

fn require_related_asset_id(operation: &PortfolioOperationEvent) -> Result<Uuid, WorkerError> {
    operation.id_related_asset.ok_or_else(|| {
        WorkerError::Job(format!(
            "operation {} requires id_related_asset for replay",
            operation.id_portfolio_operation
        ))
    })
}

fn require_quantity(operation: &PortfolioOperationEvent) -> Result<i128, WorkerError> {
    parse_optional_quantity(operation.quantity.as_deref())?.ok_or_else(|| {
        WorkerError::Job(format!(
            "operation {} requires quantity for replay",
            operation.id_portfolio_operation
        ))
    })
}

fn require_related_quantity(operation: &PortfolioOperationEvent) -> Result<i128, WorkerError> {
    parse_optional_quantity(operation.related_quantity.as_deref())?.ok_or_else(|| {
        WorkerError::Job(format!(
            "operation {} requires related_quantity for replay",
            operation.id_portfolio_operation
        ))
    })
}

fn parse_optional_quantity(value: Option<&str>) -> Result<Option<i128>, WorkerError> {
    match value {
        Some(value) => Ok(Some(parse_scaled(value, QUANTITY_SCALE).map_err(
            |error| WorkerError::Job(format!("invalid quantity `{value}`: {error}")),
        )?)),
        None => Ok(None),
    }
}

pub fn parse_scaled(input: &str, scale: u32) -> Result<i128, &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("value cannot be empty");
    }

    let negative = trimmed.starts_with('-');
    let unsigned = if negative { &trimmed[1..] } else { trimmed };
    let mut parts = unsigned.split('.');
    let integer = parts.next().ok_or("missing integer part")?;
    let fractional = parts.next();
    if parts.next().is_some() {
        return Err("too many decimal points");
    }

    if integer.is_empty() || !integer.chars().all(|char| char.is_ascii_digit()) {
        return Err("invalid integer digits");
    }

    let mut digits = integer.to_string();
    match fractional {
        Some(value) => {
            if !value.chars().all(|char| char.is_ascii_digit()) {
                return Err("invalid fractional digits");
            }
            if value.len() > scale as usize {
                return Err("too many fractional digits");
            }
            digits.push_str(value);
            digits.extend(std::iter::repeat_n('0', scale as usize - value.len()));
        }
        None => digits.extend(std::iter::repeat_n('0', scale as usize)),
    }

    let parsed = digits.parse::<i128>().map_err(|_| "decimal out of range")?;
    Ok(if negative { -parsed } else { parsed })
}

pub fn format_scaled(value: i128, scale: u32) -> String {
    let negative = value < 0;
    let absolute = value.abs();
    let scale_factor = ten_pow(scale);
    let integer = absolute / scale_factor;
    let fractional = (absolute % scale_factor) as u128;
    let mut rendered = format!("{integer}.{fractional:0width$}", width = scale as usize);
    if negative {
        rendered.insert(0, '-');
    }
    rendered
}

fn ten_pow(scale: u32) -> i128 {
    10_i128.pow(scale)
}

fn multiply_and_round(integer: i128, scaled_factor: i128, scale: u32) -> i128 {
    divide_round(integer * scaled_factor, ten_pow(scale))
}

fn divide_multiply_round(base: i128, multiply: i128, divide: i128) -> i128 {
    divide_round(base * multiply, divide)
}

fn divide_round(numerator: i128, denominator: i128) -> i128 {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let doubled = remainder.abs() * 2;
    if doubled >= denominator.abs() {
        quotient + numerator.signum() * denominator.signum()
    } else {
        quotient
    }
}

fn multiply_price_by_quantity(price_minor: i128, quantity_scaled: i128) -> i128 {
    divide_round(price_minor * quantity_scaled, ten_pow(QUANTITY_SCALE))
}

fn percentage_scaled(value: i128, total: i128, scale: u32) -> i128 {
    divide_round(value * 100 * ten_pow(scale), total)
}

fn to_i64_safely(value: i128) -> Result<i64, WorkerError> {
    i64::try_from(value).map_err(|_| WorkerError::Job("numeric overflow during rebuild".into()))
}

#[cfg(test)]
mod tests {
    use super::{
        AssetMarketValue, OperationType, PortfolioDefinition, PortfolioOperationEvent,
        PortfolioState, format_scaled,
    };
    use std::collections::HashMap;
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn operation(operation_type: OperationType) -> PortfolioOperationEvent {
        PortfolioOperationEvent {
            id_portfolio_operation: Uuid::new_v4(),
            id_asset: None,
            id_related_asset: None,
            operation_type,
            quantity: None,
            related_quantity: None,
            cash_amount_minor: 0,
            currency: "EUR".into(),
            fx_rate_to_portfolio: None,
        }
    }

    fn portfolio_state() -> PortfolioState {
        PortfolioState::new(PortfolioDefinition {
            id_portfolio: Uuid::new_v4(),
            base_currency: "EUR".into(),
        })
    }

    #[test]
    fn format_scaled_keeps_fixed_precision() {
        assert_eq!(format_scaled(105000000000, 10), "10.5000000000");
    }

    #[test]
    fn deposit_increases_cash() {
        let mut state = portfolio_state();
        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 1_000;
        state.apply(&deposit).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();
        assert_eq!(rebuilt.summary.cash_balance_minor, 1_000);
        assert_eq!(rebuilt.summary.total_invested_minor, 1_000);
    }

    #[test]
    fn withdrawal_decreases_cash() {
        let mut state = portfolio_state();
        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 1_000;
        state.apply(&deposit).unwrap();

        let mut withdrawal = operation(OperationType::Withdrawal);
        withdrawal.cash_amount_minor = 400;
        state.apply(&withdrawal).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();
        assert_eq!(rebuilt.summary.cash_balance_minor, 600);
        assert_eq!(rebuilt.summary.total_invested_minor, 600);
    }

    #[test]
    fn buy_increases_quantity_and_decreases_cash() {
        let mut state = portfolio_state();
        let asset = Uuid::new_v4();

        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 2_000;
        state.apply(&deposit).unwrap();

        let mut buy = operation(OperationType::Buy);
        buy.id_asset = Some(asset);
        buy.quantity = Some("2.0000000000".into());
        buy.cash_amount_minor = 1_000;
        state.apply(&buy).unwrap();

        let mut market_data = HashMap::new();
        market_data.insert(
            asset,
            AssetMarketValue {
                id_asset: asset,
                price_minor: 600,
                currency: "EUR".into(),
            },
        );

        let rebuilt = state
            .finalize(&market_data, OffsetDateTime::now_utc())
            .unwrap();
        assert_eq!(rebuilt.summary.cash_balance_minor, 1_000);
        assert_eq!(rebuilt.holdings[0].quantity, "2.0000000000");
        assert_eq!(rebuilt.holdings[0].market_value_minor, 1_200);
    }

    #[test]
    fn sell_decreases_quantity_and_increases_cash() {
        let mut state = portfolio_state();
        let asset = Uuid::new_v4();

        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 1_000;
        state.apply(&deposit).unwrap();

        let mut buy = operation(OperationType::Buy);
        buy.id_asset = Some(asset);
        buy.quantity = Some("2.0000000000".into());
        buy.cash_amount_minor = 1_000;
        state.apply(&buy).unwrap();

        let mut sell = operation(OperationType::Sell);
        sell.id_asset = Some(asset);
        sell.quantity = Some("1.0000000000".into());
        sell.cash_amount_minor = 700;
        state.apply(&sell).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();
        assert_eq!(rebuilt.summary.cash_balance_minor, 700);
        assert_eq!(rebuilt.holdings[0].quantity, "1.0000000000");
        assert_eq!(rebuilt.holdings[0].invested_base_minor, 500);
    }

    // Regression: the rebuild used to divide `invested_base_minor` (raw minor
    // total) by `quantity_scaled` (shares × 10^QUANTITY_SCALE), so for any
    // realistic position the numerator was ~10^10 times smaller than the
    // denominator and `avg_cost_minor` truncated to 0. With the fix, avg cost
    // must be the minor-unit price per share, and a partial sell must preserve
    // the original per-share cost.
    #[test]
    fn buy_records_per_share_average_cost_minor() {
        let mut state = portfolio_state();
        let asset = Uuid::new_v4();

        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 200_000;
        state.apply(&deposit).unwrap();

        // Buy 10 shares for 195,230 minor units total => avg cost = 19,523/share.
        let mut buy = operation(OperationType::Buy);
        buy.id_asset = Some(asset);
        buy.quantity = Some("10.0000000000".into());
        buy.cash_amount_minor = 195_230;
        state.apply(&buy).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();
        assert_eq!(rebuilt.holdings.len(), 1);
        assert_eq!(rebuilt.holdings[0].quantity, "10.0000000000");
        assert_eq!(rebuilt.holdings[0].invested_base_minor, 195_230);
        assert_eq!(rebuilt.holdings[0].avg_cost_minor, Some(19_523));
    }

    #[test]
    fn partial_sell_preserves_per_share_average_cost_minor() {
        let mut state = portfolio_state();
        let asset = Uuid::new_v4();

        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 200_000;
        state.apply(&deposit).unwrap();

        let mut buy = operation(OperationType::Buy);
        buy.id_asset = Some(asset);
        buy.quantity = Some("10.0000000000".into());
        buy.cash_amount_minor = 195_230;
        state.apply(&buy).unwrap();

        // Sell 4 of the 10 shares. The per-share avg cost must stay at 19,523
        // (pro-rata invested reduction keeps the cost basis per remaining
        // share).
        let mut sell = operation(OperationType::Sell);
        sell.id_asset = Some(asset);
        sell.quantity = Some("4.0000000000".into());
        sell.cash_amount_minor = 100_000;
        state.apply(&sell).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();
        assert_eq!(rebuilt.holdings.len(), 1);
        assert_eq!(rebuilt.holdings[0].quantity, "6.0000000000");
        assert_eq!(rebuilt.holdings[0].avg_cost_minor, Some(19_523));
    }

    #[test]
    fn dividend_increases_cash_and_fee_tax_decrease_cash() {
        let mut state = portfolio_state();
        let mut dividend = operation(OperationType::Dividend);
        dividend.cash_amount_minor = 300;
        state.apply(&dividend).unwrap();

        let mut fee = operation(OperationType::Fee);
        fee.cash_amount_minor = 40;
        state.apply(&fee).unwrap();

        let mut tax = operation(OperationType::Tax);
        tax.cash_amount_minor = 10;
        state.apply(&tax).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();
        assert_eq!(rebuilt.summary.cash_balance_minor, 250);
    }

    #[test]
    fn adjustment_affects_state() {
        let mut state = portfolio_state();
        let asset = Uuid::new_v4();
        let mut adjustment = operation(OperationType::Adjustment);
        adjustment.id_asset = Some(asset);
        adjustment.quantity = Some("1.0000000000".into());
        adjustment.cash_amount_minor = 200;
        state.apply(&adjustment).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();
        assert_eq!(rebuilt.summary.cash_balance_minor, 200);
        assert_eq!(rebuilt.holdings[0].quantity, "1.0000000000");
        assert_eq!(rebuilt.holdings[0].invested_base_minor, 200);
    }

    #[test]
    fn zero_quantity_holdings_are_removed() {
        let mut state = portfolio_state();
        let asset = Uuid::new_v4();
        let mut buy = operation(OperationType::Buy);
        buy.id_asset = Some(asset);
        buy.quantity = Some("1.0000000000".into());
        buy.cash_amount_minor = 100;
        state.apply(&buy).unwrap();

        let mut sell = operation(OperationType::Sell);
        sell.id_asset = Some(asset);
        sell.quantity = Some("1.0000000000".into());
        sell.cash_amount_minor = 100;
        state.apply(&sell).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();
        assert!(rebuilt.holdings.is_empty());
    }

    #[test]
    fn missing_market_data_falls_back_to_invested_cost() {
        let mut state = portfolio_state();
        let asset = Uuid::new_v4();

        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 1_000;
        state.apply(&deposit).unwrap();

        let mut buy = operation(OperationType::Buy);
        buy.id_asset = Some(asset);
        buy.quantity = Some("2.0000000000".into());
        buy.cash_amount_minor = 800;
        state.apply(&buy).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();

        assert_eq!(rebuilt.summary.cash_balance_minor, 200);
        assert_eq!(rebuilt.holdings.len(), 1);
        assert_eq!(rebuilt.holdings[0].market_value_minor, 800);
        assert!(rebuilt.holdings[0].is_estimated);
        assert!(rebuilt.summary.is_estimated);
        assert_eq!(rebuilt.summary.total_value_minor, 1_000);
    }

    #[test]
    fn currency_mismatched_market_data_falls_back_to_invested_cost() {
        let mut state = portfolio_state();
        let asset = Uuid::new_v4();

        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 1_000;
        state.apply(&deposit).unwrap();

        let mut buy = operation(OperationType::Buy);
        buy.id_asset = Some(asset);
        buy.quantity = Some("2.0000000000".into());
        buy.cash_amount_minor = 800;
        state.apply(&buy).unwrap();

        let mut market_data = HashMap::new();
        market_data.insert(
            asset,
            AssetMarketValue {
                id_asset: asset,
                price_minor: 50_000,
                currency: "USD".into(),
            },
        );

        let rebuilt = state
            .finalize(&market_data, OffsetDateTime::now_utc())
            .unwrap();

        assert!(rebuilt.holdings[0].is_estimated);
        assert_eq!(rebuilt.holdings[0].market_value_minor, 800);
        assert!(rebuilt.summary.is_estimated);
        assert_eq!(rebuilt.summary.total_value_minor, 1_000);
    }

    #[test]
    fn replays_user_failing_history_without_negative_total() {
        // Reproduces portfolio 9c8e1282-…: EUR base, two assets (AAPL/MSFT)
        // priced in USD by market-data, plus a foreign-currency buy with no
        // fx_rate. Before the estimated-fallback fix, all holdings were valued
        // at zero and the cash leg of the EUR buy drove total < 0 → panic.
        let mut state = portfolio_state();
        let aapl = Uuid::new_v4();
        let msft = Uuid::new_v4();

        // buy AAPL: 4 shares, €1000 (matches user)
        let mut buy_aapl = operation(OperationType::Buy);
        buy_aapl.id_asset = Some(aapl);
        buy_aapl.quantity = Some("4.0000000000".into());
        buy_aapl.cash_amount_minor = 100_000;
        state.apply(&buy_aapl).unwrap();

        // dividend AAPL: €412
        let mut dividend = operation(OperationType::Dividend);
        dividend.id_asset = Some(aapl);
        dividend.cash_amount_minor = 41_200;
        state.apply(&dividend).unwrap();

        // buy MSFT: 25 shares, $6200, no fx_rate -> cash leg estimated to 0
        let mut buy_msft = operation(OperationType::Buy);
        buy_msft.id_asset = Some(msft);
        buy_msft.quantity = Some("25.0000000000".into());
        buy_msft.cash_amount_minor = 620_000;
        buy_msft.currency = "USD".into();
        state.apply(&buy_msft).unwrap();

        // sell AAPL: 1 share, €112
        let mut sell = operation(OperationType::Sell);
        sell.id_asset = Some(aapl);
        sell.quantity = Some("1.0000000000".into());
        sell.cash_amount_minor = 11_200;
        state.apply(&sell).unwrap();

        // USD-priced market data for both holdings (mismatch vs EUR base).
        let mut market_data = HashMap::new();
        market_data.insert(
            aapl,
            AssetMarketValue {
                id_asset: aapl,
                price_minor: 19_523,
                currency: "USD".into(),
            },
        );
        market_data.insert(
            msft,
            AssetMarketValue {
                id_asset: msft,
                price_minor: 42_150,
                currency: "USD".into(),
            },
        );

        let rebuilt = state
            .finalize(&market_data, OffsetDateTime::now_utc())
            .unwrap();

        // Cash: -100000 + 41200 + 0 (USD buy, no fx) + 11200 = -47600.
        assert_eq!(rebuilt.summary.cash_balance_minor, -47_600);
        // AAPL: 3 shares remaining, invested = 75000 -> estimated market_value.
        // MSFT: 25 shares, invested = 0 (USD buy zero-converted) -> estimated 0.
        assert_eq!(rebuilt.holdings.len(), 2);
        let aapl_holding = rebuilt
            .holdings
            .iter()
            .find(|h| h.id_asset == aapl)
            .unwrap();
        assert_eq!(aapl_holding.market_value_minor, 75_000);
        assert!(aapl_holding.is_estimated);
        // Total = cash (-47600) + estimated holdings (75000 + 0) = 27400 >= 0.
        assert_eq!(rebuilt.summary.total_value_minor, 27_400);
        assert!(rebuilt.summary.is_estimated);
    }

    #[test]
    fn weights_sum_to_one_hundred_across_valued_holdings_and_null_for_zero_holdings() {
        // Holdings-only allocation contract: weight_pct is a holding's share of
        // the sum of holdings' market_value_minor (excluding cash). The
        // frontend's Dashboard allocation uses the same denominator
        // (`sum(market_value_minor)`), and the storage constraint requires
        // 0 ≤ weight_pct ≤ 100. Zero-valued holdings must surface as NULL.
        let mut state = portfolio_state();
        let asset_a = Uuid::new_v4();
        let asset_b = Uuid::new_v4();
        let asset_zero = Uuid::new_v4();

        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 5_000;
        state.apply(&deposit).unwrap();

        let mut buy_a = operation(OperationType::Buy);
        buy_a.id_asset = Some(asset_a);
        buy_a.quantity = Some("3.0000000000".into());
        buy_a.cash_amount_minor = 900;
        state.apply(&buy_a).unwrap();

        let mut buy_b = operation(OperationType::Buy);
        buy_b.id_asset = Some(asset_b);
        buy_b.quantity = Some("1.0000000000".into());
        buy_b.cash_amount_minor = 300;
        state.apply(&buy_b).unwrap();

        // Foreign-currency buy with no fx_rate → invested 0 → estimated 0 →
        // weight_pct must be NULL (zero-valued holding).
        let mut buy_zero = operation(OperationType::Buy);
        buy_zero.id_asset = Some(asset_zero);
        buy_zero.quantity = Some("10.0000000000".into());
        buy_zero.cash_amount_minor = 400;
        buy_zero.currency = "USD".into();
        state.apply(&buy_zero).unwrap();

        let mut market_data = HashMap::new();
        market_data.insert(
            asset_a,
            AssetMarketValue {
                id_asset: asset_a,
                price_minor: 400,
                currency: "EUR".into(),
            },
        );
        market_data.insert(
            asset_b,
            AssetMarketValue {
                id_asset: asset_b,
                price_minor: 700,
                currency: "EUR".into(),
            },
        );

        let rebuilt = state
            .finalize(&market_data, OffsetDateTime::now_utc())
            .unwrap();

        assert_eq!(rebuilt.holdings.len(), 3);
        let weights: Vec<(Uuid, Option<String>)> = rebuilt
            .holdings
            .iter()
            .map(|h| (h.id_asset, h.weight_pct.clone()))
            .collect();

        let zero_weight = weights
            .iter()
            .find(|(id, _)| *id == asset_zero)
            .expect("zero-valued holding present");
        assert!(
            zero_weight.1.is_none(),
            "zero-valued estimated holding must have NULL weight"
        );

        // Sum the non-null weights; expect exactly 100.0000 (in PCT_SCALE).
        let total_scaled: i128 = rebuilt
            .holdings
            .iter()
            .filter_map(|h| {
                h.weight_pct
                    .as_ref()
                    .map(|s| super::parse_scaled(s, 4).unwrap())
            })
            .sum();
        assert_eq!(total_scaled, 100 * 10_000);
    }

    #[test]
    fn negative_cash_with_no_holding_fallback_is_still_rejected() {
        // The fallback must not silently mask a real cash overdraw: when no
        // holdings exist (e.g. cash-only ledger after a withdrawal beyond
        // deposits), the negative-total guard must still trip.
        let mut state = portfolio_state();

        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 100;
        state.apply(&deposit).unwrap();

        let mut withdrawal = operation(OperationType::Withdrawal);
        withdrawal.cash_amount_minor = 500;
        state.apply(&withdrawal).unwrap();

        let err = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .expect_err("negative-total guard must reject");
        assert!(err.to_string().contains("negative total_value_minor"));
    }

    #[test]
    fn multiple_assets_replay_correctly() {
        let mut state = portfolio_state();
        let asset_a = Uuid::new_v4();
        let asset_b = Uuid::new_v4();

        let mut deposit = operation(OperationType::Deposit);
        deposit.cash_amount_minor = 1_000;
        state.apply(&deposit).unwrap();

        let mut buy_a = operation(OperationType::Buy);
        buy_a.id_asset = Some(asset_a);
        buy_a.quantity = Some("1.0000000000".into());
        buy_a.cash_amount_minor = 100;
        state.apply(&buy_a).unwrap();

        let mut buy_b = operation(OperationType::Buy);
        buy_b.id_asset = Some(asset_b);
        buy_b.quantity = Some("2.0000000000".into());
        buy_b.cash_amount_minor = 300;
        state.apply(&buy_b).unwrap();

        let rebuilt = state
            .finalize(&Default::default(), OffsetDateTime::now_utc())
            .unwrap();
        assert_eq!(rebuilt.holdings.len(), 2);
    }
}
