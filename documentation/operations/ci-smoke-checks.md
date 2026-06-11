# CI Smoke Checks

_Date : 2026-06-12_

## Workflow

File: `.github/workflows/mvp-smoke.yml`

Triggers: push to `main`, pull requests targeting `main`.

## Jobs

### `kushim-app` (lint + build)

- Node 22, `npm ci`, `npm run lint`, `npm run build`
- Validates frontend TypeScript compilation and ESLint rules

### `kushim-market-data` (fmt + clippy + test)

- Rust stable, PostgreSQL 16 service container
- Schema initialized from `infra/postgres/init/001_init.sql`
- `cargo fmt --check`, `cargo clippy`, `cargo test`
- 68 tests (unit + integration against PostgreSQL)
- Uses mock provider only — no Finnhub calls, no API key required

### `kushim-api` (fmt + clippy + test)

- Same setup as market-data
- ~165 tests (unit + integration against PostgreSQL)
- Tests use a hardcoded dev JWT secret — no `AUTH_JWT_SECRET` env var required

### `kushim-worker` (fmt + clippy + test)

- Same setup as market-data
- ~60 tests (unit + integration against PostgreSQL)

### `kushim-auth-api` (fmt + clippy + test)

- Same setup as market-data
- ~74 tests (unit + integration against PostgreSQL)
- Tests use a hardcoded dev JWT secret — no `AUTH_JWT_SECRET` env var required

## What is intentionally not checked

- **Finnhub provider calls**: CI uses mock provider only. No `FINNHUB_API_KEY` secret is configured.
- **Backend E2E smoke test** (`scripts/demo/backend-e2e.ps1`): requires Docker Compose with all 6+ services built and running, uses `docker compose run` for job execution — deferred until a Docker-based CI pipeline is designed.
- **Frontend E2E**: no browser-based testing.
- **Docker image builds**: not part of the smoke check workflow.
- **Production deployment**: entirely out of scope.
- **Release automation**: not implemented.
- **`cargo audit`**: not included in the smoke workflow to avoid flaky runs from new advisories; should be run periodically or in a separate scheduled workflow.

## Caching

- Rust dependencies are cached via `Swatinem/rust-cache@v2` (per-service workspace).
- Node dependencies are cached via `actions/setup-node@v4` built-in npm cache.

## PostgreSQL in CI

Each Rust service job starts a PostgreSQL 16 service container with:

- user: `kushim`
- password: `kushim_secret_dev`
- database: `kushim`

The full DDL schema is loaded before tests via `psql`. This matches the local development setup.

## Adding new services

To add another Rust service to CI, copy an existing job block and adjust:

- `working-directory`
- `workspaces` in the rust-cache step
- the relative path to `001_init.sql` in the schema init step
