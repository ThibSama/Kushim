# Validation Commands

## Purpose

This file centralizes the main validation commands used across Kushim services.

If a task changes only documentation, these commands do not need to be rerun automatically.

## Validation ladder

Use these levels to avoid mixing fast static checks with Docker/PostgreSQL-dependent checks.

### Level 0 - Static fast checks

Run before committing small changes:

```powershell
cd E:\Kushim
git diff --check
```

For changed frontends:

```powershell
npm run lint
npm run build
```

For `kushim-app` (session/refresh changes since P0.3):

```powershell
cd E:\Kushim\kushim-app
npm run test   # vitest run — covers tokenStorage, sessionGate, authenticatedRequest, refreshTracking
```

### Level 0.5 - Controlled short-TTL auth-api (P0.3 session layer)

The single-flight refresh + retry-at-most-once contract on the `kushim-app`
side can only be exercised end-to-end when the access token actually expires
within a manual test window. Use a one-shot Docker override; never commit a
globally short default.

```powershell
# 1. Bring up auth-api with a 10-second access TTL.
$env:ACCESS_TOKEN_TTL_SECONDS = "10"
docker compose up -d --force-recreate kushim-auth-api
docker compose exec kushim-auth-api printenv ACCESS_TOKEN_TTL_SECONDS  # -> 10

# 2. Run the Chrome scenarios (see kushim-app/README.md — Session layer (P0.3)).

# 3. Restore the canonical 900-second default and verify.
Remove-Item Env:ACCESS_TOKEN_TTL_SECONDS
docker compose up -d --force-recreate kushim-auth-api
docker compose exec kushim-auth-api printenv ACCESS_TOKEN_TTL_SECONDS  # -> 900 (or unset → service default)
```

Never shorten `REFRESH_TOKEN_TTL_SECONDS`. The refresh token TTL is what
makes single-flight + recovery testable across a sustained Chrome session.

For changed Rust services:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

### Level 1 - Service test and dependency checks

Run for changed services before a PR:

- Rust `cargo test` for changed Rust services;
- Node `npm run lint` and `npm run build` for changed frontend projects;
- `cargo audit --ignore RUSTSEC-2023-0071` for Rust dependency review.

### Level 2 - Local DB-backed checks

Requires Docker Desktop, PostgreSQL, and Redis:

For dependency failures, readiness checks, or reset decisions, use [Local reset and diagnostics](local-reset-and-diagnostics.md) before considering a destructive reset.

```powershell
cd E:\Kushim
.\scripts\validation\check-local-services.ps1 -Start
```

Then run DB-backed Rust tests with:

```powershell
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
```

If this prerequisite is skipped, DB-backed Rust tests may fail with connection timeouts such as `PoolTimedOut`. Treat that as an environment/preflight failure, not proof that the service logic is broken.

### Level 3 - MVP smoke

Run before an internal demo:

```powershell
cd E:\Kushim
.\scripts\validation\check-local-services.ps1 -Start
.\scripts\demo\backend-e2e.ps1
```

Then start `kushim-app` and do the manual frontend smoke described in `mvp-demo-runbook.md`.

### Level 4 - Manual demo smoke

Manual browser validation remains required for:

- auth/login/handoff;
- dashboard;
- benchmark demo label;
- `Catalogue d'actifs` -> `/assets`;
- asset catalogue and detail;
- positions;
- settings disabled/UI-only actions;
- logout;
- browser console check.

This level is mandatory before a supervised MVP demo. It is not a production-readiness gate.

## Recommended command sets

Before a normal frontend-only commit:

```powershell
cd E:\Kushim
git diff --check
cd E:\Kushim\kushim-app
npm run lint
npm run build
```

Before a Rust service PR:

```powershell
cd E:\Kushim
.\scripts\validation\check-local-services.ps1 -Start
cd E:\Kushim\<rust-service>
cargo fmt --check
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit --ignore RUSTSEC-2023-0071
```

Before a supervised MVP demo:

```powershell
cd E:\Kushim
.\scripts\validation\check-local-services.ps1 -Start
.\scripts\demo\backend-e2e.ps1
cd E:\Kushim\kushim-app
npm run lint
npm run build
npm run dev -- --host 127.0.0.1
```

## Common Rust validation pattern

Use explicit `DATABASE_URL` from the host when needed:

```powershell
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
```

Then run:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
cargo audit --ignore RUSTSEC-2023-0071
```

Important:

- integration tests for Rust services require PostgreSQL reachable from the host at the configured `DATABASE_URL`;
- if Docker Desktop or PostgreSQL is not running, DB-backed tests can fail with connection timeouts even when unit tests and clippy pass;
- initialize the local schema from `infra/postgres/init/001_init.sql` before treating DB-backed test results as meaningful;
- use `.\scripts\validation\check-local-services.ps1 -Start` before DB-backed Rust tests to avoid false failures from missing Docker/PostgreSQL;
- do not print `.env` files or provider API keys while validating.

## `kushim-auth/api`

```powershell
cd E:\Kushim\kushim-auth\api
cargo fmt --check
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
cargo audit --ignore RUSTSEC-2023-0071
```

Docker validation:

```powershell
cd E:\Kushim
docker compose build kushim-auth-api
docker compose up -d --force-recreate database redis kushim-auth-api
curl http://127.0.0.1:3002/health
curl http://127.0.0.1:3002/ready
```

## `kushim-api`

```powershell
cd E:\Kushim\kushim-api
cargo fmt --check
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
cargo audit --ignore RUSTSEC-2023-0071
```

Docker validation:

```powershell
cd E:\Kushim
docker compose build kushim-api
docker compose up -d --force-recreate database kushim-auth-api kushim-api
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/ready
```

## `kushim-worker`

```powershell
cd E:\Kushim\kushim-worker
cargo fmt --check
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
cargo audit --ignore RUSTSEC-2023-0071
```

Docker validation:

```powershell
cd E:\Kushim
docker compose build kushim-worker
docker compose up -d --force-recreate database redis kushim-worker
curl http://127.0.0.1:8081/health
curl http://127.0.0.1:8081/ready
```

Worker smoke examples:

- `rebuild_current_read_models`
- `generate_daily_snapshots`
- `refresh_current_portfolio_state`
- `backfill_daily_snapshots`

## `kushim-market-data`

Current state:

- implemented with `mock` and guarded `finnhub` providers
- jobs: `noop`, `refresh_current_market_data`, `fill_missing_price_history_cache`
- Finnhub current stock quotes are live-validated for AAPL/MSFT/NVDA with a tiny allowlist
- BTC/crypto and Finnhub historical `/stock/candle` are not currently live-validated with the current plan/access
- the reliable MVP demo path uses the mock provider for both current and historical data

Suggested minimal validation:

```powershell
cd E:\Kushim\kushim-market-data
cargo fmt --check
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
cargo audit --ignore RUSTSEC-2023-0071
```

Mock provider job validation, when local Docker/PostgreSQL are running:

```powershell
cd E:\Kushim
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=refresh_current_market_data `
  -e MARKET_DATA_PROVIDER=mock `
  kushim-market-data

docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=fill_missing_price_history_cache `
  -e MARKET_DATA_PROVIDER=mock `
  -e MARKET_DATA_HISTORY_DATE_FROM=2026-06-01 `
  -e MARKET_DATA_HISTORY_DATE_TO=2026-06-03 `
  kushim-market-data
```

Finnhub current stock quote smoke, only when a real `FINNHUB_API_KEY` is present in the ignored local `kushim-market-data/.env` file:

```powershell
cd E:\Kushim
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=refresh_current_market_data `
  -e MARKET_DATA_PROVIDER=finnhub `
  -e MARKET_DATA_SYMBOL_ALLOWLIST=AAPL,MSFT,NVDA `
  kushim-market-data
```

Do not print the key, do not use broad allowlists, and do not treat BTC or `fill_missing_price_history_cache` via Finnhub as validated. BTC mapping exists through `MARKET_DATA_PROVIDER_SYMBOL_MAP`; the tested mapping attempt was `BTC=BINANCE:BTCUSDT`, but the current plan/access returned `403 Forbidden`.

Finnhub validation boundaries:

- AAPL/MSFT/NVDA current quotes are the only live-validated Finnhub path today;
- BTC/crypto is not validated on the current plan/access;
- Finnhub historical `/stock/candle` is not validated on the current plan/access;
- historical MVP/demo backfills should remain on mock/seeded/manual data until provider access is decided.

## Frontends

### `kushim-auth/front`

```powershell
cd E:\Kushim\kushim-auth\front
npm run lint
npm run build
```

### `kushim-app`

```powershell
cd E:\Kushim\kushim-app
npm run lint
npm run build
```

### `kushim-website`

```powershell
cd E:\Kushim\kushim-website
npm run lint
npm run build
```

## Security and dependency review

Known accepted advisory to keep monitoring:

- `RUSTSEC-2023-0071`

Current repository interpretation:

- accepted temporarily where the actual service behavior remains HS256-only or the advisory is transitively present without currently used RSA flows

Required discipline:

- rerun `cargo audit` when Rust dependencies change
- report any new advisory explicitly
- expect plain `cargo audit` to fail while `RUSTSEC-2023-0071` is still present
- a clean result with `cargo audit --ignore RUSTSEC-2023-0071` means no advisory beyond the currently accepted/monitored one was reported
- CI must use `cargo audit --ignore RUSTSEC-2023-0071`, not a blanket advisory bypass

## Existing-database upgrade (automatic refresh / P0)

`001_init.sql` only runs on a fresh PostgreSQL volume. For an existing local
volume, apply the idempotent, non-destructive upgrade scripts (adds
`portfolio_refresh_requests`):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/dev/apply-db-upgrades.ps1
```

Safe to run multiple times; never drops/truncates/deletes data.

## Worker default mode (automatic refresh)

Docker Compose now starts `kushim-worker` as the automatic refresh consumer
(`WORKER_MODE=loop`, `WORKER_JOB=process_portfolio_refresh_requests`). To verify
the runtime path:

- `docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data`
- `docker compose logs kushim-worker --tail 20` should show the loop and
  `process portfolio refresh requests pass` lines
- post an operation; the API response includes `refresh_request`; poll
  `GET /v1/portfolios/{id}/refresh-requests/{id}` until `completed`
- `scripts/demo/backend-e2e.ps1` validates this end-to-end (18 assertions, no
  manual `rebuild_current_read_models` / `generate_daily_snapshots`)

## Documentation-only tasks

If a task modifies only Markdown files:

- verify changed docs exist
- verify they are non-empty
- verify no application code changed by mistake
- verify no DDL changed
- do not rerun full Rust/Docker suites unless accidental code changes occurred
