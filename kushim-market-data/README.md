# kushim-market-data

`kushim-market-data` is the future internal Rust service responsible for market-data ingestion and normalization.

## Current status

Status:

- **Scaffolded**

What currently exists:

- a minimal Rust binary
- tracing initialization
- a simple polling stub loop
- Docker build wiring

What does **not** exist yet:

- provider integrations
- HTTP ingestion endpoints
- writes to `asset_market_data`
- writes to `asset_price_history_cache`
- asset enrichment workflows
- health or readiness endpoints

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

## MVP note

This service is not part of the currently validated backend MVP core yet.

The current project relies on:

- fixtures
- seeded data
- or manually available current/historical asset rows

until this service is properly implemented.
