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

## What is implemented but not fully integrated

### Frontend auth

Present but not fully wired:

- auth pages
- recovery pages
- mock UX

Still missing:

- real integration with `kushim-auth/api`

### Private app frontend

Present but not fully wired:

- dashboard UI
- assets pages
- transactions UI
- settings UI

Still missing:

- real integration with `kushim-api`
- replacement of static mock portfolio data

## What is implemented with mock provider

### `kushim-market-data`

Implemented and validated locally with mock provider.

Implemented:

- `refresh_current_market_data` job (writes `asset_market_data`)
- `fill_missing_price_history_cache` job (writes `asset_price_history_cache`)
- mock provider with deterministic USD prices for common tickers
- once/loop/idle modes
- health and ready endpoints

Still not implemented:

- real market-data provider integration
- asset enrichment
- FX support

## What is now demonstrable

### Backend E2E smoke test — validated locally

The full backend MVP chain is now executable and automated via:

- `scripts/demo/backend-e2e.ps1`

The script exercises all four backend services in sequence (signup → portfolio → operations → market-data mock → worker rebuild/snapshots/backfill → API verification) and has been executed successfully with **18/18 assertions passing**.

This is a local debug/demo smoke test using the mock market-data provider. It is not a production validation.

Runbook: [documentation/operations/backend-demo-e2e.md](../operations/backend-demo-e2e.md)

## What is missing before a usable end-user demo

For a coherent end-user demo with a frontend, the remaining missing pieces are:

- auth frontend -> auth backend wiring
- app frontend -> business API wiring
- real market-data provider (or continued use of mock for demo purposes)

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
- Frontend integration: **Partial**
- Market data service: **Implemented with mock provider**
- Real market data provider: **Not implemented**
- Production readiness: **Not ready**

## What should not be claimed today

Do not claim that Kushim is:

- production-ready
- fully integrated front-to-back
- backed by live market sync
- doing full historical reconstruction everywhere
- a complete trading or execution platform

## Current MVP summary

The current MVP is best described as:

> an advanced backend MVP for portfolio tracking and derived analytics, with a validated E2E backend smoke test (mock provider), partially wired frontends, and a market-data service implemented with mock provider only.
