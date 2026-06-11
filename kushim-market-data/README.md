# kushim-market-data

`kushim-market-data` is the internal Rust service responsible for market-data ingestion and normalization.

## Current status

Status:

- **Foundation scaffolded and validated with job runner architecture**
- **First controlled write job: `refresh_current_market_data` with mock provider**
- **Second controlled write job: `fill_missing_price_history_cache` with mock provider**

What currently exists:

- modular Rust service structure (`config`, `db`, `domain`, `errors`, `health`, `jobs`, `providers`, `repositories`, `runner`, `state`)
- environment-based config loading with validation
- PostgreSQL pool with connectivity check at startup
- `/health` liveness endpoint
- `/ready` readiness endpoint (DB connectivity check via `SELECT 1`)
- explicit job runner with three modes: `idle`, `once`, `loop`
- `noop` job (logs execution, no side effects)
- `refresh_current_market_data` job (reads active assets, fetches quotes from provider, upserts into `asset_market_data`)
- provider abstraction trait (`MarketDataProvider`) with mock implementation
- `fill_missing_price_history_cache` job (reads active assets, fills missing historical prices in `asset_price_history_cache`)
- provider abstraction supports current and historical quotes
- repository layer (`assets`, `asset_market_data`, `price_history_cache`) with SQLx bind parameters
- configurable date range for historical fill (MARKET_DATA_HISTORY_DATE_FROM/TO)
- configurable run interval for loop mode
- CancellationToken-based graceful shutdown (all modes)
- FK violation (23503) resilience for concurrent asset deletion
- structured logging with `tracing` + `RUST_LOG` env-filter
- Docker build with `curl` for healthcheck
- docker-compose wiring with `depends_on database`, health checks, and port mapping
- unit tests for config, health, noop job, runner, mock provider (current + historical)
- integration tests for `refresh_current_market_data` and `fill_missing_price_history_cache` with real PostgreSQL

What does **not** exist yet:

- real provider integrations (external APIs)
- HTTP ingestion endpoints
- asset enrichment workflows
- FX logic
- queue consumption

## Intended responsibility

This service will eventually own:

- external market/provider sync
- current `asset_market_data` refresh
- historical `asset_price_history_cache` fill
- provider payload normalization
- asset enrichment where needed

## What this service must not become

It must not own:

- user-facing portfolio APIs
- auth flows
- `portfolio_operations` writes
- read model generation that belongs to `kushim-worker`
- frontend logic

## Endpoints

| Method | Path      | Auth | Description                          |
|--------|-----------|------|--------------------------------------|
| GET    | `/health` | No   | Liveness — returns `{"status":"ok"}` |
| GET    | `/ready`  | No   | Readiness — checks PostgreSQL via `SELECT 1` |

## Service modes

| Mode   | Behavior                                                    |
|--------|-------------------------------------------------------------|
| `idle` | Starts health server and waits for shutdown. No job runs.   |
| `once` | Executes the selected job once, then exits.                 |
| `loop` | Executes the selected job repeatedly at configured interval.|

## Available jobs

| Job                              | Description                                                                      |
|----------------------------------|----------------------------------------------------------------------------------|
| `noop`                           | Logs execution, performs no DB writes or calls.                                  |
| `refresh_current_market_data`    | Reads active assets, fetches quotes from provider, upserts into `asset_market_data`. |
| `fill_missing_price_history_cache` | Reads active assets, fills missing historical prices in `asset_price_history_cache` for a configured date range. |

## Available providers

| Provider | Description                                                       |
|----------|-------------------------------------------------------------------|
| `mock`   | Deterministic current and historical prices for 7 symbols (AAPL, MSFT, NVDA, BTC, ETH, SPY, VTI). No external calls. Historical prices vary deterministically by date. |

## Configuration

| Variable                            | Required | Default       | Description                     |
|-------------------------------------|----------|---------------|---------------------------------|
| `DATABASE_URL`                      | Yes      | —             | PostgreSQL connection string    |
| `MARKET_DATA_HOST`                  | No       | `0.0.0.0`     | Health server bind address      |
| `MARKET_DATA_PORT`                  | No       | `8082`        | Health server port              |
| `MARKET_DATA_MODE`                  | No       | `idle`        | Service mode: idle, once, loop  |
| `MARKET_DATA_JOB`                   | No       | `noop`        | Job to execute                  |
| `MARKET_DATA_PROVIDER`              | No       | `mock`        | Data provider: mock             |
| `MARKET_DATA_RUN_INTERVAL_SECONDS`  | No       | `300`         | Loop mode interval (seconds)    |
| `MARKET_DATA_HISTORY_DATE_FROM`     | Cond.    | —             | Start date (YYYY-MM-DD), required for `fill_missing_price_history_cache` |
| `MARKET_DATA_HISTORY_DATE_TO`       | Cond.    | —             | End date (YYYY-MM-DD), required for `fill_missing_price_history_cache` |
| `APP_ENV`                           | No       | `development` | Environment label               |
| `RUST_LOG`                          | No       | `info`        | Tracing filter                  |

## Local run

```powershell
cd E:\Kushim\kushim-market-data
copy .env.example .env
cargo run
```

## Docker

```powershell
cd E:\Kushim
docker compose build kushim-market-data
docker compose up -d --force-recreate kushim-market-data
docker compose logs -f kushim-market-data
```

Smoke tests:

```powershell
curl http://localhost:8082/health
curl http://localhost:8082/ready
```

## Validation

```powershell
cd E:\Kushim\kushim-market-data
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
```

## Demo historical backfill (Pass 6)

To populate `asset_price_history_cache` with mock USD prices for a date range:

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=fill_missing_price_history_cache `
  -e MARKET_DATA_PROVIDER=mock `
  -e MARKET_DATA_HISTORY_DATE_FROM=2026-05-10 `
  -e MARKET_DATA_HISTORY_DATE_TO=2026-06-10 `
  kushim-market-data
```

This inserts one row per active asset per day in the range. The mock provider generates deterministic USD prices only. Existing rows are not overwritten (ON CONFLICT DO NOTHING).

After populating the cache, run `kushim-worker` with `backfill_daily_snapshots` to generate historical portfolio snapshots from the cached prices. See `kushim-worker/README.md` for the full procedure.

## MVP note

This service is not part of the currently validated backend MVP core yet.

The current project relies on:

- fixtures
- seeded data
- or manually available current/historical asset rows

until this service is properly implemented.
