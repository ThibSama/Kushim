# Portfolio Reconstruction and Snapshots

## Core reconstruction rule

Kushim reconstructs portfolio state from:

- `portfolio_operations`
- valuation inputs
- historical price cache when needed

This remains the core model for both current and historical state.

## Current implemented truth

### Current state

Current-state generation is implemented in `kushim-worker`:

- read `posted portfolio_operations`
- replay portfolio state
- value holdings with `asset_market_data`
- write `rm_portfolio_summary`
- write `rm_portfolio_holdings`

### Current snapshots

Current daily snapshot generation is also implemented in `kushim-worker`:

- read current read models
- write `portfolio_snapshots_daily`
- write `portfolio_holding_snapshot_daily`

### Historical backfill V1

Historical backfill is implemented conservatively:

- one explicit portfolio only
- one explicit date range
- max 366 days
- historical valuation via `asset_price_history_cache` only
- no external fetch
- no FX conversion

## What `kushim-api` does not do

`kushim-api` does not:

- rebuild current state
- generate read models
- generate snapshots
- reconstruct historical dates
- fetch prices

It only reads existing derived data.

## Snapshot strategy

### Why snapshots exist

Daily snapshots exist to make historical views fast and reproducible.

They support:

- historical portfolio value curves
- historical holdings views
- future performance analytics

### Snapshot uniqueness

Expected uniqueness:

- one daily snapshot per portfolio per date
- one holding row per snapshot per asset

This enables idempotent worker reruns.

## Historical pricing strategy

### Current-state pricing

Used by:

- `rebuild_current_read_models`

Source:

- `asset_market_data`

### Historical pricing

Used by:

- `backfill_daily_snapshots`

Source:

- `asset_price_history_cache`

### Important distinction

Current market data and historical price cache are not interchangeable in the current architecture.

For historical backfill V1:

- no fallback to current `asset_market_data`
- no external fetch
- wrong-currency prices are ignored
- missing prices mark the result estimated

## Replay boundaries

### Current replay V1

Current replay logic supports:

- deposits
- withdrawals
- buys
- sells
- dividends
- interest
- fees
- taxes
- transfers
- adjustments
- conservative handling for split, spin-off, and symbol change

### Known limitations

Still intentionally deferred:

- advanced FX handling
- richer corporate-action interpretation
- multi-portfolio historical orchestration
- optimized incremental historical replay

## Implemented worker jobs

### `rebuild_current_read_models`

Purpose:

- rebuild current derived portfolio state

### `generate_daily_snapshots`

Purpose:

- snapshot current derived state into daily snapshot tables

### `refresh_current_portfolio_state`

Purpose:

- run rebuild, then snapshot generation

### `backfill_daily_snapshots`

Purpose:

- rebuild and persist historical daily snapshots over an explicit range for one explicit portfolio

## Long-term intended strategy

The long-term validated direction remains:

`snapshot + delta operations + deterministic price cache`

Current implementation status:

- current-state pipeline: implemented
- current daily snapshots: implemented
- controlled historical backfill V1: implemented
- generalized historical reconstruction service path: still incomplete
