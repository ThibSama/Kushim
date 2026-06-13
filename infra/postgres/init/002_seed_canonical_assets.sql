-- =========================================================================
-- 002_seed_canonical_assets.sql
--
-- Canonical MVP / dev catalogue seed for Kushim.
--
-- WHY THIS FILE EXISTS
--   001_init.sql owns the database schema (source of truth).
--   This file owns the minimal, stable catalogue of assets reused across:
--     - backend E2E demonstrations (scripts/demo/backend-e2e.ps1)
--     - mock market-data refreshes
--     - controlled Finnhub current-quote validations
--     - worker read-model reconstruction
--   Demo runs may keep creating temporary users, portfolios and operations.
--   They must NEVER create additional economic representations of AAPL,
--   MSFT, or NVDA. Reusing the rows seeded here prevents catalogue
--   pollution and ambiguous provider symbol selection.
--
-- WHAT THIS FILE IS NOT
--   This is not a production-grade asset master. It does not enrich
--   metadata, seed aliases, populate market prices, or model corporate
--   actions. External provider data (mock or guarded Finnhub) remains
--   stored separately in asset_market_data and asset_price_history_cache.
--
-- IDENTITY MODEL
--   The canonical natural identity is (ticker, exchange).
--   001_init.sql defines a partial unique index uq_assets_ticker_exchange
--   on (ticker, exchange) WHERE ticker IS NOT NULL AND exchange IS NOT NULL.
--   We target that index in ON CONFLICT to make the seed idempotent.
--
-- IDEMPOTENCY GUARANTEES
--   - Running this file zero, one, or many times converges to exactly
--     one row per (ticker, exchange) tuple seeded here.
--   - On conflict the existing row's id_asset is preserved.
--   - Canonical fields (name, asset_class, status, native_currency,
--     symbol) are normalised on rerun.
--   - Legacy rows with ticker IS NULL are NOT touched, merged, or
--     deleted. Cleaning them up is a separate operator decision.
--
-- DETERMINISTIC UUIDs
--   Hard-coded so a fresh local volume produces predictable IDs.
--   Do not regenerate these; downstream tooling may pin to them.
-- =========================================================================

INSERT INTO assets (
    id_asset, asset_class, status, name,
    native_currency, ticker, symbol, exchange
)
VALUES
    (
        '01993b00-0001-7000-8001-aaaaaaaaaaaa',
        'equity', 'active', 'Apple Inc.',
        'USD', 'AAPL', 'AAPL', 'NASDAQ'
    ),
    (
        '01993b00-0002-7000-8001-bbbbbbbbbbbb',
        'equity', 'active', 'Microsoft Corporation',
        'USD', 'MSFT', 'MSFT', 'NASDAQ'
    ),
    (
        '01993b00-0003-7000-8001-cccccccccccc',
        'equity', 'active', 'NVIDIA Corporation',
        'USD', 'NVDA', 'NVDA', 'NASDAQ'
    )
ON CONFLICT (ticker, exchange)
WHERE ticker IS NOT NULL AND exchange IS NOT NULL
DO UPDATE SET
    asset_class     = EXCLUDED.asset_class,
    status          = EXCLUDED.status,
    name            = EXCLUDED.name,
    native_currency = EXCLUDED.native_currency,
    symbol          = EXCLUDED.symbol;
