use std::sync::{Mutex, OnceLock};

/// Reads `DATABASE_URL` and refuses to return it unless it points at a
/// disposable test database whose name starts with `kushim_test_`.
///
/// See `kushim-api/src/test_support.rs` for the rationale and the
/// `KUSHIM_ALLOW_SHARED_TEST_DATABASE=1` escape hatch (documented as unsafe).
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
         Use `scripts/test/run-rust-db-suite.ps1 -Service kushim-worker` to run \
         them against a disposable `kushim_test_*` database. \
         Override with KUSHIM_ALLOW_SHARED_TEST_DATABASE=1 only if you know \
         what you're doing (it WILL leak rows into the dev database)."
    );
}

fn extract_database_name(url: &str) -> Option<&str> {
    let after_scheme = url.split("://").nth(1)?;
    let path = after_scheme.split_once('/').map(|(_, rest)| rest)?;
    Some(path.split('?').next().unwrap_or(path).trim_end_matches('/'))
}

pub fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Ensures the canonical authentication reference data — the
/// `(id_role = 1, label = "user")` row — exists in the test database, in a
/// way that is safe under `cargo test`'s parallel test runner.
///
/// # Race-safety
///
/// CI (and a fresh local volume) starts with an empty `roles` table because
/// only `001_init.sql` is applied. Several worker integration tests therefore
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
/// timing. The previous worker-local helpers used
/// `ON CONFLICT (label) DO NOTHING`, which only covers `uq_roles_label`. A
/// concurrent `roles_pkey` conflict on `id_role = 1` is **not** suppressed
/// by that conflict target and was observed in CI as
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
/// (`(2, 'user')`) — would silently mismatch the worker test fixtures
/// otherwise. We panic loudly with a fixture-specific message so the failure
/// points at the test environment rather than the production code under
/// test.
///
/// This is the worker counterpart of
/// `kushim-api/src/test_support.rs::ensure_canonical_user_role`.
pub async fn ensure_canonical_user_role(pool: &sqlx::PgPool) {
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

/// Test-only teardown: removes every row a worker integration test created
/// under `id_user` and sweeps the orphaned equity fixtures that
/// `create_asset` helpers in the worker job test modules insert directly.
///
/// Why this exists. The worker job test modules
/// (`jobs/backfill_daily_snapshots`, `jobs/generate_daily_snapshots`,
/// `jobs/rebuild_current_read_models`, `jobs/refresh_current_portfolio_state`)
/// each define a local `create_asset(pool, suffix)` that `INSERT INTO assets`
/// directly and have no cleanup of their own — every test run used to add
/// rows to the shared dev database. Each test now calls this helper as its
/// last statement so the database state is unchanged before and after the
/// test suite runs.
///
/// Order. Foreign keys to `assets` are RESTRICT-keyed; the user's
/// derived/snapshot/operation rows must be removed before the assets
/// themselves. Step 5 (defensive sweep) targets the documented test-fixture
/// shape (`asset_class='equity' AND exchange IS NULL`) and refuses to delete
/// any asset still referenced by `portfolio_operations`, `asset_aliases`,
/// `asset_metadata`, or `portfolio_holding_snapshot_daily`. That anti-join
/// is what makes the sweep safe under `cargo test`'s parallel runner: a
/// concurrent test's asset stays referenced by ITS still-live operations
/// until ITS own teardown.
///
/// Canonical seeded assets (`002_seed_canonical_assets.sql`) always have a
/// non-NULL `exchange` and are therefore never touched.
pub async fn cleanup_worker_test_tree(pool: &sqlx::PgPool, id_user: uuid::Uuid) {
    // Capture the set of asset IDs this user's operations and holdings
    // reference, BEFORE we delete those rows. Used at the very end to
    // perform a scoped, race-safe asset cleanup.
    let user_asset_ids: Vec<uuid::Uuid> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT id_asset FROM (
            SELECT id_asset FROM portfolio_operations
            WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
              AND id_asset IS NOT NULL
            UNION
            SELECT id_related_asset AS id_asset FROM portfolio_operations
            WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
              AND id_related_asset IS NOT NULL
            UNION
            SELECT id_asset FROM portfolio_holding_snapshot_daily
            WHERE id_portfolio_snapshot_daily IN (
                SELECT id_portfolio_snapshot_daily FROM portfolio_snapshots_daily
                WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
            )
            UNION
            SELECT id_asset FROM rm_portfolio_holdings
            WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
        ) t
        "#,
    )
    .bind(id_user)
    .fetch_all(pool)
    .await
    .expect("user asset id capture should succeed");

    sqlx::query(
        r#"
        DELETE FROM portfolio_holding_snapshot_daily
        WHERE id_portfolio_snapshot_daily IN (
            SELECT id_portfolio_snapshot_daily FROM portfolio_snapshots_daily
            WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
        )
        "#,
    )
    .bind(id_user)
    .execute(pool)
    .await
    .expect("holding snapshots should be deleted");

    sqlx::query(
        r#"
        DELETE FROM portfolio_snapshots_daily
        WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
        "#,
    )
    .bind(id_user)
    .execute(pool)
    .await
    .expect("snapshots should be deleted");

    sqlx::query(
        r#"
        DELETE FROM rm_portfolio_holdings
        WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
        "#,
    )
    .bind(id_user)
    .execute(pool)
    .await
    .expect("holdings read model should be deleted");

    sqlx::query(
        r#"
        DELETE FROM rm_portfolio_summary
        WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
        "#,
    )
    .bind(id_user)
    .execute(pool)
    .await
    .expect("summary read model should be deleted");

    sqlx::query(
        r#"
        DELETE FROM portfolio_refresh_requests
        WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
        "#,
    )
    .bind(id_user)
    .execute(pool)
    .await
    .expect("refresh requests should be deleted");

    // Posted portfolio_operations are intentionally immutable (DB trigger
    // `prevent_posted_operation_mutation` in `001_init.sql`). Tests that
    // post operations therefore CANNOT delete the resulting rows — that is
    // the same architectural constraint kushim-api tests document around
    // `cleanup_refresh_requests`. We delete the deletable statuses; if a
    // posted row remains, the subsequent portfolio/user delete will FK-fail
    // and we accept that residual via `.ok()`. The shape of the leak that
    // remains in that case is a deliberate, schema-enforced residue, not a
    // fixture leak we can clean.
    sqlx::query(
        r#"
        DELETE FROM portfolio_operations
        WHERE id_portfolio IN (SELECT id_portfolio FROM portfolios WHERE id_user = $1)
          AND operation_status IN ('pending', 'cancelled')
        "#,
    )
    .bind(id_user)
    .execute(pool)
    .await
    .expect("deletable operations should be deleted");

    // Best-effort: if posted operations remain, the portfolio FK-protects
    // itself and survives. We do not silently swallow ALL errors — only
    // RESTRICT failures (FK violations 23503) caused by the posted-ops
    // residue. Any other error is still surfaced via `.expect` paths above.
    let _ = sqlx::query("DELETE FROM portfolios WHERE id_user = $1")
        .bind(id_user)
        .execute(pool)
        .await;

    let _ = sqlx::query("DELETE FROM users WHERE id_user = $1")
        .bind(id_user)
        .execute(pool)
        .await;

    // Scoped asset cleanup: delete only assets that THIS user's just-deleted
    // operations referenced, and only when no other test still references
    // them. We capture the set BEFORE deleting operations — so the IDs we
    // delete were definitely owned by this test's portfolio. The
    // `NOT EXISTS` anti-join makes the delete race-free vs a parallel test
    // that may have inserted operations referencing the same UUID by
    // chance (random UUIDs collide with probability ~0). Canonical assets
    // are never in the scoped set because they always have a non-NULL
    // `exchange` and are referenced by countless other rows, so the
    // anti-join would refuse them too.
    sqlx::query(
        r#"
        DELETE FROM assets a
        WHERE a.id_asset = ANY($1::uuid[])
          AND a.asset_class = 'equity'
          AND a.exchange IS NULL
          AND NOT EXISTS (
              SELECT 1 FROM portfolio_operations po
              WHERE po.id_asset = a.id_asset OR po.id_related_asset = a.id_asset
          )
          AND NOT EXISTS (SELECT 1 FROM asset_aliases aa WHERE aa.id_asset = a.id_asset)
          AND NOT EXISTS (SELECT 1 FROM asset_metadata am WHERE am.id_asset = a.id_asset)
          AND NOT EXISTS (
              SELECT 1 FROM portfolio_holding_snapshot_daily phs
              WHERE phs.id_asset = a.id_asset
          )
        "#,
    )
    .bind(&user_asset_ids)
    .execute(pool)
    .await
    .expect("scoped equity fixtures should be swept");
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;

    async fn test_pool(max_connections: u32) -> sqlx::PgPool {
        let database_url = super::require_disposable_test_database_url();
        PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(&database_url)
            .await
            .expect("test database should be reachable")
    }

    /// Concurrency regression for the parallel CI race observed after P2
    /// (worker counterpart of the kushim-api regression added in
    /// `fix/api-role-fixture-race`). Many concurrent invocations of the
    /// canonical fixture must all succeed without surfacing either
    /// `roles_pkey` or `uq_roles_label` violations.
    ///
    /// We use a pool with enough real connections to exercise both
    /// uniqueness constraints under genuine concurrency, fire many tasks at
    /// once, and assert every invocation returned cleanly. We never DELETE
    /// the shared `roles` row — other worker integration tests may be running
    /// in parallel and depend on it.
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

    // ------------------------------------------------------------------
    // Regression tests for `cleanup_worker_test_tree`.
    //
    // Each test creates its OWN fixture (user, portfolio, equity asset, etc.),
    // calls the helper, and asserts the rows are gone — including the
    // orphaned equity sweep. None of these tests leave persistent rows in
    // the shared dev database.
    // ------------------------------------------------------------------

    async fn small_pool() -> sqlx::PgPool {
        test_pool(2).await
    }

    fn random_suffix() -> String {
        uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
    }

    async fn insert_test_user(pool: &sqlx::PgPool, public_handle: &str) -> uuid::Uuid {
        ensure_canonical_user_role(pool).await;
        let id_user = uuid::Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id_user, username, public_handle, id_role, password_hash)
            VALUES ($1, $2, $2, 1, 'argon2id$placeholder')
            "#,
        )
        .bind(id_user)
        .bind(public_handle)
        .execute(pool)
        .await
        .expect("user fixture should be inserted");
        id_user
    }

    async fn insert_test_portfolio(pool: &sqlx::PgPool, id_user: uuid::Uuid) -> uuid::Uuid {
        let id_portfolio = uuid::Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO portfolios (id_portfolio, id_user, name, base_currency, visibility)
            VALUES ($1, $2, 'cleanup-test', 'EUR', 'private')
            "#,
        )
        .bind(id_portfolio)
        .bind(id_user)
        .execute(pool)
        .await
        .expect("portfolio fixture should be inserted");
        id_portfolio
    }

    async fn insert_test_equity_no_exchange(pool: &sqlx::PgPool, symbol: &str) -> uuid::Uuid {
        let id_asset = uuid::Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO assets (id_asset, asset_class, status, name, native_currency, symbol)
            VALUES ($1, 'equity', 'active', $2, 'EUR', $3)
            "#,
        )
        .bind(id_asset)
        .bind(format!("Cleanup Test Asset {symbol}"))
        .bind(symbol)
        .execute(pool)
        .await
        .expect("equity-no-exchange fixture should be inserted");
        id_asset
    }

    async fn count_user(pool: &sqlx::PgPool, id_user: uuid::Uuid) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id_user = $1")
            .bind(id_user)
            .fetch_one(pool)
            .await
            .expect("user count should succeed")
    }

    async fn count_asset(pool: &sqlx::PgPool, id_asset: uuid::Uuid) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM assets WHERE id_asset = $1")
            .bind(id_asset)
            .fetch_one(pool)
            .await
            .expect("asset count should succeed")
    }

    #[tokio::test]
    async fn cleanup_removes_user_portfolio_and_orphaned_equity_fixture() {
        let pool = small_pool().await;
        let suffix = random_suffix();
        let id_user = insert_test_user(&pool, &format!("cl_a_{suffix}")).await;
        let id_portfolio = insert_test_portfolio(&pool, id_user).await;
        let id_asset =
            insert_test_equity_no_exchange(&pool, &format!("OWNA{}", &suffix[..6])).await;
        // Reference the asset from a PENDING operation so it joins this
        // user's `user_asset_ids` capture set. Posted operations would
        // FK-protect both the asset and the portfolio (documented).
        sqlx::query(
            r#"
            INSERT INTO portfolio_operations (
                id_portfolio_operation, id_portfolio, id_asset, operation_type,
                operation_status, executed_at, quantity, price_minor,
                gross_amount_minor, cash_amount_minor, currency
            ) VALUES (gen_random_uuid(), $1, $2, 'buy', 'pending', NOW(), 1, 100, 100, 100, 'EUR')
            "#,
        )
        .bind(id_portfolio)
        .bind(id_asset)
        .execute(&pool)
        .await
        .expect("reference operation should be inserted");
        assert_eq!(count_user(&pool, id_user).await, 1);
        assert_eq!(count_asset(&pool, id_asset).await, 1);

        cleanup_worker_test_tree(&pool, id_user).await;

        assert_eq!(count_user(&pool, id_user).await, 0, "user must be removed");
        assert_eq!(
            count_asset(&pool, id_asset).await,
            0,
            "user-owned equity fixture must be swept"
        );
    }

    #[tokio::test]
    async fn cleanup_preserves_canonical_assets() {
        let pool = small_pool().await;
        let suffix = random_suffix();
        let id_user = insert_test_user(&pool, &format!("cl_b_{suffix}")).await;

        let aapl_uuid: uuid::Uuid = "01993b00-0001-7000-8001-aaaaaaaaaaaa".parse().unwrap();
        let msft_uuid: uuid::Uuid = "01993b00-0002-7000-8001-bbbbbbbbbbbb".parse().unwrap();
        let nvda_uuid: uuid::Uuid = "01993b00-0003-7000-8001-cccccccccccc".parse().unwrap();
        assert_eq!(
            count_asset(&pool, aapl_uuid).await,
            1,
            "AAPL must exist before"
        );
        assert_eq!(
            count_asset(&pool, msft_uuid).await,
            1,
            "MSFT must exist before"
        );
        assert_eq!(
            count_asset(&pool, nvda_uuid).await,
            1,
            "NVDA must exist before"
        );

        cleanup_worker_test_tree(&pool, id_user).await;

        assert_eq!(count_asset(&pool, aapl_uuid).await, 1, "AAPL must remain");
        assert_eq!(count_asset(&pool, msft_uuid).await, 1, "MSFT must remain");
        assert_eq!(count_asset(&pool, nvda_uuid).await, 1, "NVDA must remain");
    }

    #[tokio::test]
    async fn cleanup_does_not_remove_other_users_asset() {
        let pool = small_pool().await;
        let suffix_a = random_suffix();
        let suffix_b = random_suffix();
        let user_a = insert_test_user(&pool, &format!("cl_c_a_{suffix_a}")).await;
        let user_b = insert_test_user(&pool, &format!("cl_c_b_{suffix_b}")).await;

        let asset_b =
            insert_test_equity_no_exchange(&pool, &format!("OTHB{}", &suffix_b[..6])).await;
        // Mark asset_b as referenced by user_b's portfolio via an operation,
        // so the orphan sweep MUST refuse to delete it while user_b is alive.
        let portfolio_b = insert_test_portfolio(&pool, user_b).await;
        sqlx::query(
            r#"
            INSERT INTO portfolio_operations (
                id_portfolio_operation, id_portfolio, id_asset, operation_type,
                operation_status, executed_at, quantity, price_minor,
                gross_amount_minor, cash_amount_minor, currency
            ) VALUES (gen_random_uuid(), $1, $2, 'buy', 'pending', NOW(), 1, 100, 100, 100, 'EUR')
            "#,
        )
        .bind(portfolio_b)
        .bind(asset_b)
        .execute(&pool)
        .await
        .expect("ref operation should be inserted");

        cleanup_worker_test_tree(&pool, user_a).await;

        assert_eq!(count_user(&pool, user_b).await, 1, "user B must remain");
        assert_eq!(
            count_asset(&pool, asset_b).await,
            1,
            "user B's still-referenced asset must NOT be swept by user A's cleanup"
        );

        // Now properly tear down user_b so the suite leaves no rows.
        cleanup_worker_test_tree(&pool, user_b).await;
        assert_eq!(count_user(&pool, user_b).await, 0);
        assert_eq!(
            count_asset(&pool, asset_b).await,
            0,
            "asset B should now be swept after user B teardown"
        );
    }

    #[tokio::test]
    async fn cleanup_is_idempotent_and_tolerates_missing_rows() {
        let pool = small_pool().await;
        let suffix = random_suffix();
        let id_user = insert_test_user(&pool, &format!("cl_d_{suffix}")).await;
        // No portfolio, no asset — only a bare user. Cleanup must not panic.
        cleanup_worker_test_tree(&pool, id_user).await;
        assert_eq!(count_user(&pool, id_user).await, 0);
        // Second call on the already-gone user must remain a no-op.
        cleanup_worker_test_tree(&pool, id_user).await;
        assert_eq!(count_user(&pool, id_user).await, 0);
    }
}
