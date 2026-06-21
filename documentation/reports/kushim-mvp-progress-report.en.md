# Kushim MVP Progress Report

_Updated on: 2026-06-11_

## 1. Executive summary

Kushim now has a serious backend MVP core built around a clear architecture:

- `portfolio_operations` as the source of truth
- `kushim-api` for synchronous user-facing writes and reads
- `kushim-worker` for read models, snapshots, and controlled backfills
- `asset_price_history_cache` as the deterministic historical cache

The global state of the project:

- the backend core is advanced and validated in multiple areas;
- the backend E2E chain is now demonstrable locally via an automated smoke test (`scripts/powershell/demo/backend-e2e.ps1`, 18/18 assertions passed);
- `kushim-market-data` has a mock provider (safe default) and a guarded Finnhub provider; Finnhub current stock quotes are live-validated for AAPL, MSFT, and NVDA only;
- `kushim-app` no longer exposes any simulated user-facing financial data: the demo benchmark block, the simulated swap flow, and the non-functional Settings forms (preferences, password change, account deletion) have been removed. Every visible value comes from the API, read models, persisted operations, or an explicitly unavailable state;
- the MVP asset catalogue now has a stable canonical seed (`infra/postgres/init/002_seed_canonical_assets.sql`) for AAPL, MSFT and NVDA; backend demos, controlled Finnhub validations and market-data integration tests reuse these rows instead of creating new ones per run;
- a fresh database supports signup with no manual SQL insertion: the `user` role (reference data, `id_role = 1`, no credentials) is seeded by `infra/postgres/init/003_seed_auth_roles.sql`; backend E2E now validates `DemoPrefix` locally before the signup call; a dedicated `fresh-db-bootstrap` CI job validates the complete bootstrap (schema + asset seed + role seed) idempotently;
- automatic portfolio refresh (P0): posting an operation enqueues a durable `portfolio_refresh_requests` row in the same PostgreSQL transaction; `kushim-worker` (loop mode, `process_portfolio_refresh_requests`) consumes it automatically (`FOR UPDATE SKIP LOCKED`, bounded retry, stale-lock recovery) and rebuilds current read models + the current daily snapshot; the frontend follows the request via `GET /v1/portfolios/{id}/refresh-requests/{id}` and reloads real data on completion. No manual worker command after an operation. Validated end-to-end (18 assertions, no manual rebuild/snapshot);
- `kushim-auth/front` is wired to `kushim-auth/api` for login, signup, recovery, and Redis-backed handoff;
- production readiness is not the current status or claim.

In one sentence:

> Kushim is now suitable for a supervised internal MVP demo: backend E2E validated, no simulated user-facing data in the frontend (demo benchmark, simulated swap, and non-functional Settings forms removed), market data with mock provider (safe default) and guarded Finnhub (current stock quotes validated), not production-ready. Market prices may still originate from the configured mock provider or the guarded Finnhub provider, and the source remains explicit in the UI.

## 2. Product MVP objective

The Kushim MVP aims to demonstrate that a user can eventually:

- authenticate;
- create portfolios;
- record `portfolio_operations`;
- browse and select assets;
- view current summaries and holdings;
- view historical snapshots when available;
- audit corrections and portfolio operation history;
- prepare performance and historical evolution features.

Kushim is not currently trying to be:

- a broker;
- an execution platform;
- a bank;
- a payment provider;
- a market data vendor.

## 3. High-level architecture

### Services

- `kushim-auth/api`: authentication service
- `kushim-api`: synchronous user-facing business API
- `kushim-worker`: background jobs, read models, snapshots, controlled backfills
- `kushim-market-data`: market-data service with mock provider and guarded Finnhub provider
- `kushim-app`: authenticated frontend
- `kushim-website`: marketing website
- `kushim-auth/front`: auth frontend
- `infra/postgres`: PostgreSQL
- `infra/redis`: Redis
- `infra/nginx`: local reverse proxy

### Key separation

- `kushim-api` writes user-facing source-of-truth data
- `kushim-worker` computes and persists derived data
- `kushim-market-data` supplies market data (mock by default, guarded Finnhub for dev validation)

### Current data flow

```text
portfolio_operations
  -> kushim-worker rebuild_current_read_models
  -> rm_portfolio_summary / rm_portfolio_holdings
  -> kushim-worker generate_daily_snapshots
  -> portfolio_snapshots_daily / portfolio_holding_snapshot_daily
  -> kushim-api read-only endpoints
```

## 4. Service status matrix

| Service | Status | Comment |
|---|---|---|
| `kushim-auth/api` | Implemented and validated | Real hardened auth backend |
| `kushim-api` | Implemented and validated | Advanced MVP business API |
| `kushim-worker` | Implemented and validated | Current-state pipeline + snapshots + backfill V1 |
| `kushim-market-data` | Implemented with mock + guarded Finnhub | Two jobs validated, Finnhub current quotes live-validated for AAPL/MSFT/NVDA |
| `kushim-auth/front` | Implemented for MVP demo | Auth UI wired to API; Redis-backed handoff operational |
| `kushim-app` | Largely implemented | Zero user-facing simulated data (demo benchmark, simulated swap, non-functional Settings forms removed); every visible value comes from API/read models/operations/explicit unavailable states |
| `kushim-website` | Implemented | Marketing site present |
| `infra/postgres` | Implemented and validated | Rich coherent V3 DDL |
| `infra/redis` | Minimally implemented | Useful today for auth and worker checks |
| `infra/nginx` | Implemented for dev | Minimal local reverse proxy |

## 5. Database status

## 5.1 DDL

The DDL in `infra/postgres/init/001_init.sql` is rich and aligned with the target architecture.

It covers:

- auth
- assets
- portfolios
- `portfolio_operations` ledger
- read models
- snapshots
- historical price cache

## 5.2 Confirmed points

- `portfolio_operations` is the source of truth
- corrections use `adjustment + id_corrected_operation`
- users and portfolios are soft-deletable
- `updated_at` triggers exist
- a trigger protects posted operation immutability
- read models are rebuildable
- snapshots are derived
- `asset_price_history_cache` is deterministic

## 5.3 MVP status

Database:

- **Implemented and validated**

## 6. `kushim-auth/api` status

## 6.1 Features

Implemented:

- signup
- login
- refresh
- logout
- `/auth/me`
- recovery setup
- password reset

## 6.2 Security

Implemented:

- Argon2id
- JWT access/refresh
- refresh rotation
- `revoked_tokens`
- Redis rate limiting
- `no-store` headers
- strict JSON DTOs
- redacted security logs

## 6.3 Database ownership

Writes:

- `users`
- `user_recovery_phrases`
- `revoked_tokens`

Reads:

- `roles`

## 6.4 MVP status

`kushim-auth/api`:

- **Implemented and validated**

## 6.5 Known limitation

- no token family/session table yet

## 7. `kushim-api` status

## 7.1 Business features

Implemented:

- portfolios
- `portfolio_operations` lifecycle
- corrections
- audit views
- read-only assets
- read-only summary
- read-only holdings
- read-only daily snapshots
- read-only historical holdings by snapshot

## 7.2 Architecture guarantees

Confirmed:

- no worker logic inside `kushim-api`
- no read-model generation
- no snapshot generation
- no historical reconstruction

## 7.3 Security / HTTP robustness

Implemented:

- access-token validation
- refresh token rejection
- normalized JSON errors
- strict JSON request handling
- cross-user -> `404`
- soft-delete hidden from user-facing reads

## 7.4 Database ownership

Writes:

- `portfolios`
- `portfolio_operations`

Reads:

- assets
- current market data
- read models
- snapshots
- historical price cache

## 7.5 MVP status

`kushim-api`:

- **Implemented and validated**

## 7.6 Known limitation

- `kushim-api` depends on the worker for all derived portfolio data

## 8. `kushim-worker` status

## 8.1 Foundation

Implemented:

- config loading
- PostgreSQL PgPool
- optional Redis check
- `/health` and `/ready`
- `idle | once | loop`
- graceful shutdown

## 8.2 Current jobs

Implemented:

- `noop`
- `rebuild_current_read_models`
- `generate_daily_snapshots`
- `refresh_current_portfolio_state`
- `backfill_daily_snapshots`

## 8.3 Current-state pipeline

Implemented:

- current read-model rebuild
- current daily snapshot generation
- composite current refresh

## 8.4 Historical backfill V1

Implemented:

- explicit mono-portfolio targeting
- explicit date range
- max 366 days
- historical valuation through `asset_price_history_cache` only
- no external fetch
- no FX
- idempotent reruns

## 8.5 Database ownership

Writes:

- `rm_portfolio_summary`
- `rm_portfolio_holdings`
- `portfolio_snapshots_daily`
- `portfolio_holding_snapshot_daily`

Does not write:

- `portfolio_operations`
- `portfolios`
- `asset_market_data`
- `asset_price_history_cache`

## 8.6 MVP status

`kushim-worker`:

- **Implemented and validated**

## 8.7 Known limitations

- conservative V1 handling for some corporate actions
- no multi-portfolio backfill
- no Redis queue
- no distributed locks
- no advanced scheduler

## 9. `kushim-market-data` status

## 9.1 Real state

Implemented with two providers:

- `refresh_current_market_data`: writes `asset_market_data` for active supported assets
- `fill_missing_price_history_cache`: writes `asset_price_history_cache` for missing dates
- **mock provider**: deterministic USD prices for 7 symbols (AAPL, MSFT, NVDA, BTC, ETH, SPY, VTI) — safe default for MVP demos
- **Finnhub provider**: first real provider, guarded by configuration and allowlist
  - current stock quotes live-validated for AAPL, MSFT, NVDA
  - BTC has a provider-symbol mapping path (`BTC=BINANCE:BTCUSDT`), but the current free plan returns `403 Forbidden` — BTC is not live-validated
  - historical candles (`/stock/candle`) are implemented, but access depends on Finnhub plan/entitlement — returns `403 Forbidden` with the current plan
  - typed handling of provider errors (401, 403, 429) with no silent fallback to mock
  - mandatory allowlist before any Finnhub call
- `once | loop | idle` modes
- `/health` and `/ready` endpoints
- 68 tests passing (unit + integration)

## 9.2 Missing or still deferred

- production provider strategy (guarded Finnhub MVP ≠ production strategy)
- broader asset coverage beyond the current allowlist
- BTC/crypto live validation (depends on provider plan)
- Finnhub historical candle entitlement validation (depends on provider plan)
- asset enrichment
- FX support in market-data pipeline
- freshness and reconciliation policy
- production scheduler, queues, locks

## 9.3 MVP status

`kushim-market-data`:

- **Implemented and validated locally (mock provider + guarded Finnhub for current stock quotes)**
- the service is not production-ready

## 10. Frontend status

## 10.1 `kushim-auth/front`

Present:

- auth pages
- auth UX shell

Missing:

- real integration with `kushim-auth/api`

Status:

- **Partially implemented**

## 10.2 `kushim-app`

Present and wired to real API:

- authentication (handoff, session validation, refresh, logout)
- portfolio list/create/select
- operations list/create (cash + asset-linked: buy, sell, dividend)
- dashboard KPIs, evolution chart, allocation, top 5 assets (real read models)
- asset catalogue (`/assets`) with search, filters, pagination (real data)
- asset detail (`/assets/:id`) with identity, market data, metadata (real data)
- portfolio positions (`/positions`) with search, filters, sort, pagination (real holdings)
- transactions page with search, filters, metrics (real operations)
- `data_available=false` / `read_model_missing` / `snapshot_missing` states

Remaining user-facing mock:

- none. The demo benchmark section, the simulated "Échanger des actifs" quick action, and the non-functional Settings forms (preferences, password change, account deletion) have been removed from the authenticated app. The `kushim-app/src/mocks/demoPortfolio.ts` file has been deleted and the `src/mocks/` directory no longer exists.

Status:

- **Zero user-facing simulated data — every visible value comes from the API, read models, persisted operations, or an explicit unavailable state**

## 10.3 `kushim-website`

Present:

- marketing landing

Status:

- **Implemented**

## 10.4 MVP consequence

The main visible MVP work remaining on `kushim-app` is now the native wiring of `kushim-auth/front` (preferences, password change, account deletion) and integrating a real benchmark once a backend index-history endpoint exists. No simulated frontend data remains in normal app paths.

## 11. Docker / infra status

## 11.1 Docker Compose

Present:

- main services
- Postgres
- Redis
- Nginx

## 11.2 Health checks

Present for:

- auth API
- main API
- worker
- database
- redis

## 11.3 Reverse proxy

Nginx currently routes:

- website
- auth frontend
- app frontend
- API

## 11.4 MVP status

Local infrastructure:

- **sufficient for development and local validation**
- **not yet a production deployment strategy**

## 12. Testing and validation status

## 12.1 Known state

Rust services documented as validated:

- `kushim-auth/api`
- `kushim-api`
- `kushim-worker`
- `kushim-market-data` (mock provider)

Observed test counts in the repository:

- auth: ~63 tests
- API: ~157 tests
- worker: ~60 tests
- market-data: ~68 tests

## 12.2 Backend E2E smoke test

**Validated locally.**

An automated script executes the full backend chain:

- script: `scripts/powershell/demo/backend-e2e.ps1`
- runbook: `documentation/operations/backend-demo-e2e.md`
- result: **18/18 assertions passed**
- services covered: `kushim-auth/api`, `kushim-api`, `kushim-market-data`, `kushim-worker`
- scenario: signup → portfolio → deposit → buy → market-data refresh (mock) → worker rebuild/snapshots/backfill → API verification

Smoke test limitations:

- uses mock provider (no real market data)
- does not validate frontends
- does not validate production deployment
- does not validate FX conversions
- multi-day backfill is limited by portfolio `created_at` date

## 12.3 Coverage

Strong areas:

- auth
- business API
- read models
- snapshots
- backfill V1
- market-data mock provider
- backend E2E chain (smoke test)

Weak or absent:

- frontends
- full-stack E2E (frontend + backend)
- Finnhub provider under production conditions (only current stock quotes validated in dev)

## 13. Security status

## 13.1 Strengths

- access vs refresh token separation
- refresh rejection in `kushim-api`
- Redis rate limiting in auth
- strict JSON bodies
- normalized API errors
- posted operation immutability
- clear DB ownership

## 13.2 Limitations

- no token family
- no revoke-all-sessions on password reset
- no access-token revocation checks in `kushim-api`
- incomplete production observability

## 13.3 Accepted risk

- `RUSTSEC-2023-0071` remains monitored as a known advisory

## 14. Deferred TODO list

Main deferred workstreams:

- token family / session table
- revoke all sessions on password reset
- operation type <-> asset class matrix
- more nuanced inactive/delisted/merged handling
- FX history cache / FX policy
- multi-portfolio backfill orchestration
- optimized incremental backfill
- richer split / spin-off / symbol-change logic
- Redis queues
- distributed locks
- production scheduler
- production-grade market-data provider strategy and broader rollout beyond guarded Finnhub MVP
- auth frontend production hardening (`kushim-auth/front` -> `kushim-auth/api` is wired; token storage remains MVP-grade)
- dashboard benchmark real data (currently demo)
- settings backend handlers (preferences, password, account deletion)
- correction/audit UX
- CI/CD
- production secrets
- backups
- observability
- nginx hardening
- deployment strategy
- ongoing `cargo audit` monitoring for `RUSTSEC-2023-0071`

Recently completed (formerly deferred):

- dashboard frontend wiring (Pass 5/5b — KPIs, evolution, allocation, top assets)
- `data_available=false` / `read_model_missing` / `snapshot_missing` UI states (Pass 5)
- assets page real data wiring (Pass 7)
- AssetDetail page real data wiring (Pass 7)
- positions page real data wiring (Pass 8)

Central reference:

- [documentation/mvp/deferred-todos.md](../mvp/deferred-todos.md)

## 15. MVP readiness assessment

## 15.1 Backend MVP

The core backend is at a strong MVP level and **demonstrable end-to-end locally**:

- auth
- ledger
- synchronous business API
- worker rebuilds
- worker snapshots
- historical backfill V1
- market-data mock provider + guarded Finnhub (current stock quotes)
- **automated E2E smoke test: 18/18 assertions passed** (`scripts/powershell/demo/backend-e2e.ps1`)

All four backend services (`kushim-auth/api`, `kushim-api`, `kushim-market-data`, `kushim-worker`) are integrated in the smoke test scenario.

## 15.1b 10-minute dry run — 2026-06-11

**Validated on 2026-06-11** — Scenario A (mock provider only).

End-to-end flow validated:

- auth (signup, login, session)
- USD portfolio created
- 2 operations (deposit $10,000 + buy 10 AAPL @ $195.23)
- mock market-data refresh (10 updated, 2 historical inserted, 0 errors)
- worker rebuild (1 holding, $10,000), snapshot (2026-06-11), backfill (1 snapshot)
- all browser pages validated: Dashboard, Positions, Transactions, Assets, AssetDetail, Settings, Logout
- zero blocking console errors

Non-blocking observations:

- evolution chart: 1 data point only (portfolio created same day)
- P&L = 0 (mock price = buy price — expected)
- `created_at` displays "Non disponible" (frontend fallback in place, root fix in auth API deferred)

## 15.2 User demo MVP — Pass 6: multi-day history validated

**The Dashboard "Portfolio evolution" chart now displays real multi-day history.**

Validated on 2026-06-11 with:

- a USD portfolio containing 4 operations spread from May 10 to June 1, 2026
- 32 historical AAPL USD prices in `asset_price_history_cache` (mock provider)
- 33 daily snapshots generated (32 backfill + 1 current)
- API `data_available: true` with sort, date filtering, pagination all working
- Dashboard: chart visible, period selectors (1M, 3M, 6M, 1Y, MAX) working, zero console errors

Limitations of this demo:

- mock provider only (deterministic USD prices, no real data)
- USD portfolio required (mock only generates USD prices)
- no FX conversion
- Redis-backed auth handoff is wired; manual token injection is troubleshooting only

Still needed for a full user demo:

- production-grade auth frontend/session hardening (handoff is wired; localStorage token storage remains an MVP limitation)
- stabilize and extend provider access beyond the guarded Finnhub MVP path (mock remains sufficient for supervised demo)
- removal of remaining mock remnants (benchmark, settings buttons)

## 15.3 User demo MVP — Pass 7: real asset catalogue

**The Assets and AssetDetail pages in `kushim-app` now display real data.**

Validated on 2026-06-11 with:

- `/assets`: real catalogue via `GET /v1/assets`, search, filters (class, status), pagination
- `/assets/:id`: real detail via `GET /v1/assets/{id}`, identity, market data, metadata, aliases
- English routes (`/assets`, `/assets/:id`), French UI labels
- `/actifs` and `/actifs/:id` redirect to `/assets` and `/assets/:id`
- dedicated Zustand store (`src/stores/assets.ts`)
- loading, empty, error states for both pages
- 25 assets displayed on load, "AAPL" search → 2 results, full Apple Inc. detail
- zero console errors, lint clean, build OK

Terminology decisions:

- `/assets` = catalogue of instruments available in Kushim (not user positions)
- `/positions` reserved for future portfolio positions page
- `/holding` and `/holdings` are not used as user-facing routes

Limitations:

- no historical price chart on the detail page
- no link to operations related to this asset
- market data depends on mock provider (no real data)

## 15.4 User demo MVP — Pass 8: real portfolio positions page

**The `/positions` page in `kushim-app` now displays real portfolio positions from the active portfolio.**

Validated on 2026-06-11 with:

- `/positions`: real data via `GET /v1/portfolios/{id}/holdings`
- Summary cards: position count, total market value, total P&L
- Table: name, ticker, exchange, class, quantity, avg cost, value, P&L (% + amount), weight
- Search by name/ticker, filter by class, sort (weight, value, name)
- Click a position → `/assets/:id` (catalogue detail)
- "Estimated" badge when `is_estimated=true`
- States: loading, error, `data_available=false` / `read_model_missing`, empty holdings, no portfolio
- Zero console errors, lint clean, build OK

Terminology decisions:

- User-facing route: `/positions` (not `/holdings`)
- UI label: "Positions" (not "Holdings")
- Backend API uses `holdings` internally — intentional
- `/holding` and `/holdings` are not user-facing routes

Polished in Pass 8b:

- Quantity formatting: trailing zeros removed (`8.0000000000` → `8`), French locale (`Intl.NumberFormat`)
- Pagination: initial load of 25 positions, "Charger plus de positions" button using API pagination (`has_more`, offset)
- Currency consistency: summary cards now derive currency from holdings data with portfolio fallback, preventing stale EUR display during portfolio load race condition

Limitations:

- no client-side column sorting (API-side sort only)
- depends on worker/read-model generation (`kushim-worker`)
- market data depends on mock provider

## 15.5 Production readiness

No.

The project should not currently be described as production-ready.

## 16. Recommended next steps

### ~~Priority 1~~ — Largely done

~~Wire the frontends~~ → `kushim-app` is now largely wired to `kushim-api` (auth, portfolios, operations, dashboard, assets, positions). Remaining mocks are isolated.

### Priority 1 (new)

Wire `kushim-auth/front` → `kushim-auth/api`:

- harden the handoff/session model for production usage
- improve native login/signup flow

### Priority 2

Stabilize and extend the market-data strategy:

- decide the production provider strategy beyond guarded Finnhub MVP
- validate historical candles entitlement or keep historical backfills on mock/seeded data
- extend asset coverage beyond the current allowlist (AAPL, MSFT, NVDA)
- plan FX support later

### ~~Priority 3~~ — Done

~~Add one end-to-end demo workflow~~ → **Done.**

The backend E2E smoke test is implemented and validated: `scripts/powershell/demo/backend-e2e.ps1` (18/18 assertions).

### Priority 3 (new)

Integrate the E2E smoke test into a CI pipeline.

## 17. Risks and required decisions

Important near-term decisions:

- production market-data provider strategy (guarded Finnhub MVP is a first step, not a final strategy)
- historical candle entitlement and crypto/BTC coverage
- whether frontend wiring or market-data extension comes first
- how sophisticated corporate-action handling must become before broader demo exposure
- CI integration of the E2E smoke test

## 18. Appendix

### Key documents

- [Root README](../../README.md)
- [Architecture overview](../architecture/overview.md)
- [Service boundaries](../architecture/service-boundaries.md)
- [Data flow](../architecture/data-flow.md)
- [Database architecture](../database/database-architecture.md)
- [Portfolio reconstruction](../database/portfolio-reconstruction.md)
- [MVP scope](../mvp/mvp-scope.md)
- [Deferred TODOs](../mvp/deferred-todos.md)
- [Docker local dev](../operations/docker-local-dev.md)
- [Validation commands](../operations/validation-commands.md)
- [MVP demo runbook (frontend + backend)](../operations/mvp-demo-runbook.md)
- [Backend E2E demo runbook](../operations/backend-demo-e2e.md)
- [Backend E2E smoke test script](../../scripts/powershell/demo/backend-e2e.ps1)
