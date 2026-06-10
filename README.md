# Kushim

Kushim is an investment portfolio tracking and analytics application.

Kushim is **not**:

- a broker;
- a trading execution platform;
- a bank;
- a payment provider;
- a market data vendor.

Kushim helps users:

- centralize portfolios;
- record `portfolio_operations`;
- search and select assets;
- view current summaries and holdings;
- view historical snapshots when they exist;
- audit corrections and portfolio history;
- progressively analyze historical performance and portfolio evolution.

## Current status

Current repository state, at a high level:

- `kushim-auth/api`: implemented and hardened
- `kushim-api`: implemented and validated for the current synchronous MVP perimeter
- `kushim-worker`: implemented for current-state rebuilds, daily snapshots, composite refresh, and first controlled historical backfill
- `kushim-market-data`: implemented with mock provider (two controlled write jobs validated, no real provider yet)
- `kushim-auth/front`: interactive frontend scaffold, not fully wired to the auth API yet
- `kushim-app`: private frontend with advanced mock UI, not fully wired to `kushim-api` yet
- `kushim-website`: marketing website present

Important:

- the backend is significantly more advanced than the frontends;
- the backend E2E chain is **demonstrable locally** via an automated smoke test (`scripts/demo/backend-e2e.ps1`, 18/18 assertions passed);
- the project is **MVP-oriented**, not production-ready;
- some services are fully usable internally, while others are still partially wired.

## Service map

```text
E:/Kushim/
├── kushim-website/       # public marketing website (Next.js)
├── kushim-auth/
│   ├── front/            # auth frontend (Next.js)
│   └── api/              # authentication service (Rust/Axum/SQLx)
├── kushim-app/           # authenticated app (React/Vite)
├── kushim-api/           # main synchronous business API (Rust/Axum/SQLx)
├── kushim-worker/        # worker jobs, rebuilds, snapshots, backfills (Rust)
├── kushim-market-data/   # market-data service with mock provider (Rust)
└── infra/
    ├── postgres/
    ├── redis/
    └── nginx/
```

## Core architecture truth

Critical project rules:

- `portfolio_operations` is the source of truth.
- read models are derived and rebuildable.
- snapshots are derived historical states.
- `asset_price_history_cache` is the deterministic historical price cache.
- `kushim-api` writes user-facing source-of-truth actions and exposes read-only derived data.
- `kushim-api` does **not** generate read models or snapshots.
- `kushim-worker` generates read models, snapshots, and controlled historical backfills.
- `kushim-market-data` is the future service responsible for market provider sync and price cache population.
- PostgreSQL DDL remains the schema source of truth:
  - `infra/postgres/init/001_init.sql`

## Quick local Docker start

```powershell
cd E:\Kushim
docker compose build
docker compose up -d --force-recreate database redis kushim-auth-api kushim-api kushim-worker
```

Useful health checks:

```powershell
curl http://127.0.0.1:3002/health
curl http://127.0.0.1:3002/ready
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/ready
curl http://127.0.0.1:8081/health
curl http://127.0.0.1:8081/ready
```

Why `--force-recreate` matters:

- after rebuilds, it avoids validating stale containers still running older binaries.

## Detailed documentation

Start here:

- [Documentation index](documentation/README.md)
- [Architecture overview](documentation/architecture/overview.md)
- [Service boundaries](documentation/architecture/service-boundaries.md)
- [Data flow](documentation/architecture/data-flow.md)
- [Database architecture](documentation/database/database-architecture.md)
- [Portfolio reconstruction and snapshots](documentation/database/portfolio-reconstruction.md)
- [MVP scope](documentation/mvp/mvp-scope.md)
- [Deferred TODOs](documentation/mvp/deferred-todos.md)
- [Docker local development](documentation/operations/docker-local-dev.md)
- [Validation commands](documentation/operations/validation-commands.md)
- [Backend MVP demo runbook](documentation/operations/backend-demo-e2e.md)
- [Backend E2E smoke test](scripts/demo/README.md)

Current progress reports:

- [MVP progress report (FR)](documentation/reports/kushim-mvp-progress-report.fr.md)
- [MVP progress report (EN)](documentation/reports/kushim-mvp-progress-report.en.md)

Agent guidance for Codex or similar tooling:

- [AGENTS.md](AGENTS.md)

## Known non-production status

The repository is not production-ready yet.

Main reasons:

- `kushim-market-data` uses a mock provider only (no real market-data API);
- frontends are not fully wired to the backend;
- there is no complete CI/CD or deployment strategy visible in the repo;
- observability, production secrets handling, and backup strategy are still incomplete;
- some V1 business calculations intentionally remain conservative.

Note: the backend E2E chain is validated locally via `scripts/demo/backend-e2e.ps1` (18/18 assertions), but this is a local debug/demo smoke test, not a production validation.

This repository is the private project workspace and should be treated as the real working repository, including its documentation.

## Service-specific READMEs

- [kushim-auth/api](kushim-auth/api/README.md)
- [kushim-api](kushim-api/README.md)
- [kushim-worker](kushim-worker/README.md)
- [kushim-market-data](kushim-market-data/README.md)
- [kushim-app](kushim-app/README.md)
- [kushim-auth/front](kushim-auth/front/README.md)
- [kushim-website](kushim-website/README.md)
