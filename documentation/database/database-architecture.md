# Database Architecture

## Source of truth

The schema source of truth is:

- `infra/postgres/init/001_init.sql`

Do not infer schema from old docs when the DDL says otherwise.

## Fresh-database seeds

Two seed files, separate from the schema DDL, provide the minimal reference data a fresh database needs. Both are idempotent, loaded automatically by Docker on fresh local volumes (in lexical order after `001_init.sql`), and validated by the `fresh-db-bootstrap` CI job.

### Canonical MVP asset seed

- `infra/postgres/init/002_seed_canonical_assets.sql`

This file owns exactly three rows: `(AAPL, NASDAQ, USD, equity, active)`, `(MSFT, NASDAQ, USD, equity, active)` and `(NVDA, NASDAQ, USD, equity, active)`, with fixed documented UUIDs. It is idempotent (ON CONFLICT on `(ticker, exchange)`), it never deletes or merges legacy rows, and it does not seed market prices, aliases, metadata, operations or portfolios. It is not a production-grade asset master.

### Auth role seed

- `infra/postgres/init/003_seed_auth_roles.sql`

This file owns the minimal authentication reference data: a single `user` role with deterministic `id_role = 1`. `kushim-auth/api` signup assigns this role to every new account via `RoleRepository::find_by_label("user")`, so on a brand-new database signup fails until this row exists. With this seed, a fresh database supports signup **without any manual SQL insertion**. The `user` row is reference data, not a demo user: it stores no credentials, password hashes, or recovery phrases. It is idempotent (ON CONFLICT on `label`); if an unrelated role already occupies `id_role = 1` with a different label, the primary-key conflict fails loudly rather than silently overwriting it.

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
