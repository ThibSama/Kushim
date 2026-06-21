# Kushim MVP Scope

## Purpose

This document clarifies what the current Kushim MVP really is, what is already usable, and what remains incomplete.

## Product goal

The MVP goal is not to build a broker or a complete wealth platform.

The MVP goal is to prove that Kushim can:

- authenticate users without email-centric banking assumptions;
- record portfolio operations as source-of-truth ledger entries;
- rebuild current derived portfolio state;
- generate daily snapshots;
- expose current and historical views through a clean API;
- support basic auditability of corrections;
- prepare the path for richer historical analytics later.

## What is implemented and validated

### Backend core

Implemented and validated:

- PostgreSQL V3 DDL
- auth service backend
- synchronous business API backend
- worker current-state rebuild
- worker daily snapshot generation
- worker composite current refresh
- worker first historical backfill V1
- automatic portfolio refresh (P0): durable `portfolio_refresh_requests` queue
  enqueued atomically on operation posting, consumed by the worker loop job
  `process_portfolio_refresh_requests` (PostgreSQL-only, no external queue)

### Data model

Implemented and validated:

- `portfolio_operations` source of truth
- corrections via `adjustment + id_corrected_operation`
- read models
- daily snapshots
- historical price cache table

### Security baseline

Implemented and validated:

- JWT access/refresh split
- refresh revocation
- Redis auth rate limiting
- normalized API errors
- strict JSON request bodies
- posted operation immutability in DB

## What is implemented and demo-ready in the frontends

### Frontend auth

Implemented and wired for MVP demo:

- login and signup call `kushim-auth/api`;
- recovery setup and password reset call `kushim-auth/api`;
- Redis-backed handoff is wired from `kushim-auth/front` to `kushim-app`;
- the auth frontend no longer merely simulates auth flows.

Known limitation:

- token storage remains localStorage-based for local MVP convenience and is not production-grade.

### Private app frontend

Implemented and largely wired to real APIs:

- auth/session validation, refresh, logout;
- portfolio list/create/select;
- operations list/create;
- dashboard KPIs, evolution chart, allocation, top assets, and recent transactions;
- asset catalogue and asset detail;
- portfolio positions;
- settings profile display and logout.

Remaining mock/UI-only areas:

- dashboard benchmark remains demo/mock data;
- settings preference/password/delete actions are UI-only;
- dashboard "Ajouter un actif" modal may remain placeholder/UI-only;
- complex operation UX such as split, spin-off, symbol change, and adjustment remains deferred.

## What is implemented with mock and guarded Finnhub providers

### `kushim-market-data`

Implemented and validated locally with mock provider for the reliable demo path.

Implemented:

- `refresh_current_market_data` job (writes `asset_market_data`)
- `fill_missing_price_history_cache` job (writes `asset_price_history_cache`)
- mock provider with deterministic USD current and historical prices for supported symbols
- guarded Finnhub provider for controlled allowlisted current stock quotes
- Finnhub current quotes live-validated for AAPL/MSFT/NVDA
- provider-symbol mapping support, including the tested BTC mapping `BTC=BINANCE:BTCUSDT`
- once/loop/idle modes
- health and ready endpoints

Still blocked or deferred:

- BTC/crypto live Finnhub validation is blocked by `403 Forbidden` with the current plan/access
- Finnhub historical `/stock/candle` is blocked by `403 Forbidden` with the current plan/access
- BTC and historical data remain mock/seeded/manual until a provider/access decision is made
- asset enrichment
- FX support
- production scheduler
- production-grade provider strategy

## What is now demonstrable

### Scenario A supervised MVP dry-run — validated locally

The current project state is **GO for a supervised internal MVP demo**.

Validated local flow:

- auth
- portfolio creation
- operations
- market-data mock
- worker rebuild
- snapshots
- backfill
- dashboard
- positions
- transactions
- assets
- asset detail
- settings
- logout

Browser validation during the dry-run showed zero blocking console errors.

### Backend E2E smoke test — validated locally

The full backend MVP chain is now executable and automated via both implementations:

- PowerShell: `scripts/powershell/demo/backend-e2e.ps1`
- Bash: `scripts/bash/demo/backend-e2e.sh`

Both scripts exercise all four backend services in sequence (signup → portfolio → operations → market-data mock → worker rebuild/snapshots/backfill → API verification) and have been executed successfully with **18/18 assertions passing**. Both must preserve functional parity.

This is a local debug/demo smoke test using the mock market-data provider. It is not a production validation.

Runbook: [documentation/operations/backend-demo-e2e.md](../operations/backend-demo-e2e.md)

## What remains limited during a supervised MVP demo

The reliable demo path uses the mock market-data provider. Finnhub current equities may be shown only as optional dev validation for AAPL/MSFT/NVDA.

Do not present as validated:

- BTC/crypto via Finnhub;
- Finnhub historical candles;
- FX conversion;
- benchmark data;
- settings write actions;
- production readiness.

## What is missing before production

Clearly still missing:

- production deployment strategy
- full CI/CD
- backup and restore strategy
- stronger observability
- production secrets lifecycle
- richer auth session model
- advanced market data pipeline
- more complete FX and corporate-action handling

## Status language

Use these labels when describing Kushim:

- Backend MVP core: **Implemented and validated**
- Backend E2E smoke test: **Validated locally (mock provider, 18/18 assertions)**
- Scenario A mock dry-run: **Validated end-to-end locally**
- Frontend demo path: **Demo-ready with explicit warnings**
- Frontend integration: **Largely wired to real APIs**
- Market data service: **Implemented with mock provider and guarded Finnhub provider**
- Finnhub current equities: **Live-validated for AAPL/MSFT/NVDA only**
- Finnhub BTC/crypto: **Not validated; current plan/access returns 403**
- Finnhub historical: **Not validated; current plan/access returns 403**
- Production readiness: **Not ready**

## What should not be claimed today

Do not claim that Kushim is:

- production-ready
- backed by broad live market sync
- validating BTC/crypto through Finnhub
- validating historical Finnhub candles
- supporting FX conversion
- doing full historical reconstruction everywhere
- a complete trading or execution platform

## Current MVP summary

The current MVP is best described as:

> a local MVP checkpoint for portfolio tracking and derived analytics, GO for supervised internal demo: backend E2E and Scenario A mock dry-run validated, frontends largely wired to real APIs, market-data implemented with mock provider plus guarded Finnhub current-equity validation, and production readiness explicitly out of scope.
