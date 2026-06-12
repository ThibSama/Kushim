# Kushim Architecture Overview

## Purpose

Kushim is an investment portfolio tracking and analytics application.

It is designed to:

- record portfolio-changing events;
- reconstruct and value current state;
- persist rebuildable read models;
- persist historical snapshots;
- expose synchronous read/write APIs to user-facing clients;
- keep heavy or derived calculations out of the synchronous API.

It is not designed to be:

- a broker;
- a trade execution venue;
- a bank;
- a payment system;
- a raw market data vendor.

## Current architecture map

### Implemented and validated for the local MVP

- `kushim-auth/api`
- `kushim-api`
- `kushim-worker`
- `kushim-market-data` with mock provider and guarded Finnhub current-equity validation
- PostgreSQL V3 DDL in `infra/postgres`

### Implemented and demo-ready with known MVP limits

- `kushim-auth/front`
- `kushim-app`
- `kushim-website`

Important limits:

- Finnhub current equities are live-validated only for AAPL, MSFT, and NVDA.
- Finnhub BTC/crypto is not validated with the current plan/access (`403 Forbidden` on the tested mapping).
- Finnhub historical candles are not validated with the current plan/access (`403 Forbidden` on `/stock/candle`).
- FX, production scheduling, production observability, and production deployment remain out of scope.

## Main services

### `kushim-auth/api`

Authentication service.

Owns:

- signup
- login
- refresh
- logout
- current identity
- recovery phrase setup / reset
- refresh revocation
- Redis-backed rate limiting

### `kushim-api`

Synchronous user-facing business API.

Owns:

- portfolios
- `portfolio_operations`
- operation lifecycle
- corrections and audit views
- assets read-only exposure
- read-only current read model exposure
- read-only snapshot exposure

Does not generate derived portfolio state.

### `kushim-worker`

Background and derived-state service.

Owns:

- current read model rebuild
- current daily snapshot generation
- composite refresh of current state
- first controlled historical daily snapshot backfill

### `kushim-market-data`

Market-data ingestion and sync service.

Owns:

- asset enrichment
- current `asset_market_data` refresh
- historical `asset_price_history_cache` fill
- provider integration and normalization

Current state:

- implemented beyond scaffold level;
- mock provider is the safe default for deterministic MVP demos;
- guarded Finnhub provider exists behind explicit API key and symbol allowlist;
- Finnhub current stock quotes are live-validated for AAPL, MSFT, and NVDA only;
- BTC exists as a canonical local asset/concept and provider-symbol mapping exists (`BTC=BINANCE:BTCUSDT`), but live Finnhub BTC validation is blocked by `403 Forbidden` with the current plan/access;
- Finnhub historical `/stock/candle` is implemented but blocked by `403 Forbidden` with the current plan/access;
- no FX conversion, no production scheduler, and no production market-data guarantee.

## Data architecture in one sentence

Kushim is built around:

`portfolio_operations` -> worker rebuild -> read models -> snapshots -> API reads

## Why API, worker, and market-data are separated

The separation is intentional:

- user-facing writes must remain fast and deterministic;
- rebuilds and backfills are asynchronous and potentially expensive;
- market data fetching has external dependencies and separate failure modes.

This produces three different responsibilities:

- synchronous business interaction in `kushim-api`
- asynchronous derived-state generation in `kushim-worker`
- external pricing/provider sync in `kushim-market-data`

## Current maturity assessment

### Backend maturity

The backend core is already substantial:

- auth is implemented
- the business API is implemented for the current MVP perimeter
- the worker materializes read models, snapshots, and a first historical backfill

### Frontend maturity

The frontend layer is suitable for a supervised internal MVP demo:

- `kushim-auth/front` is wired to `kushim-auth/api` for login, signup, recovery, and Redis-backed handoff into the app;
- `kushim-app` is largely wired to real APIs for auth/session validation, portfolios, operations, dashboard KPIs/evolution/allocation/top assets, transactions, assets, asset detail, positions, settings profile, and logout;
- browser validation during Scenario A showed zero blocking console errors.

Remaining frontend MVP limits:

- dashboard benchmark remains demo/mock data;
- settings preference/password/delete actions are UI-only;
- the dashboard "Ajouter un actif" modal may remain placeholder/UI-only;
- token storage remains localStorage-based and is not production-grade.

### Market-data maturity

Implemented for MVP/dev usage with clear limits:

- mock provider is validated for deterministic current and historical demo data;
- guarded Finnhub provider is partially live-validated for current equities only;
- BTC/crypto and historical Finnhub are not validated with the current plan/access.

## Non-production status

Kushim is not production-ready yet.

Key reasons:

- market-data is MVP/dev-grade, not a production provider strategy;
- BTC/crypto, historical provider access, FX, provider quotas, and freshness policies still need decisions;
- token/session handling is not production-grade;
- CI/CD and deployment strategy are incomplete;
- observability, backups, secret management, and ingress/container hardening remain limited;
- some business logic intentionally remains V1-conservative.
