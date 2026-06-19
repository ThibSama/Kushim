# Historical Valuation Provenance

Status: **Approved contract, not yet implemented.** Paired with
[portfolio-performance-contract.md](portfolio-performance-contract.md) and
[cashflow-classification.md](cashflow-classification.md). This document
defines the per-position-per-day and per-portfolio-per-day provenance
vocabulary for historical valuation, strictly mirroring the **current**
persisted provenance on `rm_portfolio_holdings`.

## Source of truth for the vocabulary

The current persisted vocabulary is defined and enforced by:

- `infra/postgres/init/001_init.sql` (`rm_portfolio_holdings` columns +
  `chk_rm_portfolio_holdings_provenance_combination`);
- `infra/postgres/upgrades/003_holding_valuation_provenance.sql` (existing
  schema upgrade);
- `kushim-worker/src/domain/portfolio_state.rs` (enums `ValuationSource`,
  `MarketDataStatus`, struct `HoldingValuationProvenance`);
- `kushim-api/src/domain/portfolio_read_model.rs` (struct
  `HoldingMarketDataQuality`).

The reference description is
[market-data-quality-contract.md](market-data-quality-contract.md).

This document does not invent new codes. It promotes the existing codes to
the historical model.

## Goal

Two architectural guarantees:

1. The historical model must **not regress** to an `is_estimated`-only
   provenance. The current `rm_portfolio_holdings` row already exposes the
   full provenance richness; the historical-per-day row must offer the
   same.
2. Valuation availability and return credibility are two orthogonal
   concepts. A day can have a fully available `total_value_minor` and at
   the same time a null `daily_return`. The contract must let the API and
   the frontend report both axes independently.

## Per-position-per-day provenance

Each historical position row carries the same seven fields already present
on `rm_portfolio_holdings`:

| Field | Type | Meaning |
| --- | --- | --- |
| `valuation_source` | varchar(32), NOT NULL | `market_data` \| `invested_cost_fallback`. Identifies which input drove the persisted `market_value_minor`. |
| `market_data_status` | varchar(32), NOT NULL | `available` \| `missing` \| `unsupported_currency`. Identifies the state of the market-data input the worker considered. |
| `market_data_price_minor` | bigint | The exact price the worker consumed (or rejected, for `unsupported_currency`). NULL when no row existed. |
| `market_data_currency` | char(3) | Currency of `market_data_price_minor`. NULL when no row existed. |
| `market_data_provider` | varchar(50) | `asset_price_history_cache.source` captured at rebuild time. Nullable even when a price existed (some sources do not stamp a provider name). |
| `market_data_as_of` | timestamptz | The provider-reported instant of the price (typically end-of-day for historical close prices). |
| `market_data_record_updated_at` | timestamptz | The `fetched_at` of the historical price row at rebuild time. **Not** a fetch instant in the network sense — it is record persistence time, exactly as documented in [market-data-quality-contract.md](market-data-quality-contract.md). |

Note on `market_data_as_of` vs `market_data_record_updated_at` for historical
rows:

- the current `rm_portfolio_holdings` captures these from `asset_market_data`
  (current cache);
- the historical per-day row captures them from
  `asset_price_history_cache` (historical cache). The current cache stores
  `as_of` and `updated_at`; the historical cache stores `as_of` and
  `fetched_at`. They map 1-to-1 in semantic role, and the historical model
  uses the **same column names** for cross-layer consistency.

## Valid combinations

The historical per-position row enforces the same combinational rule
already enforced on the current row:

| `valuation_source` | `market_data_status` | numeric `market_data_*` fields | When |
| --- | --- | --- | --- |
| `market_data` | `available` | populated (price, currency, as_of, record_updated_at; provider may still be NULL when the source did not stamp one) | An in-tolerance price was found and was compatible with the portfolio base currency. |
| `invested_cost_fallback` | `missing` | all NULL | No in-tolerance price was found at all. |
| `invested_cost_fallback` | `unsupported_currency` | populated (the rejected price is preserved for transparency, with its currency and the timestamps) | A price was found but its currency could not be converted to the portfolio base currency within the FX tolerance. |

Exactly **three** combinations are valid in the historical model. The
schema CHECK constraint
`chk_rm_portfolio_history_holding_daily_provenance_combination` enforces
this set and **rejects every other combination**, including the
all-NULL pair.

Historical `(NULL, NULL)` is **not** a valid persisted state. The
`(NULL, NULL)` combination remains documented in
[market-data-quality-contract.md](market-data-quality-contract.md) only
as a compatibility state for **current** `rm_portfolio_holdings` rows
that pre-date migration 003. That compatibility state is **not**
propagated into the historical model, which is created post-migration
and rebuilt deterministically from canonical operations and historical
market data.

If, while rebuilding, the worker cannot deterministically assign one of
the three valid combinations to a position-day, it must **fail the
rebuild** and not publish the affected range. See section "Rebuild
failure when provenance cannot be reconstructed" below.

## Stale prices and the carry-forward tolerance

The current contract uses a carry-forward tolerance of **7 calendar days**
by default (see
[portfolio-performance-contract.md, section 5](portfolio-performance-contract.md#5-daily-valuation-boundary)).

- A price within tolerance is treated as `market_data_status = available`.
- A price outside tolerance is treated as not present at all:
  `valuation_source = invested_cost_fallback`,
  `market_data_status = missing`.

This is intentional: the contract does not introduce a third
`market_data_status` for "stale". The boundary is binary so the aggregate
status remains simple and the API contract stable. The tolerance is the
single tuning knob.

`market_data_as_of` (when populated) tells consumers how old the price
actually is. Operators auditing data quality should query for
`market_data_as_of < valuation_date - threshold` to surface drift.

## Per-portfolio-per-day aggregation

Each historical day row carries counters that summarize the day's
per-position provenance:

| Counter | Meaning |
| --- | --- |
| `positions_total` | Number of open positions on the day (excludes cash). |
| `positions_market_data_available` | Positions with `valuation_source = market_data` and `market_data_status = available`. |
| `positions_fallback_missing` | Positions with `valuation_source = invested_cost_fallback` and `market_data_status = missing`. |
| `positions_fallback_unsupported_currency` | Positions with `valuation_source = invested_cost_fallback` and `market_data_status = unsupported_currency`. |

Invariant (enforced by CHECK):

```
positions_market_data_available
  + positions_fallback_missing
  + positions_fallback_unsupported_currency
  = positions_total
```

There is no `positions_legacy_provenance` counter. Historical
`(NULL, NULL)` provenance is invalid (see CHECK above); a row with
`positions_total > 0` whose three above counters sum to a smaller value
is therefore impossible.

## Aggregate valuation status

Each historical day row carries `aggregate_valuation_status`, which
takes **exactly five values** (no other value is permitted):

| `aggregate_valuation_status` | Condition | `total_value_minor` |
| --- | --- | --- |
| `complete` | `positions_market_data_available = positions_total` | always produced |
| `partial_market_data` | `positions_fallback_missing > 0` AND `positions_fallback_unsupported_currency = 0` | always produced |
| `partial_currency` | `positions_fallback_unsupported_currency > 0` AND `positions_fallback_missing = 0` | always produced |
| `partial_mixed` | `positions_fallback_missing > 0` AND `positions_fallback_unsupported_currency > 0` | always produced |
| `cash_only` | `positions_total = 0` | always produced (= `cash_value_minor`) |

Operation semantics are **not** part of this axis. A day with full
market-data coverage and an ambiguous transfer still has
`aggregate_valuation_status = 'complete'`; the ambiguity is captured by
the **independent** `daily_return_status` axis defined in
[portfolio-performance-contract.md, section 22](portfolio-performance-contract.md#22-partial-data-behavior).

Two key consequences:

- `cash_only` **is** a complete valuation state. Pure-cash days carry
  Modified Dietz returns normally (subject to the anchor rule for the
  first day).
- Any non-`complete` and non-`cash_only` `aggregate_valuation_status`
  forces the day's `daily_return_status` to `unavailable` with a reason
  picked from the performance contract's reason vocabulary
  (`incomplete_valuation`, `fx_rate_missing`).

## Return calculability axis

`daily_return_status ∈ {available, anchor, unavailable}` is defined and
governed entirely by
[portfolio-performance-contract.md, section 22](portfolio-performance-contract.md#22-partial-data-behavior).
It is **separate** from `aggregate_valuation_status`. The two combine
deterministically (see the combination table in section 22).

## Period return implication

Period returns (chain-linked daily Modified Dietz) follow section 10 of
the performance contract:

- **anchor** rows are excluded from the chain-link product and do
  **not** invalidate the period;
- **available** rows are multiplied (Π (1 + r_d) − 1);
- a single **unavailable** row in the requested range nulls the
  period return with the day's reason;
- a period containing zero **available** rows (e.g. a `1D` query on
  the portfolio's first-ever day) yields
  `period_return = NULL`, `return_is_meaningful = false`,
  `return_unavailable_reason = 'insufficient_history'`.

`ALL` is **not** invalidated by the presence of the first-day anchor.

## Provenance vs. cash effects

Cash and operation effects are **always** applied to the day's state,
even when the day is `partial_*`. The fallback to
`invested_cost_fallback` affects only **how a position is valued**, not
whether the operations of the day moved cash, quantity or cost basis.
This is the same behavior as the current `rm_portfolio_holdings`
rebuild path.

Therefore:

- `cash_value_minor`, `total_value_minor`,
  `cumulative_net_contributions_minor` are always populated;
- `realized_gain_minor`, `unrealized_gain_minor`, `income_minor`,
  `fees_minor`, `taxes_minor` are populated to the extent the
  underlying operations are not ambiguous;
- `daily_return` is gated by `daily_return_status` as described in the
  performance contract.

## API surface

The historical endpoints (defined in a later PR) expose:

- the seven persisted provenance fields per position (when the
  per-position read model is introduced);
- the four counters and `aggregate_valuation_status` per day on the
  portfolio-level endpoint;
- `daily_return`, `daily_return_status` and
  `daily_return_unavailable_reason` per day;
- `period_return`, `return_is_meaningful`, `return_unavailable_reason`,
  `first_available_return_date` and `last_available_return_date` at
  the period level.

The existing per-position `unavailable_reason` vocabulary used today on
`rm_portfolio_holdings` (`market_data_missing`,
`unsupported_market_data_currency`, `valuation_provenance_missing`) is
**reused as is** for the **current** rm row only. The historical
per-position rows produce only the first two reasons in derived APIs;
`valuation_provenance_missing` is a current-rm compatibility reason
exclusively and is never produced by the historical contract.

The historical contract introduces only one new code surface: the
day-level `daily_return_unavailable_reason` plus the
`return_unavailable_reason` aggregate at the period level (including
`insufficient_history`).

## Rebuild failure when provenance cannot be reconstructed

If, while rebuilding a range, the worker cannot deterministically assign
one of the three valid provenance combinations to a position-day:

1. the worker **fails** the affected rebuild;
2. the new range is **not** published;
3. the previous complete range remains visible to readers, unchanged;
4. the refresh request transitions to a retryable failure state, with
   the cause preserved in `last_error`;
5. no partially-specified provenance is ever written;
6. no historical `(NULL, NULL)` row exists.

There is no "legacy historical row" concept. The historical model is
created post-migration and is rebuildable from canonical operations and
historical market data. Reconstruction failures are operational
problems (provider error, FX cache gap, edge-case operation shape) and
are surfaced as such.

## What is intentionally not in this document

- The exact target table names and column types — defined by the schema
  PR.
- The exact DTO shapes — defined by the API PR.
- The frontend UX for partial / unsupported days — defined by the
  frontend PR.
- The FX provider that resolves `unsupported_currency` cases — explicitly
  provider-agnostic in this contract (see
  [portfolio-performance-contract.md](portfolio-performance-contract.md)
  and [../mvp/deferred-todos.md](../mvp/deferred-todos.md)).
