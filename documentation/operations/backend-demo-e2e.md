# Backend MVP Demo Runbook (E2E)

## Purpose

This document is a step-by-step backend-only runbook for demonstrating the Kushim MVP chain end to end.

It validates that the four backend services work together in sequence:

```
kushim-auth/api  →  kushim-api  →  kushim-market-data  →  kushim-worker  →  kushim-api (read endpoints)
```

The full chain proves:

- user signup and JWT-based authentication;
- portfolio creation and operation management;
- market data ingestion via a controlled mock provider;
- read model rebuild from source-of-truth operations;
- daily snapshot generation from read models;
- historical snapshot backfill from operations and price history cache;
- API consumption of derived data (summary, holdings, snapshots).

## What this does not prove

- **No frontend is involved.** All interactions are via HTTP requests in PowerShell.
- **No real market-data provider is used.** Only the mock provider is available.
- **No FX conversion is tested.** All prices are USD, no cross-currency logic is exercised.
- **No production deployment is validated.** This runs in local Docker Compose only.
- **No broker or trading behavior exists.** Kushim is a portfolio tracker, not a broker.

## Prerequisites

### Infrastructure

- Docker Desktop running.
- Repository cloned to local development environment.
- All backend services built and started:

| Service | Compose name | Port | Role |
|---|---|---|---|
| PostgreSQL | `database` | 5432 | Schema and data storage |
| Redis | `redis` | 6379 (internal) | Rate limiting, worker state |
| kushim-auth/api | `kushim-auth-api` | 3002 | Authentication (signup, login, JWT) |
| kushim-api | `kushim-api` | 8080 | User-facing business API |
| kushim-worker | `kushim-worker` | 8081 | Read models, snapshots, backfill jobs |
| kushim-market-data | `kushim-market-data` | 8082 | Market data ingestion jobs |

### Database state

- PostgreSQL schema initialized (DDL V3 via `infra/postgres/init/001_init.sql`).
- Role `user` (id_role=1) must exist in the `roles` table. It is created by the DDL init script.
- At least one mock-supported asset must exist in the `assets` table, or will be seeded during the demo.

### Service modes

- `kushim-worker` and `kushim-market-data` should be running in **idle** mode (default Docker Compose configuration). They will be invoked in **once** mode via `docker compose run --rm` for each job step.

## Mock provider constraints

The mock provider is deterministic and supports exactly 7 symbols:

| Symbol | Current price (minor) | Currency | Asset type |
|---|---|---|---|
| AAPL | 19,523 | USD | equity |
| MSFT | 42,150 | USD | equity |
| NVDA | 87,640 | USD | equity |
| BTC | 670,000,000 | USD | crypto |
| ETH | 350,000 | USD | crypto |
| SPY | 52,830 | USD | ETF |
| VTI | 26,410 | USD | ETF |

Important constraints:

- The mock provider writes **USD prices only**, for both current (`asset_market_data`) and historical (`asset_price_history_cache`) data.
- Historical prices vary deterministically by date (not random, not constant).
- The current persistent database may not contain all 7 assets. Test-created assets may exist with these symbols but are test artifacts.
- **Demo portfolio must use `base_currency = "USD"`** for non-estimated valuations.
- **Demo asset must use `native_currency = "USD"`** to match mock provider output.
- If portfolio or asset currency does not match, holdings will show `market_value_minor = 0` and `is_estimated = true`.

## Safe demo data policy

- Use a clearly named demo user (e.g., `demo_e2e_user`, `demo_jury_user`).
- Use a clearly named demo portfolio (e.g., `"E2E Demo Portfolio"`).
- Use a fresh UUID for the demo asset if seeding manually.
- Do not wipe the database. Do not run `TRUNCATE` or `DELETE FROM` on shared tables.
- Do not rely on test-created data from `cargo test` runs for a clean demo. Those rows may be cleaned up or have unpredictable names.
- Each demo run should create its own user and portfolio. Reusing a `username` that already exists will produce a 409 conflict.

---

## Demo runbook

### Step A: Verify infrastructure

```powershell
(Invoke-WebRequest -Uri "http://localhost:3002/health" -UseBasicParsing).Content
(Invoke-WebRequest -Uri "http://localhost:8080/health" -UseBasicParsing).Content
(Invoke-WebRequest -Uri "http://localhost:8081/health" -UseBasicParsing).Content
(Invoke-WebRequest -Uri "http://localhost:8082/health" -UseBasicParsing).Content
```

All four should return `{"status":"ok",...}`.

If any service is not running:

```powershell
docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data
```

### Step B: Signup demo user

```powershell
$signupBody = '{"username":"demo_e2e_user","password":"DemoP@ss2026!"}'
$signupResponse = Invoke-WebRequest `
  -Uri "http://localhost:3002/auth/signup" `
  -Method POST `
  -ContentType "application/json" `
  -Body $signupBody `
  -UseBasicParsing
$signupData = $signupResponse.Content | ConvertFrom-Json
$signupData | ConvertTo-Json -Depth 5
```

If the `username` already exists, you will get a 409 conflict. Change the username (e.g., `demo_e2e_user_2`).

### Step C: Store access token

```powershell
$token = $signupData.access_token
$userId = $signupData.user.id_user
$headers = @{ Authorization = "Bearer $token" }

# Verify token works
(Invoke-WebRequest -Uri "http://localhost:8080/v1/me" -Headers $headers -UseBasicParsing).Content
```

The access token is valid for 15 minutes (900 seconds). If it expires during the demo, re-authenticate:

```powershell
$loginBody = '{"username":"demo_e2e_user","password":"DemoP@ss2026!"}'
$loginResponse = Invoke-WebRequest `
  -Uri "http://localhost:3002/auth/login" `
  -Method POST `
  -ContentType "application/json" `
  -Body $loginBody `
  -UseBasicParsing
$loginData = $loginResponse.Content | ConvertFrom-Json
$token = $loginData.access_token
$headers = @{ Authorization = "Bearer $token" }
```

### Step D: Create USD portfolio

```powershell
$portfolioBody = '{"name":"E2E Demo Portfolio","base_currency":"USD"}'
$portfolioResponse = Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios" `
  -Method POST `
  -ContentType "application/json" `
  -Headers $headers `
  -Body $portfolioBody `
  -UseBasicParsing
$portfolioData = ($portfolioResponse.Content | ConvertFrom-Json).portfolio
$portfolioId = $portfolioData.id_portfolio
Write-Host "Portfolio ID: $portfolioId"
```

The portfolio must use `base_currency = "USD"`. Using EUR will cause all holdings to be estimated.

### Step E: Seed demo AAPL asset

Check if a clean AAPL asset already exists:

```powershell
docker exec kushim_database psql -U kushim -d kushim -c "SELECT id_asset, symbol, name, status, native_currency FROM assets WHERE symbol = 'AAPL' AND status = 'active' AND native_currency = 'USD' LIMIT 1"
```

If a suitable row exists, store its `id_asset`:

```powershell
$assetId = "<paste the id_asset UUID from the query above>"
```

If no suitable row exists, insert one:

```powershell
$assetId = [guid]::NewGuid().ToString()
docker exec kushim_database psql -U kushim -d kushim -c "INSERT INTO assets (id_asset, asset_class, status, name, native_currency, symbol, ticker, exchange) VALUES ('$assetId', 'equity', 'active', 'Apple Inc. (Demo)', 'USD', 'AAPL', 'AAPL', 'NASDAQ')"
Write-Host "Asset ID: $assetId"
```

Note: if an AAPL symbol already exists from test runs, inserting a second row is allowed (the DDL does not enforce symbol uniqueness). Use your newly inserted `id_asset` in subsequent steps.

### Step F: Create and post deposit operation

Create a deposit of 10,000.00 USD (= 1,000,000 minor units):

```powershell
$depositBody = @{
    operation_type = "deposit"
    executed_at = "2026-06-01T10:00:00Z"
    gross_amount_minor = 1000000
    cash_amount_minor = 1000000
    currency = "USD"
    metadata = @{}
} | ConvertTo-Json

$depositResponse = Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios/$portfolioId/operations" `
  -Method POST `
  -ContentType "application/json" `
  -Headers $headers `
  -Body $depositBody `
  -UseBasicParsing
$depositId = ($depositResponse.Content | ConvertFrom-Json).operation.id_portfolio_operation
Write-Host "Deposit ID: $depositId"
```

Post the deposit:

```powershell
Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios/$portfolioId/operations/$depositId/post" `
  -Method POST `
  -Headers $headers `
  -UseBasicParsing | Select-Object -ExpandProperty Content
```

### Step G: Create and post buy operation

Buy 10 AAPL at 195.23 USD each (= 19,523 minor units per share, total 195,230 minor units):

```powershell
$buyBody = @{
    id_asset = $assetId
    operation_type = "buy"
    executed_at = "2026-06-02T14:00:00Z"
    quantity = "10.0000000000"
    price_minor = 19523
    gross_amount_minor = 195230
    cash_amount_minor = 195230
    currency = "USD"
    metadata = @{}
} | ConvertTo-Json

$buyResponse = Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios/$portfolioId/operations" `
  -Method POST `
  -ContentType "application/json" `
  -Headers $headers `
  -Body $buyBody `
  -UseBasicParsing
$buyId = ($buyResponse.Content | ConvertFrom-Json).operation.id_portfolio_operation
Write-Host "Buy ID: $buyId"
```

Post the buy:

```powershell
Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios/$portfolioId/operations/$buyId/post" `
  -Method POST `
  -Headers $headers `
  -UseBasicParsing | Select-Object -ExpandProperty Content
```

### Step H: Run market-data — refresh current market data

This job reads all active assets, fetches current quotes from the mock provider, and upserts into `asset_market_data`.

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=refresh_current_market_data `
  -e MARKET_DATA_PROVIDER=mock `
  kushim-market-data
```

Expected log output includes `refresh_current_market_data completed` with `updated` and `skipped` counts.

After this step, the demo AAPL asset has a row in `asset_market_data` with `price_minor = 19523, currency = "USD"`.

### Step I: Run market-data — fill missing price history cache

This job fills `asset_price_history_cache` with deterministic historical prices for all active mock-supported assets, for the specified date range.

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=fill_missing_price_history_cache `
  -e MARKET_DATA_PROVIDER=mock `
  -e MARKET_DATA_HISTORY_DATE_FROM=2026-06-01 `
  -e MARKET_DATA_HISTORY_DATE_TO=2026-06-09 `
  kushim-market-data
```

Expected log output includes `fill_missing_price_history_cache completed` with `inserted` and `already_present` counts.

### Step J: Run worker — rebuild current read models

This job replays all posted operations for the target portfolio, looks up current prices from `asset_market_data`, and writes `rm_portfolio_summary` and `rm_portfolio_holdings`.

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=rebuild_current_read_models `
  -e WORKER_TARGET_PORTFOLIO_ID=$portfolioId `
  kushim-worker
```

Expected log output includes `rebuilt current portfolio read models` with `holdings_count = 1`.

### Step K: Run worker — generate daily snapshot

This job snapshots the current read models into `portfolio_snapshots_daily` and `portfolio_holding_snapshot_daily` for the specified date.

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=generate_daily_snapshots `
  -e WORKER_TARGET_PORTFOLIO_ID=$portfolioId `
  -e WORKER_SNAPSHOT_DATE=2026-06-09 `
  kushim-worker
```

Expected log output includes `generated daily snapshot from current read models`.

### Step L: Run worker — backfill daily snapshots

This job independently replays operations through each historical date, uses `asset_price_history_cache` for pricing, and writes historical snapshots.

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=backfill_daily_snapshots `
  -e WORKER_TARGET_PORTFOLIO_ID=$portfolioId `
  -e WORKER_BACKFILL_DATE_FROM=2026-06-01 `
  -e WORKER_BACKFILL_DATE_TO=2026-06-08 `
  kushim-worker
```

Expected log output includes `completed historical daily snapshot backfill job` with `snapshots_written = 8`.

Notes:

- Dates before portfolio creation (2026-06-01 in this demo) are skipped automatically.
- Missing historical prices for a held asset produce `is_estimated = true` on the snapshot.
- Maximum backfill range: 366 days.

### Step M: API verification

If the access token has expired, re-authenticate first (see Step C).

#### Portfolio summary

```powershell
(Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios/$portfolioId/summary" `
  -Headers $headers `
  -UseBasicParsing).Content | ConvertFrom-Json | ConvertTo-Json -Depth 5
```

#### Portfolio holdings

```powershell
(Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios/$portfolioId/holdings" `
  -Headers $headers `
  -UseBasicParsing).Content | ConvertFrom-Json | ConvertTo-Json -Depth 5
```

#### Daily snapshots list

```powershell
(Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios/$portfolioId/snapshots/daily" `
  -Headers $headers `
  -UseBasicParsing).Content | ConvertFrom-Json | ConvertTo-Json -Depth 5
```

#### Specific snapshot holdings

```powershell
(Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios/$portfolioId/snapshots/daily/2026-06-09/holdings" `
  -Headers $headers `
  -UseBasicParsing).Content | ConvertFrom-Json | ConvertTo-Json -Depth 5
```

#### Operations list

```powershell
(Invoke-WebRequest `
  -Uri "http://localhost:8080/v1/portfolios/$portfolioId/operations" `
  -Headers $headers `
  -UseBasicParsing).Content | ConvertFrom-Json | ConvertTo-Json -Depth 5
```

### Step N: Reset services to idle

The `docker compose run --rm` pattern creates ephemeral containers that exit after completion. The main idle-mode containers from `docker compose up` continue running independently.

Verify the resident containers are still in idle mode:

```powershell
(Invoke-WebRequest -Uri "http://localhost:8081/health" -UseBasicParsing).Content
(Invoke-WebRequest -Uri "http://localhost:8082/health" -UseBasicParsing).Content
```

If for any reason you need to restart them explicitly:

```powershell
docker compose up -d --force-recreate kushim-worker kushim-market-data
```

This restarts them with the default Docker Compose configuration (`WORKER_MODE=idle`, `MARKET_DATA_MODE=idle`).

---

## Expected results

### Canonical demo values

| Item | Value |
|---|---|
| Deposit | 1,000,000 minor = 10,000.00 USD |
| Buy | 10 AAPL at 19,523 minor = 1,952.30 USD total |
| Cash after buy | 804,770 minor = 8,047.70 USD |
| AAPL holding market value | 195,230 minor (10 x 19,523) |
| Total portfolio value | 1,000,000 minor |
| Total invested | 1,000,000 minor |
| Current PnL | 0 (buy price = mock current price) |
| `is_estimated` | `false` (USD portfolio, USD asset, USD prices) |

### Expected API responses after worker runs

| Endpoint | `data_available` | Key observations |
|---|---|---|
| `GET /v1/portfolios/{id}/summary` | `true` | `cash_balance_minor = 804770`, `total_value_minor = 1000000`, `portfolio_status = "active"` |
| `GET /v1/portfolios/{id}/holdings` | `true` | 1 holding: AAPL, `quantity = "10.0000000000"`, `market_value_minor = 195230` |
| `GET /v1/portfolios/{id}/snapshots/daily` | `true` | 9 snapshots (Jun 1 through Jun 9) |
| `GET /v1/portfolios/{id}/snapshots/daily/2026-06-01/holdings` | `true` | 0 holdings (deposit only, no buy yet) |
| `GET /v1/portfolios/{id}/snapshots/daily/2026-06-09/holdings` | `true` | 1 holding: AAPL with historical mock price |

### Before worker runs

| Endpoint | `data_available` | `reason` |
|---|---|---|
| `GET /v1/portfolios/{id}/summary` | `false` | `"read_model_missing"` |
| `GET /v1/portfolios/{id}/holdings` | `false` | `"read_model_missing"` |
| `GET /v1/portfolios/{id}/snapshots/daily` | `false` | — (empty list) |

---

## Troubleshooting

### Token expired

**Symptom:** API returns 401 Unauthorized.

**Cause:** Access token TTL is 900 seconds (15 minutes).

**Fix:** Re-authenticate via login (see Step C).

### Asset missing

**Symptom:** Buy operation creation returns 400 or references an unknown `id_asset`.

**Cause:** The `id_asset` UUID does not exist in the `assets` table.

**Fix:** Verify the asset was seeded (Step E). Check with:

```powershell
docker exec kushim_database psql -U kushim -d kushim -c "SELECT id_asset, symbol, status FROM assets WHERE id_asset = '<uuid>'"
```

### Asset currency mismatch

**Symptom:** After worker rebuild, holdings show `market_value_minor = 0` and `is_estimated = true`.

**Cause:** The asset's `native_currency` or the mock provider's price currency does not match the portfolio's `base_currency`.

**Fix:** Ensure the demo portfolio uses `base_currency = "USD"` and the demo asset uses `native_currency = "USD"`.

### Portfolio currency not USD

**Symptom:** Same as currency mismatch above, globally.

**Cause:** Portfolio was created with `base_currency = "EUR"` or other.

**Fix:** Create a new portfolio with `"base_currency": "USD"`.

### Market-data still in idle mode

**Symptom:** No rows appear in `asset_market_data` after expecting a refresh.

**Cause:** The market-data job was not executed, or was run in idle mode instead of once mode.

**Fix:** Re-run Step H using `docker compose run --rm` with the correct env vars.

### Worker still in idle mode

**Symptom:** `data_available = false` on summary/holdings endpoints after expecting a rebuild.

**Cause:** The worker rebuild job was not executed.

**Fix:** Re-run Step J using `docker compose run --rm`.

### No read model summary

**Symptom:** `data_available = false` with `reason = "read_model_missing"`.

**Cause:** Worker `rebuild_current_read_models` was never run for this portfolio.

**Fix:** Run Step J.

### No snapshots

**Symptom:** Snapshots endpoint returns empty list.

**Cause:** Neither `generate_daily_snapshots` nor `backfill_daily_snapshots` was run.

**Fix:** Run Step K and/or Step L.

### Historical holdings estimated

**Symptom:** Historical snapshot holdings show `is_estimated = true`.

**Cause:** `asset_price_history_cache` does not contain a row for the held asset on the snapshot date with matching currency.

**Fix:** Ensure Step I (`fill_missing_price_history_cache`) was run with a date range covering the snapshot dates, and that the asset has a mock-supported symbol.

### Docker Compose env var overrides not applied

**Symptom:** Job runs as `noop` or `idle` instead of the expected job.

**Cause:** Using `docker compose up -e` which is not valid syntax. Or using `$env:VAR` overrides with `docker compose up` but the `docker-compose.yml` hardcodes the env values (not using `${VAR:-default}` syntax).

**Fix:** Use `docker compose run --rm -e VAR=value` for once-mode jobs. This creates a new ephemeral container with the specified environment, independent of the compose file defaults.

### Duplicate demo user

**Symptom:** Signup returns 409 Conflict.

**Cause:** The `username` is already taken.

**Fix:** Use a different `username` (e.g., `demo_e2e_user_2`, `demo_jury_20260609`).

---

## Cleanup and reset

### Default state

After the demo, the system should be in this state:

- `kushim-worker` running in idle mode (default compose config).
- `kushim-market-data` running in idle mode (default compose config).
- Demo data (user, portfolio, operations, read models, snapshots) remains in the database.

### Re-running the demo

Each new demo run should use:

- a new `username` for the demo user;
- a new portfolio (automatically gets a new UUID);
- optionally a new asset, or reuse the same one via its `id_asset`.

### Manual cleanup

If you need to clean up demo data, this must be done manually by someone who understands the schema. There are no automated cleanup scripts. The general cleanup order (respecting foreign keys) would be:

1. `portfolio_holding_snapshot_daily` (by snapshot IDs)
2. `portfolio_snapshots_daily` (by portfolio ID)
3. `rm_portfolio_holdings` (by portfolio ID)
4. `rm_portfolio_summary` (by portfolio ID)
5. `portfolio_operations` (by portfolio ID, only pending/cancelled; posted operations are protected by trigger)
6. `portfolios` (soft-delete via `deleted_at`, or hard-delete if no posted operations reference it)

Do not run bulk `DELETE` or `TRUNCATE` on shared tables.

---

## Automated script

An automated PowerShell script implements this runbook:

- **`scripts/demo/backend-e2e.ps1`** — executes the full chain automatically with unique demo identifiers, Docker job execution, and API verification assertions.
- See `scripts/demo/README.md` for usage and parameters.

```powershell
.\scripts\demo\backend-e2e.ps1              # full run
.\scripts\demo\backend-e2e.ps1 -VerboseJson # with JSON output
.\scripts\demo\backend-e2e.ps1 -DryRun      # health check only
```

## Future automation

This runbook and script can later evolve into:

- **A CI smoke test** — a headless version that runs after Docker build, creates ephemeral demo data, verifies all endpoints, and reports pass/fail.
- **A test fixture module** — reusable Rust test helpers that set up the full chain in integration tests.

---

## Reference

### Service execution order

```
1. database + redis                                    (must be running)
2. kushim-auth-api                                     (must be running)
3. kushim-api                                          (must be running)
4. [DB seed: demo asset]                               (manual, one-time)
5. [API: signup -> portfolio -> operations -> post]     (Steps B-G)
6. kushim-market-data: refresh_current_market_data      (Step H)
7. kushim-market-data: fill_missing_price_history_cache (Step I)
8. kushim-worker: rebuild_current_read_models           (Step J)
9. kushim-worker: generate_daily_snapshots              (Step K)
10. kushim-worker: backfill_daily_snapshots             (Step L)
11. [API: read verification]                            (Step M)
```

Steps 6-7 are independent of each other (they write to different tables).
Step 8 must come before Step 9 (snapshots read from read models).
Step 10 is independent of Steps 8-9 (backfill replays from source operations + price cache directly).

### Key tables involved

| Table | Written by | Read by |
|---|---|---|
| `users` | kushim-auth/api | kushim-auth/api, kushim-api |
| `portfolios` | kushim-api | kushim-api, kushim-worker |
| `portfolio_operations` | kushim-api | kushim-worker |
| `assets` | DB seed / future admin | kushim-api, kushim-worker, kushim-market-data |
| `asset_market_data` | kushim-market-data | kushim-worker (rebuild) |
| `asset_price_history_cache` | kushim-market-data | kushim-worker (backfill) |
| `rm_portfolio_summary` | kushim-worker | kushim-api |
| `rm_portfolio_holdings` | kushim-worker | kushim-api, kushim-worker (snapshot) |
| `portfolio_snapshots_daily` | kushim-worker | kushim-api |
| `portfolio_holding_snapshot_daily` | kushim-worker | kushim-api |
