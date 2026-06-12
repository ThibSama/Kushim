# Kushim Service Boundaries

## Why this file exists

Kushim uses multiple services on purpose.  
This file defines what each service owns and what it must not do.

Keeping these boundaries clear prevents architecture drift.

## `kushim-auth/api`

### Responsibilities

- authenticate users
- issue JWT access and refresh tokens
- rotate refresh tokens
- revoke refresh tokens
- expose current authenticated identity
- manage recovery phrase setup and password reset
- rate limit sensitive auth flows

### May write

- `users`
- `user_recovery_phrases`
- `revoked_tokens`

### May read

- `roles`

### Must not do

- manage portfolios
- manage `portfolio_operations`
- expose portfolio analytics
- generate read models
- generate snapshots
- fetch market data

## `kushim-api`

### Responsibilities

- synchronous user-facing business API
- validate access tokens issued by `kushim-auth/api`
- own portfolios
- own `portfolio_operations`
- expose assets in read-only mode
- expose read models in read-only mode
- expose snapshots in read-only mode
- expose corrections and audit views

### May write

- `portfolios`
- `portfolio_operations`

### May read

- `assets`
- `asset_aliases`
- `asset_metadata`
- `asset_market_data`
- `rm_portfolio_summary`
- `rm_portfolio_holdings`
- `portfolio_snapshots_daily`
- `portfolio_holding_snapshot_daily`
- `asset_price_history_cache`

### Must not do

- generate read models
- refresh `rm_portfolio_summary`
- refresh `rm_portfolio_holdings`
- generate snapshots
- run reconstruction or backfill jobs
- fetch market data
- call external market providers
- own worker loops, queues, or distributed locks

## `kushim-worker`

### Responsibilities

- background and derived-data jobs
- current read model rebuild
- current daily snapshot generation
- composite current-state refresh
- controlled historical snapshot backfill

### May write

- `rm_portfolio_summary`
- `rm_portfolio_holdings`
- `portfolio_snapshots_daily`
- `portfolio_holding_snapshot_daily`

### May read

- `portfolios`
- `portfolio_operations`
- `asset_market_data`
- `asset_price_history_cache`
- `rm_portfolio_summary`
- `rm_portfolio_holdings`

### Must not do

- expose a user-facing business API
- own signup/login/auth flows
- fetch external market data in the current implementation
- own frontend logic
- silently absorb business writes that belong to `kushim-api`

## `kushim-market-data`

### Responsibilities

- fetch market/provider data
- normalize provider payloads
- update `asset_market_data`
- populate `asset_price_history_cache`
- enrich asset metadata where needed

### Current state

- implemented beyond scaffold level for MVP/dev usage
- mock provider supports deterministic current and historical USD data for the demo path
- guarded Finnhub provider supports tightly allowlisted current equity quote validation
- Finnhub current quotes are live-validated for AAPL/MSFT/NVDA only
- BTC provider-symbol mapping exists (`BTC=BINANCE:BTCUSDT`), but live BTC validation is blocked by `403 Forbidden` with the current plan/access
- Finnhub historical `/stock/candle` is blocked by `403 Forbidden` with the current plan/access
- no FX conversion and no production scheduler

### Must not do

- expose the main user-facing portfolio API
- own portfolio write workflows
- own snapshot/read-model business logic that belongs to `kushim-worker`
- reconstruct portfolios
- own auth flows

## Frontends

### `kushim-auth/front`

Owns:

- auth-facing user interface
- signup/login/recovery UX

Must not own:

- auth business rules
- password hashing
- token issuance logic

### `kushim-app`

Owns:

- authenticated UI for dashboard, assets, transactions, and settings

Must not own:

- portfolio reconstruction rules
- worker-generated calculations
- market pricing rules

### `kushim-website`

Owns:

- marketing and product presentation

Must not own:

- business rules
- auth backend logic
- portfolio analytics logic

## PostgreSQL and infra

### `infra/postgres`

Owns:

- schema source of truth
- initialization scripts

Must not be treated as:

- a place for application business logic
- a substitute for worker calculations

### `infra/redis`

Current role:

- auth rate limiting
- auth handoff codes
- optional worker connectivity check

Not implemented yet:

- queues
- distributed locks

### `infra/nginx`

Current role:

- local reverse proxy for dev routing

Not yet:

- a fully hardened production ingress setup
