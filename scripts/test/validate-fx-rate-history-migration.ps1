<#
.SYNOPSIS
    End-to-end validator for migration 004_fx_rate_history_cache.sql.

.DESCRIPTION
    Creates a disposable PostgreSQL database, bootstraps the committed
    pre-migration schema (taken from the migration-004 immutable
    baseline), verifies the FX table does not yet exist, applies the
    migration, verifies the new table and constraints, applies the
    migration a SECOND time to prove idempotence, exercises representative
    constraints (canonical pair ordering, positivity, uppercase format)
    and inserts a canonical row to confirm the STORED GENERATED inverse
    matches the round-12 of `1 / canonical_rate`, plus uniqueness on
    (pair, date, provider) with multiple providers permitted.

    Safety invariants:
      - Disposable database name MUST start with `kushim_test_`.
      - The script NEVER touches `kushim`, `postgres`, `template0`,
        `template1`, or any name failing the safety regex.
      - The disposable database is ALWAYS dropped in the `finally` block
        (unless `-KeepDatabaseOnFailure` is set on failure).
      - The migration is applied to the disposable database only — the
        development database is never modified.

.PARAMETER KeepDatabaseOnFailure
    When set, do NOT drop the disposable database on failure so the
    operator can connect and inspect it. Always dropped on success.

.PARAMETER CiMode
    When set, run psql directly against the host pointed to by the
    standard PG* environment variables (PGHOST / PGPORT / PGUSER /
    PGPASSWORD / PGDATABASE) instead of going through `docker exec` on a
    local container.

.PARAMETER BaseRef
    Git ref pointing to the pre-migration schema baseline. This script
    is bound to **migration 004** and its correct pre-migration baseline
    is **immutable**: the commit that was the head of `main` immediately
    before migration 004 was introduced.

    That commit is `4d674d409a6bf560ec056fe8efcdf89741a83e13` and is
    hard-coded in `$Migration004BaselineRef` below.

    Resolution order:

      1. `-BaseRef <commit>` passed by the caller (manual override).
      2. Otherwise the migration-004 immutable baseline.

    NO topology-based fallback is ever used. Specifically, the validator
    NEVER consults `HEAD`, `HEAD^1`, `git merge-base`,
    `github.event.pull_request.base.sha`, or `github.event.before`. Once
    migration 004 is on main, all of those degenerate to a
    post-migration commit and would silently invalidate the test. A
    content guard (`infra/postgres/init/001_init.sql` at the chosen ref
    must NOT yet declare the FX table) still runs and rejects any
    candidate that is already post-migration.

.EXAMPLE
    .\scripts\test\validate-fx-rate-history-migration.ps1

.EXAMPLE
    pwsh -NoProfile -File ./scripts/test/validate-fx-rate-history-migration.ps1 -CiMode
#>

[CmdletBinding()]
param(
    [switch]$KeepDatabaseOnFailure,
    [switch]$CiMode,
    [string]$BaseRef = '',
    # Diagnostic-only switch: skip the full migration run and just exercise
    # the content guard against an explicit bootstrap SQL file. The file
    # must NOT yet declare `fx_rate_history_cache` (i.e., it must look
    # like a true pre-migration-004 `001_init.sql`). Used by the test
    # harness to prove that a *post-migration* bootstrap is rejected
    # before any disposable database is created.
    [switch]$ContentGuardOnly,
    [string]$BootstrapInitFile = ''
)

# --------------------------------------------------------------------------
# IMMUTABLE BASELINE for migration 004.
#
# This is the commit that was the head of `main` immediately before
# `infra/postgres/upgrades/004_fx_rate_history_cache.sql` was added.
# It does not depend on the current branch, the current event, or whether
# the migration has been merged. Updating this value requires editing this
# script — that's intentional: the validator is migration-004-specific and
# the baseline is part of its contract.
# --------------------------------------------------------------------------
$Migration004BaselineRef = '4d674d409a6bf560ec056fe8efcdf89741a83e13'

$repoRootInit = (Resolve-Path "$PSScriptRoot\..\..").Path

# ---------------------------------------------------------------------------
# Reusable content-guard. Returns the number of `CREATE TABLE
# fx_rate_history_cache` declarations found in `$InitSqlText` (case
# insensitive, anchored at the start of a logical line). A real
# pre-migration-004 bootstrap MUST return 0; any positive result means the
# candidate is post-migration and must be rejected before the disposable
# database is created.
# ---------------------------------------------------------------------------
function Get-FxBootstrapMatchCount {
    param([Parameter(Mandatory = $true)][string]$InitSqlText)
    $fxTablePattern = '(?m)^\s*CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?fx_rate_history_cache\b'
    ([regex]::Matches(
        $InitSqlText,
        $fxTablePattern,
        [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    ).Count
}

# ---------------------------------------------------------------------------
# Diagnostic-only mode: run the content guard against an explicit local
# bootstrap SQL file and exit. Used by the test harness to prove the
# guard rejects a *post-migration* bootstrap before any disposable
# database is created.
# ---------------------------------------------------------------------------
if ($ContentGuardOnly) {
    if ([string]::IsNullOrWhiteSpace($BootstrapInitFile)) {
        throw '-ContentGuardOnly requires -BootstrapInitFile <path-to-001_init.sql>'
    }
    if (-not (Test-Path $BootstrapInitFile)) {
        throw "Bootstrap init file not found: $BootstrapInitFile"
    }
    $candidateText = Get-Content -Raw -Path $BootstrapInitFile
    $count = Get-FxBootstrapMatchCount -InitSqlText $candidateText
    Write-Host "[content-guard] File: $BootstrapInitFile"
    Write-Host "[content-guard] fx_rate_history_cache declarations found: $count"
    if ($count -gt 0) {
        Write-Host '[content-guard] REJECT: candidate already declares fx_rate_history_cache.'
        exit 2
    }
    Write-Host '[content-guard] ACCEPT: candidate is pre-migration-004.'
    exit 0
}

if ([string]::IsNullOrWhiteSpace($BaseRef)) {
    $BaseRef = $Migration004BaselineRef
    Write-Host "[validator] BaseRef defaulted to Migration004BaselineRef = $BaseRef"
}

# Resolve the ref to a stable SHA and reject candidates that are already
# post-migration. The content check reads `infra/postgres/init/001_init.sql`
# from $BaseRef and looks for the FX table declaration. This is safer than
# relying solely on commit topology.
$resolvedSha = ((git -C $repoRootInit rev-parse --verify "$BaseRef" 2>$null) -join '').Trim()
if ($LASTEXITCODE -ne 0 -or -not $resolvedSha) {
    throw "BaseRef '$BaseRef' does not resolve to any commit. Run ``git fetch`` and retry, or pass a different -BaseRef."
}
Write-Host "[validator] BaseRef = $BaseRef (resolved SHA = $resolvedSha)"

$candidateInit = (git -C $repoRootInit show "${BaseRef}:infra/postgres/init/001_init.sql" 2>&1) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw "Cannot read infra/postgres/init/001_init.sql from BaseRef '$BaseRef': $candidateInit"
}
$alreadyPresent = Get-FxBootstrapMatchCount -InitSqlText $candidateInit
if ($alreadyPresent -gt 0) {
    throw @"
BaseRef '$BaseRef' ($resolvedSha) ALREADY declares fx_rate_history_cache in
infra/postgres/init/001_init.sql.

This is NOT a pre-migration baseline for migration 004. The validator
refuses to use it because bootstrapping it would mask whatever the
upgrade is supposed to add, and the post-migration assertions would
also fail or be vacuous.

This script is migration-004-specific. Its baseline is the IMMUTABLE
commit pinned at $Migration004BaselineRef — set deliberately so it
cannot drift as main moves forward.

Remediation:
  * Most operators: run without -BaseRef so the immutable baseline is used.
  * Diagnostic / manual override: pass -BaseRef <pre-migration commit>
    whose infra/postgres/init/001_init.sql does NOT yet declare
    fx_rate_history_cache.
  * Do NOT pass HEAD, HEAD^1, origin/main, or any derivative of the
    current branch topology — once migration 004 is on main, those refs
    are all post-migration.
"@
}
Write-Host "[validator] BaseRef content check: fx_rate_history_cache absent (pre-migration confirmed)."

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

$repoRoot   = (Resolve-Path "$PSScriptRoot\..\..").Path
$container  = 'kushim_database'  # local-mode docker container
$adminUser  = if ($CiMode) { $env:PGUSER     } else { 'kushim' }
$adminDb    = if ($CiMode) { $env:PGDATABASE } else { 'kushim' }
$migration  = Join-Path $repoRoot 'infra\postgres\upgrades\004_fx_rate_history_cache.sql'

$safeNameRegex = '^kushim_test_[A-Za-z0-9_]+$'

if ($CiMode) {
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

function Invoke-PsqlCore {
    param(
        [Parameter(Mandatory = $true)][string]$Database,
        [Parameter(Mandatory = $true)][string]$Sql,
        [switch]$Tuples
    )
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
    param([Parameter(Mandatory = $true)][string]$Database,
          [Parameter(Mandatory = $true)][string]$Sql)
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
    param([Parameter(Mandatory = $true)][string]$Database,
          [Parameter(Mandatory = $true)][string]$Sql)
    Assert-SafeDatabaseName -Name $Database
    $r = Invoke-PsqlCore -Database $Database -Sql $Sql -Tuples
    if ($r.ExitCode -ne 0) {
        throw "psql scalar failed on $Database (exit $($r.ExitCode)): $($r.Output)"
    }
    return ($r.Output | Out-String).Trim()
}

function Apply-FileFromGit {
    param([Parameter(Mandatory = $true)][string]$Database,
          [Parameter(Mandatory = $true)][string]$RelativePath)
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
    param([Parameter(Mandatory = $true)][string]$Database,
          [Parameter(Mandatory = $true)][string]$Path)
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
    return "kushim_test_fxmigval_${ts}_$rand"
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
    [void](Invoke-PsqlAdmin -Sql "CREATE DATABASE `"$dbName`"")
    $created = $true
    Write-Host "[validator] CREATE DATABASE OK"

    # Bootstrap the pre-migration schema from $BaseRef.
    $initFiles = git -C $repoRoot ls-tree -r --name-only $BaseRef -- 'infra/postgres/init/' |
        Where-Object { $_ -like '*.sql' } |
        Sort-Object
    if (-not $initFiles) { throw "No init/*.sql tracked at $BaseRef." }
    Write-Host "[validator] Bootstrapping from ${BaseRef} init/*.sql ($($initFiles.Count) files)"
    foreach ($f in $initFiles) {
        Apply-FileFromGit -Database $dbName -RelativePath $f
    }

    # Confirm the old schema does NOT yet contain the FX table.
    $preTableCount = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM information_schema.tables
WHERE table_name = 'fx_rate_history_cache';
"@
    if ($preTableCount -ne '0') {
        throw "Pre-migration schema already contains fx_rate_history_cache -- bootstrap is wrong."
    }
    Write-Host "[validator] Pre-migration: fx_rate_history_cache absent (expected)."

    # Apply migration 004 (first run).
    Write-Host "[validator] First migration run..."
    Apply-LocalFile -Database $dbName -Path $migration
    Write-Host "[validator] First migration run: OK."

    $postTableCount = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM information_schema.tables
WHERE table_name = 'fx_rate_history_cache';
"@
    if ($postTableCount -ne '1') {
        throw "Post-migration: fx_rate_history_cache table missing."
    }

    $colCount = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM information_schema.columns
WHERE table_name = 'fx_rate_history_cache'
  AND column_name IN ('id_fx_rate_history_cache','rate_date','canonical_base_currency',
                      'canonical_quote_currency','canonical_rate','inverse_rate',
                      'provider','provider_as_of','dataset_version','created_at','updated_at');
"@
    if ($colCount -ne '11') {
        throw "Post-migration: expected 11 columns, found $colCount."
    }
    Write-Host "[validator] Post-migration: 11/11 expected columns present."

    $checkCount = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM pg_constraint
WHERE conrelid = 'fx_rate_history_cache'::regclass
  AND conname IN (
    'chk_fx_rate_history_cache_base_currency_format',
    'chk_fx_rate_history_cache_quote_currency_format',
    'chk_fx_rate_history_cache_pair_canonical_ordering',
    'chk_fx_rate_history_cache_canonical_rate_positive',
    'chk_fx_rate_history_cache_provider_not_blank',
    'chk_fx_rate_history_cache_dataset_version_not_blank'
);
"@
    if ($checkCount -ne '6') {
        throw "Post-migration: expected 6 new CHECK constraints, found $checkCount."
    }
    Write-Host "[validator] Post-migration: 6/6 CHECK constraints present."

    # Idempotence — second run is a no-op.
    Write-Host "[validator] Second migration run (idempotence)..."
    Apply-LocalFile -Database $dbName -Path $migration
    Write-Host "[validator] Second migration run: OK."

    $tableCount2 = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'fx_rate_history_cache';
"@
    if ($tableCount2 -ne '1') {
        throw "After 2nd run: fx_rate_history_cache table count unexpected ($tableCount2)."
    }
    Write-Host "[validator] After 2nd run: still 1 table, no duplicate."

    # Insert a canonical row and verify the GENERATED inverse_rate.
    [void](Invoke-Psql -Database $dbName -Sql @"
INSERT INTO fx_rate_history_cache
  (canonical_base_currency, canonical_quote_currency, rate_date,
   provider, canonical_rate, provider_as_of, dataset_version)
VALUES
  ('EUR', 'USD', '2026-06-18', 'mock_ecb_fixture', 1.1461, '2026-06-18T14:00:00Z'::timestamptz, 'mock-ecb-2026-06-18-v1');
"@)

    $persistedCanonical = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT canonical_rate::text FROM fx_rate_history_cache
WHERE canonical_base_currency='EUR' AND canonical_quote_currency='USD' AND rate_date='2026-06-18';
"@
    $persistedInverse = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT inverse_rate::text FROM fx_rate_history_cache
WHERE canonical_base_currency='EUR' AND canonical_quote_currency='USD' AND rate_date='2026-06-18';
"@
    if ($persistedCanonical -notmatch '^1\.1461') {
        throw "Persisted canonical_rate unexpected: '$persistedCanonical'."
    }
    # 1 / 1.1461 = 0.872524212547... (rounded at 12 dp by the GENERATED column)
    if ($persistedInverse -notmatch '^0\.872524212547') {
        throw "Persisted inverse_rate unexpected: '$persistedInverse' (expected ~0.872524212547)."
    }
    Write-Host "[validator] Canonical/inverse stored generated column: OK ($persistedCanonical / $persistedInverse)."

    # Canonical ordering rejection: base >= quote must fail.
    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        if ($CiMode) {
            $rejection = "INSERT INTO fx_rate_history_cache (canonical_base_currency, canonical_quote_currency, rate_date, provider, canonical_rate, dataset_version) VALUES ('USD','EUR','2026-06-18','mock_ecb_fixture',0.872,'mock-ecb-2026-06-18-v1');" |
                psql -v ON_ERROR_STOP=1 -U $adminUser -d $dbName -q 2>&1
        } else {
            $rejection = "INSERT INTO fx_rate_history_cache (canonical_base_currency, canonical_quote_currency, rate_date, provider, canonical_rate, dataset_version) VALUES ('USD','EUR','2026-06-18','mock_ecb_fixture',0.872,'mock-ecb-2026-06-18-v1');" |
                docker exec -i $container psql -v ON_ERROR_STOP=1 -U $adminUser -d $dbName -q 2>&1
        }
        $rejectionCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prevPref
    }
    if ($rejectionCode -eq 0) {
        throw "Non-canonical pair (USD>EUR) was NOT rejected! Ordering CHECK is missing or wrong."
    }
    if (($rejection | Out-String) -notmatch 'chk_fx_rate_history_cache_pair_canonical_ordering') {
        throw "Non-canonical pair rejected but by an unexpected constraint:`n$rejection"
    }
    Write-Host "[validator] Canonical ordering CHECK rejected USD>EUR as expected."

    # Non-positive rate rejection.
    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        if ($CiMode) {
            $rejection = "INSERT INTO fx_rate_history_cache (canonical_base_currency, canonical_quote_currency, rate_date, provider, canonical_rate, dataset_version) VALUES ('AUD','EUR','2026-06-18','mock_ecb_fixture',0,'mock-ecb-2026-06-18-v1');" |
                psql -v ON_ERROR_STOP=1 -U $adminUser -d $dbName -q 2>&1
        } else {
            $rejection = "INSERT INTO fx_rate_history_cache (canonical_base_currency, canonical_quote_currency, rate_date, provider, canonical_rate, dataset_version) VALUES ('AUD','EUR','2026-06-18','mock_ecb_fixture',0,'mock-ecb-2026-06-18-v1');" |
                docker exec -i $container psql -v ON_ERROR_STOP=1 -U $adminUser -d $dbName -q 2>&1
        }
        $rejectionCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prevPref
    }
    if ($rejectionCode -eq 0) {
        throw "Zero canonical_rate was NOT rejected!"
    }
    Write-Host "[validator] Non-positive rate CHECK rejected canonical_rate=0 as expected."

    # Multiple providers permitted for the same pair/date.
    [void](Invoke-Psql -Database $dbName -Sql @"
INSERT INTO fx_rate_history_cache
  (canonical_base_currency, canonical_quote_currency, rate_date,
   provider, canonical_rate, dataset_version)
VALUES
  ('EUR', 'USD', '2026-06-18', 'second_provider', 1.150, 'second-v1');
"@)
    $rowsPair = Invoke-PsqlScalar -Database $dbName -Sql @"
SELECT COUNT(*) FROM fx_rate_history_cache
WHERE canonical_base_currency='EUR' AND canonical_quote_currency='USD' AND rate_date='2026-06-18';
"@
    if ($rowsPair -ne '2') {
        throw "Expected 2 rows (two providers) for EUR/USD 2026-06-18, found $rowsPair."
    }
    Write-Host "[validator] Multiple providers coexist (2 rows for same pair/date)."

    # Duplicate provider rejection.
    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        if ($CiMode) {
            $rejection = "INSERT INTO fx_rate_history_cache (canonical_base_currency, canonical_quote_currency, rate_date, provider, canonical_rate, dataset_version) VALUES ('EUR','USD','2026-06-18','mock_ecb_fixture',1.1462,'mock-ecb-2026-06-18-v1');" |
                psql -v ON_ERROR_STOP=1 -U $adminUser -d $dbName -q 2>&1
        } else {
            $rejection = "INSERT INTO fx_rate_history_cache (canonical_base_currency, canonical_quote_currency, rate_date, provider, canonical_rate, dataset_version) VALUES ('EUR','USD','2026-06-18','mock_ecb_fixture',1.1462,'mock-ecb-2026-06-18-v1');" |
                docker exec -i $container psql -v ON_ERROR_STOP=1 -U $adminUser -d $dbName -q 2>&1
        }
        $rejectionCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prevPref
    }
    if ($rejectionCode -eq 0) {
        throw "Duplicate (pair, date, provider) row was NOT rejected!"
    }
    Write-Host "[validator] Unique (pair, date, provider) rejected duplicate as expected."

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
