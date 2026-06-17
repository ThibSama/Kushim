//! Shared `#[cfg(test)]` helpers for kushim-api integration tests.
//!
//! This module exists to centralize fixtures that are otherwise duplicated
//! across the HTTP integration-test modules. Compiled only under
//! `#[cfg(test)]` (see `lib.rs`), so it ships nothing into the production
//! binary.

use sqlx::PgPool;

/// Reads `DATABASE_URL` and refuses to return it unless it points at a
/// disposable test database whose name starts with `kushim_test_`.
///
/// Database-backed tests must never run against the persistent development
/// database `kushim`: the schema enforces immutability on posted
/// portfolio_operations, so every test that posts an operation leaks a
/// schema-protected residue that the per-test cleanup cannot remove. The
/// only reliable isolation is a brand-new database per suite invocation,
/// which the `scripts/test/run-rust-db-suite.ps1` runner provides.
///
/// This guard is the defense-in-depth lid: even if the runner is bypassed,
/// the tests refuse to touch the wrong database. The escape hatch
/// `KUSHIM_ALLOW_SHARED_TEST_DATABASE=1` is documented as UNSAFE and exists
/// only for the unlikely case where CI must temporarily fall back to a
/// shared development pool — it is never enabled by the runner or by
/// normal CI.
pub fn require_disposable_test_database_url() -> String {
    let url = std::env::var("DATABASE_URL").expect(
        "DATABASE_URL must be set for integration tests (use scripts/test/run-rust-db-suite.ps1)",
    );
    let dbname = extract_database_name(&url).unwrap_or_default();
    if dbname.starts_with("kushim_test_") {
        return url;
    }
    if std::env::var("KUSHIM_ALLOW_SHARED_TEST_DATABASE").as_deref() == Ok("1") {
        return url;
    }
    panic!(
        "REFUSED to run database-backed tests against database '{dbname}'. \
         Use `scripts/test/run-rust-db-suite.ps1 -Service kushim-api` to run \
         them against a disposable `kushim_test_*` database. \
         Override with KUSHIM_ALLOW_SHARED_TEST_DATABASE=1 only if you know \
         what you're doing (it WILL leak rows into the dev database)."
    );
}

/// Extracts the path-segment database name from a `postgresql://…/<db>`
/// URL. Returns `None` for malformed URLs.
fn extract_database_name(url: &str) -> Option<&str> {
    let after_scheme = url.split("://").nth(1)?;
    let path = after_scheme.split_once('/').map(|(_, rest)| rest)?;
    // Strip any query string (`?…`) — the database name is the path segment
    // up to the first `?`.
    Some(path.split('?').next().unwrap_or(path).trim_end_matches('/'))
}

/// Ensures the canonical authentication reference data — the
/// `(id_role = 1, label = "user")` row — exists in the test database, in a
/// way that is safe under `cargo test`'s parallel test runner.
///
/// # Race-safety
///
/// CI (and a fresh local volume) starts with an empty `roles` table because
/// only `001_init.sql` is applied. Several HTTP integration tests therefore
/// have to seed the canonical row themselves before inserting a `users` row.
///
/// `roles` has two relevant uniqueness constraints (see
/// `infra/postgres/init/001_init.sql`):
///
/// - `roles_pkey` on `(id_role)`;
/// - `uq_roles_label` on `(label)`.
///
/// Two concurrent tests that both run this helper at the same time pass
/// PostgreSQL's per-row uniqueness check independently, and then both go to
/// commit. The loser fails — and which constraint trips first depends on
/// timing. A previous version used `ON CONFLICT (label) DO NOTHING`, which
/// only covers `uq_roles_label`. A concurrent `roles_pkey` conflict on
/// `id_role = 1` is **not** suppressed by that conflict target and was
/// observed to fail CI as
/// `duplicate key value violates unique constraint "roles_pkey"`.
///
/// The targetless `ON CONFLICT DO NOTHING` shape used here suppresses **any**
/// uniqueness conflict on the row — both `roles_pkey` and `uq_roles_label` —
/// so the helper is safe regardless of which constraint the loser trips
/// first.
///
/// # Invariant verification
///
/// After the insert this helper verifies the canonical mapping:
///
/// - the row with `id_role = 1` exists and has `label = "user"`;
/// - the row with `label = "user"` has `id_role = 1`.
///
/// An inconsistent state — e.g. another role occupying `id_role = 1` with a
/// different label (`(1, 'admin')`), or `'user'` mapped to a different id
/// (`(2, 'user')`) — would silently mismatch the test fixtures otherwise.
/// We panic loudly with a fixture-specific message so the failure points at
/// the test environment rather than the production code under test.
pub async fn ensure_canonical_user_role(pool: &PgPool) {
    sqlx::query(
        r#"
        INSERT INTO roles (id_role, label)
        VALUES (1, 'user')
        ON CONFLICT DO NOTHING
        "#,
    )
    .execute(pool)
    .await
    .expect("canonical user-role fixture insert should succeed");

    let row: Option<(i16, String)> =
        sqlx::query_as("SELECT id_role, label FROM roles WHERE id_role = 1")
            .fetch_optional(pool)
            .await
            .expect("canonical user-role lookup should succeed");

    let (id_role, label) = row.expect(
        "canonical user-role fixture invariant violated: no row with id_role = 1 after insert",
    );
    assert_eq!(
        id_role, 1,
        "canonical user-role fixture invariant violated: id_role lookup returned {id_role}"
    );
    assert_eq!(
        label, "user",
        "canonical user-role fixture invariant violated: \
        id_role = 1 exists but its label is {label:?} instead of \"user\". \
        Another role appears to occupy the canonical id — refusing to overwrite."
    );

    let id_for_label: Option<i16> =
        sqlx::query_scalar("SELECT id_role FROM roles WHERE label = 'user'")
            .fetch_optional(pool)
            .await
            .expect("canonical user-role label lookup should succeed");

    let id_for_label = id_for_label.expect(
        "canonical user-role fixture invariant violated: no row with label = 'user' after insert",
    );
    assert_eq!(
        id_for_label, 1,
        "canonical user-role fixture invariant violated: label 'user' exists \
        but is mapped to id_role = {id_for_label} instead of 1."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;

    async fn test_pool(max_connections: u32) -> PgPool {
        let database_url = super::require_disposable_test_database_url();
        PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    /// Concurrency regression for the parallel CI race observed after PR #2:
    /// many concurrent invocations of the canonical fixture must all succeed
    /// without surfacing either `roles_pkey` or `uq_roles_label` violations.
    ///
    /// We use a pool with enough real connections to exercise both uniqueness
    /// constraints under genuine concurrency, fire many tasks at once, and
    /// assert every invocation returned cleanly. We never DELETE the shared
    /// `roles` row — other integration tests may be running in parallel and
    /// depend on it.
    #[tokio::test]
    async fn ensure_canonical_user_role_is_safe_under_parallel_invocations() {
        let pool = Arc::new(test_pool(16).await);

        let mut handles = Vec::with_capacity(64);
        for _ in 0..64 {
            let pool = Arc::clone(&pool);
            handles.push(tokio::spawn(async move {
                ensure_canonical_user_role(&pool).await;
            }));
        }

        for handle in handles {
            handle
                .await
                .expect("ensure_canonical_user_role task panicked under parallel invocation");
        }

        // Final state: exactly one canonical mapping.
        let (id_role, label): (i16, String) =
            sqlx::query_as("SELECT id_role, label FROM roles WHERE id_role = 1")
                .fetch_one(pool.as_ref())
                .await
                .expect("canonical row should exist");
        assert_eq!(id_role, 1);
        assert_eq!(label, "user");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM roles WHERE label = 'user'")
            .fetch_one(pool.as_ref())
            .await
            .expect("label count should succeed");
        assert_eq!(count, 1, "canonical 'user' label must remain unique");
    }
}
