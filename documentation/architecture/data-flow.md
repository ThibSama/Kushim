# Kushim Data Flow

## Core principle

`portfolio_operations` is the source of truth.

Everything else is derived from it:

- current read models
- daily snapshots
- historical reconstructed views

## Current-state flow

### 1. User writes

The user interacts with `kushim-api`:

- create portfolio
- create `portfolio_operations`
- update pending operations
- cancel pending operations
- post operations
- create compensating corrections

`kushim-api` writes only the source-of-truth tables it owns:

- `portfolios`
- `portfolio_operations`

It does not generate derived state.

### 2. Worker rebuild

`kushim-worker` reads:

- `portfolios`
- `portfolio_operations` where `operation_status = 'posted'`
- `asset_market_data`

It rebuilds:

- `rm_portfolio_summary`
- `rm_portfolio_holdings`

### 3. Worker snapshots

`kushim-worker` then reads:

- `rm_portfolio_summary`
- `rm_portfolio_holdings`

It writes:

- `portfolio_snapshots_daily`
- `portfolio_holding_snapshot_daily`

### 4. API reads

`kushim-api` exposes:

- current summaries and holdings from read models
- daily snapshots and snapshot holdings from snapshot tables

If derived data is missing, the API returns clear read-only availability signals such as:

- `data_available=false`
- `read_model_missing`
- `snapshot_missing`

## Historical flow

### Current implemented V1 backfill

`kushim-worker` historical backfill job:

- requires one explicit target portfolio
- requires explicit `date_from` and `date_to`
- replays `posted` operations up to each date
- values holdings using `asset_price_history_cache` only
- writes historical daily snapshots

### Historical price rule

For backfill V1:

- no external fetch
- no fallback to current market data
- no FX conversion
- missing price means estimated valuation and `market_value_minor = 0`

## High-level sequence

```text
User / Frontend
    -> kushim-api
        -> portfolios / portfolio_operations
            -> kushim-worker rebuild_current_read_models
                -> rm_portfolio_summary / rm_portfolio_holdings
                    -> kushim-worker generate_daily_snapshots
                        -> portfolio_snapshots_daily / portfolio_holding_snapshot_daily
                            -> kushim-api read-only endpoints
```

## Composite current refresh

`refresh_current_portfolio_state` performs:

1. `rebuild_current_read_models`
2. `generate_daily_snapshots`

This gives one worker job for end-to-end current-state refresh.

## Historical backfill V1 sequence

```text
Target portfolio
    + explicit date range
        -> load portfolio
        -> load posted operations up to end of date D
        -> replay state as of D
        -> load historical cached prices for D
        -> value holdings
        -> upsert daily snapshot for D
        -> replace daily holding snapshot rows for D
```

## What is intentionally missing today

- no market-data provider sync in `kushim-market-data`
- no automatic historical price fetch during backfill
- no multi-portfolio backfill orchestration
- no FX conversion layer
- no event queue or distributed lock orchestration

## Why this matters

This separation keeps the project understandable:

- synchronous writes stay simple
- calculations stay rebuildable
- historical state stays reproducible
- market data remains a separate future concern
