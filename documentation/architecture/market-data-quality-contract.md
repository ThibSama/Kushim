# Market-Data Quality Contract

Status: **Implemented**. Valuation provenance is now **persisted atomically
with the holding value** in `rm_portfolio_holdings`. The API no longer joins
the live `asset_market_data` cache, so a quote written after the worker
rebuild cannot appear beside an older holding value. Provider-error
persistence and the freshness ("stale") threshold remain open product
decisions.

## Cache vs. persisted provenance — the architectural fix

`asset_market_data` is a **live cache** continuously rewritten by
`kushim-market-data`. `rm_portfolio_holdings` is a **derived snapshot**
written by `kushim-worker` only when it rebuilds the read model. Until the
fix, the API joined the two at read time, producing a false contract: a
holding's `market_value_minor` computed from quote P1 could be displayed
beside the provenance of a later quote P2 just because the cache had moved.

The fix persists the exact market-data inputs the worker consumed on the
holding row itself, and the API reads provenance **strictly** from
`rm_portfolio_holdings` — no join to `asset_market_data` anywhere on the read
path. Until a worker rebuild happens, neither the value nor the provenance
moves: this is the desired guarantee.

## Why `record_updated_at` is not `fetched_at`

`asset_market_data` exposes `as_of` (the market-quote timestamp reported by
the provider) and `updated_at` (the wall-clock time at which the row was last
written by `kushim-market-data`). It has **no actual fetch timestamp**. The
previous pass exposed `updated_at` under the name `fetched_at`, which was
misleading. This field has been **removed** and replaced by
`record_updated_at` (the persisted snapshot of `asset_market_data.updated_at`
captured at rebuild time). The name reflects the truth: it is the time the
upstream cache row was last *written*, not when the provider was *called*.

## Persisted columns on `rm_portfolio_holdings`

Added by upgrade `infra/postgres/upgrades/003_holding_valuation_provenance.sql`
and also present in fresh `001_init.sql`:

| Column                          | Type           | Meaning                                                                 |
| ------------------------------- | -------------- | ----------------------------------------------------------------------- |
| `valuation_source`              | varchar(32)    | `market_data` \| `invested_cost_fallback` \| NULL (legacy)              |
| `market_data_status`            | varchar(32)    | `available` \| `missing` \| `unsupported_currency` \| NULL (legacy)     |
| `market_data_price_minor`       | bigint         | Exact price the worker used (or rejected, for unsupported_currency).    |
| `market_data_currency`          | char(3)        | Currency of `market_data_price_minor`.                                  |
| `market_data_provider`          | varchar(50)    | `asset_market_data.data_source` captured at rebuild time. Nullable.     |
| `market_data_as_of`             | timestamptz    | `asset_market_data.as_of` captured at rebuild time.                     |
| `market_data_record_updated_at` | timestamptz    | `asset_market_data.updated_at` captured at rebuild time.                |

The database CHECK `chk_rm_portfolio_holdings_provenance_combination`
enforces the documented combinations:

| `valuation_source`         | `market_data_status`     | Numeric md_* fields        |
| -------------------------- | ------------------------ | -------------------------- |
| `market_data`              | `available`              | populated                  |
| `invested_cost_fallback`   | `missing`                | all NULL                   |
| `invested_cost_fallback`   | `unsupported_currency`   | populated (transparency)   |
| NULL (legacy)              | NULL (legacy)            | all NULL                   |

`market_data_provider` may be NULL even in the `available` row because
`asset_market_data.data_source` is itself nullable upstream.

## Legacy row handling

A `rm_portfolio_holdings` row that pre-dates the migration carries `NULL`
provenance. The API surfaces it as:

```jsonc
"market_data": {
  "available": false,
  "valuation_source": null,
  "status": "unavailable",
  "unavailable_reason": "valuation_provenance_missing",
  "price_minor": null,
  "currency": null,
  "provider": null,
  "market_data_as_of": null,
  "record_updated_at": null
}
```

The aggregate `valuation_status` does **not** count legacy rows as valued.
Operators must trigger a full worker rebuild after applying the migration so
every open holding gets accurate provenance. The legacy state therefore
degrades safely (visible, distinct reason code) rather than silently
mimicking real provenance.

## Migration procedure

The Kushim development environment runs PostgreSQL inside the Docker
container `kushim_database` (verified via `docker compose ps`). The
migration is delivered via shell stdin, but the **redirection syntax differs
between Bash and PowerShell** — both forms below are valid and produce the
exact same result.

### PowerShell (Windows, primary developer environment)

PowerShell 5.1 does not support Bash's `< file` redirection; pipe the file
contents in with `Get-Content -Raw` and use a backtick (`` ` ``) for
line continuation:

```powershell
Get-Content -Raw .\infra\postgres\upgrades\003_holding_valuation_provenance.sql |
    docker exec -i kushim_database `
        psql -v ON_ERROR_STOP=1 -U kushim -d kushim
```

### Bash / POSIX shells

```bash
docker exec -i kushim_database \
    psql -v ON_ERROR_STOP=1 -U kushim -d kushim \
    < infra/postgres/upgrades/003_holding_valuation_provenance.sql
```

### Idempotence

The script is **idempotent**: every `ADD COLUMN` uses `IF NOT EXISTS` and
every `ADD CONSTRAINT` is gated by a `pg_constraint` lookup inside a
`DO $$ ... $$` block. Running it a second time logs `NOTICE` messages for
already-present columns and is a no-op for constraints.

### Automated validation

Before applying to any persistent database, run the disposable validator
which bootstraps the committed pre-migration schema, inserts a
representative legacy row through the real foreign-key chain, applies the
migration twice, asserts every original financial field is unchanged, every
new provenance column is NULL, and that a forbidden combination is rejected
by the CHECK:

```powershell
.\scripts\test\validate-holding-valuation-provenance-migration.ps1
```

The script creates and drops a `kushim_test_migval_*` database in a
`finally` block; the development database is never touched.

### Backups before applying the migration

The migration is additive, but the worker rebuild that must follow rewrites
every `rm_portfolio_holdings` row for every portfolio. Take a plain-SQL
backup pair first; both are produced by `pg_dump` in its **default plain
text format**, so the restore uses `psql` — **not** `pg_restore`.

```powershell
$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$backupDir = Join-Path $env:TEMP "kushim_valuation_provenance_$timestamp"
New-Item -ItemType Directory -Path $backupDir -Force | Out-Null

# Plain-SQL schema dump (NOT custom/tar format → restore with psql, not pg_restore).
docker exec -i kushim_database pg_dump -U kushim -d kushim --schema-only `
    > "$backupDir\schema.sql"

# Plain-SQL data dump of the single read-model table the rebuild rewrites.
docker exec -i kushim_database pg_dump -U kushim -d kushim --data-only `
    --table=rm_portfolio_holdings > "$backupDir\rm_portfolio_holdings.sql"

# Row count snapshot for later operator decision.
$count = (docker exec -i kushim_database psql -U kushim -d kushim -t -A `
    -c "SELECT COUNT(*) FROM rm_portfolio_holdings;" | Out-String).Trim()
$count | Out-File -Encoding ascii "$backupDir\rm_portfolio_holdings_count.txt"
```

Keep `$backupDir` outside the repository tree (`$env:TEMP\...`) so it is
never staged accidentally.

### Rollback procedure (plain SQL → psql)

> **Operator decision required.** Restoring `rm_portfolio_holdings` from a
> pre-migration backup is **destructive** — it discards every read-model
> row written by every rebuild that ran since the backup, including
> provenance that the new contract relies on. Never `TRUNCATE` or restore
> this table silently. Read every step below before running anything.

The backups produced above are **plain SQL**, so:

- **DO** use `psql` to apply them.
- **DO NOT** use `pg_restore` — it only handles the custom (`-Fc`) and
  directory (`-Fd`) / tar (`-Ft`) formats; on plain SQL it exits with an
  unhelpful error.

#### Step 0 — verify target

```powershell
$db = (docker exec -i kushim_database psql -U kushim -d kushim -t -A `
        -c "SELECT current_database();" | Out-String).Trim()
if ($db -ne 'kushim') {
    throw "Refused: target is '$db', expected 'kushim'."
}
```

#### Step 1 — schema rollback (rarely needed)

The migration is **additive**: dropping the new columns and constraints is
itself a destructive operation that breaks any code reading them. If you
truly need to undo the schema additions, do it column-by-column / constraint
-by-constraint by hand. Re-applying the schema dump in full is **not**
recommended on a live database because it will fail on every existing
table. The schema dump is kept as a **reference**, not an automatic
rollback target.

#### Step 2 — `rm_portfolio_holdings` data rollback (operator-supervised)

If a rebuild produced incorrect derived data and you want to restore the
pre-migration snapshot of `rm_portfolio_holdings`:

```powershell
$backupDir = "$env:TEMP\kushim_valuation_provenance_<timestamp>"

# Explicit operator confirmation: read this row count first.
Get-Content "$backupDir\rm_portfolio_holdings_count.txt"

# Truncate ONLY after the operator has confirmed the target and that
# losing the post-rebuild rows is acceptable. The CASCADE keyword is
# omitted intentionally — this table has no incoming FKs and we want
# the command to fail loudly if a future migration introduces one.
docker exec -i kushim_database psql -v ON_ERROR_STOP=1 -U kushim -d kushim `
    -c "TRUNCATE TABLE rm_portfolio_holdings;"

# Restore plain-SQL data dump via psql (NOT pg_restore).
Get-Content -Raw "$backupDir\rm_portfolio_holdings.sql" |
    docker exec -i kushim_database `
        psql -v ON_ERROR_STOP=1 -U kushim -d kushim
```

#### Plain SQL vs custom format — quick reference

| Dump format produced by      | Restore tool |
| ---------------------------- | ------------ |
| `pg_dump` (default plain)    | `psql`       |
| `pg_dump -Fc` (custom)       | `pg_restore` |
| `pg_dump -Fd` (directory)    | `pg_restore` |
| `pg_dump -Ft` (tar)          | `pg_restore` |

The backups described above are all plain SQL.

### After migration: rebuild required

Existing read-model rows carry `NULL` provenance and are surfaced by the
API as `valuation_provenance_missing`. The operator must trigger a full
worker rebuild for every active portfolio so each open holding gets
accurate provenance.

## Timestamp wording

`market_data_record_updated_at` (column) and `record_updated_at` (API/UI
field) refer **only** to the time at which the accepted
`asset_market_data` row was last written or updated in the local database
(captured at rebuild time). It is **not**:

- the provider fetch-start time;
- the provider response time;
- the last synchronization attempt;
- an upstream exchange timestamp.

Use the wording «  Market-data record updated at  » or «  Enregistrement
de marché mis à jour le  ». Do **not** call this field "last
synchronization" or "fetched_at".

## Goal

Expose the **provenance** and **availability** of every market price along the
full pipeline so consumers (UI, future analytics) can reason about a price
explicitly instead of inferring `0.00` or silently estimating values.

## Pipeline

`provider → asset_market_data → kushim-worker (read models) → kushim-api → kushim-app`

This document describes the contract delivered to `kushim-api` and `kushim-app`.

## `asset_market_data` write guarantees (`kushim-market-data`)

The upsert in `kushim-market-data/src/repositories/asset_market_data.rs`
`upsert_current` enforces:

- **First write wins.** No row → insert.
- **Older incoming data never overwrites newer.** `ON CONFLICT` updates only
  when `EXCLUDED.as_of >= asset_market_data.as_of`. Stale provider responses
  are silently no-op; the function returns `rows_affected = 0` and the job
  counts the asset as `skipped`.
- **Idempotent.** Repeating the same quote bumps `updated_at` only.
- **Provider failure is never persisted as zero/NULL.** When the provider
  errors, no upsert is attempted and the previous row is preserved.

### Conflict guard — observable idempotence

The `WHERE` clause on the conflict update is:

```
EXCLUDED.as_of > stored.as_of
OR (
    EXCLUDED.as_of = stored.as_of
    AND (price_minor / currency / data_source IS DISTINCT FROM stored)
)
```

Consequences:

| Incoming                              | `rows_affected` | `updated_at` | Notes                  |
| ------------------------------------- | --------------- | ------------ | ---------------------- |
| Strictly older `as_of`                | 0               | unchanged    | Silently dropped       |
| Same `as_of`, identical payload       | 0               | unchanged    | Deterministic replay   |
| Same `as_of`, payload differs         | 1               | advances     | Correction accepted    |
| Strictly newer `as_of`                | 1               | advances     | Normal refresh         |

This guarantees that a deterministic replay (same provider, same response)
never advances `asset_market_data.updated_at` — and therefore never
advances the `record_updated_at` the worker later snapshots into
`rm_portfolio_holdings`. Consumers can trust that `record_updated_at`
only moves when the stored cache snapshot actually changed.

### Not yet implemented

- **Provider error persistence.** There is no `last_error_at` /
  `last_error_code` column. The API therefore cannot expose a stable `error`
  status. Adding it requires a DDL migration.
- **Concurrency.** The single-row `UNIQUE (id_asset)` constraint + `ON
  CONFLICT` already serialize writers. No explicit additional lock is needed
  for the current single-job scheduler. If multiple concurrent refresh jobs
  are ever introduced, the existing guard remains correct.

## Read contract exposed by `kushim-api`

### Holdings — `GET /v1/portfolios/{id}/holdings`

Every holding now carries an explicit `market_data` block produced by a
**LEFT JOIN onto `asset_market_data`** in
`kushim-api/src/repositories/portfolio_read_models.rs`. The join is read-only;
the architectural guard test in `http/portfolio_read_models.rs` forbids any
`INSERT/UPDATE/DELETE` against this table from `kushim-api`.

```jsonc
"market_data": {
  "available": true,
  "valuation_source": "market_data",
  "status": "available",
  "unavailable_reason": null,
  "price_minor": 6000,
  "currency": "EUR",
  "provider": "test-static",
  "market_data_as_of": "2026-06-18T11:30:00Z",       // provider quote timestamp captured at rebuild
  "record_updated_at": "2026-06-18T11:42:01Z"        // local cache row's last-write timestamp captured at rebuild
}
```

When no `asset_market_data` row existed at the time of the rebuild:

```jsonc
"market_data": {
  "available": false,
  "valuation_source": "invested_cost_fallback",
  "status": "unavailable",
  "unavailable_reason": "market_data_missing",
  "price_minor": null,
  "currency": null,
  "provider": null,
  "market_data_as_of": null,
  "record_updated_at": null
}
```

When a row existed but its currency is incompatible with the holding's
base currency (the worker fell back to invested cost; the incompatible
row's provenance is preserved for transparency):

```jsonc
"market_data": {
  "available": false,
  "valuation_source": "invested_cost_fallback",
  "status": "unavailable",
  "unavailable_reason": "unsupported_market_data_currency",
  "price_minor": 6000,
  "currency": "USD",
  "provider": "test-static",
  "market_data_as_of": "2026-06-18T11:30:00Z",
  "record_updated_at": "2026-06-18T11:42:01Z"
}
```

For a legacy row created before the migration (not yet rebuilt):

```jsonc
"market_data": {
  "available": false,
  "valuation_source": null,
  "status": "unavailable",
  "unavailable_reason": "valuation_provenance_missing",
  "price_minor": null,
  "currency": null,
  "provider": null,
  "market_data_as_of": null,
  "record_updated_at": null
}
```

The numeric `market_value_minor` is still populated by the worker (it falls
back to invested cost when no price exists, and the existing `is_estimated`
flag stays set). The app uses `market_data.available` to decide whether to
**display** that number or render `—`. This avoids silently converting missing
data into `0.00` without changing worker behaviour.

### Summary — `GET /v1/portfolios/{id}/summary`

The summary now exposes an aggregate **valuation status** derived from open
positions vs. matching market-data rows:

```jsonc
{
  "valuation_status": "complete" | "partial" | "unavailable" | "empty",
  "positions_total": 4,    // number of open positions
  "positions_valued": 3    // subset with a market-data row
}
```

Mapping (`PortfolioValuationStatus::from_counts`):

| `positions_total` | `positions_valued` | Status        |
| ----------------- | ------------------ | ------------- |
| 0                 | —                  | `empty`       |
| ≥1                | 0                  | `unavailable` |
| ≥1                | < total            | `partial`     |
| ≥1                | ≥ total            | `complete`    |

**A holding is counted as valued only when its `asset_market_data` row exists
*and* its currency matches the holding's base currency (case-insensitive,
trimmed).** This mirrors the worker's price-usability rule in
`portfolio_state.rs::finalize`. Without this guard, a USD market-data row
attached to a EUR portfolio would silently inflate `positions_valued` while
the worker still flags the position as estimated.

This is distinct from `portfolio_status` (`active` / `empty` / `archived`),
which only reflects the lifecycle of the portfolio container.

## Frontend usage (`kushim-app`)

`Positions` reads `holding.market_data` and:

- Renders `—` (instead of `formatMinorCurrency(0)`) when `available === false`.
- Shows an `Indispo.` chip on rows with no market data, distinct from the
  existing `Est.` chip (which still flags FX / split / spinoff / missing-price
  estimation).
- Surfaces a banner when at least one open position has no price, including a
  `(valued / total)` count.
- Tooltips on the value/P&L cells display the provider, fetch timestamp and
  market-quote timestamp.

The summary card aggregates the same information; the totals shown sum **only
valued positions** so the headline figure cannot silently include an
invested-cost fallback.

## Deliberate non-decisions

The following remain **open product / architecture decisions** and are not
emitted by this pass:

1. **Stale threshold.** No `stale` status is exposed because no freshness
   duration is documented. Until product picks a threshold (per asset class or
   global), the raw `market_data_as_of` / `record_updated_at` timestamps are exposed and
   consumers may compute staleness client-side if they need to.
2. **Provider error status.** `unavailable_reason = "provider_error"` is **not**
   emitted because errors are not persisted in `asset_market_data`. Adding it
   requires a DDL migration to introduce `last_error_at` and `last_error_code`.
3. **Worker valuation rule under partial portfolios.** The worker still falls
   back to invested cost for unpriced positions and flags them with
   `is_estimated`. Changing this contract (e.g. excluding them from
   `total_value_minor` entirely) is deferred — the API/app already distinguish
   the two cases via `market_data.available` and `valuation_status`.
4. **FX support.** Foreign-currency positions remain flagged via the existing
   `is_estimated` boolean; the new `market_data` block does not yet carry an FX
   reason code.

## Files touched

- `kushim-market-data/src/repositories/asset_market_data.rs` — guard +
  return type + tests.
- `kushim-market-data/src/jobs/refresh_current_market_data.rs` — handle
  `rows_affected = 0` as `skipped`.
- `kushim-api/src/domain/portfolio_read_model.rs` — new types.
- `kushim-api/src/repositories/portfolio_read_models.rs` — LEFT JOIN +
  `valuation_breakdown`.
- `kushim-api/src/services/portfolio_read_models.rs` — populate summary
  valuation fields.
- `kushim-api/src/http/portfolio_read_models.rs` — DTO additions + tests.
- `kushim-app/src/lib/api/businessApi.ts` — TS contract.
- `kushim-app/src/app/pages/Positions.tsx` — provenance / unavailability UI.
- `documentation/architecture/market-data-quality-contract.md` — this file.
