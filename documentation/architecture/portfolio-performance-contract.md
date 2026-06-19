# Portfolio Performance Contract

Status: **Approved contract, not yet implemented.** This document is the
normative reference for the schema, worker, API and frontend PRs that will
deliver historical portfolio valuation and performance. It does not describe
runtime behavior that exists today; current behavior is summarized in section
2 and clearly separated from the target contract.

This contract is paired with:

- [cashflow-classification.md](cashflow-classification.md) — per-operation
  financial categorization;
- [historical-valuation-provenance.md](historical-valuation-provenance.md) —
  per-position and per-day provenance.

## 1. Scope and non-goals

In scope:

- daily historical portfolio valuation in the portfolio base currency;
- daily Modified Dietz return per portfolio;
- chain-linked period performance derived from daily returns at query time;
- realized and unrealized gain / loss decomposition;
- income, fees and taxes aggregation;
- valuation completeness reporting per day;
- persisted historical provenance per day per position;
- correction semantics that recompute history from the earliest affected
  date and publish the new range atomically.

Out of scope (explicitly deferred — see section 26):

- exact intraday time-weighted return (TWR);
- money-weighted return (MWR) / internal rate of return (IRR);
- benchmark comparison;
- volatility, Sharpe ratio, drawdown statistics;
- corporate-action-adjusted provider prices (splits, dividends adjusted at
  source);
- automatic FX provider integration;
- intra-portfolio asset transfers and multi-portfolio accounting.

## 2. Source of truth

The economic timeline of a portfolio is reconstructed from
`portfolio_operations` (`operation_status = 'posted'`). Posted rows are
immutable (`prevent_posted_operation_mutation` trigger) and corrections are
expressed as new operations linked through `id_corrected_operation`.

The historical contract derives every persisted value from this source plus
the price and FX caches written by `kushim-market-data`. It never invents an
external input. When an input is missing it is recorded as such (see
sections 19–22).

Current runtime behavior contributing to the new contract:

- `kushim-worker` already replays `portfolio_operations` deterministically
  via `PortfolioState` (avg cost, prorata SELL reduction);
- `rm_portfolio_holdings` already persists the per-position valuation
  provenance defined in
  [market-data-quality-contract.md](market-data-quality-contract.md);
- `portfolio_snapshots_daily` already stores one row per portfolio per day,
  but **without** flows, completeness counters, daily return or
  provenance-aware aggregates.

The new contract adds a dedicated historical read model (named
`rm_portfolio_history_daily` in later PRs). `portfolio_snapshots_daily` is
not modified.

## 3. Terminology

| Term | Meaning |
| --- | --- |
| `valuation_date` | UTC calendar date the daily row describes. |
| `V_begin` | Closing portfolio value of the previous `valuation_date` (or 0 if this is the first day). |
| `V_end` | Closing portfolio value of `valuation_date` after all operations and revaluation. |
| `CF_i` | Signed external cash flow, evaluated in the portfolio base currency, with the sign convention of section 7. |
| `w_i` | Time weight of `CF_i` within the UTC day, see section 9. |
| `r_d` | Daily Modified Dietz return for `valuation_date`. |
| `R_period` | Chain-linked period return, derived at query time. |
| Posted operation | `portfolio_operations.operation_status = 'posted'`. |
| Effective timestamp | The instant the contract attributes the operation to (currently `executed_at`). |
| Flow | Synonym for external cash flow (`CF_i`). |
| Position | A non-zero quantity held in a single asset for the portfolio. |
| Base currency | `portfolios.base_currency`, immutable in MVP. |

## 4. Monetary and decimal precision

| Concern | Rule |
| --- | --- |
| Persisted monetary fields | `bigint` minor units. `unsigned` semantics are encoded by `CHECK >= 0`; signed semantics (e.g. `net_external_flow_minor`) are stored as a regular `bigint` and explicitly documented per column. |
| Intermediate arithmetic | Wider checked integer (e.g. `i128`) for multiplications and aggregations, the same pattern `PortfolioState` already uses. |
| Ratios and returns | Fixed decimal arithmetic, `numeric(18,8)` in PostgreSQL and `rust_decimal::Decimal` (or equivalent) in Rust. **Never** binary floating-point. |
| Rounding (money) | Banker's rounding (`ROUND_HALF_EVEN`) when an `i128` intermediate is reduced to `i64`. |
| Rounding (ratios) | Truncation to 8 fractional digits at persistence, banker's rounding when displayed. |
| FX conversion | Performed in the integer minor-unit domain after rate application; the rate itself is decimal (see section 9 in [cashflow-classification.md](cashflow-classification.md) and the FX subsection of this document). |
| Quantity | `numeric(30,10)`, unchanged from current schema. |

## 5. Daily valuation boundary

| Rule | Value |
| --- | --- |
| Canonical timezone | UTC. |
| `valuation_date` derivation | UTC calendar date of the cutoff instant. |
| Daily cutoff | `valuation_date` end-of-day = `valuation_date + 1 day - 1 microsecond` in UTC. |
| Operation inclusion | Operations whose effective timestamp `≤` cutoff are included in `V_end` of `valuation_date`. |
| Price selection | Last `asset_price_history_cache.close_minor` for the asset with `price_date ≤ valuation_date`, currency = portfolio base currency (after FX conversion if needed), source priority configurable, ties broken by `fetched_at DESC`. |
| Carry-forward tolerance | A price whose `price_date` is older than the cutoff by more than the approved tolerance (MVP default: 7 calendar days) does not apply; the position falls back to `invested_cost_fallback / missing`. |
| Display timezone | Frontend may render in the user locale; the persisted date does not change. |

The cutoff is the same for every portfolio. Per-portfolio settlement
conventions are not modeled in MVP.

## 6. Daily row generation

| Rule | Value |
| --- | --- |
| First day | The UTC date of the first posted financial operation (deposit, withdrawal, buy, sell, dividend, interest, fee, tax). Splits, spin-offs, symbol changes and adjustments do not trigger first-day creation by themselves. |
| Last day | The portfolio's most recent UTC date up to and including "today UTC". |
| Density | One row per calendar day from first to last day, inclusive. No gaps. |
| Weekends and market holidays | Generated; positions are carried forward at the last in-tolerance price; cash is unchanged unless an operation acts on it. |
| Cash-only portfolios | Generated; `aggregate_valuation_status = 'cash_only'`, which is a **complete** status for performance purposes (see section 18). |
| Portfolio with no posted financial operation | No history row is generated. The portfolio simply has no historical curve until the first posted financial operation arrives. |
| Soft-deleted portfolio | History is preserved as-is. No automatic deletion. |
| Day with operations but unchanged composition | Generated as any other day; flows, fees, taxes etc. of the day are reflected. |

## 7. External flows

`CF_i` is signed in the portfolio base currency:

- contributions (`+`) increase the portfolio's available capital from
  outside the portfolio;
- withdrawals (`−`) reduce it.

The MVP supports as external flows only:

- `deposit` (sign `+`);
- `withdrawal` (sign `−`).

Income (`dividend`, `interest`) is **not** an external flow. Income increases
NAV and increases performance (section 14). Buys and sells are internal
transformations of the portfolio (cash ↔ position), not external flows.
Fees and taxes are NAV-reducing events but **not** external flows.

Ambiguous operations (`transfer_in`, `transfer_out`, `adjustment`) are
treated as defined in section 21 and in the per-type tables in
[cashflow-classification.md](cashflow-classification.md). Encountering an
ambiguous operation on a given day sets that day's
`daily_return_status = 'unavailable'` with
`daily_return_unavailable_reason = 'unsupported_operation_semantics'`. It
does **not** change `aggregate_valuation_status`, which stays driven by
the per-position market-data / FX axes (see section 22 for the two
orthogonal axes).

`net_external_flow_minor` of a day = `Σ CF_i` for that day (signed).
`cumulative_net_contributions_minor` = running sum of `net_external_flow_minor`
since the first day (signed).

## 8. Daily Modified Dietz formula

For a `valuation_date` with `V_begin`, `V_end` and a set `{CF_i}` of signed
external flows at effective timestamps inside the day:

```
r_d = (V_end − V_begin − Σ CF_i) / (V_begin + Σ w_i × CF_i)
```

This is the **daily Modified Dietz return**. It is the canonical MVP
performance metric. It is **not** an exact intraday time-weighted return and
must never be described as such anywhere in the codebase, the API or the
frontend.

The result `r_d` is a decimal ratio (e.g. `0.00350000` = +0.35 %).

### Daily return status

Each daily row carries `daily_return_status` taking exactly one of three
values:

| `daily_return_status` | Meaning | `daily_return` |
| --- | --- | --- |
| `available` | A non-null, financially meaningful Modified Dietz return was computed. | non-null |
| `anchor` | No prior closing value exists (this is the first generated day of the portfolio's history). Anchors are **not** data-quality failures — they are the technical opening of the chain. | NULL |
| `unavailable` | A return should exist on this day but cannot be considered reliable. The specific cause is in `daily_return_unavailable_reason`. | NULL |

### Daily return unavailable reasons

When and only when `daily_return_status = 'unavailable'`, the column
`daily_return_unavailable_reason varchar(40)` is populated with one of:

| Reason | Condition |
| --- | --- |
| `incomplete_valuation` | `aggregate_valuation_status` is one of `partial_market_data`, `partial_currency`, `partial_mixed`. The value is produced from a fallback, the return cannot be trusted. |
| `unsupported_operation_semantics` | At least one operation on the day requires a future contract (ambiguous transfer, adjustment that cannot be collapsed safely). |
| `denominator_non_positive` | `V_begin + Σ w_i × CF_i ≤ 0`. Modified Dietz is undefined. |
| `fx_rate_missing` | A flow requires FX conversion to base currency and no in-tolerance rate exists. |

A row with `daily_return_status = 'anchor'` always has
`daily_return_unavailable_reason = NULL`. An anchor is **not** an
unavailability; it is the start of the chain.

Reason precedence: when more than one cause coexists, the most specific
cause wins in this order: `unsupported_operation_semantics` >
`fx_rate_missing` > `incomplete_valuation` > `denominator_non_positive`.
The aggregate valuation status already carries the per-position fallback
breakdown; the reason is not duplicated when `incomplete_valuation` is
implied by a non-`complete` / non-`cash_only` `aggregate_valuation_status`.

## 9. Flow timing weights

For a UTC day with start `T_start` and end `T_end` (1 microsecond before the
next day's start):

```
total_seconds_in_day        = (T_end − T_start)
remaining_seconds_after_flow = (T_end − effective_timestamp(CF_i))
w_i                         = remaining_seconds_after_flow / total_seconds_in_day
```

Properties:

- `0 ≤ w_i ≤ 1`;
- a flow at exactly `T_start` has `w_i ≈ 1` (invested almost the full day);
- a flow at exactly `T_end` has `w_i ≈ 0` (invested for almost no time);
- the effective timestamp is the operation's `executed_at` in MVP. A future
  `effective_at` column may take precedence; until then `executed_at` is the
  contract.

Edge cases:

- if `T_start = T_end` the day is invalid and `daily_return_status =
  'unavailable'` with `daily_return_unavailable_reason =
  'denominator_non_positive'`;
- if multiple flows happen at the same instant, each contributes
  independently with the same `w`;
- weights apply uniformly to contributions and withdrawals (the sign is on
  `CF_i`, not on `w_i`).

FX conversion: a `CF_i` in a non-base currency is converted to base currency
**before** Modified Dietz is applied. The conversion uses the rate at the
flow's effective timestamp (intraday) when available, otherwise the daily
close rate for `valuation_date`, otherwise (within tolerance) the most
recent earlier close. If none of these exist within the FX tolerance,
`daily_return_status = 'unavailable'` with
`daily_return_unavailable_reason = 'fx_rate_missing'`.

## 10. Chain-linked period return

`cumulative_twr` is **not persisted**. The historical read model persists
only `daily_return` (alongside its `daily_return_status` and
`daily_return_unavailable_reason`). Period performance is computed at
query time from these persisted values.

### Chain-linking rules

For a contiguous range of `valuation_date`:

1. **Anchor rows are excluded** from the return product. An anchor is a
   technical opening, not a missing day. It does **not** make the period
   return invalid.
2. **Available rows are chain-linked**:
   ```
   R_period = Π_{d ∈ period, daily_return_status = 'available'} (1 + r_d) − 1
   ```
3. **Any unavailable row inside the period makes the period return null**:
   ```
   period_return        = NULL
   return_is_meaningful = false
   return_unavailable_reason = <first offending day's daily_return_unavailable_reason>
   ```
4. **A period containing only anchor rows (zero available rows)** has:
   ```
   period_return        = NULL
   return_is_meaningful = false
   return_unavailable_reason = 'insufficient_history'
   ```

In every case, the API also returns `first_available_return_date` and
`last_available_return_date` (the earliest and latest `valuation_date`
inside the period with `daily_return_status = 'available'`), or both NULL
when no available row exists.

### Custom-range opening anchor

For a requested range starting on date `D`, the historical service should
fetch the row at `D − 1` as a **technical opening anchor**:

- `D − 1`'s `total_value_minor` serves as `V_begin` for `D`'s Modified
  Dietz when `D` is `available`;
- `D − 1` is **not** returned in the public points list (the response
  range starts at `D`);
- if no row at `D − 1` exists (the requested range begins at or before
  the portfolio's first historical row), `D` is itself an anchor and
  chaining begins on the next `available` day inside the range.

### Period aliases and behaviors

| Alias / range | Behavior |
| --- | --- |
| `1D` | Single day = the latest persisted day. If that day is `available`, `R_period = r_d`. If `unavailable`, period is null with the day's reason. If it is the very first portfolio day (`anchor`), period is null with `insufficient_history`. |
| `1W, 1M, 3M, 6M, YTD, 1Y` | Bounded range ending on the latest persisted day. The technical `D − 1` anchor lookup applies to the start of the range. Anchors inside the range are excluded from the product; one unavailable day nulls the period. |
| `ALL` | From the portfolio's first historical row (always an anchor) to the latest persisted day. The first day is excluded from the product because it is an anchor — `ALL` is **not** invalidated merely by the presence of the first portfolio day. As long as one `available` day exists, `R_period` is computed by chain-linking the available days. |
| Custom `(date_from, date_to)` | `D − 1` anchor lookup applies. If `date_from` ≤ first historical day, the first day is treated as an anchor and chaining starts at the next `available` day. |

### Cases

| Case | Outcome |
| --- | --- |
| First-ever portfolio day | `daily_return_status = 'anchor'`; `daily_return = NULL`; no reason. Does not make any period invalid by itself. |
| Custom range starting after portfolio creation | `D − 1` exists; used as technical opening; `D`'s return is chain-linked normally. |
| Custom range starting at or before first historical day | `D − 1` does not exist; first day acts as anchor; period spans `[next available day .. date_to]`. |
| Range containing exactly one `unavailable` day | `period_return = NULL`, `return_is_meaningful = false`, reason = that day's reason. |
| Range containing only anchors (e.g. `1D` on the first-ever day) | `period_return = NULL`, `return_is_meaningful = false`, reason = `insufficient_history`. |

Rounding: `R_period` is computed at full decimal precision and rounded for
display only. The persisted daily returns are the source of truth.

## 11. Gain versus performance

Two distinct concepts must never be conflated:

| Concept | Formula | Persisted | Use |
| --- | --- | --- | --- |
| Absolute gain (patrimony) | `total_value_minor − cumulative_net_contributions_minor` | Derivable; `total_value_minor`, `cumulative_net_contributions_minor` are persisted. | KPI "Gain absolu" / "Plus-value globale". |
| Period performance | Chain-linked Modified Dietz, see section 10. | Only `daily_return` is persisted. | KPI "Performance période (Modified Dietz)". |

A deposit increases absolute gain's denominator (`cumulative_net_contributions_minor`)
but **does not** by itself change performance for the day. This is why the
front must label the two metrics distinctly and the API must expose both.

## 12. Realized P&L

Realized P&L is generated by `sell` operations and, in MVP, by `sell`
operations only.

For each `sell`:

```
realized_gain_minor =
    proceeds_in_base
  − cost_basis_reduction_in_base
  − sell.fees_in_base
  − sell.taxes_in_base
```

Where:

- `proceeds_in_base = sell.gross_amount_minor` converted to base currency
  using the operation's `fx_rate_to_portfolio` when present, or the daily FX
  rate of `executed_at` otherwise;
- `cost_basis_reduction_in_base` is the proportional reduction of the
  position's `invested_base_minor` for the sold quantity (the exact path
  already implemented in `PortfolioState::reduce_position`);
- `sell.fees_in_base` and `sell.taxes_in_base` are the operation's
  `fees_minor` and `taxes_minor` converted to base currency.

Per day:

```
realized_gain_minor (daily column) = Σ realized_gain_minor of sells of the day (signed)
cumulative_realized_gain_minor     = running sum of the above (signed)
```

Note: `realized_gain_minor` is **signed**. A loss is a negative value.
Splits, spin-offs and symbol changes never produce realized P&L. Adjustments
require the reconstruction rules in section 21.

## 13. Unrealized P&L

For a day:

```
unrealized_gain_minor (per position) =
    market_value_position
  − cost_basis_position

unrealized_gain_minor (portfolio day, signed) =
    Σ unrealized_gain_minor of every open position
```

Where:

- `market_value_position` is `quantity × price` in base currency for the day
  (FX conversion applied if needed);
- `cost_basis_position` is the position's `invested_base_minor` after all
  posted operations up to and including the cutoff;
- if a position falls back to `invested_cost_fallback`, then by
  construction `market_value_position = cost_basis_position` and the
  contribution is zero. This is intentional: the daily flag is captured by
  the aggregate status, not by faking gains.

Unrealized is recomputed each day from scratch on top of the reconstructed
positions. It is not cumulative.

## 14. Income

`income_minor` (daily, signed) = `Σ gross_amount_minor of dividend operations on the day`
+ `Σ gross_amount_minor of interest operations on the day`,
each converted to base currency.

Income is **not** an external flow. It enters NAV through the `cash_amount_minor`
of the operation (already wired through `PortfolioState`). Modified Dietz
therefore treats income as part of `V_end − V_begin` and **does not** subtract
it via `Σ CF_i`. This is the deliberate behavior that makes income show up
as performance.

Dividend withholding tax (where applicable) is **not** silently inferred. It
must be represented either by an explicit `tax` operation or by an extension
to the operation contract that is not part of MVP. Until that extension
exists, the `dividend.gross_amount_minor` is taken as the cash actually
received.

`cumulative_income_minor` is derivable at query time; it is **not**
persisted in MVP (it can be derived by summing `income_minor` over the
range).

## 15. Fees

| Source | Treatment |
| --- | --- |
| Standalone `fee` operations | Reduce NAV via `cash_amount_minor`. Counted in the daily `fees_minor` aggregate. Reduce daily return through `V_end − V_begin`. |
| `buy.fees_minor` | Added to the position's acquisition cost basis (see section 17). Counted in the daily `fees_minor` aggregate for reporting transparency. |
| `sell.fees_minor` | Subtracted from sale proceeds when computing `realized_gain_minor` (section 12). Counted in the daily `fees_minor` aggregate. |

`fees_minor` (daily, unsigned by sign convention but signed as `bigint`) is
the sum of every fee touching the portfolio during the day, regardless of
its accounting treatment. It is a reporting aggregate, not a re-application
to NAV (which is already done by the underlying operation's cash effect).

## 16. Taxes

Symmetric to fees:

| Source | Treatment |
| --- | --- |
| Standalone `tax` operations | Reduce NAV via `cash_amount_minor`. Counted in daily `taxes_minor`. Reduce daily return through `V_end − V_begin`. |
| `buy.taxes_minor` | Added to the position's acquisition cost basis. Counted in daily `taxes_minor`. |
| `sell.taxes_minor` | Subtracted from sale proceeds when computing realized gain. Counted in daily `taxes_minor`. |

Generic transaction tax and dividend withholding tax are kept distinct.
Withholding tax is **not** inferred from a `dividend` operation. If a
`dividend` is net of withholding, the gross/net distinction must be carried
by an explicit operation pair or a future operation extension. The MVP
treats `dividend.gross_amount_minor` as the cash received.

## 17. Cost basis

Per-position cost basis is the existing avg-cost mechanism in
`PortfolioState`:

```
on buy:
  position.quantity        += buy.quantity
  position.invested_base   += (buy.gross_amount_minor + buy.fees_minor + buy.taxes_minor) in base ccy

on sell:
  reduction = invested_base × (sell.quantity / position.quantity)
  position.quantity      −= sell.quantity
  position.invested_base −= reduction
  realized_gain         = proceeds_in_base − reduction − sell.fees_in_base − sell.taxes_in_base
```

Average cost per share is `invested_base / quantity` and is exposed as
`avg_cost_minor` (already present on `rm_portfolio_holdings`).

Splits, spin-offs and symbol changes redistribute quantity / identity
without modifying `invested_base`. The implementation already handles
splits via `PortfolioState`. Spin-off and symbol-change require explicit
rules before broader rollout (see `documentation/mvp/deferred-todos.md`,
worker section).

## 18. Cash-only portfolios

A day where the portfolio holds no open position is `cash_only`. For
performance purposes:

- `total_value_minor = cash_value_minor`;
- `positions_value_minor = 0`;
- `positions_total = 0`;
- `aggregate_valuation_status = 'cash_only'`;
- `daily_return` follows Modified Dietz exactly: a pure-cash day with no
  flow and no interest yields `r_d = 0`; a pure-cash day with a deposit
  yields `r_d = 0` because `V_end − V_begin − Σ CF_i = 0`;
- `return_is_meaningful = true` (cash-only is a complete state, not a
  missing one).

A `cash_only` row is generated for every UTC day inside the portfolio's
historical span, exactly like a position-bearing day. There are no
"absent" days inside the span.

## 19. Missing prices

For a day where at least one open position has no in-tolerance price in
`asset_price_history_cache` (and no FX issue):

- the position is valued at its `invested_base_minor` (`invested_cost_fallback`);
- the position's `valuation_source = 'invested_cost_fallback'`,
  `market_data_status = 'missing'`;
- portfolio counters:
  `positions_fallback_missing > 0`,
  `positions_market_data_available = positions_total − Σ fallbacks`;
- `aggregate_valuation_status = 'partial_market_data'` (or `partial_mixed`
  if combined with an `unsupported_currency` fallback);
- `daily_return_status = 'unavailable'`,
  `daily_return_unavailable_reason = 'incomplete_valuation'`;
- `total_value_minor` is still produced (valuation is available, return is
  not).

Valuation availability and return credibility are kept strictly separate.

## 20. Unsupported currencies

For a day where at least one open position has a price in
`asset_price_history_cache` whose currency cannot be converted to the
portfolio base currency (no in-tolerance FX rate):

- the position is valued at its `invested_base_minor`
  (`invested_cost_fallback`);
- the position's `valuation_source = 'invested_cost_fallback'`,
  `market_data_status = 'unsupported_currency'`;
- the raw provider price and currency are still persisted on the historical
  position row for transparency (the four numeric `market_data_*` fields);
- portfolio counters:
  `positions_fallback_unsupported_currency > 0`;
- `aggregate_valuation_status = 'partial_currency'` (or `partial_mixed`);
- `daily_return_status = 'unavailable'`,
  `daily_return_unavailable_reason = 'fx_rate_missing'` (the canonical
  reason when the failure is a missing rate, whether it affected a
  position revaluation or a flow conversion; `incomplete_valuation` is
  used only when a price was found but the position still fell back for
  another reason);
- `total_value_minor` is still produced.

This is symmetric to the current behavior of `rm_portfolio_holdings`, which
already exposes `unsupported_market_data_currency` as a documented
`unavailable_reason`.

## 21. Ambiguous operations

`transfer_in`, `transfer_out` and certain `adjustment` shapes do not have
unambiguous financial semantics yet (see
[cashflow-classification.md](cashflow-classification.md) for the per-type
table). On a day that contains at least one such operation:

- valuation is computed as best as possible (positions and cash are still
  reconstructed by the worker);
- `aggregate_valuation_status` is **unchanged** by the ambiguous
  operation — it remains driven by the per-position market-data / FX
  axes. A day with full market-data coverage and an ambiguous transfer
  has `aggregate_valuation_status = 'complete'`;
- `daily_return_status = 'unavailable'`,
  `daily_return_unavailable_reason = 'unsupported_operation_semantics'`;
- the responsible operation ID is preserved in the daily row's
  `unsupported_operation_id uuid` field (target schema) for traceability.

The two contracts are intentionally separate:

- **valuation quality** (`aggregate_valuation_status`) describes whether
  the displayed `total_value_minor` is built from authoritative
  per-position inputs;
- **return calculability** (`daily_return_status`) describes whether the
  daily return can be presented as a financially meaningful number.

An ambiguous operation does not corrupt the value; it corrupts only the
return.

Period return spanning a day with `daily_return_status = 'unavailable'`
is null (section 10). This is the deliberate "fail-loud" behavior until
explicit transfer subtypes and counterparty semantics exist.

Adjustments require the additional reconstruction rule in section 23.

## 22. Partial-data behavior

The contract has **two strictly separate** status axes per daily row.

### Axis 1 — `aggregate_valuation_status` (5 values)

Describes the per-position composition of the portfolio's valuation. It is
fully determined by the position-level provenance breakdown.

| `aggregate_valuation_status` | Condition | `total_value_minor` |
| --- | --- | --- |
| `complete` | Every open position is `(market_data, available)`. | always produced |
| `partial_market_data` | At least one position fell back because the market-data row was missing (and zero positions fell back for unsupported currency). | always produced |
| `partial_currency` | At least one position fell back because the currency was unsupported (and zero positions fell back for missing market data). | always produced |
| `partial_mixed` | Both `missing` and `unsupported_currency` fallbacks coexist on the same day. | always produced |
| `cash_only` | No open positions; cash valuation is complete. | always produced (= `cash_value_minor`) |

No other value is permitted. Operation semantics are **not** part of this
axis.

### Axis 2 — `daily_return_status` (3 values)

Describes whether the daily return is a financially meaningful number.

| `daily_return_status` | Meaning | `daily_return` |
| --- | --- | --- |
| `available` | Computed normally. | non-null |
| `anchor` | Technical opening of the chain (no prior closing value). | NULL |
| `unavailable` | A return cannot be considered reliable for this day. The reason is in `daily_return_unavailable_reason`. | NULL |

### Combinations

| `aggregate_valuation_status` | `daily_return_status` | Typical cause |
| --- | --- | --- |
| `complete` | `available` | Normal day. |
| `complete` | `anchor` | First-ever portfolio day with full market-data coverage. |
| `complete` | `unavailable` (`unsupported_operation_semantics`) | Ambiguous transfer or adjustment on a fully covered day. |
| `complete` | `unavailable` (`denominator_non_positive`) | Modified Dietz denominator collapses (e.g. full withdrawal at start of day). |
| `partial_market_data` | `unavailable` (`incomplete_valuation`) | One position priced from cost. |
| `partial_currency` | `unavailable` (`fx_rate_missing`) | One position rejected for unsupported currency. |
| `partial_mixed` | `unavailable` (`fx_rate_missing` if any FX failure, else `incomplete_valuation`) | Both fallbacks coexist. |
| `cash_only` | `available` | Pure-cash day with prior value. |
| `cash_only` | `anchor` | First-ever portfolio day = pure-cash deposit. |

Period return is null as soon as **any** day in the period has
`daily_return_status = 'unavailable'`. Anchor rows are excluded from the
product and do **not** invalidate the period (section 10).

The API returns all of:

- `total_value_minor`, `cash_value_minor`, `positions_value_minor`;
- `aggregate_valuation_status`;
- `daily_return` (nullable);
- `daily_return_status`;
- `daily_return_unavailable_reason` (nullable; populated only when
  `daily_return_status = 'unavailable'`);
- the per-axis counters listed in
  [historical-valuation-provenance.md](historical-valuation-provenance.md).

The frontend renders the value, the valuation status, and grays out the
return with the reason in a tooltip when not `available`. No partial
subset return is presented as a complete portfolio return.

## 23. Rebuild and invalidation

| Event | Earliest invalidation date | Range to recompute |
| --- | --- | --- |
| Insert posted operation `op` | `DATE(op.executed_at)` | `[that date, today]` |
| Post a previously pending operation | `DATE(op.executed_at)` | `[that date, today]` |
| Cancel a pending operation | none | none |
| Insert adjustment `adj` on posted `orig` | `MIN(DATE(orig.executed_at), DATE(adj.executed_at))` | `[that date, today]` |
| Insert historical price `p` | `p.price_date` | for each portfolio holding the asset: `[p.price_date, today]` |
| Correct a historical price | `p.price_date` | same |
| Insert / correct historical FX rate | `rate.rate_date` | for each portfolio with a position in the affected currency: `[rate.rate_date, today]` |
| Asset identity / currency correction | date of the earliest impacted op | `[that date, today]` |
| Portfolio soft-delete | none | none |
| Portfolio hard-delete | n/a | the read model rows are cascade-deleted by FK |

Idempotence is required: re-running the same range with the same inputs
must produce identical persisted rows.

Adjustment-specific rule: when reconstructing a day that contains both an
original posted operation and one or more adjustment operations linked via
`id_corrected_operation`, the reconstruction must:

1. resolve the chain of adjustments back to the original;
2. derive the corrected economic timeline as if the original had carried
   the corrected payload from the start (no double counting);
3. mark the adjustment relation in the reconstruction trace so that an
   operator can audit which input drove the row.

If the current implementation cannot guarantee non-double-counting for a
specific adjustment shape (e.g. an adjustment that introduces a new
quantity but does not specify the corrected price), the day is recorded
with `daily_return_status = 'unavailable'` and
`daily_return_unavailable_reason = 'unsupported_operation_semantics'`.
`aggregate_valuation_status` continues to reflect only the per-position
market-data / FX axes — the adjustment ambiguity is captured in the
return axis only.

## 24. Transactional publication

`rebuild_version` alone is not a sufficient consistency guarantee in MVP.
The contract requires:

- the worker computes the entire affected range in memory (or in a
  temporary structure);
- the worker writes the entire affected range to
  `rm_portfolio_history_daily` and (when introduced)
  `rm_portfolio_history_holding_daily` inside **one** PostgreSQL
  transaction;
- readers therefore observe either the previous complete range or the new
  complete range — never a mixed old/new range;
- `rebuild_version` is incremented as part of the same transaction for
  observability and cache invalidation downstream (frontend stores, API
  caches).

Generation-based publication (write-into-new-generation-table, swap
pointer) is documented as a future scalability option for very long
ranges. It is **not** the MVP mechanism.

If a range is too large for a single transaction in the future (multi-year
backfill of many portfolios), the contract may be revised to allow
day-by-day publication with explicit consumer support for the transitional
state. The MVP single-transaction guarantee is the baseline.

### Provenance reconstruction failure

If, while rebuilding a range, the worker cannot produce explicit
per-position provenance for at least one position-day inside the range
(no `(market_data, available)`, no `(invested_cost_fallback, missing)`,
no `(invested_cost_fallback, unsupported_currency)` can be assigned
deterministically), the worker:

- **fails** the affected rebuild;
- does **not** publish the new range;
- leaves the previous complete range visible to readers;
- reports the failure operationally (logs + the refresh request's
  `last_error`).

Historical `(NULL, NULL)` provenance is **not** a valid persisted state in
the new historical model. Partially specified provenance is never
published. See
[historical-valuation-provenance.md](historical-valuation-provenance.md)
for the per-position contract.

## 25. API-facing semantics

The historical contract is exposed (in a later PR) through two endpoints:

```
GET /v1/portfolios/{id}/history
GET /v1/portfolios/{id}/performance
```

The DTO is provider-agnostic, money-as-minor-units, ratios-as-decimal-strings,
and reuses the existing `data_available` / `reason` envelope. Performance
endpoint computes period returns by chain-linking persisted daily returns
(section 10); it does not read a persisted `cumulative_twr`.

### Daily history point — required fields

Each point in `GET /v1/portfolios/{id}/history.points[]` carries at
minimum the following semantic fields (exact field names are finalized in
the API PR):

| Field | Type | Notes |
| --- | --- | --- |
| `valuation_date` | ISO date | The UTC date this point describes. |
| `total_value_minor` | i64 | Always present (valuation is always produced). |
| `cash_value_minor` | i64 | Always present. |
| `positions_value_minor` | i64 | Always present (= `total − cash`). |
| `aggregate_valuation_status` | string | One of the five values of section 22. |
| `daily_return` | decimal string \| null | Null when `daily_return_status ≠ 'available'`. |
| `daily_return_status` | string | One of `available`, `anchor`, `unavailable`. |
| `daily_return_unavailable_reason` | string \| null | Populated **only** when `daily_return_status = 'unavailable'`; otherwise null (including for `anchor`). |

An anchor row is **not** returned as an error. It is a normal point with
`daily_return = null`, `daily_return_status = 'anchor'`, and a non-null
`total_value_minor`.

### Period performance — required fields

`GET /v1/portfolios/{id}/performance.metrics[<period>]` carries at
minimum:

| Field | Type | Notes |
| --- | --- | --- |
| `period_start` | ISO date | Inclusive. |
| `period_end` | ISO date | Inclusive. |
| `period_return` | decimal string \| null | Null when any included day is `unavailable`, or when the period contains zero `available` days. |
| `return_is_meaningful` | bool | False whenever `period_return` is null. |
| `return_unavailable_reason` | string \| null | When null: the period is meaningful. When non-null: one of the daily reason vocabulary, or `insufficient_history` when zero `available` days exist in the period. |
| `first_available_return_date` | ISO date \| null | Earliest `valuation_date` inside the period with `daily_return_status = 'available'`, or null. |
| `last_available_return_date` | ISO date \| null | Latest `valuation_date` inside the period with `daily_return_status = 'available'`, or null. |

The API does **not** ship pre-formatted human messages. The frontend
renders labels in its own i18n.

The exact DTO shape (envelope, casing, error codes) is finalized in the
later API PR. This contract constrains only the financial semantics and
the field surface.

## 26. Explicitly deferred metrics

The following metrics are **out of scope** for the MVP historical
performance milestone:

- exact intraday time-weighted return (TWR);
- money-weighted return / IRR (Newton-Raphson root finding on signed
  cash-flow series);
- benchmark comparison (no index history is persisted);
- volatility (daily / annualized);
- Sharpe ratio;
- maximum drawdown;
- corporate-action-adjusted provider prices.

These are tracked in [../mvp/deferred-todos.md](../mvp/deferred-todos.md)
and will be re-evaluated after the MVP historical curve is in production.

## Appendix A — Golden scenarios

These scenarios are the deterministic reference. Every later PR (schema,
worker, API, frontend) must reproduce them exactly. Every intermediate
value is shown. Money is in `EUR` minor units (cents) unless stated
otherwise. Operation timestamps are UTC.

Convention for compact tables:

- `V_b`, `V_e`: opening / closing portfolio value (in EUR, minor units);
- `Σ CF`: signed sum of external flows of the day (in EUR, minor units);
- `Σ w·CF`: signed sum of weighted external flows;
- `r_d`: daily Modified Dietz return (decimal, 8 fractional digits);
- `R_period`: chain-linked period return so far;
- `src`: `valuation_source`;
- `mds`: `market_data_status`;
- `agg`: `aggregate_valuation_status` (one of `complete`,
  `partial_market_data`, `partial_currency`, `partial_mixed`,
  `cash_only`);
- `drs`: `daily_return_status` (one of `available`, `anchor`,
  `unavailable`);
- `reason`: `daily_return_unavailable_reason` (populated only when
  `drs = unavailable`).

Sign convention: contributions positive, withdrawals negative.

### Scenario 1 — Deposit only

Setup: portfolio created, single `deposit` on day 0, no positions.

| date | op | UTC ts | flow | w | V_b | V_e | cash | r_d | R_period | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | deposit 1 000 000 | 12:00:00Z | +1 000 000 | 0.5 | — | 1 000 000 | 1 000 000 | NULL | — | `cash_only` | `anchor` | null |
| J1 | — | — | 0 | — | 1 000 000 | 1 000 000 | 1 000 000 | 0.00000000 | 0.00000000 | `cash_only` | `available` | null |

J0: first generated day. No prior closing value exists ⇒
`daily_return_status = 'anchor'`, `daily_return = NULL`, no reason. The
cash balance is 1 000 000. The anchor row is **not** an unavailability;
it is the technical opening of the chain.

J1: `V_b = 1 000 000`, `V_e = 1 000 000`, `Σ CF = 0`.
`r_d = (1 000 000 − 1 000 000 − 0) / (1 000 000 + 0) = 0`.
`daily_return_status = 'available'`.

`R_period(J1..J1) = 0`. `R_period(J0..J1)` excludes J0 (anchor) and
chain-links only J1 ⇒ also 0. `ALL` is valid.

### Scenario 2 — Buy at unchanged price

Setup: continuation of S1 cash. J1 buys 100 shares of AAA at 80.00 EUR;
EOD price = 80.00. Positions: 100 AAA at cost basis 800 000.

| date | op | UTC ts | flow | V_b | V_e | cash | qty | cost basis | price | pos value | r_d | src | mds | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J1 | buy 100 @ 800 (minor units / share) | 12:00:00Z | 0 | 1 000 000 | 1 000 000 | 200 000 | 100 | 800 000 | 800 | 800 000 | 0.00000000 | `market_data` | `available` | `complete` | `available` | null |

Computation: cash 1 000 000 − 100 × 800 = 200 000. Cost basis 800 000.
EOD price 800 ⇒ pos value 800 000. `V_e = 200 000 + 800 000 = 1 000 000`.
`r_d = (1 000 000 − 1 000 000 − 0) / (1 000 000 + 0) = 0`.

### Scenario 3 — Price increase

Setup: continuation of S2. J2 has no operation; EOD price = 880.

| date | op | flow | V_b | V_e | cash | qty | cost basis | price | pos value | unrealized | r_d | R_period | src | mds | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J2 | — | 0 | 1 000 000 | 1 080 000 | 200 000 | 100 | 800 000 | 880 | 880 000 | +80 000 | 0.08000000 | 0.08000000 (J1..J2) | `market_data` | `available` | `complete` | `available` | null |

Computation: `pos value = 100 × 880 = 880 000`. `V_e = 200 000 + 880 000
= 1 080 000`. `r_d = (1 080 000 − 1 000 000 − 0) / (1 000 000 + 0) =
0.08`. `R_period(J1..J2) = (1+0)(1+0.08) − 1 = 0.08`.

### Scenario 4 — Contribution after gain

Setup: continuation of S3. J3 receives `deposit 500 000` at 12:00Z. EOD
price unchanged = 880.

| date | op | UTC ts | flow | w | V_b | V_e | cash | pos value | r_d | R_period | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J3 | deposit 500 000 | 12:00:00Z | +500 000 | 0.5 | 1 080 000 | 1 580 000 | 700 000 | 880 000 | 0.00000000 | 0.08000000 (J1..J3) | `complete` | `available` | null |

Computation: `Σ CF = +500 000`, `w = 0.5`, `Σ w·CF = +250 000`.
`r_d = (1 580 000 − 1 080 000 − 500 000) / (1 080 000 + 250 000) = 0 / 1 330 000
= 0`. Deposit did **not** add to performance.

### Scenario 5 — Contribution halfway through the day, with a later market move

Setup: at start of J0 the portfolio holds 100 AAA (avg cost 800) and
cash 200 000, so `V_b = 200 000 + 100 × 800 = 280 000`. At 12:00Z the
user deposits 100 000. EOD J0 price moves to 900. (This scenario uses
the daily-cutoff revaluation; Modified Dietz captures the weight of the
intraday flow against the day's net value change.)

After deposit: cash = 300 000. EOD pos value = 100 × 900 = 90 000.
`V_e = 300 000 + 90 000 = 390 000`.

| date | op | UTC ts | flow | w | V_b | V_e | cash | qty | price | pos value | r_d | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | deposit 100 000 | 12:00:00Z | +100 000 | 0.5 | 280 000 | 390 000 | 300 000 | 100 | 900 | 90 000 | 0.03030303 | `complete` | `available` | null |

Computation:

- numerator = `V_e − V_b − Σ CF = 390 000 − 280 000 − 100 000 = 10 000`;
- denominator = `V_b + Σ w·CF = 280 000 + 0.5 × 100 000 = 330 000`;
- `r_d = 10 000 / 330 000 = 0.03030303...` rounded to 8 digits =
  `0.03030303`.

The same revaluation **without** the intraday flow would have given
`r_d = (370 000 − 280 000) / 280 000 = 0.03214286`. The flow weight
correctly dampens the return because the deposited capital was only at
work for half the day.

### Scenario 6 — Withdrawal halfway through the day

Setup: portfolio starts at `V_b = 1 000 000` (cash 200 000, pos 800 000
at price 800, 100 AAA). At 06:00Z (w = 0.75) user withdraws 50 000. EOD
price = 800 (no market move).

| date | op | UTC ts | flow | w | V_b | V_e | cash | qty | price | pos value | r_d | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | withdrawal 50 000 | 06:00:00Z | −50 000 | 0.75 | 1 000 000 | 950 000 | 150 000 | 100 | 800 | 800 000 | 0.00000000 | `complete` | `available` | null |

Computation: `Σ CF = −50 000`, `Σ w·CF = −37 500`.
`r_d = (950 000 − 1 000 000 − (−50 000)) / (1 000 000 + (−37 500))
     = 0 / 962 500
     = 0`. A pure cash movement with no market move yields zero return.

### Scenario 7 — Partial sale

Setup (restated standalone in EUR for readability; persisted values use
minor units = EUR × 100):

- J0: deposit 10 000 EUR
- J1: buy 100 AAA @ 80 EUR
- J2: EOD price 88 ⇒ `V_e = 10 800`
- J3 12:00Z: `sell 50 @ 88`, no fees, no taxes. EOD price still 88.

| date | op | UTC ts | flow | V_b | V_e | cash | qty | cost basis | price | pos value | realized (day) | cum realized | unrealized | r_d | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J3 | sell 50 @ 88 | 12:00:00Z | 0 | 10 800 | 10 800 | 6 400 | 50 | 4 000 | 88 | 4 400 | +400 | +400 | +400 | 0.00000000 | `complete` | `available` | null |

Computation:

- proceeds = 50 × 88 = 4 400 EUR;
- cost-basis reduction (prorata) = 8 000 × (50 / 100) = 4 000 EUR;
- realized gain = 4 400 − 4 000 − 0 (fees) − 0 (taxes) = **+400** EUR;
- remaining cost basis = 8 000 − 4 000 = 4 000 EUR (for 50 shares);
- remaining position value at EOD = 50 × 88 = 4 400 EUR;
- unrealized = 4 400 − 4 000 = **+400** EUR;
- cash = 2 000 + 4 400 = 6 400 EUR;
- `V_e = 6 400 + 4 400 = 10 800` EUR;
- `r_d = (10 800 − 10 800 − 0) / (10 800 + 0) = 0`.

Reconciliation: total gain since J1 = +800 EUR. Of which **realized
+400 EUR** and **unrealized +400 EUR**. The sale itself does not change
`V_e` because the proceeds equal the position value sold (no intraday
move).

### Scenario 8 — Buy fee

Setup: cash 10 000 EUR (anchor day in this standalone scenario is the
day before J0; for compactness we focus on J0..J1). J0 12:00Z:
`buy 100 AAA @ 80, fees 25 EUR`. EOD price = 80.

| date | op | UTC ts | flow | V_b | V_e | cash | qty | cost basis | price | pos value | fees | r_d | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | buy 100 @ 80, fees 25 | 12:00:00Z | 0 | 10 000 | 9 975 | 1 975 | 100 | 8 025 | 80 | 8 000 | 25 | NULL | `complete` | `anchor` | null |
| J1 | — | — | 0 | 9 975 | 9 975 | 1 975 | 100 | 8 025 | 80 | 8 000 | 0 | 0.00000000 | `complete` | `available` | null |

Computation J0: `gross 8 000 + fees 25 = total cash out 8 025`. Cash =
10 000 − 8 025 = 1 975. Cost basis = 8 025 (fees included). EOD pos
value = 100 × 80 = 8 000. `V_e = 1 975 + 8 000 = 9 975`. Unrealized =
8 000 − 8 025 = **−25** (the buy fee is already a latent loss). J0 is
the portfolio's first day in this standalone scenario ⇒
`daily_return_status = 'anchor'`.

J1: no movement, EOD price unchanged. `r_d = 0`, `daily_return_status =
'available'`.

The buy fee is captured in the **cost basis** and in the **`fees_minor`
daily aggregate of J0** (= 25), but does not double-count against NAV
(NAV was already reduced by the buy's cash out).

### Scenario 9 — Sell fee

Setup: at start of J0, holds 100 AAA at cost basis 8 000 (avg 80), cash
2 000, EOD J−1 price = 88, so `V_b = 10 800`. J0 12:00Z: `sell 100 @ 88,
fees 30 EUR`. EOD price unchanged 88.

| date | op | UTC ts | V_b | V_e | cash | qty | cost basis | price | pos value | realized | fees | r_d | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | sell 100 @ 88, fees 30 | 12:00:00Z | 10 800 | 10 770 | 10 770 | 0 | 0 | — | 0 | +770 | 30 | −0.00277778 | `complete` | `available` | null |

Computation:

- proceeds = 100 × 88 = 8 800;
- cost basis reduction = 8 000;
- realized = 8 800 − 8 000 − 30 (fees) = **+770**;
- net cash effect = +8 800 − 30 = +8 770;
- new cash = 2 000 + 8 770 = 10 770;
- pos value = 0;
- `V_e = 10 770`;
- `r_d = (10 770 − 10 800 − 0) / (10 800 + 0) = −30 / 10 800 =
  −0.00277778`.

The sell fee reduces realized gain and reduces daily return.

### Scenario 10 — Transaction tax (on a buy)

Setup: cash 10 000 EUR. J0 12:00Z: `buy 100 AAA @ 80, fees 0, taxes 15`.
EOD price = 80. J0 is the portfolio's first day (anchor).

| date | op | UTC ts | V_b | V_e | cash | qty | cost basis | price | pos value | fees | taxes | r_d | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | buy 100 @ 80, taxes 15 | 12:00:00Z | — | 9 985 | 1 985 | 100 | 8 015 | 80 | 8 000 | 0 | 15 | NULL | `complete` | `anchor` | null |

Cost basis = 8 000 + 15 (tax) = 8 015. Same logic as scenario 8.

### Scenario 11 — Standalone fee

Setup: cash 1 000, no positions, J0 is the portfolio's first day. J0
12:00Z: `fee 5`. EOD nothing else.

| date | op | UTC ts | V_b | V_e | cash | r_d | fees | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | fee 5 | 12:00:00Z | — | 995 | 995 | NULL | 5 | `cash_only` | `anchor` | null |
| J1 | — | — | 995 | 995 | 995 | 0.00000000 | 0 | `cash_only` | `available` | null |

J0 standalone fee directly reduces NAV by 5. J0 is the first day ⇒
`anchor`. On a non-first day the return would have been
`(V_e − V_b − 0) / V_b = (995 − 1 000) / 1 000 = −0.00500000`.

### Scenario 12 — Standalone tax

Symmetric to S11 with `tax 7` instead of `fee 5`.

| date | op | UTC ts | V_b | V_e | cash | r_d | taxes | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | tax 7 | 12:00:00Z | — | 993 | 993 | NULL | 7 | `cash_only` | `anchor` | null |

### Scenario 13 — Dividend

Setup: at start of J0 holds 100 AAA at cost basis 8 000, cash 2 000, EOD
J−1 price 80, so `V_b = 10 000`. J0 12:00Z: `dividend 50 EUR` (gross,
no separate withholding tax). EOD price 80 unchanged.

| date | op | UTC ts | flow | V_b | V_e | cash | qty | price | pos value | income | r_d | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | dividend 50 | 12:00:00Z | 0 (income, not external) | 10 000 | 10 050 | 2 050 | 100 | 80 | 8 000 | 50 | +0.00500000 | `complete` | `available` | null |

Computation: dividend cash +50 reaches NAV but is **not** an external
flow. `r_d = (10 050 − 10 000 − 0) / (10 000 + 0) = 0.00500000`. The
dividend appears as positive performance.

### Scenario 14 — Interest

Symmetric to S13 with `interest 30` instead of `dividend 50`.

| date | op | UTC ts | V_b | V_e | cash | income | r_d | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | interest 30 | 12:00:00Z | 10 000 | 10 030 | 2 030 | 30 | +0.00300000 | `complete` | `available` | null |

### Scenario 15 — Missing price

Setup: at start of J0 holds 100 AAA at cost basis 8 000, cash 2 000.
J0: no price in `asset_price_history_cache` for AAA at any date within
tolerance.

| date | V_b | V_e | cash | qty | cost basis | price | pos value | r_d | src | mds | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | 10 000 | 10 000 | 2 000 | 100 | 8 000 | — | 8 000 (fallback to cost) | NULL | `invested_cost_fallback` | `missing` | `partial_market_data` | `unavailable` | `incomplete_valuation` |

Computation: position valued at `invested_base_minor = 8 000`. `V_e =
2 000 + 8 000 = 10 000`. Valuation is produced; return is unavailable
with reason `incomplete_valuation`. Counters: `positions_total = 1`,
`positions_market_data_available = 0`, `positions_fallback_missing = 1`.

### Scenario 16 — Unsupported currency

Setup: portfolio base EUR. Holds 100 BBB at cost basis 8 000 EUR. Price
in `asset_price_history_cache` for BBB at J0 is `90 USD`. No EUR/USD FX
rate within tolerance.

| date | V_b | V_e | cash | qty | cost basis | provider price | provider ccy | pos value | r_d | src | mds | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | 10 000 | 10 000 | 2 000 | 100 | 8 000 | 90 USD | USD | 8 000 (fallback) | NULL | `invested_cost_fallback` | `unsupported_currency` | `partial_currency` | `unavailable` | `fx_rate_missing` |

The rejected price (90 USD) is **persisted** for transparency
(`market_data_price_minor = 9 000`, `market_data_currency = 'USD'`,
provider, `as_of`, `record_updated_at`). Position is still valued at
cost. Return is unavailable with reason `fx_rate_missing`.

### Scenario 17 — Stale price beyond tolerance

Setup: holds 100 AAA at cost basis 8 000. Last in-cache price for AAA
has `price_date = J0 − 14 days` (tolerance = 7). J0 has no recent price.

| date | V_b | V_e | cash | qty | cost basis | most recent price | days stale | pos value | r_d | src | mds | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | 10 000 | 10 000 | 2 000 | 100 | 8 000 | 80 (J0 − 14d) | 14 | 8 000 (fallback) | NULL | `invested_cost_fallback` | `missing` | `partial_market_data` | `unavailable` | `incomplete_valuation` |

The carry-forward boundary is binary: beyond 7 days, the price is treated
as if it did not exist (`market_data_status = missing`). The fact that
the price existed but was too old is **not** persisted on the historical
row in MVP; operators audit drift by querying `market_data_as_of` versus
`valuation_date` in the `available` band only.

### Scenario 18 — Cash-only multi-day history

Setup: J0 `deposit 1 000`. No other operations. Generate J0..J5.

| date | V_b | V_e | cash | r_d | R_period | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | — | 1 000 | 1 000 | NULL | — | `cash_only` | `anchor` | null |
| J1 | 1 000 | 1 000 | 1 000 | 0.00000000 | 0.00000000 | `cash_only` | `available` | null |
| J2 | 1 000 | 1 000 | 1 000 | 0.00000000 | 0.00000000 | `cash_only` | `available` | null |
| J3 | 1 000 | 1 000 | 1 000 | 0.00000000 | 0.00000000 | `cash_only` | `available` | null |
| J4 | 1 000 | 1 000 | 1 000 | 0.00000000 | 0.00000000 | `cash_only` | `available` | null |
| J5 | 1 000 | 1 000 | 1 000 | 0.00000000 | 0.00000000 | `cash_only` | `available` | null |

A row is generated for every day. `cash_only` is a **complete**
valuation status. Period return `R_period(J1..J5) = 0`.
`R_period(ALL) = R_period(J0..J5) = 0` because the J0 anchor is
excluded from the product but does not invalidate the period.

### Scenario 19 — Ambiguous transfer with complete prices

Setup: portfolio holds 100 AAA at cost basis 8 000, cash 2 000, all
positions fully priced at EOD J−1 (`V_b = 10 000`). J0 12:00Z:
`transfer_in 500` with no subtype (current schema permits this). EOD
J0 price unchanged 80.

| date | op | UTC ts | V_b | V_e | cash | qty | price | pos value | r_d | agg | drs | reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| J0 | transfer_in 500 | 12:00:00Z | 10 000 | 10 500 (best-effort) | 2 500 | 100 | 80 | 8 000 | NULL | `complete` | `unavailable` | `unsupported_operation_semantics` |

Notice the two axes are independent:

- `aggregate_valuation_status = 'complete'` — every position is
  `(market_data, available)`. Valuation is produced normally.
- `daily_return_status = 'unavailable'` with reason
  `unsupported_operation_semantics` — performance cannot be presented.

The triggering operation ID is persisted on the day row
(`unsupported_operation_id`). Until an explicit transfer subtype is
added, every day containing `transfer_in` or `transfer_out` will
surface this combination.

### Scenario 20 — Operation adjustment

Setup:

- J1: `buy 100 AAA @ 80` (posted, original).
- J1..J10: history rebuilt; rows present, `complete`.
- J11: user creates an `adjustment` linked via `id_corrected_operation`
  to the J1 buy, correcting quantity from 100 to 110.

Expected reconstruction (rule of section 23):

1. Worker resolves the chain: adjustment ↔ original J1 buy.
2. Worker derives the corrected timeline as if J1 had recorded
   `buy 110 AAA @ 80` from the start.
3. Worker recomputes the affected range `[J1, today]` in a single
   PostgreSQL transaction.
4. Worker increments `rebuild_version` in the same transaction.

| date | range state before J11 | range state after J11 |
| --- | --- | --- |
| J0 | row v=1 | row v=1 (untouched) |
| J1..J10 | rows v=1 with qty 100 in cost basis | rows v=2 with qty 110 in cost basis, atomic publish |
| J11 | (would be partially computed) | row v=2 reflecting corrected timeline |

Readers see either v=1 over `[J1..J10]` or v=2 over `[J1..J11]`. They
never see a mix.

If the adjustment shape does not provide enough information to apply the
collapse safely (e.g. it adds a new `quantity` without specifying the
corrected price), then the day is recomputed with
`daily_return_status = 'unavailable'` and
`daily_return_unavailable_reason = 'unsupported_operation_semantics'`.
`aggregate_valuation_status` continues to reflect only the per-position
market-data / FX axes. The reconstruction does **not** silently treat
the adjustment as a new independent buy.

### Scenario 21 — Historical provenance combinations (P1/P2 only)

This scenario fixes the **exactly three** valid historical per-position
provenance combinations. Historical `(NULL, NULL)` is **not** a valid
persisted state in the new historical model — see Scenario 23 for the
reconstruction-failure behavior.

| case | `valuation_source` | `market_data_status` | price | ccy | provider | as_of | record_updated_at | accepted by historical CHECK |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P1 (market_data / available) | `market_data` | `available` | 8 000 | EUR | (any non-null source name allowed; provider field may be null when the source did not stamp one) | T | T' | yes |
| P2a (fallback / missing) | `invested_cost_fallback` | `missing` | NULL | NULL | NULL | NULL | NULL | yes |
| P2b (fallback / unsupported_currency) | `invested_cost_fallback` | `unsupported_currency` | 9 000 | USD | (any) | T | T' | yes |
| Historical legacy (NULL / NULL) | NULL | NULL | NULL | NULL | NULL | NULL | NULL | **invalid** — historical worker must never produce this; see Scenario 23 |
| Invalid-1 | `market_data` | `available` | NULL | EUR | (any) | T | T' | **rejected by CHECK** |
| Invalid-2 | `invested_cost_fallback` | `missing` | 8 000 | EUR | (any) | T | T' | **rejected by CHECK** |

The CHECK constraint
`chk_rm_portfolio_history_holding_daily_provenance_combination` enforces
exactly the three P1/P2 combinations. Historical legacy NULL/NULL is
explicitly excluded — it remains documented in
[market-data-quality-contract.md](market-data-quality-contract.md) only
as a compatibility state for pre-migration **current** holding rows; it
is **not** propagated to the new historical model.

### Scenario 22 — Historical correction and transactional rebuild

Setup:

- J1: posted `buy 100 AAA @ 80` (cost basis 8 000).
- J1..J20: history exists, `complete`, prices 80..100 (monotonic), `r_d`
  positive each day. `R_period(J2..J20)` ≈ +25 %.
- J21: a corrected historical price for J10 arrives in
  `asset_price_history_cache`: the previous J10 price of 90 was wrong;
  the correct value is 85.

Expected behavior:

1. The market-data correction triggers an invalidation event for every
   portfolio holding AAA, with `invalidate_from_date = J10`.
2. The worker recomputes the range `[J10, today (J21)]` for the affected
   portfolios.
3. The recomputation publishes the new range inside one PostgreSQL
   transaction. Readers see either:
   - the old `[J10..J20]` (v=k) plus no J21, or
   - the new `[J10..J21]` (v=k+1) atomically.
4. `daily_return` of every recomputed day is rewritten; period returns
   spanning J10 must be re-derived at query time from the new daily
   values.

| range | before correction | after correction |
| --- | --- | --- |
| `[J1..J9]` | v=k | v=k (untouched) |
| `[J10..J20]` | v=k | v=k+1 (rewritten) |
| `J21` | n/a | v=k+1 |

Validation invariants:

- `rebuild_version = k+1` on every row of `[J10..J21]`.
- No row of `[J1..J9]` is rewritten.
- The atomic publication guarantees that no reader observes the new J10
  alongside the old J11, even momentarily.

If the price correction also affects FX (rate correction), the same rule
applies with the rate's `rate_date` as `invalidate_from_date` and the
affected positions being those denominated in the corrected currency.

After the rewrite, every recomputed day has its own
`daily_return_status` and `aggregate_valuation_status` as defined in
section 22. Days that change from `available` to `unavailable` (or vice
versa) will change the visible period returns at query time.

### Scenario 23 — Historical provenance unresolved → rebuild abort

Setup: portfolio holds 100 AAA at cost basis 8 000. A historical rebuild
of range `[J5, today]` is requested. While computing J7 the worker
encounters a position-day for which it cannot deterministically assign
**any** of the three valid provenance combinations (no in-tolerance
price, no in-tolerance FX failure to attribute to
`unsupported_currency`, and the input shape does not allow the worker
to confidently mark `missing` — e.g. an unresolved provider error that
is neither "row absent" nor "row present with wrong currency").

Expected behavior:

| step | effect |
| --- | --- |
| 1 | Worker computes `[J5, J7)` in memory but does **not** open the publication transaction yet (or aborts the in-progress transaction). |
| 2 | Worker fails the rebuild with an explicit error (e.g. `provenance_unresolvable_for_position_day`). |
| 3 | The new range is **not** published. The previous complete range remains visible to readers. |
| 4 | The refresh request transitions to a retryable failure state with the error preserved in `last_error`. |
| 5 | No partially specified provenance (e.g. half-NULL columns) is ever written. No historical row with `(NULL, NULL)` provenance exists. |

There is no "legacy" historical row. The user-visible state continues to
expose whatever the previous complete rebuild produced; the API surface
is unchanged until a successful rebuild publishes a new range.

### Scenario 24 — Range queries with anchor handling

This scenario shows the technical-anchor lookup defined in section 10.

Portfolio has full history `[J0, J20]`, all `available` except J0 which
is `anchor`. `r_d` is `+0.01` on every day J1..J20.

| request | technical opening anchor | rows returned in `points[]` | chain-linked days | `period_return` | `return_unavailable_reason` |
| --- | --- | --- | --- | --- | --- |
| `1D` on J20 | J19 used as `V_begin` | only J20 | `{J20}` | `r_J20 = +0.01` | null |
| `1W` on J14..J20 | J13 fetched as anchor (not returned) | J14..J20 | `{J14..J20}` (7 available rows) | `(1.01)^7 − 1 ≈ 0.07213535` | null |
| `ALL` (J0..J20) | none (J0 is the portfolio's first row) | J0..J20 | `{J1..J20}` (20 available rows, J0 anchor excluded) | `(1.01)^20 − 1 ≈ 0.22019004` | null |
| Custom `J0..J5` | none | J0..J5 | `{J1..J5}` (5 available rows; J0 anchor excluded) | `(1.01)^5 − 1 ≈ 0.05101005` | null |
| Custom `J3..J5` | J2 fetched as anchor (not returned) | J3..J5 | `{J3..J5}` | `(1.01)^3 − 1 ≈ 0.03030100` | null |
| `1D` on J0 (the very first day) | none | only J0 (anchor) | empty | NULL | `insufficient_history` |
| Custom `J7..J9` with J8 made `unavailable` (e.g. an ambiguous transfer that day) | J6 fetched as anchor | J7..J9 | empty (chain-linking aborts on unavailable) | NULL | `unsupported_operation_semantics` |

Key observations:

- the technical anchor at `D − 1` is **never** returned in `points[]`;
- `ALL` is **not** invalidated by the existence of the first-day anchor;
- a range that contains only anchors yields `insufficient_history`;
- one `unavailable` day inside the range nulls `period_return` with the
  day's specific reason;
- `first_available_return_date` and `last_available_return_date` give
  the consumer a tight bracket on the available subset.

---

This appendix is normative. Any implementation discrepancy from these
scenarios is a bug in the implementation, not in the contract.
