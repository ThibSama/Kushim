# Deferred TODOs

This file centralizes intentionally deferred work.

Use these labels:

- Deferred
- Planned
- Not started
- Known limitation
- Accepted risk

## Database / data model

### Resolved

- repeated demo runs creating new economic representations of AAPL is fixed: backend E2E now resolves the canonical `(AAPL, NASDAQ)` row seeded by `infra/postgres/init/002_seed_canonical_assets.sql` and never inserts a catalogue asset (see `documentation/operations/backend-demo-e2e.md`, Step E)
- fresh-database auth bootstrap defect is fixed: the `user` role is seeded by `infra/postgres/init/003_seed_auth_roles.sql`, so signup works on a brand-new database with no manual SQL insertion. The role is reference data (deterministic `id_role = 1`, no credentials)
- backend E2E now validates `DemoPrefix` locally (auth username contract `^[a-z0-9_][a-z0-9_-]{2,39}$`) and fails fast before calling `/auth/signup` instead of emitting an opaque HTTP error
- market-data integration tests no longer use canonical provider symbols for temporary fixtures; they use the `TEST_CURRENT_*` / `TEST_HISTORY_*` / `TEST_TICKER_*` technical prefixes and a `#[cfg(test)]`-only deterministic provider that no Finnhub allowlist resolves
- CI now validates the complete fresh-database bootstrap (schema + canonical asset seed + auth role seed) idempotently in the dedicated `fresh-db-bootstrap` job

### Deferred

- production-grade asset-master ingestion, enrichment, alias and metadata workflow (the canonical seed is intentionally minimal — three rows, no aliases, no metadata, no prices)
- broader canonical seed coverage beyond AAPL/MSFT/NVDA
- operation type to asset class validation matrix
- nuanced handling of inactive, delisted, or merged assets for historical operations
- richer FX history cache strategy
- explicit policy for historical restatement if portfolio base currency changes

### Known limitation

- existing local databases created before the canonical seed may still contain legacy `test_hist_*` / `test_*` / `Apple Inc. (E2E Demo)` rows that resolve to AAPL/MSFT/NVDA via `COALESCE(ticker, symbol)`. They are referenced by posted operations, holdings and snapshots, so this pass does not auto-merge or delete them. Cleanup is an optional local maintenance action (see `scripts/dev/audit-asset-catalog.ps1` for the current local state); the cleanest reset is `docker compose down -v` followed by `docker compose up -d ...` when the local data is disposable
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

### Resolved

- automatic portfolio refresh after an operation is posted (P0): `kushim-api`
  enqueues a durable `portfolio_refresh_requests` row in the same transaction
  that posts the operation, and `kushim-worker` runs
  `process_portfolio_refresh_requests` in loop mode to rebuild current read
  models + the current daily snapshot automatically. The manual
  `rebuild_current_read_models` / `generate_daily_snapshots` invocation after an
  operation is no longer required (validated end-to-end via
  `scripts/demo/backend-e2e.ps1`). Uses PostgreSQL `FOR UPDATE SKIP LOCKED` as a
  durable queue — no Redis/queue infrastructure.

### Deferred

- multi-portfolio backfill orchestration
- optimized incremental backfill
- smarter split/spin-off/corporate-action logic
- richer FX support
- production scheduler
- partial failure strategy for broader batch jobs
- queue-based orchestration if scaling later requires it
- **cross-currency operation contribution** (`kushim-worker/src/domain/portfolio_state.rs::convert_amount_to_base`): a posted operation whose `currency` differs from the portfolio's `base_currency` and that carries no `fx_rate_to_portfolio` converts to zero and marks the portfolio as estimated. **P1 status update**: option (a) has been implemented in `kushim-api` — new posted cross-currency operations without a positive `fx_rate_to_portfolio` are now blocked at write time by the `unsupported_cross_currency` (HTTP 422) guard on every posting path (direct posted create, pending → posted transition, posted correction creation). Consequences:
  - new invalid posted operations can no longer be created;
  - legacy rows posted before P1 may still exist with no FX and remain readable; the worker fallback (zero contribution, `is_estimated = true`) is intentionally preserved for backward compatibility with them — DDL and rows are not mutated;
  - automatic provider-based remediation (option b: server-side FX lookup, historical restatement when a better rate lands later) still depends on the deferred Market-data **FX rate provider selection and integration** TODO below.

  Until the FX provider lands, the only supported way to post a cross-currency monetary operation is for the user to supply a positive `fx_rate_to_portfolio` (manual entry in the operation modal, or direct API payload) interpreted as `1 unit of operation currency = fx_rate_to_portfolio units of portfolio base currency`.

### Known limitation

- backfill V1 is mono-portfolio only
- backfill V1 is range-limited to 366 days
- backfill V1 rejects loop mode
- **`rm_portfolio_holdings.weight_pct` (and the daily snapshot equivalent) are intentionally holdings-only allocation**: a holding's share of `sum(market_value_minor)` across the portfolio's open holdings, excluding cash. This matches the frontend Dashboard allocation chart denominator (`kushim-app/src/app/pages/Dashboard.tsx:385-401`), satisfies the DDL `CHECK (weight_pct BETWEEN 0 AND 100)` even when cash is negative, and makes the non-null weights sum to 100. Zero-valued holdings (foreign-currency buys with no fx) surface `weight_pct = NULL`. If a future product decision requires net-portfolio weighting (including cash), revise the rebuild calculation, the DDL constraint, the API DTO docs and the frontend Dashboard allocation simultaneously.

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
- **FX rate provider selection and integration** *(reaffirmed by P1
  currency contract; P1 explicitly does not integrate any provider)*: evaluate
  and select a reliable provider for current AND historical foreign-exchange
  rates. The decision must explicitly evaluate:
  - current FX rates;
  - historical FX rates;
  - supported currency pairs;
  - direct pairs versus triangulation;
  - base/quote conventions;
  - rate timestamps and trading dates;
  - weekend and holiday behavior;
  - precision and rounding;
  - update frequency;
  - historical depth;
  - pricing;
  - rate limits;
  - reliability and availability;
  - provider fallback;
  - caching;
  - licensing;
  - redistribution rights;
  - provenance and auditability;
  - historical restatement policy when a better FX rate arrives later.

  Constraints carried forward from P1:
  - Kushim must not invent an automatic FX rate.
  - Provider integration is required before any automatic conversion.
  - Until then, cross-currency posted monetary operations require a
    user-supplied validated `fx_rate_to_portfolio` enforced at API write time
    by `unsupported_cross_currency` (HTTP 422).
  - The frontend operation modal must keep its manual FX field as the
    single way to supply a rate for a cross-currency posted operation until
    the provider lands.

## Frontend

### Completed / current MVP checkpoint

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
- Scenario A browser dry-run validated with zero blocking console errors
- logout validated in the supervised MVP flow
- **P2 — durable operation asset identity** (implemented and validated):
  every operation response (list/get/create/update/cancel/post/correction/
  audit/audit-timeline) embeds compact `asset` / `related_asset` references
  resolved through one deduplicated batch query, with identities prefetched
  before the mutation to keep write responses non-ambiguous. The
  `assetDisplayCache` / `hydrateAssetDisplayCache` N+1 path has been
  removed. Chrome acceptance on `http://app.kushim.localhost` /
  `http://api.kushim.localhost` confirmed: ticker/name visible immediately
  after full F5 reload, no `GET /v1/assets/{id}` per row in the Network
  panel, newly created asset operations display their label directly from
  the create response, cash operations render `—`, and portfolio switching
  does not leak labels between portfolios.

### Deferred

- correction and audit UX
- complex operation types UX (split, spin_off, symbol_change, adjustment)
- dashboard benchmark real data wiring — block removed from the UI until a real index-history endpoint and contract exist
- real asset swap / exchange product capability — removed from the UI; requires product and accounting semantics before any implementation
- settings preference editing inside `kushim-app` (deferred to the dedicated auth frontend)
- in-app password change (deferred to the dedicated auth frontend)
- in-app account deletion (no backend contract; deferred)
- dashboard "Ajouter un actif" production flow

### Known limitation

- the dashboard no longer exposes a simulated benchmark; the section is hidden until real index history is available
- the swap quick action has been removed; no fake conversion flow remains
- the Settings page only exposes profile information and logout — preference, password and delete forms are no longer shown as if they were near-functional
- **browser token storage uses `localStorage`** (`kushim_access_token`, `kushim_refresh_token`). It is readable by any script that runs in the page and survives cross-tab. The P0.3 session layer (`tokenStorage.ts` / `sessionGate.ts` / `authenticatedRequest.ts`) centralizes the access pattern (single source of truth, single-flight refresh, retry-at-most-once, logout race protection) but does **not** upgrade the storage primitive. Production-grade browser session security (HttpOnly cookie + CSRF defence, or a service-worker-isolated token vault) requires an auth-API protocol change (cookie issuance, OPTIONS/CORS for credentials, CSRF token endpoint) and is out of scope for the MVP.
- **Refresh tracking sessionStorage** (`kushim_active_portfolio_refresh`) persists `portfolioId` + `refreshRequestId` + `startedAt` only — no token, no `last_error`, no financial values. Recovery TTL: 15 minutes. Frontend polling budget: 60 s per cycle. Both constants live in `kushim-app/src/lib/api/refreshTrackingStorage.ts` and `kushim-app/src/stores/refreshTracking.ts`.

## Infra / DevOps

### Planned

- CI/CD
- deployment strategy
- production secrets management
- backups and restore drills
- stronger observability
- nginx hardening and routing strategy
- deployment target decision

### Deferred

- Redis queues
- distributed locks
- production job scheduling strategy

## Backend E2E / CI

### Implemented

- backend E2E smoke test script: `scripts/demo/backend-e2e.ps1` — validated locally, 18/18 assertions
- backend demo runbook: `documentation/operations/backend-demo-e2e.md`
- MVP smoke GitHub Actions workflow for frontend lint/build, Rust fmt/clippy/test with PostgreSQL, and `cargo audit --ignore RUSTSEC-2023-0071`
- local backend prerequisite preflight: `scripts/validation/check-local-services.ps1`

### Still deferred

- CI integration of the E2E smoke test
- Docker image build validation in CI
- multi-day historical backfill demo (requires portfolio with older `created_at`)
- frontend E2E testing
- production market-data provider in E2E scenario beyond tightly allowlisted Finnhub stock quote dev validation
- FX conversion in E2E scenario
- Upgrade GitHub Actions dependencies that still emit Node.js 20 deprecation warnings before GitHub forces the Node.js 24 runtime; current warnings are non-blocking and the workflow remains green

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
