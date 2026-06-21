-- =========================================================================
-- upgrades/001_portfolio_refresh_requests.sql
--
-- Idempotent, non-destructive upgrade for EXISTING local databases.
--
-- WHY THIS FILE EXISTS
--   infra/postgres/init/001_init.sql runs only when the PostgreSQL data
--   directory is empty (fresh volume). Existing local volumes therefore do
--   not get the new portfolio_refresh_requests table automatically. This
--   script adds it safely. It can be applied with:
--     powershell -ExecutionPolicy Bypass -File scripts/powershell/dev/apply-db-upgrades.ps1
--
-- SAFETY
--   - Uses IF NOT EXISTS everywhere; re-running is a no-op.
--   - Never drops, truncates, or deletes application data.
--   - Does not alter portfolio_operations, read models, or snapshots.
-- =========================================================================

CREATE TABLE IF NOT EXISTS portfolio_refresh_requests (
    id_portfolio_refresh_request uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_portfolio uuid NOT NULL,
    id_triggering_operation uuid,
    status varchar(20) NOT NULL DEFAULT 'pending',
    attempts integer NOT NULL DEFAULT 0,
    requested_at timestamptz NOT NULL DEFAULT now(),
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    processing_started_at timestamptz,
    completed_at timestamptz,
    locked_by varchar(100),
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_portfolio_refresh_requests_portfolio_id_portfolio
        FOREIGN KEY (id_portfolio) REFERENCES portfolios (id_portfolio) ON DELETE CASCADE,
    CONSTRAINT fk_portfolio_refresh_requests_triggering_operation
        FOREIGN KEY (id_triggering_operation)
        REFERENCES portfolio_operations (id_portfolio_operation)
        ON DELETE SET NULL,
    CONSTRAINT chk_portfolio_refresh_requests_status
        CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    CONSTRAINT chk_portfolio_refresh_requests_attempts_non_negative
        CHECK (attempts >= 0),
    CONSTRAINT chk_portfolio_refresh_requests_processing_after_requested
        CHECK (processing_started_at IS NULL OR processing_started_at >= requested_at),
    CONSTRAINT chk_portfolio_refresh_requests_completed_after_requested
        CHECK (completed_at IS NULL OR completed_at >= requested_at)
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_portfolio_refresh_requests_pending_per_portfolio
    ON portfolio_refresh_requests (id_portfolio)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_portfolio_refresh_requests_claim
    ON portfolio_refresh_requests (status, next_attempt_at)
    WHERE status IN ('pending', 'processing');

CREATE INDEX IF NOT EXISTS idx_portfolio_refresh_requests_portfolio_recent
    ON portfolio_refresh_requests (id_portfolio, requested_at DESC);

-- The set_updated_at() trigger function is created by 001_init.sql and always
-- exists on a database that has the schema. Create the trigger only if missing.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'trg_portfolio_refresh_requests_set_updated_at'
    ) THEN
        CREATE TRIGGER trg_portfolio_refresh_requests_set_updated_at
            BEFORE UPDATE ON portfolio_refresh_requests
            FOR EACH ROW
            EXECUTE FUNCTION set_updated_at();
    END IF;
END;
$$;
