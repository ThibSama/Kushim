-- =============================================================================
-- Migration 0001 — Persist valuation provenance on rm_portfolio_holdings.
--
-- Status: ADDITIVE, IDEMPOTENT.
--
-- This migration must be applied to any existing PostgreSQL database that was
-- bootstrapped before the corresponding change to `001_init.sql`.
-- New databases bootstrapped from `001_init.sql` already contain these
-- columns and constraints, so the script is a no-op against them.
--
-- Application:
--
--   docker exec -i kushim_database psql \
--     -v ON_ERROR_STOP=1 \
--     -U kushim \
--     -d kushim \
--     < infra/postgres/migrations/0001_holding_valuation_provenance.sql
--
-- The script may be re-run; every `ADD COLUMN` and `ADD CONSTRAINT` step is
-- guarded by an explicit existence check so a second execution is a no-op.
--
-- Safety:
-- * Only ADD COLUMN (nullable) and ADD CONSTRAINT — no DROP, no UPDATE, no
--   data backfill.
-- * Existing holding rows are preserved byte-for-byte (financial values
--   untouched). Their new provenance columns remain NULL until the worker
--   rebuilds them.
-- * `kushim-api` treats legacy NULL provenance as
--   `valuation_provenance_missing` so the contract remains correct before any
--   rebuild has happened.
-- =============================================================================

BEGIN;

-- ----------------------------------------------------------------------------
-- 1. Additive columns. `IF NOT EXISTS` is supported by `ADD COLUMN` since
--    PostgreSQL 9.6.
-- ----------------------------------------------------------------------------

ALTER TABLE rm_portfolio_holdings
    ADD COLUMN IF NOT EXISTS valuation_source              varchar(32),
    ADD COLUMN IF NOT EXISTS market_data_status            varchar(32),
    ADD COLUMN IF NOT EXISTS market_data_price_minor       bigint,
    ADD COLUMN IF NOT EXISTS market_data_currency          char(3),
    ADD COLUMN IF NOT EXISTS market_data_provider          varchar(50),
    ADD COLUMN IF NOT EXISTS market_data_as_of             timestamptz,
    ADD COLUMN IF NOT EXISTS market_data_record_updated_at timestamptz;

-- ----------------------------------------------------------------------------
-- 2. CHECK constraints. PostgreSQL does NOT support `ADD CONSTRAINT IF NOT
--    EXISTS`; we use a `DO $$ ... $$` block that probes `pg_constraint`
--    before issuing the `ADD CONSTRAINT`. The block is therefore safe to
--    re-run.
-- ----------------------------------------------------------------------------

DO $do$
DECLARE
    table_oid oid := 'rm_portfolio_holdings'::regclass;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = table_oid
          AND conname = 'chk_rm_portfolio_holdings_valuation_source'
    ) THEN
        ALTER TABLE rm_portfolio_holdings
            ADD CONSTRAINT chk_rm_portfolio_holdings_valuation_source
            CHECK (
                valuation_source IS NULL
                OR valuation_source IN ('market_data', 'invested_cost_fallback')
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = table_oid
          AND conname = 'chk_rm_portfolio_holdings_market_data_status'
    ) THEN
        ALTER TABLE rm_portfolio_holdings
            ADD CONSTRAINT chk_rm_portfolio_holdings_market_data_status
            CHECK (
                market_data_status IS NULL
                OR market_data_status IN ('available', 'missing', 'unsupported_currency')
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = table_oid
          AND conname = 'chk_rm_portfolio_holdings_md_price_non_negative'
    ) THEN
        ALTER TABLE rm_portfolio_holdings
            ADD CONSTRAINT chk_rm_portfolio_holdings_md_price_non_negative
            CHECK (
                market_data_price_minor IS NULL OR market_data_price_minor >= 0
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = table_oid
          AND conname = 'chk_rm_portfolio_holdings_md_currency_format'
    ) THEN
        ALTER TABLE rm_portfolio_holdings
            ADD CONSTRAINT chk_rm_portfolio_holdings_md_currency_format
            CHECK (
                market_data_currency IS NULL OR market_data_currency ~ '^[A-Z]{3}$'
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = table_oid
          AND conname = 'chk_rm_portfolio_holdings_provenance_combination'
    ) THEN
        ALTER TABLE rm_portfolio_holdings
            ADD CONSTRAINT chk_rm_portfolio_holdings_provenance_combination
            CHECK (
                (valuation_source IS NULL AND market_data_status IS NULL)
                OR (
                    valuation_source = 'market_data'
                    AND market_data_status = 'available'
                    AND market_data_price_minor IS NOT NULL
                    AND market_data_currency IS NOT NULL
                    AND market_data_as_of IS NOT NULL
                    AND market_data_record_updated_at IS NOT NULL
                )
                OR (
                    valuation_source = 'invested_cost_fallback'
                    AND market_data_status = 'missing'
                    AND market_data_price_minor IS NULL
                    AND market_data_currency IS NULL
                    AND market_data_provider IS NULL
                    AND market_data_as_of IS NULL
                    AND market_data_record_updated_at IS NULL
                )
                OR (
                    valuation_source = 'invested_cost_fallback'
                    AND market_data_status = 'unsupported_currency'
                    AND market_data_price_minor IS NOT NULL
                    AND market_data_currency IS NOT NULL
                    AND market_data_as_of IS NOT NULL
                    AND market_data_record_updated_at IS NOT NULL
                )
            );
    END IF;
END
$do$;

COMMIT;
