# Docker Local Development

## Purpose

This document gives the current local Docker workflow for Kushim.

It is for development and validation, not for production deployment.

For restart choices, safe diagnostics, existing-volume upgrades, worktree port collisions, and the explicitly destructive local reset procedure, see [Local reset and diagnostics](local-reset-and-diagnostics.md).

## Canonical URLs (Docker vs direct dev)

In Docker the browser must use the nginx domains (the internal ports 3000 /
3001 / 5173 are **not** published). Direct-port URLs are only for running a
service outside Docker. `docker-compose.yml` injects the Docker URLs into each
frontend at runtime (`NEXT_PUBLIC_*` / `VITE_*`) and the canonical browser
origins into each API's `CORS_ALLOWED_ORIGINS`.

| Service | Docker browser URL | Direct dev URL |
|---|---|---|
| Website | http://kushim.localhost | http://localhost:3000 |
| Auth UI | http://auth.kushim.localhost | http://localhost:3001 |
| Auth API | http://auth-api.kushim.localhost | http://localhost:3002 |
| App | http://app.kushim.localhost | http://localhost:5173 |
| Business API | http://api.kushim.localhost | http://localhost:8080 |

`*.kushim.localhost` resolve to `127.0.0.1` automatically (the `.localhost`
TLD). nginx routes each host to the right service; `auth-api.kushim.localhost`
proxies to `kushim-auth-api:3002`.

Auth handoff flow (Docker): landing → `http://auth.kushim.localhost/inscription`
or `/connexion` → auth API → handoff code → redirect to
`http://app.kushim.localhost/?handoff_code=...` → app exchanges the one-time
code against `http://auth-api.kushim.localhost` → tokens stored → `/dashboard`.
Only the short-lived one-time handoff code ever appears in a URL.

## Same-origin service-health endpoints

nginx exposes four read-only health routes on **both** browser hosts
(`http://kushim.localhost` and `http://app.kushim.localhost`). They let the app
and the public status page probe backend readiness same-origin (no CORS, no
token):

| Same-origin path | Proxies to | Service |
|---|---|---|
| `/_health/api` | `http://kushim-api:8080/ready` | Business API |
| `/_health/auth` | `http://kushim-auth-api:3002/ready` | Auth API |
| `/_health/worker` | `http://kushim-worker:8081/ready` | Worker |
| `/_health/market-data` | `http://kushim-market-data:8082/ready` | Market-data |

Each is an **exact** nginx location (`location = /_health/<service>`) that proxies
only to the corresponding `/ready` endpoint — no arbitrary service path can be
reached. Short proxy timeouts (connect 2s, send/read 4s) make an unreachable
upstream surface quickly as a non-2xx (nginx `502`) instead of hanging the
browser probe. No new public ports are published and no CORS headers are added.

**Semantic limitation:** `/ready` means only that the service is reachable and
considers itself ready (it runs its own DB/Redis check). It is **not** a worker
job-processing heartbeat, **not** a market-data freshness signal, and **not** a
provider-availability check. Those are out of scope for this pass.

The public status page lives at `http://kushim.localhost/health`. It shows the
website, auth, API, worker and market-data rows and refreshes ~every 30s.

Quick probe (returns the upstream `/ready` JSON, or `502` when stopped):

```powershell
curl -i http://kushim.localhost/_health/api
curl -i http://app.kushim.localhost/_health/worker
```

Failure simulation (stop a backend, watch the route flip to non-2xx, then
restore):

```powershell
docker compose stop kushim-worker
curl -i http://kushim.localhost/_health/worker   # expect 502
docker compose start kushim-worker
```

When the **API** is stopped, the private app replaces the page with a blocking
"temporairement indisponible" fallback (navbar/theme/footer preserved, session
kept, `Réessayer` re-probes without a reload). When only the **worker** or
**market-data** is stopped, the app stays usable and shows a non-blocking yellow
banner. The public `/health` page reflects the same states.

The app links to the public health page via `VITE_SITE_URL` (Docker:
`http://kushim.localhost`; direct dev defaults to `http://localhost:3000`).

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
