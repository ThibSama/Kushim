# kushim-market-data

`kushim-market-data` is the internal Rust service responsible for market-data ingestion, provider normalization, current market-data refresh, and historical price cache population.

## Current status

Status:

- implemented beyond scaffold level with a validated job runner architecture
- mock provider remains the safest default
- Finnhub.io is available as the first real provider, guarded by an explicit symbol allowlist
- Finnhub current quote refresh has been validated live for AAPL, MSFT, and NVDA in local Docker/dev usage
- BTC has a provider-symbol mapping path, but live validation with the current free Finnhub plan returned `403 Forbidden` for the mapped crypto symbol
- Finnhub historical candle fill is implemented, but `/stock/candle` may require plan access or endpoint entitlement
- no production scheduling, no queue consumer, no FX conversion, and no real-time market-data guarantee

What currently exists:

- modular Rust service structure (`config`, `db`, `domain`, `errors`, `health`, `jobs`, `providers`, `repositories`, `runner`, `state`)
- environment-based config loading with validation
- PostgreSQL pool with connectivity check at startup
- `/health` liveness endpoint
- `/ready` readiness endpoint using `SELECT 1`
- explicit job runner with three modes: `idle`, `once`, `loop`
- `noop` job with no side effects
- `refresh_current_market_data` job that reads active assets and upserts into `asset_market_data`
- `fill_missing_price_history_cache` job that reads active assets and inserts missing rows into `asset_price_history_cache`
- provider abstraction with `mock` and `finnhub` implementations
- Finnhub quote endpoint integration for current prices
- Finnhub daily candle endpoint integration for historical close prices
- explicit allowlist filtering for Finnhub before provider calls
- repository layer with SQLx bind parameters
- configurable historical date range with a 366-day maximum
- configurable loop interval, HTTP timeout, and provider delay
- graceful shutdown with `CancellationToken`
- FK violation (`23503`) resilience for concurrent asset deletion
- structured logging with `tracing` and `RUST_LOG`
- unit tests for config, health, runner, providers, parsing, price conversion, and allowlist selection
- integration tests for market-data jobs against PostgreSQL

What does not exist:

- FX logic or currency conversion
- broad asset enrichment workflows
- queue consumption
- distributed locks
- production scheduler
- production observability guarantees

## Service boundaries

This service owns:

- external market/provider sync
- current `asset_market_data` refresh
- historical `asset_price_history_cache` fill
- provider payload normalization

It must not own:

- user-facing portfolio APIs
- auth flows
- `portfolio_operations` writes
- read-model generation owned by `kushim-worker`
- portfolio reconstruction
- frontend logic

## Endpoints

| Method | Path      | Auth | Description |
|--------|-----------|------|-------------|
| GET    | `/health` | No   | Liveness, returns `{"status":"ok"}` |
| GET    | `/ready`  | No   | Readiness, checks PostgreSQL via `SELECT 1` |

## Service modes

| Mode   | Behavior |
|--------|----------|
| `idle` | Starts health server and waits for shutdown. No job runs. |
| `once` | Executes the selected job once, then exits. |
| `loop` | Executes the selected job repeatedly at the configured interval. |

## Jobs

| Job | Description |
|-----|-------------|
| `noop` | Logs execution, performs no DB writes or provider calls. |
| `refresh_current_market_data` | Reads active assets, applies provider filtering when configured, fetches current quotes, and upserts `asset_market_data`. |
| `fill_missing_price_history_cache` | Reads active assets, applies provider filtering when configured, fetches daily close prices for the configured range, and inserts missing rows into `asset_price_history_cache`. |

Historical cache inserts keep the existing architecture: `ON CONFLICT DO NOTHING`; existing rows are not overwritten.

## Providers

| Provider | Description |
|----------|-------------|
| `mock` | Default provider. Deterministic current and historical USD prices for AAPL, MSFT, NVDA, BTC, ETH, SPY, and VTI. No external calls. |
| `finnhub` | Real provider using Finnhub quote and daily candle endpoints. Requires an API key and `MARKET_DATA_SYMBOL_ALLOWLIST`. Current quotes are validated live for AAPL, MSFT, and NVDA. BTC mapping exists but depends on crypto endpoint access. Historical candles depend on Finnhub plan/entitlement. |

Finnhub notes:

- requests are made one symbol at a time
- current quotes use Finnhub `/quote`
- historical prices use Finnhub `/stock/candle` with daily resolution and daily close
- `/stock/candle` can return `403 Forbidden` on plans or keys without historical candle entitlement; this is treated as a provider access limitation, not as mock fallback
- canonical Kushim symbols stay unchanged in `assets`; provider-specific request symbols can be configured with `MARKET_DATA_PROVIDER_SYMBOL_MAP`
- BTC is represented canonically as `BTC` in Kushim; use a provider-specific mapping such as `BTC=BINANCE:BTCUSDT` for Finnhub crypto requests
- with the current tested free Finnhub plan, mapped BTC current refresh returned `403 Forbidden`; do not treat BTC as live-validated until the key/plan allows the mapped crypto symbol
- prices are converted deterministically to minor units (cents)
- `data_source` / `source` is `finnhub`
- currency is stored as `USD`; no FX conversion is performed
- provider timestamps are used when available for current quotes; otherwise the service uses current UTC time
- rate-limit and provider error responses are handled as typed provider errors and are not retried in this MVP pass

Recommended MVP/dev usage:

- use `MARKET_DATA_PROVIDER=finnhub` for current quote refresh with a tiny allowlist, for example `AAPL`
- use `MARKET_DATA_PROVIDER_SYMBOL_MAP=BTC=BINANCE:BTCUSDT` when testing BTC current quotes through Finnhub, but expect possible `403 Forbidden` on plans without crypto quote access
- keep using `mock` or seeded historical data for `asset_price_history_cache` unless Finnhub historical access is confirmed for the key/plan
- do not treat a `403 Forbidden` from `/stock/candle` as a successful historical backfill

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | - | PostgreSQL connection string |
| `MARKET_DATA_HOST` | No | `0.0.0.0` | Health server bind address |
| `MARKET_DATA_PORT` | No | `8082` | Health server port |
| `MARKET_DATA_MODE` | No | `idle` | Service mode: `idle`, `once`, `loop` |
| `MARKET_DATA_JOB` | No | `noop` | Job to execute |
| `MARKET_DATA_PROVIDER` | No | `mock` | Provider: `mock`, `finnhub` |
| `MARKET_DATA_RUN_INTERVAL_SECONDS` | No | `300` | Loop mode interval in seconds |
| `MARKET_DATA_HISTORY_DATE_FROM` | Conditional | - | Start date `YYYY-MM-DD`, required for `fill_missing_price_history_cache` |
| `MARKET_DATA_HISTORY_DATE_TO` | Conditional | - | End date `YYYY-MM-DD`, required for `fill_missing_price_history_cache` |
| `FINNHUB_API_KEY` | Conditional | - | Required only when `MARKET_DATA_PROVIDER=finnhub`; never log or commit a real key |
| `FINNHUB_BASE_URL` | No | `https://finnhub.io/api/v1` | Override for tests/local mock servers |
| `MARKET_DATA_SYMBOL_ALLOWLIST` | Conditional | - | Required for Finnhub. Comma-separated symbols, for example `AAPL,MSFT,NVDA` |
| `MARKET_DATA_PROVIDER_SYMBOL_MAP` | No | - | Optional comma-separated canonical-to-provider mapping, for example `BTC=BINANCE:BTCUSDT` |
| `MARKET_DATA_HTTP_TIMEOUT_SECONDS` | No | `10` | HTTP timeout for provider calls |
| `MARKET_DATA_PROVIDER_DELAY_MS` | No | `1100` | Delay before each provider request for quota protection |
| `APP_ENV` | No | `development` | Environment label |
| `RUST_LOG` | No | `info` | Tracing filter |

For `finnhub`, startup fails fast if `FINNHUB_API_KEY` is missing, still set to `change_me`, or if `MARKET_DATA_SYMBOL_ALLOWLIST` is missing/blank.

## Local secrets file

`kushim-market-data/.env` is a local-only file ignored by Git. It is intended for developer secrets and local overrides.

Keep the default safe startup values unless you are explicitly validating the live provider:

- keep `MARKET_DATA_PROVIDER=mock` for normal local Docker/dev startup
- set `FINNHUB_API_KEY=...` locally when you want live Finnhub validation
- switch to `MARKET_DATA_PROVIDER=finnhub` only for that validation
- keep `MARKET_DATA_SYMBOL_ALLOWLIST` tiny, for example `AAPL`
- never commit `.env`

The committed `.env.example` contains placeholders only. Do not put real API keys in `.env.example`, README files, or Docker Compose files.

## Local run

```powershell
cd E:\Kushim\kushim-market-data
cargo run
```

## Docker

```powershell
cd E:\Kushim
docker compose build kushim-market-data
docker compose up -d --force-recreate database kushim-market-data
docker compose logs -f kushim-market-data
```

Health checks:

```powershell
curl http://localhost:8082/health
curl http://localhost:8082/ready
```

## Job examples

Mock current refresh:

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=refresh_current_market_data `
  -e MARKET_DATA_PROVIDER=mock `
  kushim-market-data
```

Finnhub current refresh for validated stock symbols:

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=refresh_current_market_data `
  -e MARKET_DATA_PROVIDER=finnhub `
  -e MARKET_DATA_SYMBOL_ALLOWLIST=AAPL,MSFT,NVDA `
  kushim-market-data
```

This command expects a real `FINNHUB_API_KEY` to be present in the local ignored `kushim-market-data/.env` file.

Finnhub current refresh for BTC:

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=refresh_current_market_data `
  -e MARKET_DATA_PROVIDER=finnhub `
  -e MARKET_DATA_SYMBOL_ALLOWLIST=BTC `
  -e MARKET_DATA_PROVIDER_SYMBOL_MAP=BTC=BINANCE:BTCUSDT `
  kushim-market-data
```

The DB asset remains `symbol=BTC`; the mapping only changes the provider request symbol.
If this command logs `Finnhub endpoint access forbidden`, the key/plan likely does not allow the mapped crypto quote. Do not use the raw `BTC` quote as a substitute for Bitcoin unless provider behavior is explicitly verified.

Finnhub historical fill for a tiny AAPL date range:

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=fill_missing_price_history_cache `
  -e MARKET_DATA_PROVIDER=finnhub `
  -e MARKET_DATA_SYMBOL_ALLOWLIST=AAPL `
  -e MARKET_DATA_HISTORY_DATE_FROM=2026-06-01 `
  -e MARKET_DATA_HISTORY_DATE_TO=2026-06-03 `
  kushim-market-data
```

Keep Finnhub ranges and allowlists small. Quotas and rate limits depend on the Finnhub account plan.

If this command logs `Finnhub historical candles access forbidden`, the key/plan likely does not allow `/stock/candle`. In that case, keep historical MVP/demo backfills on mock or seeded data until provider access is available.

## Validation

```powershell
cd E:\Kushim\kushim-market-data
cargo fmt --check
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
```

## MVP note

This service is suitable for supervised internal MVP/demo use with the mock provider or a tightly controlled Finnhub allowlist. It is not production-ready.
