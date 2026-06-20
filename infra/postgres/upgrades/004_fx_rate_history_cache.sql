-- =============================================================================
-- Migration 004 — Add fx_rate_history_cache for historical FX rates.
--
-- Status: ADDITIVE, IDEMPOTENT.
--
-- This migration must be applied to any existing PostgreSQL database that was
-- bootstrapped before the corresponding addition to `001_init.sql`. New
-- databases bootstrapped from `001_init.sql` already contain this table, so
-- the script is a no-op against them.
--
-- Application:
--
--   docker exec -i kushim_database psql \
--     -v ON_ERROR_STOP=1 \
--     -U kushim \
--     -d kushim \
--     < infra/postgres/upgrades/004_fx_rate_history_cache.sql
--
-- Safety:
-- * CREATE TABLE IF NOT EXISTS — no DROP, no UPDATE, no destructive DDL.
-- * Existing tables, rows and constraints are not touched.
-- * Re-running the script is a no-op.
-- * Applying it to a fresh `001_init.sql` bootstrap is also a no-op.
--
-- Contract: provider-agnostic; one canonical unordered currency-pair record
-- per (pair, date, provider); inverse_rate is a STORED GENERATED column,
-- guaranteed to never diverge from canonical_rate. The supported mock
-- currency set is not pinned in the schema — adding a new currency requires
-- no migration.
-- =============================================================================

BEGIN;

CREATE TABLE IF NOT EXISTS fx_rate_history_cache (
    id_fx_rate_history_cache uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    rate_date date NOT NULL,
    canonical_base_currency char(3) NOT NULL,
    canonical_quote_currency char(3) NOT NULL,
    canonical_rate numeric(28, 12) NOT NULL,
    -- inverse_rate is mechanically derived from canonical_rate so the two
    -- directions can never diverge. The expression is IMMUTABLE (required
    -- for STORED generated columns) and rounds to 12 fractional digits so
    -- the persisted value fits numeric(28, 12) without overflow even for
    -- very small canonical_rate values.
    inverse_rate numeric(28, 12) GENERATED ALWAYS AS (
        ROUND((1::numeric / canonical_rate)::numeric, 12)
    ) STORED,
    provider varchar(50) NOT NULL,
    provider_as_of timestamptz,
    dataset_version varchar(64) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT chk_fx_rate_history_cache_base_currency_format
        CHECK (canonical_base_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_fx_rate_history_cache_quote_currency_format
        CHECK (canonical_quote_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_fx_rate_history_cache_pair_canonical_ordering
        CHECK (canonical_base_currency < canonical_quote_currency),
    CONSTRAINT chk_fx_rate_history_cache_canonical_rate_positive
        CHECK (canonical_rate > 0),
    CONSTRAINT chk_fx_rate_history_cache_provider_not_blank
        CHECK (btrim(provider) <> ''),
    CONSTRAINT chk_fx_rate_history_cache_dataset_version_not_blank
        CHECK (btrim(dataset_version) <> '')
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_fx_rate_history_cache_pair_date_provider
    ON fx_rate_history_cache (
        canonical_base_currency,
        canonical_quote_currency,
        rate_date,
        provider
    );

CREATE INDEX IF NOT EXISTS idx_fx_rate_history_cache_pair_date_desc
    ON fx_rate_history_cache (
        canonical_base_currency,
        canonical_quote_currency,
        rate_date DESC
    );

CREATE INDEX IF NOT EXISTS idx_fx_rate_history_cache_date_desc
    ON fx_rate_history_cache (rate_date DESC);

CREATE INDEX IF NOT EXISTS idx_fx_rate_history_cache_provider_date
    ON fx_rate_history_cache (provider, rate_date DESC);

-- The set_updated_at() trigger function is created by 001_init.sql and
-- always exists on a database that has the schema. Create the trigger only
-- if missing.
DO $do$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'trg_fx_rate_history_cache_set_updated_at'
    ) THEN
        CREATE TRIGGER trg_fx_rate_history_cache_set_updated_at
            BEFORE UPDATE ON fx_rate_history_cache
            FOR EACH ROW
            EXECUTE FUNCTION set_updated_at();
    END IF;
END
$do$;

COMMIT;
