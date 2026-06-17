-- =========================================================================
-- upgrades/002_portfolio_operation_idempotency.sql
--
-- P3: Durable idempotency for portfolio-operation creation and correction
-- creation.
--
-- Idempotent, non-destructive upgrade for EXISTING local databases. Adds the
-- portfolio_operation_idempotency table required by the P3 contract so a
-- retry that arrives after the original transaction committed can replay
-- the same operation/refresh identity instead of creating a duplicate row.
--
-- SAFETY
--   - Uses IF NOT EXISTS / guarded DO blocks; re-running is a no-op.
--   - Never drops, truncates, or deletes application data.
--   - Does not alter portfolio_operations or portfolio_refresh_requests.
-- =========================================================================

CREATE TABLE IF NOT EXISTS portfolio_operation_idempotency (
    id_portfolio_operation_idempotency uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_user uuid NOT NULL,
    id_portfolio uuid NOT NULL,
    idempotency_key uuid NOT NULL,
    request_kind varchar(40) NOT NULL,
    id_corrected_operation uuid,
    request_fingerprint jsonb NOT NULL,
    id_portfolio_operation uuid,
    id_portfolio_refresh_request uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- Audit-log semantics — see init/001_init.sql for the rationale.
    CONSTRAINT fk_portfolio_operation_idempotency_user
        FOREIGN KEY (id_user) REFERENCES users (id_user) ON DELETE RESTRICT,
    CONSTRAINT fk_portfolio_operation_idempotency_portfolio
        FOREIGN KEY (id_portfolio) REFERENCES portfolios (id_portfolio) ON DELETE RESTRICT,
    CONSTRAINT fk_portfolio_operation_idempotency_operation
        FOREIGN KEY (id_portfolio_operation)
        REFERENCES portfolio_operations (id_portfolio_operation)
        ON DELETE RESTRICT,
    CONSTRAINT fk_portfolio_operation_idempotency_corrected_operation
        FOREIGN KEY (id_corrected_operation)
        REFERENCES portfolio_operations (id_portfolio_operation)
        ON DELETE SET NULL,
    CONSTRAINT fk_portfolio_operation_idempotency_refresh_request
        FOREIGN KEY (id_portfolio_refresh_request)
        REFERENCES portfolio_refresh_requests (id_portfolio_refresh_request)
        ON DELETE SET NULL,
    CONSTRAINT chk_portfolio_operation_idempotency_request_kind
        CHECK (request_kind IN ('create_operation', 'create_correction')),
    CONSTRAINT chk_portfolio_operation_idempotency_correction_link
        CHECK (
            (request_kind = 'create_operation' AND id_corrected_operation IS NULL)
            OR (request_kind = 'create_correction' AND id_corrected_operation IS NOT NULL)
        ),
    CONSTRAINT chk_portfolio_operation_idempotency_fingerprint_object
        CHECK (jsonb_typeof(request_fingerprint) = 'object')
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_portfolio_operation_idempotency_user_key
    ON portfolio_operation_idempotency (id_user, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_portfolio_operation_idempotency_portfolio_created
    ON portfolio_operation_idempotency (id_portfolio, created_at DESC);

-- Audit-history FK tightening for databases that received an earlier draft
-- of this upgrade with CASCADE on user/portfolio/operation FKs. Re-creates
-- the three FKs with ON DELETE RESTRICT only when they are not already in
-- the desired shape. Safe and idempotent on every subsequent run.
DO $$
DECLARE
    fk record;
    desired_action constant char := 'r'; -- pg_constraint.confdeltype 'r' = RESTRICT
BEGIN
    FOR fk IN
        SELECT conname,
               confdeltype
          FROM pg_constraint
         WHERE conrelid = 'portfolio_operation_idempotency'::regclass
           AND conname IN (
               'fk_portfolio_operation_idempotency_user',
               'fk_portfolio_operation_idempotency_portfolio',
               'fk_portfolio_operation_idempotency_operation'
           )
    LOOP
        IF fk.confdeltype <> desired_action THEN
            EXECUTE format(
                'ALTER TABLE portfolio_operation_idempotency DROP CONSTRAINT %I',
                fk.conname
            );
            CASE fk.conname
                WHEN 'fk_portfolio_operation_idempotency_user' THEN
                    EXECUTE 'ALTER TABLE portfolio_operation_idempotency
                             ADD CONSTRAINT fk_portfolio_operation_idempotency_user
                             FOREIGN KEY (id_user) REFERENCES users (id_user) ON DELETE RESTRICT';
                WHEN 'fk_portfolio_operation_idempotency_portfolio' THEN
                    EXECUTE 'ALTER TABLE portfolio_operation_idempotency
                             ADD CONSTRAINT fk_portfolio_operation_idempotency_portfolio
                             FOREIGN KEY (id_portfolio) REFERENCES portfolios (id_portfolio) ON DELETE RESTRICT';
                WHEN 'fk_portfolio_operation_idempotency_operation' THEN
                    EXECUTE 'ALTER TABLE portfolio_operation_idempotency
                             ADD CONSTRAINT fk_portfolio_operation_idempotency_operation
                             FOREIGN KEY (id_portfolio_operation)
                             REFERENCES portfolio_operations (id_portfolio_operation)
                             ON DELETE RESTRICT';
            END CASE;
        END IF;
    END LOOP;
END;
$$;
