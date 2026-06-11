# Docker Local Development

## Purpose

This document gives the current local Docker workflow for Kushim.

It is for development and validation, not for production deployment.

## Main command pattern

Build a service:

```powershell
cd E:\Kushim
docker compose build <service-name>
```

Start or recreate services:

```powershell
docker compose up -d --force-recreate <service-a> <service-b>
```

Important:

- after a rebuild, `--force-recreate` avoids validating stale containers that still run old binaries.

## Core local services

Main Compose services:

- `database`
- `redis`
- `kushim-auth-api`
- `kushim-api`
- `kushim-worker`
- `kushim-market-data`
- `kushim-auth-front`
- `kushim-app`
- `kushim-website`
- `nginx`

## Known ports

Current useful exposed ports:

- PostgreSQL: `5432`
- `kushim-auth-api`: `3002`
- `kushim-api`: `8080`
- `kushim-worker`: `8081`
- nginx: `80`

Frontend ports exist inside Compose but are typically accessed via nginx hostnames.

## Hostnames through nginx

Configured local nginx routing:

- `kushim.localhost` -> `kushim-website`
- `auth.kushim.localhost` -> `kushim-auth-front`
- `app.kushim.localhost` -> `kushim-app`
- `api.kushim.localhost` -> `kushim-api`

## Common health checks

### Auth API

```powershell
curl http://127.0.0.1:3002/health
curl http://127.0.0.1:3002/ready
```

### Main API

```powershell
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/ready
```

### Worker

```powershell
curl http://127.0.0.1:8081/health
curl http://127.0.0.1:8081/ready
```

## Typical backend validation runs

### Auth API

```powershell
docker compose build kushim-auth-api
docker compose up -d --force-recreate database redis kushim-auth-api
```

### Main API

```powershell
docker compose build kushim-api
docker compose up -d --force-recreate database kushim-auth-api kushim-api
```

### Worker

```powershell
docker compose build kushim-worker
docker compose up -d --force-recreate database redis kushim-worker
```

## Worker job examples

### No-op

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=noop `
  kushim-worker
```

### Rebuild current read models

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=rebuild_current_read_models `
  kushim-worker
```

### Generate current daily snapshots

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=generate_daily_snapshots `
  -e WORKER_TARGET_PORTFOLIO_ID=<uuid> `
  -e WORKER_SNAPSHOT_DATE=2026-06-08 `
  kushim-worker
```

### Refresh current portfolio state end-to-end

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=refresh_current_portfolio_state `
  -e WORKER_TARGET_PORTFOLIO_ID=<uuid> `
  -e WORKER_SNAPSHOT_DATE=2026-06-08 `
  kushim-worker
```

### Controlled historical backfill

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=backfill_daily_snapshots `
  -e WORKER_TARGET_PORTFOLIO_ID=<uuid> `
  -e WORKER_BACKFILL_DATE_FROM=2026-06-01 `
  -e WORKER_BACKFILL_DATE_TO=2026-06-03 `
  kushim-worker
```

## Current non-goals in Docker local dev

Local Compose currently does not imply:

- production hardening
- production ingress strategy
- production secrets lifecycle
- production scheduler
- full CI/CD
