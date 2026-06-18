<#
.SYNOPSIS
    End-to-end validator for migration 003_holding_valuation_provenance.sql.

.DESCRIPTION
    Creates a disposable PostgreSQL database, bootstraps the committed
    pre-migration schema (taken from the current `HEAD`'s
    `infra/postgres/init/*.sql`), inserts a representative legacy
    `rm_portfolio_holdings` row using the real foreign-key chain, applies
    the migration, asserts the legacy row's financial fields are unchanged
    and its new provenance fields are NULL, applies the migration a SECOND
    time to prove idempotence, then verifies every CHECK constraint
    introduced by the migration including the combinational rule.

    Safety invariants:
      - The disposable database name MUST start with `kushim_test_` -- the
        runner refuses to operate on any other name.
      - The script NEVER touches `kushim`, `postgres`, `template0`,
        `template1`, or any name failing the safety regex.
      - The disposable database is ALWAYS dropped in the `finally` block,
        even on failure (unless -KeepDatabaseOnFailure is set).
      - The migration is applied to the disposable database only -- the
        development database is never modified.

.PARAMETER KeepDatabaseOnFailure
    When set, do NOT drop the disposable database on failure so the
    operator can connect and inspect it. Always dropped on success.

.PARAMETER CiMode
    When set, run psql directly against the host pointed to by the standard
    PG* environment variables (PGHOST / PGPORT / PGUSER / PGPASSWORD /
    PGDATABASE) instead of going through `docker exec` on a local container.
    Used by the GitHub Actions job which provides PostgreSQL as a service.

.PARAMETER BaseRef
    Git ref pointing to the pre-migration schema baseline. The validator
    reads `infra/postgres/init/*.sql` from that ref to bootstrap the old
    schema.

    This script is bound to **migration 003**
    (`infra/postgres/upgrades/003_holding_valuation_provenance.sql`). Its
    correct pre-migration baseline is therefore **immutable**: the commit
    that was the head of `main` immediately before migration 003 was
    introduced. That commit is `bcf8a009e1e7147f02335bfb1a676ad3c2dd84e8`
    and is hard-coded in `$Migration003BaselineRef` below.

    Resolution order:

      1. `-BaseRef <commit>` passed by the caller (manual override / diag).
      2. Otherwise the migration-003 immutable baseline.

    NO topology-based fallback is ever used. Specifically, the validator
    NEVER consults:
        - `HEAD`, `HEAD^1`
        - `git merge-base HEAD origin/main`
        - `github.event.pull_request.base.sha`
        - `github.event.before`
    Once migration 003 is on main, all of those degenerate to a
    post-migration commit and would silently invalidate the test. The
    content guard would still catch it, but the right answer for a
    migration-specific historical validator is to pin the baseline at the
    migration's pre-image.

    The content guard (`infra/postgres/init/001_init.sql` at the chosen
    ref must contain zero provenance column declarations) still runs and
    rejects any candidate that is already post-migration, regardless of
    how it was selected — this protects against an operator passing a
    bad `-BaseRef`.

.EXAMPLE
    # Local developer (Docker Desktop, uncommitted WIP):
    .\scripts\test\validate-holding-valuation-provenance-migration.ps1

.EXAMPLE
    # GitHub Actions (postgres service container, no docker exec needed):
    pwsh -NoProfile -File ./scripts/test/validate-holding-valuation-provenance-migration.ps1 -CiMode

.EXAMPLE
    # Explicit baseline ref (e.g., a tag or a specific commit):
    .\scripts\test\validate-holding-valuation-provenance-migration.ps1 -BaseRef v0.4.0
#>

[CmdletBinding()]
param(
    [switch]$KeepDatabaseOnFailure,
    [switch]$CiMode,
    [string]$BaseRef = ''
)

# --------------------------------------------------------------------------
# IMMUTABLE BASELINE for migration 003.
# This is the commit that was the head of `main` immediately before
# `infra/postgres/upgrades/003_holding_valuation_provenance.sql` was added.
# It does not depend on the current branch, the current event, or whether
# the migration has been merged. Updating this value requires editing this
# script — that's intentional: the validator is migration-003-specific and
# the baseline is part of its contract.
# --------------------------------------------------------------------------
$Migration003BaselineRef = 'bcf8a009e1e7147f02335bfb1a676ad3c2dd84e8'

$repoRootInit = (Resolve-Path "$PSScriptRoot\..\..").Path
if ([string]::IsNullOrWhiteSpace($BaseRef)) {
    $BaseRef = $Migration003BaselineRef
    Write-Host "[validator] BaseRef defaulted to Migration003BaselineRef = $BaseRef"
}

# Resolve the ref to a stable SHA and reject candidates that are already
# post-migration. The content check reads `infra/postgres/init/001_init.sql`
# from $BaseRef and looks for the seven provenance column NAMES. This is
# safer than relying solely on commit topology, which can lie after merges,
# rebases, force-pushes, or when -BaseRef is misused.
$resolvedSha = ((git -C $repoRootInit rev-parse --verify "$BaseRef" 2>$null) -join '').Trim()
if ($LASTEXITCODE -ne 0 -or -not $resolvedSha) {
    throw "BaseRef '$BaseRef' does not resolve to any commit. Run ``git fetch`` and retry, or pass a different -BaseRef."
}
Write-Host "[validator] BaseRef = $BaseRef (resolved SHA = $resolvedSha)"

$candidateInit = (git -C $repoRootInit show "${BaseRef}:infra/postgres/init/001_init.sql" 2>&1) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw "Cannot read infra/postgres/init/001_init.sql from BaseRef '$BaseRef': $candidateInit"
}
# Match a *column definition* (column name followed by a SQL type keyword
# at the start of a line), not arbitrary mentions of these names elsewhere
# in the file. This avoids false positives if the names appear as
# comments, strings or CHECK constraint expressions.
$provenanceColumnPattern = '(?m)^\s+(valuation_source|market_data_status|market_data_price_minor|market_data_currency|market_data_provider|market_data_as_of|market_data_record_updated_at)\s+(varchar|bigint|char|timestamptz)'
$alreadyPresent = ([regex]::Matches($candidateInit, $provenanceColumnPattern)).Count
if ($alreadyPresent -gt 0) {
    throw @"
BaseRef '$BaseRef' ($resolvedSha) ALREADY contains $alreadyPresent provenance column declaration(s) in infra/postgres/init/001_init.sql.

This is NOT a pre-migration baseline for migration 003. The validator
refuses to use it because bootstrapping it would mask whatever the
upgrade is supposed to add, and the post-migration assertions would
also fail or be vacuous.

This script is migration-003-specific. Its baseline is the IMMUTABLE
commit pinned at $Migration003BaselineRef -- set deliberately so it
cannot drift as main moves forward.

Remediation:
  * Most operators: run without -BaseRef so the immutable baseline is
    used.
  * Diagnostic / manual override: pass -BaseRef <pre-migration commit>
    where the chosen commit's infra/postgres/init/001_init.sql does
    NOT yet contain the seven provenance columns.
  * Do NOT pass HEAD, HEAD^1, origin/main, or any derivative of the
    current branch topology -- once migration 003 is on main, those
    refs are all post-migration.
"@
}
Write-Host "[validator] BaseRef content check: 0 provenance column declarations (pre-migration confirmed)."

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

$repoRoot   = (Resolve-Path "$PSScriptRoot\..\..").Path
$container  = 'kushim_database'  # local-mode docker container
$adminUser  = if ($CiMode) { $env:PGUSER     } else { 'kushim' }
$adminDb    = if ($CiMode) { $env:PGDATABASE } else { 'kushim' }
$initSqlDir = Join-Path $repoRoot 'infra\postgres\init'
$migration  = Join-Path $repoRoot 'infra\postgres\upgrades\003_holding_valuation_provenance.sql'

$safeNameRegex = '^kushim_test_[A-Za-z0-9_]+$'

if ($CiMode) {
    # `kushim_test_ci_admin` is the documented service-database name CI
    # bootstraps; the validator then CREATEs a kushim_test_migval_* database
    # alongside it so the safety regex is satisfied without renaming the
    # service container.
    if ([string]::IsNullOrEmpty($env:PGUSER))     { throw "CI mode requires PGUSER." }
    if ([string]::IsNullOrEmpty($env:PGDATABASE)) { throw "CI mode requires PGDATABASE." }
    if ([string]::IsNullOrEmpty($env:PGPASSWORD)) { throw "CI mode requires PGPASSWORD." }
    if ([string]::IsNullOrEmpty($env:PGHOST))     { $env:PGHOST = 'localhost' }
    if ([string]::IsNullOrEmpty($env:PGPORT))     { $env:PGPORT = '5432' }
    Write-Host "[validator] Mode: CI (psql direct, host=$($env:PGHOST):$($env:PGPORT), user=$($env:PGUSER), admin db=$($env:PGDATABASE))"
} else {
    Write-Host "[validator] Mode: local (docker exec -i $container)"
}

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

function Assert-SafeDatabaseName {
    param([string]$Name)
    if ([string]::IsNullOrWhiteSpace($Name)) {
        throw "Refused: database name is empty."
    }
    if ($Name -notmatch $safeNameRegex) {
        throw "Refused: database name '$Name' does not match $safeNameRegex."
    }
    foreach ($forbidden in @('kushim', 'postgres', 'template0', 'template1')) {
        if ($Name -eq $forbidden) {
            throw "Refused: hard-coded forbidden name '$Name'."
        }
    }
}

# Build the platform-appropriate psql command prefix. In local mode we go
# through `docker exec -i kushim_database psql ...`; in CI mode we use the
# psql binary preinstalled on ubuntu-latest, talking to the postgres service
# directly via PG* env vars.
function Invoke-PsqlCore {
    param(
        [Parameter(Mandatory = $true)][string]$Database,
        [Parameter(Mandatory = $true)][string]$Sql,
        [switch]$Tuples
    )
    # psql writes NOTICEs to stderr; under $ErrorActionPreference='Stop' that
    # would terminate the script even when the SQL succeeded. Locally
    # downgrade to 'Continue' for the native call and rely on $LASTEXITCODE
    # for truth.
    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        if ($CiMode) {
            $args = @('-v', 'ON_ERROR_STOP=1', '-U', $adminUser, '-d', $Database)
            if ($Tuples) { $args += @('-t', '-A') } else { $args += '-q' }
            $output = $Sql | psql @args 2>&1
        } else {
            $args = @('exec', '-i', $container, 'psql', '-v', 'ON_ERROR_STOP=1', '-U', $adminUser, '-d', $Database)
            if ($Tuples) { $args += @('-t', '-A') } else { $args += '-q' }
            $output = $Sql | docker @args 2>&1
        }
        $exit = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $prevPref
    }
    return @{ Output = ($output | Out-String); ExitCode = $exit }
}

function Invoke-Psql {
    param(
        [Parameter(Mandatory = $true)][string]$Database,
        [Parameter(Mandatory = $true)][string]$Sql
    )
    Assert-SafeDatabaseName -Name $Database
    $r = Invoke-PsqlCore -Database $Database -Sql $Sql
    if ($r.ExitCode -ne 0) {
        throw "psql failed on $Database (exit $($r.ExitCode)): $($r.Output)"
    }
    return $r.Output
}

function Invoke-PsqlAdmin {
    param([Parameter(Mandatory = $true)][string]$Sql)
    $r = Invoke-PsqlCore -Database $adminDb -Sql $Sql
    if ($r.ExitCode -ne 0) {
        throw "psql admin failed (exit $($r.ExitCode)): $($r.Output)"
    }
    return $r.Output
}

function Invoke-PsqlScalar {
    param(
        [Parameter(Mandatory = $true)][string]$Database,
        [Parameter(Mandatory = $true)][string]$Sql
    )
    Assert-SafeDatabaseName -Name $Database
    $r = Invoke-PsqlCore -Database $Database -Sql $Sql -Tuples
    if ($r.ExitCode -ne 0) {
        throw "psql scalar failed on $Database (exit $($r.ExitCode)): $($r.Output)"
    }
    return ($r.Output | Out-String).Trim()
}

function Apply-FileFromGit {
    param(
        [Parameter(Mandatory = $true)][string]$Database,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )
    Assert-SafeDatabaseName -Name $Database
    Write-Host "  - applying $RelativePath from $BaseRef"
    $sql = (git -C $repoRoot show "${BaseRef}:$RelativePath") -join "`n"
    if ($LASTEXITCODE -ne 0) {
        throw "git show failed for ${BaseRef}:$RelativePath -> $sql"
    }
    $r = Invoke-PsqlCore -Database $Database -Sql $sql
    if ($r.ExitCode -ne 0) {
        throw "Bootstrap $RelativePath failed on ${Database}: $($r.Output)"
    }
}

function Apply-LocalFile {
    param(
        [Parameter(Mandatory = $true)][string]$Database,
        [Parameter(Mandatory = $true)][string]$Path
    )
    Assert-SafeDatabaseName -Name $Database
    if (-not (Test-Path $Path)) { throw "File not found: $Path" }
    Write-Host "  - applying $Path"
    $sql = Get-Content -Raw -Path $Path
    $r = Invoke-PsqlCore -Database $Database -Sql $sql
    if ($r.ExitCode -ne 0) {
        throw "Apply $Path failed on ${Database}: $($r.Output)"
    }
}

function New-DisposableDatabaseName {
    $ts   = (Get-Date).ToUniversalTime().ToString('yyyyMMddHHmmss')
    $rand = -join ((48..57) + (97..102) | Get-Random -Count 6 | ForEach-Object { [char]$_ })
    return "kushim_test_migval_${ts}_$rand"
}

function Drop-DisposableDatabase {
    param([Parameter(Mandatory = $true)][string]$Name)
    Assert-SafeDatabaseName -Name $Name
    Write-Host "[validator] DROP DATABASE $Name"
    $terminate = @"
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE datname = '$Name' AND pid <> pg_backend_pid();
"@
    [void](Invoke-PsqlAdmin -Sql $terminate)
    [void](Invoke-PsqlAdmin -Sql "DROP DATABASE IF EXISTS `"$Name`"")
}

# ---------------------------------------------------------------------------
# Pre-flight
# ---------------------------------------------------------------------------

if (-not $CiMode) {
    $running = docker ps --filter "name=^/$container$" --filter 'status=running' --format '{{.Names}}'
    if (-not $running) {
        throw "PostgreSQL container '$container' is not running. Start with: docker compose up -d database"
    }
}

if (-not (Test-Path $migration)) {
    throw "Migration file not found: $migration"
}

$dbName = New-DisposableDatabaseName
Assert-SafeDatabaseName -Name $dbName
Write-Host "[validator] Disposable DB: $dbName"

$exitCode = 1
$created = $false

try {
    # -----------------------------------------------------------------------
    # 1. Create disposable DB and bootstrap committed old schema from HEAD.
    # -----------------------------------------------------------------------
    [void](Invoke-PsqlAdmin -Sql "CREATE DATABASE `"$dbName`"")
    $created = $true
    Write-Host "[validator] CREATE DATABASE OK"

    # Apply all init/*.sql in lexicographic order from the chosen baseline
    # ref so the bootstrap reflects the pre-migration state byte-for-byte.
    # The ref is `$BaseRef` (default 'HEAD' locally, 'origin/main' in CI).
    $initFiles = git -C $repoRoot ls-tree -r --name-only $BaseRef -- 'infra/postgres/init/' |
        Where-Object { $_ -like '*.sql' } |
        Sort-Object
    if (-not $initFiles) { throw "No init/*.sql tracked at $BaseRef." }
    Write-Host "[validator] Bootstrapping from ${BaseRef} init/*.sql ($($initFiles.Count) files)"
    foreach ($f in $initFiles) {
        # Apply-FileFromGit reads through `git show ${BaseRef}:` so any
        # working-tree edits and any commits past `$BaseRef` are invisible
        # by design — the bootstrap is the pre-migration snapshot.
        Apply-FileFromGit -Database $dbName -RelativePath $f
    }

    # -----------------------------------------------------------------------
    # 2. Confirm the old schema does NOT yet contain the new provenance columns.
    # -----------------------------------------------------------------------
    $preColCount = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM information_schema.columns
WHERE table_name = 'rm_portfolio_holdings'
  AND column_name IN ('valuation_source','market_data_status','market_data_price_minor',
                      'market_data_currency','market_data_provider','market_data_as_of',
                      'market_data_record_updated_at');
"@
    if ($preColCount -ne '0') {
        throw "Pre-migration schema already contains $preColCount provenance columns -- bootstrap is wrong."
    }
    Write-Host "[validator] Pre-migration: 0/7 provenance columns present (expected 0)."

    # -----------------------------------------------------------------------
    # 3. Insert representative legacy row through the real FK chain.
    #    Roles + users + portfolios + assets + rm_portfolio_holdings.
    # -----------------------------------------------------------------------
    $legacyRowSql = @"
INSERT INTO roles (id_role, label) VALUES (1, 'user') ON CONFLICT (id_role) DO NOTHING;
INSERT INTO users (id_user, id_role, username, public_handle, password_hash)
VALUES ('11111111-1111-1111-1111-111111111111', 1, 'mig_validator', 'mig_validator', 'x');
INSERT INTO portfolios (id_portfolio, id_user, name, base_currency, visibility)
VALUES ('22222222-2222-2222-2222-222222222222',
        '11111111-1111-1111-1111-111111111111',
        'Mig Validator', 'EUR', 'private');
INSERT INTO assets (id_asset, asset_class, status, name, native_currency, ticker, exchange)
VALUES ('33333333-3333-3333-3333-333333333333', 'equity', 'active',
        'MigAsset', 'EUR', 'MIGV', 'NYSE');
INSERT INTO rm_portfolio_holdings (
  id_portfolio, id_asset, base_currency, quantity, avg_cost_minor,
  invested_base_minor, market_value_minor, pnl_base_minor,
  pnl_pct, weight_pct, position_status, is_estimated, as_of
)
VALUES (
  '22222222-2222-2222-2222-222222222222',
  '33333333-3333-3333-3333-333333333333',
  'EUR', 1.5000000000, 33333,
  50000, 75000, 25000,
  50.0000, 100.0000, 'open', false, '2026-06-10T08:00:00Z'::timestamptz
);
"@
    [void](Invoke-Psql -Database $dbName -Sql $legacyRowSql)
    Write-Host "[validator] Legacy fixture inserted: roles+users+portfolios+assets+rm_portfolio_holdings."

    # -----------------------------------------------------------------------
    # 4. Record every original financial+identity field.
    # -----------------------------------------------------------------------
    $beforeCsv = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT id_portfolio||'|'||id_asset||'|'||trim(base_currency)||'|'||quantity::text||'|'||
       COALESCE(avg_cost_minor::text,'NULL')||'|'||invested_base_minor::text||'|'||
       market_value_minor::text||'|'||pnl_base_minor::text||'|'||
       COALESCE(pnl_pct::text,'NULL')||'|'||COALESCE(weight_pct::text,'NULL')||'|'||
       position_status||'|'||is_estimated::text||'|'||
       to_char(as_of AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS.US')||'|'||
       to_char(updated_at AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS.US')
FROM rm_portfolio_holdings
WHERE id_portfolio = '22222222-2222-2222-2222-222222222222'
  AND id_asset = '33333333-3333-3333-3333-333333333333';
"@
    Write-Host "[validator] BEFORE snapshot: $beforeCsv"

    # -----------------------------------------------------------------------
    # 5. Apply the migration (first run).
    # -----------------------------------------------------------------------
    Write-Host "[validator] First migration run..."
    Apply-LocalFile -Database $dbName -Path $migration
    Write-Host "[validator] First migration run: OK."

    # -----------------------------------------------------------------------
    # 6. Assertions after first run.
    # -----------------------------------------------------------------------
    $postColCount = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM information_schema.columns
WHERE table_name = 'rm_portfolio_holdings'
  AND column_name IN ('valuation_source','market_data_status','market_data_price_minor',
                      'market_data_currency','market_data_provider','market_data_as_of',
                      'market_data_record_updated_at');
"@
    if ($postColCount -ne '7') {
        throw "Post-migration: expected 7 new columns, found $postColCount."
    }
    Write-Host "[validator] Post-migration: 7/7 provenance columns present."

    $checkCount = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM pg_constraint
WHERE conrelid = 'rm_portfolio_holdings'::regclass
  AND conname IN (
    'chk_rm_portfolio_holdings_valuation_source',
    'chk_rm_portfolio_holdings_market_data_status',
    'chk_rm_portfolio_holdings_md_price_non_negative',
    'chk_rm_portfolio_holdings_md_currency_format',
    'chk_rm_portfolio_holdings_provenance_combination'
);
"@
    if ($checkCount -ne '5') {
        throw "Post-migration: expected 5 new CHECK constraints, found $checkCount."
    }
    Write-Host "[validator] Post-migration: 5/5 new CHECK constraints present."

    # Same snapshot -- every original field byte-for-byte unchanged.
    $afterCsv = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT id_portfolio||'|'||id_asset||'|'||trim(base_currency)||'|'||quantity::text||'|'||
       COALESCE(avg_cost_minor::text,'NULL')||'|'||invested_base_minor::text||'|'||
       market_value_minor::text||'|'||pnl_base_minor::text||'|'||
       COALESCE(pnl_pct::text,'NULL')||'|'||COALESCE(weight_pct::text,'NULL')||'|'||
       position_status||'|'||is_estimated::text||'|'||
       to_char(as_of AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS.US')||'|'||
       to_char(updated_at AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS.US')
FROM rm_portfolio_holdings
WHERE id_portfolio = '22222222-2222-2222-2222-222222222222'
  AND id_asset = '33333333-3333-3333-3333-333333333333';
"@
    Write-Host "[validator] AFTER snapshot:  $afterCsv"
    if ($beforeCsv -ne $afterCsv) {
        throw "Legacy financial fields changed across migration!`nBEFORE: $beforeCsv`nAFTER:  $afterCsv"
    }
    Write-Host "[validator] Legacy financial fields: byte-for-byte unchanged."

    $nullCount = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT (CASE WHEN valuation_source IS NULL THEN 1 ELSE 0 END
      + CASE WHEN market_data_status IS NULL THEN 1 ELSE 0 END
      + CASE WHEN market_data_price_minor IS NULL THEN 1 ELSE 0 END
      + CASE WHEN market_data_currency IS NULL THEN 1 ELSE 0 END
      + CASE WHEN market_data_provider IS NULL THEN 1 ELSE 0 END
      + CASE WHEN market_data_as_of IS NULL THEN 1 ELSE 0 END
      + CASE WHEN market_data_record_updated_at IS NULL THEN 1 ELSE 0 END)::text
FROM rm_portfolio_holdings
WHERE id_portfolio = '22222222-2222-2222-2222-222222222222';
"@
    if ($nullCount -ne '7') {
        throw "Legacy row provenance: expected 7 NULLs, found $nullCount."
    }
    Write-Host "[validator] Legacy row provenance: 7/7 NULL."

    # -----------------------------------------------------------------------
    # 7. Apply the migration a SECOND time -- must succeed without duplicating.
    # -----------------------------------------------------------------------
    Write-Host "[validator] Second migration run (idempotence)..."
    Apply-LocalFile -Database $dbName -Path $migration
    Write-Host "[validator] Second migration run: OK."

    $postColCount2 = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM information_schema.columns
WHERE table_name = 'rm_portfolio_holdings'
  AND column_name LIKE 'market_data_%' OR column_name = 'valuation_source';
"@
    # The query above is a sanity probe; the strict check is that NO column
    # was duplicated, which would have surfaced as 14 instead of 7+something.
    # An exact-count probe:
    $strictColCount = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM information_schema.columns
WHERE table_name = 'rm_portfolio_holdings'
  AND column_name IN ('valuation_source','market_data_status','market_data_price_minor',
                      'market_data_currency','market_data_provider','market_data_as_of',
                      'market_data_record_updated_at');
"@
    if ($strictColCount -ne '7') {
        throw "After 2nd run: expected 7 provenance columns, found $strictColCount."
    }
    Write-Host "[validator] After 2nd run: still 7/7 columns, no duplicates."

    $checkCount2 = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM pg_constraint
WHERE conrelid = 'rm_portfolio_holdings'::regclass
  AND conname IN (
    'chk_rm_portfolio_holdings_valuation_source',
    'chk_rm_portfolio_holdings_market_data_status',
    'chk_rm_portfolio_holdings_md_price_non_negative',
    'chk_rm_portfolio_holdings_md_currency_format',
    'chk_rm_portfolio_holdings_provenance_combination'
);
"@
    if ($checkCount2 -ne '5') {
        throw "After 2nd run: expected 5 CHECK constraints, found $checkCount2."
    }
    Write-Host "[validator] After 2nd run: still 5/5 CHECK constraints, no duplicates."

    # -----------------------------------------------------------------------
    # 8. Negative test: a forbidden combination must be rejected by the
    #    provenance_combination CHECK. Pick "valuation_source = market_data
    #    AND market_data_status = available" with NULL price (forbidden by
    #    the combination CHECK).
    # -----------------------------------------------------------------------
    # Build an extra asset to satisfy the unique (portfolio, asset) constraint.
    [void](Invoke-Psql -Database $dbName -Sql @"
INSERT INTO assets (id_asset, asset_class, status, name, native_currency, ticker, exchange)
VALUES ('44444444-4444-4444-4444-444444444444', 'equity', 'active',
        'Forbidden Combo', 'EUR', 'FBCM', 'NYSE');
"@)
    $forbiddenSql = @"
INSERT INTO rm_portfolio_holdings (
  id_portfolio, id_asset, base_currency, quantity, avg_cost_minor,
  invested_base_minor, market_value_minor, pnl_base_minor,
  pnl_pct, weight_pct, position_status, is_estimated, as_of,
  valuation_source, market_data_status,
  market_data_price_minor, market_data_currency, market_data_provider,
  market_data_as_of, market_data_record_updated_at
) VALUES (
  '22222222-2222-2222-2222-222222222222',
  '44444444-4444-4444-4444-444444444444',
  'EUR', 1.0, 100, 100, 100, 0, 0, 100, 'open', false, now(),
  'market_data', 'available',
  NULL, 'EUR', 'test', now(), now()
);
"@
    # Locally relax $ErrorActionPreference so we can capture psql's stderr
    # (which is where it writes CHECK violation messages) without PowerShell
    # treating the captured ErrorRecord as a fatal exception.
    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        if ($CiMode) {
            $rejection = $forbiddenSql | psql -v ON_ERROR_STOP=1 -U $adminUser -d $dbName -q 2>&1
        } else {
            $rejection = $forbiddenSql | docker exec -i $container psql -v ON_ERROR_STOP=1 -U $adminUser -d $dbName -q 2>&1
        }
        $rejectedCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prevPref
    }
    if ($rejectedCode -eq 0) {
        throw "Forbidden combination was NOT rejected! CHECK constraint is missing or wrong."
    }
    $rejectionText = ($rejection | Out-String)
    if ($rejectionText -notmatch 'chk_rm_portfolio_holdings_provenance_combination|chk_rm_portfolio_holdings_market_data_status') {
        throw "Forbidden combination rejected but by an unexpected constraint:`n$rejectionText"
    }
    Write-Host "[validator] Forbidden combination correctly rejected by CHECK constraint."

    # -----------------------------------------------------------------------
    # 9. Also assert a VALID provenance combination IS accepted.
    # -----------------------------------------------------------------------
    [void](Invoke-Psql -Database $dbName -Sql @"
INSERT INTO assets (id_asset, asset_class, status, name, native_currency, ticker, exchange)
VALUES ('55555555-5555-5555-5555-555555555555', 'equity', 'active',
        'Valid Combo', 'EUR', 'VALC', 'NYSE');
INSERT INTO rm_portfolio_holdings (
  id_portfolio, id_asset, base_currency, quantity, avg_cost_minor,
  invested_base_minor, market_value_minor, pnl_base_minor,
  pnl_pct, weight_pct, position_status, is_estimated, as_of,
  valuation_source, market_data_status,
  market_data_price_minor, market_data_currency, market_data_provider,
  market_data_as_of, market_data_record_updated_at
) VALUES (
  '22222222-2222-2222-2222-222222222222',
  '55555555-5555-5555-5555-555555555555',
  'EUR', 1.0, 200, 200, 250, 50, 25.0, 100, 'open', false, now(),
  'market_data', 'available',
  250, 'EUR', 'test-static', now(), now()
);
"@)
    Write-Host "[validator] Valid (market_data, available) row inserted successfully."

    $exitCode = 0
    Write-Host ""
    Write-Host "[validator] ALL CHECKS PASSED on $dbName."
}
catch {
    Write-Host ""
    Write-Host "[validator] FAILURE: $_"
    $exitCode = 2
    throw
}
finally {
    if ($created) {
        $stillThere = $false
        try {
            if ($CiMode) {
                $count = ("SELECT COUNT(*) FROM pg_database WHERE datname = '$dbName'" |
                    psql -t -A -U $adminUser -d $adminDb).Trim()
            } else {
                $count = ("SELECT COUNT(*) FROM pg_database WHERE datname = '$dbName'" |
                    docker exec -i $container psql -t -A -U $adminUser -d $adminDb).Trim()
            }
            $stillThere = ($count -eq '1')
        }
        catch {
            Write-Host "[validator] Could not query existence of $dbName : $_"
        }
        if ($stillThere) {
            if ($exitCode -ne 0 -and $KeepDatabaseOnFailure) {
                Write-Host "[validator] -KeepDatabaseOnFailure set: leaving $dbName in place."
                Write-Host "[validator] Drop manually with: docker exec -i $container psql -U $adminUser -d $adminDb -c 'DROP DATABASE `"$dbName`"'"
            }
            else {
                try {
                    Drop-DisposableDatabase -Name $dbName
                    Write-Host "[validator] $dbName dropped successfully."
                }
                catch {
                    Write-Host "[validator] Cleanup error dropping ${dbName}: $_"
                    if ($exitCode -eq 0) { $exitCode = 3 }
                }
            }
        }
    }
    Write-Host "[validator] disposable database name: $dbName"
    Write-Host "[validator] exit $exitCode"
}

exit $exitCode
