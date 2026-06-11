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

### Implemented and validated

- `kushim-auth/api`
- `kushim-api`
- `kushim-worker`
- PostgreSQL V3 DDL in `infra/postgres`

### Implemented but not fully integrated

- `kushim-auth/front`
- `kushim-app`
- `kushim-website`

### Scaffolded / planned

- `kushim-market-data`

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

Future market-data ingestion and sync service.

Will eventually own:

- asset enrichment
- current `asset_market_data` refresh
- historical `asset_price_history_cache` fill
- provider integration and normalization

It is not implemented yet beyond scaffold level.

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

The frontend layer is visually advanced but not fully integrated:

- auth frontend still simulates auth flows
- app frontend still uses mock data for the dashboard and portfolio views

### Market-data maturity

Still at scaffold stage.

## Non-production status

Kushim is not production-ready yet.

Key reasons:

- market-data service not implemented
- frontend/backend wiring incomplete
- CI/CD and deployment strategy not completed
- observability still limited
- some business logic intentionally remains V1-conservative
