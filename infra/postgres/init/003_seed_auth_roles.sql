-- =========================================================================
-- 003_seed_auth_roles.sql
--
-- Minimal authentication reference data for a fresh Kushim database.
--
-- WHY THIS FILE EXISTS
--   kushim-auth/api signup assigns the default 'user' role to every new
--   account. It resolves that role with RoleRepository::find_by_label("user")
--   (see kushim-auth/api/src/repositories/roles.rs). On a brand-new database
--   the `roles` table is empty, so signup fails until this reference row
--   exists. This seed provides it so a fresh volume supports signup with no
--   manual SQL insertion.
--
-- WHAT THIS FILE IS NOT
--   This is reference data, not a demo user. It stores no credentials,
--   password hashes, recovery phrases, or per-user rows.
--
-- IDENTITY MODEL
--   id_role = 1 is the deterministic identity for the canonical 'user' role.
--   `roles` (001_init.sql) has id_role as PRIMARY KEY and a UNIQUE (label)
--   index (uq_roles_label).
--
-- IDEMPOTENCY / SAFETY
--   - On a fresh database this inserts exactly (id_role = 1, label = 'user').
--   - Re-running converges to a single 'user' row: ON CONFLICT (label)
--     keeps the existing row and only bumps updated_at.
--   - If an UNRELATED role already occupies id_role = 1 with a different
--     label (e.g. (1, 'admin')), the primary-key conflict is NOT caught by
--     ON CONFLICT (label); the statement fails loudly instead of silently
--     overwriting that unrelated role. That is the intended behaviour.
-- =========================================================================

INSERT INTO roles (id_role, label)
VALUES (1, 'user')
ON CONFLICT (label) DO UPDATE
SET updated_at = now();