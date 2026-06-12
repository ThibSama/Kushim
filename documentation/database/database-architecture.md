# Database Architecture

## Source of truth

The schema source of truth is:

- `infra/postgres/init/001_init.sql`

Do not infer schema from old docs when the DDL says otherwise.

## Main data layers

### 1. Identity and auth

Core tables:

- `roles`
- `users`
- `user_recovery_phrases`
- `revoked_tokens`

Notable rules:

- users are soft-deletable
- recovery phrases are hashed
- revoked refresh tokens are stored by `jti`

### 2. Asset catalog and pricing

Core tables:

- `assets`
- `asset_aliases`
- `asset_metadata`
- `asset_market_data`
- `asset_price_history_cache`

Role split:

- `asset_market_data` = current/latest market information
- `asset_price_history_cache` = deterministic historical price cache

### 3. Portfolio source of truth

Core tables:

- `portfolios`
- `portfolio_operations`

Key truths:

- `portfolio_operations` is the source of truth
- `operation_status` drives business meaning
- `id_corrected_operation` supports the correction model
- `posted` rows are immutable at database level

### 4. Derived current state

Core tables:

- `rm_portfolio_summary`
- `rm_portfolio_holdings`

These are:

- derived
- rebuildable
- worker-generated
- read-only in `kushim-api`

### 5. Derived historical state

Core tables:

- `portfolio_snapshots_daily`
- `portfolio_holding_snapshot_daily`

These are:

- derived
- rebuildable from source truth + pricing inputs
- worker-generated
- read-only in `kushim-api`

## Important invariants

### Soft delete

Confirmed in schema:

- `users.deleted_at`
- `portfolios.deleted_at`

### Posted operation immutability

The database defines a trigger preventing direct mutation of `posted portfolio_operations`.

This matters because:

- the worker can trust posted ledger rows as immutable business truth
- corrections must use compensating operations

### Corrections model

Corrections are represented through:

- `adjustment`
- linked via `id_corrected_operation`

This preserves auditability instead of rewriting history.

### Monetary values

Monetary values use `*_minor`.

This avoids float-based storage for money and is consistent across:

- operations
- read models
- snapshots
- cached prices

## Current service ownership

### `kushim-auth/api`

May write:

- `users`
- `user_recovery_phrases`
- `revoked_tokens`

### `kushim-api`

May write:

- `portfolios`
- `portfolio_operations`

Must not write:

- read models
- snapshots
- price cache
- market data

### `kushim-worker`

May write:

- `rm_portfolio_summary`
- `rm_portfolio_holdings`
- `portfolio_snapshots_daily`
- `portfolio_holding_snapshot_daily`

### `kushim-market-data`

Owns market-data writes:

- `asset_market_data`
- `asset_price_history_cache`

## Current maturity assessment

Implemented and validated at schema level:

- auth tables
- portfolios
- source-of-truth operations
- read models
- snapshots
- historical price cache
- correction model
- soft-delete fields
- immutability and `updated_at` triggers

## Known limitations at repository level

The schema supports more than the MVP currently exercises:

- `kushim-market-data` is implemented for mock current/history data and guarded Finnhub current-equity validation, but it is not a production market-data strategy;
- Finnhub current equities are validated for AAPL/MSFT/NVDA only;
- BTC/crypto and Finnhub historical candles are not validated with the current plan/access;
- FX conversion is not implemented;
- some advanced corporate-action or FX rules remain service-level future work;
- some schema support is broader than the currently implemented MVP logic.
