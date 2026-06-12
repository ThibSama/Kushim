# Validation Commands

## Purpose

This file centralizes the main validation commands used across Kushim services.

If a task changes only documentation, these commands do not need to be rerun automatically.

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
```

Important:

- integration tests for Rust services require PostgreSQL reachable from the host at the configured `DATABASE_URL`;
- if Docker Desktop or PostgreSQL is not running, DB-backed tests can fail with connection timeouts even when unit tests and clippy pass;
- initialize the local schema from `infra/postgres/init/001_init.sql` before treating DB-backed test results as meaningful;
- do not print `.env` files or provider API keys while validating.

## `kushim-auth/api`

```powershell
cd E:\Kushim\kushim-auth\api
cargo fmt --check
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
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
- a clean result with `cargo audit --ignore RUSTSEC-2023-0071` means no advisory beyond the currently accepted/monitored one was reported

## Documentation-only tasks

If a task modifies only Markdown files:

- verify changed docs exist
- verify they are non-empty
- verify no application code changed by mistake
- verify no DDL changed
- do not rerun full Rust/Docker suites unless accidental code changes occurred
