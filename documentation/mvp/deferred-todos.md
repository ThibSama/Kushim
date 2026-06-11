# Deferred TODOs

This file centralizes intentionally deferred work.

Use these labels:

- Deferred
- Planned
- Not started
- Known limitation
- Accepted risk

## Database / data model

### Deferred

- operation type to asset class validation matrix
- nuanced handling of inactive, delisted, or merged assets for historical operations
- richer FX history cache strategy
- explicit policy for historical restatement if portfolio base currency changes

### Known limitation

- current schema supports more advanced cases than the currently implemented service logic uses

## Auth

### Deferred

- token family / session table
- revoke all sessions on password reset
- richer session management beyond revoked refresh `jti`
- stronger proxy trust strategy for forwarded IP extraction

### Accepted risk

- known `RUSTSEC-2023-0071` remains monitored while JWT usage stays HS256-only

## Main API

### Deferred

- richer inactive/delisted asset handling during operation posting
- broader audit and correction workflows
- possible future token revocation checks for protected business API requests
- additional user-facing derived read models when needed

### Known limitation

- `kushim-api` reads derived data only and returns `data_available=false` when the worker has not generated it yet

### Needs product/security decision

- token revocation checks in `kushim-api` require a clear boundary with `kushim-auth/api`
- richer correction workflows require product rules for auditability and accounting semantics

## Worker

### Deferred

- multi-portfolio backfill orchestration
- optimized incremental backfill
- smarter split/spin-off/corporate-action logic
- richer FX support
- production scheduler
- partial failure strategy for broader batch jobs
- queue-based orchestration if scaling later requires it

### Known limitation

- backfill V1 is mono-portfolio only
- backfill V1 is range-limited to 366 days
- backfill V1 rejects loop mode

### Needs product/architecture decision

- split, spin-off, symbol-change, delisted, and merged-asset handling need explicit business rules before expanding replay logic
- richer FX support needs a price-source and restatement policy
- queues, distributed locks, and production scheduling should be chosen together with the deployment model

## Market-data

### Implemented with mock and guarded Finnhub providers

- `refresh_current_market_data` job — validated locally
- `fill_missing_price_history_cache` job — validated locally
- mock provider with deterministic USD prices
- Finnhub provider for controlled allowlisted current stock quotes
- Finnhub current quotes live-validated for AAPL/MSFT/NVDA
- provider-symbol mapping support exists for canonical-to-provider symbols, including the tested BTC attempt `BTC=BINANCE:BTCUSDT`

### Known limitations

- BTC provider-symbol mapping exists (`BTC=BINANCE:BTCUSDT`), but live BTC quote is not validated with the current free Finnhub plan (returned `403 Forbidden`)
- Finnhub historical candles (`/stock/candle`) are implemented, but access may require plan/endpoint entitlement (returned `403 Forbidden` with the current plan)
- Finnhub support is currently dev/MVP guarded, not a production market-data strategy
- no FX conversion in the market-data pipeline
- no provider retry/backoff strategy beyond MVP behavior
- no production scheduler for market-data jobs

### Still deferred

- production-grade market-data provider strategy and broader provider rollout
- broader asset coverage beyond the current allowlist (AAPL, MSFT, NVDA)
- crypto live support unless Finnhub plan allows the mapped symbols
- historical candle entitlement validation or alternative historical data source
- asset enrichment workflow
- FX support in market-data pipeline
- provider fallback strategy
- freshness and reconciliation policy
- production scheduling for market-data jobs
- queues/locks if scaling later requires them

## Frontend

### Completed

- auth frontend wiring to `kushim-auth/api` (login, signup, handoff exchange)
- portfolio list/create/select wiring to `kushim-api`
- operations list/create wiring (cash + asset-linked: buy, sell, dividend)
- asset search/select component wiring to `GET /v1/assets`
- dashboard KPIs wired to `/summary` read model
- dashboard top 5 assets wired to `/holdings` read model
- dashboard evolution chart wired to `/snapshots/daily`
- dashboard allocation derived from real holdings
- UI handling of `data_available=false`, `read_model_missing`, `snapshot_missing`
- worker-generated snapshot visibility in the UI (evolution chart)
- Assets page real data wiring (Pass 7)
- AssetDetail page real data wiring (Pass 7)
- Portfolio positions page real data wiring (Pass 8)
- dashboard allocation stats (open positions, best/worst performance) derived from real holdings (Pass 5b)

### Deferred

- correction and audit UX
- complex operation types UX (split, spin_off, symbol_change, adjustment)
- dashboard benchmark real data wiring

### Known limitation

- dashboard demo notice banner states benchmark is demo — accurate; allocation stats are now real (Pass 5b)
- asset display in Transactions table falls back to truncated UUID after page refresh (in-memory cache only)

## Infra / DevOps

### Planned

- CI/CD
- deployment strategy
- production secrets management
- backups and restore drills
- stronger observability
- nginx hardening and routing strategy

### Deferred

- Redis queues
- distributed locks
- production job scheduling strategy

## Backend E2E / CI

### Implemented

- backend E2E smoke test script: `scripts/demo/backend-e2e.ps1` — validated locally, 18/18 assertions
- backend demo runbook: `documentation/operations/backend-demo-e2e.md`

### Still deferred

- CI integration of the E2E smoke test
- multi-day historical backfill demo (requires portfolio with older `created_at`)
- frontend E2E testing
- production market-data provider in E2E scenario beyond tightly allowlisted Finnhub stock quote dev validation
- FX conversion in E2E scenario

## Documentation

### Planned

- keep root README aligned with actual service maturity
- keep service READMEs aligned with current code
- track accepted security advisories in a central review cadence

### Small safe future passes

- keep `kushim-api` docs explicit that read models and snapshots are read-only and may be unavailable until worker jobs run
- keep `kushim-worker` docs explicit about V1 backfill limits before expanding orchestration
- keep validation-command snippets aligned with the current service READMEs

### Known limitation

- some older docs remain reference material and should not be treated as the shortest path to understand the current MVP

## Security

### Deferred

- production threat-model pass
- expanded deployment hardening
- CI security gates such as `cargo deny` if adopted later

### Accepted risk

- `RUSTSEC-2023-0071` monitoring must remain part of periodic dependency review
