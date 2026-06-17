# scripts/test — Isolated Rust test database runner

## Why this exists

Direct `cargo test` against the local development database `kushim`
accumulates persistent rows that the per-test cleanup cannot remove.
The schema-level immutability trigger `prevent_posted_operation_mutation`
protects posted `portfolio_operations` against any DELETE — so every test
that posts an operation leaves a residue (the posted row plus the user
and portfolio FK-protected by it). The previous cleanup pass reduced but
did not eliminate this growth.

## Canonical commands

```powershell
.\scripts\test\run-rust-db-suite.ps1 -Service kushim-api
.\scripts\test\run-rust-db-suite.ps1 -Service kushim-worker
```

These run, **in a brand-new disposable PostgreSQL database**, every
quality gate for the requested service:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo audit --ignore RUSTSEC-2023-0071`

The runner exits non-zero if any step fails.

## Database lifecycle

| Step | Action |
|---|---|
| 1 | Generate name `kushim_test_<service>_<utc_ts>_<rand>` |
| 2 | `CREATE DATABASE` via the local `kushim_database` container |
| 3 | Bootstrap from `infra/postgres/init/*.sql` in lexical order |
| 4 | Set `DATABASE_URL` for the child cargo process only |
| 5 | Run fmt → clippy → test → audit, preserving exit codes |
| 6 | Terminate live connections, `DROP DATABASE`, verify gone |

The `finally` block always runs cleanup — even when a step fails.

## Safety invariants

1. Generated database names start with `kushim_test_` and match
   `^kushim_test_[A-Za-z0-9_]+$`. The runner refuses to drop any name
   that does not match this regex.
2. Hard-coded forbidden names: `kushim`, `postgres`, `template0`,
   `template1`.
3. The parent shell's `DATABASE_URL` is preserved byte-for-byte across
   every invocation (saved and restored around each `cargo` call).
4. API and worker invocations always create distinct database names,
   so they can run in parallel without collision.
5. Bootstrap uses the canonical source-of-truth SQL files only, never a
   clone of the polluted dev database.

## Fail-closed test guard

Both crates expose `require_disposable_test_database_url()` in their
test-support module:

- `kushim-api/src/test_support.rs`
- `kushim-worker/src/test_utils.rs`

Every database-backed `test_pool()` calls this guard, which panics
unless the database name starts with `kushim_test_`. This is
defense-in-depth: if someone bypasses the runner and tries
`cargo test` with the dev `DATABASE_URL`, the test panics BEFORE
inserting a single row.

The escape hatch `KUSHIM_ALLOW_SHARED_TEST_DATABASE=1` exists for the
unlikely case where CI must temporarily fall back to a shared
development pool. It is **unsafe** (will leak rows) and is never
enabled by the runner or normal CI.

## Options

| Flag | Purpose |
|---|---|
| `-Service <kushim-api\|kushim-worker>` | Required. Selects the suite. |
| `-KeepDatabaseOnFailure` | Leave the temporary DB in place when the suite fails (for inspection). Always dropped on success. |
| `-SkipAudit` | Skip `cargo audit`. Useful for offline local runs. CI must not pass this. |

## Diagnostic recipes

Inspect a kept-on-failure database:

```powershell
docker exec -it kushim_database psql -U kushim -d kushim_test_<...>
```

Manually drop a leftover (rare — only if the runner crashed before
its `finally` block):

```powershell
docker exec -i kushim_database psql -U kushim -d kushim `
  -c 'DROP DATABASE "kushim_test_<...>"'
```

List leftovers:

```powershell
docker exec kushim_database psql -U kushim -d kushim -t -A `
  -c "SELECT datname FROM pg_database WHERE datname LIKE 'kushim_test_%'"
```

## CI usage

Replace any direct `cargo test` step in CI with the runner. The
`-SkipAudit` flag must not be passed in CI.

## Why not transaction rollback?

The Axum-based API and the worker each acquire independent pool
connections per request/job. Wrapping each test in a single transaction
and rolling it back at the end is incompatible with that architecture
(the test would never see the committed state the application code
expects). Disposable databases per suite invocation provide equivalent
isolation without changing production behavior.
