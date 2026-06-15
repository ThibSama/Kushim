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
- `portfolio_refresh_requests` (durable refresh queue; enqueue only)

It does not generate derived state (read models / snapshots).

### 1b. Automatic refresh enqueue (P0)

Whenever an operation transitions into `operation_status = 'posted'`, `kushim-api`
enqueues a durable refresh request in the **same PostgreSQL transaction** as the
operation write. This is a request/outbox row in `portfolio_refresh_requests`,
never a calculation. The transitions covered are:

- direct creation of a posted operation;
- posting an existing pending operation;
- direct creation of a posted correction.

Pending creations and cancellations enqueue nothing. At most one `pending`
request exists per portfolio (partial unique index): concurrent posted writes
coalesce onto the same pending request, while an operation posted *during*
processing produces a fresh pending request so nothing is lost.

The write response returns the refresh-request identity:

```json
{ "operation": { ... }, "refresh_request": { "id_portfolio_refresh_request": "...", "status": "pending", "requested_at": "..." } }
```

Pending creations return `"refresh_request": null`.

### 2. Worker rebuild (automatic consumer)

`kushim-worker` runs `process_portfolio_refresh_requests` in loop mode. It:

- claims eligible `pending` (and stale `processing`) rows with
  `FOR UPDATE SKIP LOCKED`, marks them `processing`, records the worker name and
  start time, and increments `attempts` — all in a short claim transaction that
  is released before the heavy rebuild;
- runs the existing `refresh_current_portfolio_state` for the request's target
  portfolio only;
- marks the request `completed`, or schedules a bounded retry / terminal
  `failed` after the configured maximum attempts.

It reads:

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

## High-level sequence (automatic refresh)

```text
User / Frontend
    -> kushim-api  (post operation + enqueue portfolio_refresh_requests, one transaction)
        -> kushim-worker process_portfolio_refresh_requests (loop, claims with SKIP LOCKED)
            -> refresh_current_portfolio_state for the target portfolio
                -> rebuild_current_read_models -> rm_portfolio_summary / rm_portfolio_holdings
                -> generate_daily_snapshots   -> portfolio_snapshots_daily / portfolio_holding_snapshot_daily
                    -> mark refresh request completed
                        -> kushim-app polls GET /v1/portfolios/{id}/refresh-requests/{id}
                            -> on completed, reloads summary / holdings / snapshots / operations
```

The frontend never triggers calculations; it only polls the refresh-request
status and reloads read-only data when the worker reports `completed`.

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
- no external event queue (Redis/Kafka/…) or distributed lock orchestration —
  the automatic refresh uses PostgreSQL as a durable queue with row locks only;
  a production scheduler and queue infrastructure remain deferred

## Why this matters

This separation keeps the project understandable:

- synchronous writes stay simple
- calculations stay rebuildable
- historical state stays reproducible
- market data remains a separate future concern
