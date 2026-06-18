<#
.SYNOPSIS
    Disposable-database test runner for kushim-api / kushim-worker.

.DESCRIPTION
    Creates a fresh PostgreSQL database (named `kushim_test_<service>_<ts>_<rand>`),
    bootstraps it with the canonical schema and seed scripts, runs the requested
    Rust suite (fmt, clippy, test, audit) against it with DATABASE_URL pointing
    ONLY at the temporary database for the child process, then drops the
    temporary database in a `finally` block — even when the suite fails.

    Why this exists. Direct `cargo test` against the local `kushim` development
    database accumulates persistent rows from tests that POST operations
    (schema-immutable trigger `prevent_posted_operation_mutation`) and from
    tests that panic before reaching their explicit cleanup. The earlier
    per-test cleanup pass reduced but did not eliminate this growth. This
    runner makes every test invocation operate on a brand-new database whose
    lifetime is bounded by this script's process — so the development
    database is byte-for-byte unchanged after every suite run.

    Safety invariants.
      - Generated database name MUST start with `kushim_test_`.
      - The script REFUSES to drop any database whose name does not match
        `^kushim_test_[A-Za-z0-9_]+$`.
      - The script NEVER touches the development database named `kushim`,
        the system database `postgres`, the `template0`/`template1`
        templates, or any name supplied by environment that fails the
        pattern.
      - The parent shell's `DATABASE_URL` environment variable is
        preserved exactly across invocation (per-process scope only).
      - Bootstrap uses the canonical source-of-truth files at
        `infra/postgres/init/00*.sql`, NEVER a clone of the polluted
        development database.

.PARAMETER Service
    The Rust service to validate. Must be one of: kushim-api, kushim-worker,
    kushim-market-data.

.PARAMETER KeepDatabaseOnFailure
    When the suite fails, do NOT drop the temporary database so the operator
    can connect and inspect it. Always dropped on success regardless.

.PARAMETER SkipAudit
    Skip `cargo audit` (useful for fast local re-runs when network is
    unavailable; CI must NOT pass this).

.EXAMPLE
    .\scripts\test\run-rust-db-suite.ps1 -Service kushim-api
    .\scripts\test\run-rust-db-suite.ps1 -Service kushim-worker
    .\scripts\test\run-rust-db-suite.ps1 -Service kushim-api -KeepDatabaseOnFailure
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('kushim-api', 'kushim-worker', 'kushim-market-data')]
    [string]$Service,

    [switch]$KeepDatabaseOnFailure,
    [switch]$SkipAudit
)

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

$repoRoot   = (Resolve-Path "$PSScriptRoot\..\..").Path
$container  = 'kushim_database'
$adminUser  = 'kushim'
$adminDb    = 'kushim'   # used only for admin commands like CREATE/DROP DATABASE
$initSqlDir = Join-Path $repoRoot 'infra\postgres\init'

# Database name pattern enforced for ALL drop / target operations.
$safeNameRegex = '^kushim_test_[A-Za-z0-9_]+$'

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

function Assert-SafeDatabaseName {
    param([string]$Name)
    if ([string]::IsNullOrWhiteSpace($Name)) {
        throw "Refused: database name is empty or whitespace."
    }
    if ($Name -notmatch $safeNameRegex) {
        throw "Refused to operate on database '$Name': name does not match $safeNameRegex. " +
              "Cleanup is only allowed on disposable databases this runner created."
    }
    foreach ($forbidden in @('kushim', 'postgres', 'template0', 'template1')) {
        if ($Name -eq $forbidden) {
            throw "Refused to operate on database '$Name': hard-coded forbidden name."
        }
    }
}

function Invoke-AdminPsql {
    param([string]$Sql, [string]$Database = $adminDb)
    # Admin commands like CREATE/DROP DATABASE must run outside any
    # transaction; psql connects to a maintenance database (default `kushim`)
    # to issue them.
    $output = $Sql | docker exec -i $container psql -v ON_ERROR_STOP=1 -U $adminUser -d $Database -q 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "psql admin command failed (exit $LASTEXITCODE): $output"
    }
    return $output
}

function Test-DatabaseExists {
    param([string]$Name)
    $count = ("SELECT COUNT(*) FROM pg_database WHERE datname = '$Name'" |
        docker exec -i $container psql -t -A -U $adminUser -d $adminDb 2>&1).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to query database existence for '$Name': $count"
    }
    return ($count -eq '1')
}

function New-DisposableDatabaseName {
    param([string]$Suffix)
    # Format: kushim_test_<service-token>_<utc-yyyymmddhhmmss>_<6-char-random-hex>
    $ts   = (Get-Date).ToUniversalTime().ToString('yyyyMMddHHmmss')
    $rand = -join ((48..57) + (97..102) |
        Get-Random -Count 6 |
        ForEach-Object { [char]$_ })
    return "kushim_test_${Suffix}_${ts}_$rand"
}

function New-Database {
    param([string]$Name)
    Assert-SafeDatabaseName -Name $Name
    Write-Host "[runner] CREATE DATABASE $Name"
    [void](Invoke-AdminPsql -Sql "CREATE DATABASE `"$Name`"")
}

function Bootstrap-Database {
    param([string]$Name)
    Assert-SafeDatabaseName -Name $Name
    Write-Host "[runner] Bootstrapping $Name from infra/postgres/init/*.sql"
    $sqlFiles = Get-ChildItem -Path $initSqlDir -Filter '*.sql' | Sort-Object Name
    if ($sqlFiles.Count -eq 0) {
        throw "No bootstrap SQL files found in $initSqlDir"
    }
    foreach ($f in $sqlFiles) {
        Write-Host "  - applying $($f.Name)"
        $sql = Get-Content $f.FullName -Raw
        $output = $sql | docker exec -i $container psql -v ON_ERROR_STOP=1 -U $adminUser -d $Name -q 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "Bootstrap step failed for $($f.Name) (exit $LASTEXITCODE): $output"
        }
    }
}

function Remove-Database {
    param([string]$Name)
    # Re-validate even here as a tripwire — if anyone changes the caller,
    # the assertion still guards us.
    Assert-SafeDatabaseName -Name $Name
    Write-Host "[runner] Terminating connections to $Name and DROP DATABASE"
    $terminate = @"
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE datname = '$Name' AND pid <> pg_backend_pid();
"@
    [void](Invoke-AdminPsql -Sql $terminate)
    [void](Invoke-AdminPsql -Sql "DROP DATABASE IF EXISTS `"$Name`"")
    if (Test-DatabaseExists -Name $Name) {
        throw "Drop succeeded silently but database '$Name' still exists."
    }
    Write-Host "[runner] Database $Name dropped."
}

# ---------------------------------------------------------------------------
# Pre-flight
# ---------------------------------------------------------------------------

# Refuse to run if the postgres container is not up.
$running = docker ps --filter "name=^/$container$" --filter 'status=running' --format '{{.Names}}'
if (-not $running) {
    throw "PostgreSQL container '$container' is not running. Start with: docker compose up -d database"
}

# Determine the host PostgreSQL port the test process should use.
$hostPort = 5432

# ---------------------------------------------------------------------------
# Disposable database lifecycle
# ---------------------------------------------------------------------------

$tag = switch ($Service) {
    'kushim-api'         { 'api' }
    'kushim-worker'      { 'worker' }
    'kushim-market-data' { 'md' }
    default              { throw "Unsupported service tag for '$Service'" }
}
$dbName = New-DisposableDatabaseName -Suffix $tag
Assert-SafeDatabaseName -Name $dbName

$exitCode = 1
$bootstrapped = $false
$created = $false
try {
    New-Database -Name $dbName
    $created = $true
    Bootstrap-Database -Name $dbName
    $bootstrapped = $true

    # ---------------------------------------------------------------------
    # Run the suite with a CHILD-PROCESS-ONLY DATABASE_URL. We never mutate
    # the parent shell's env via $env: assignments that outlive the call —
    # cargo is started with an explicit `Environment` map.
    # ---------------------------------------------------------------------
    $childUrl = "postgresql://kushim:kushim_secret_dev@127.0.0.1:$hostPort/$dbName"

    $serviceDir = Join-Path $repoRoot $Service
    if (-not (Test-Path $serviceDir)) {
        throw "Service directory not found: $serviceDir"
    }

    $steps = @(
        @{ Name = 'fmt';    Args = @('fmt', '--check') },
        @{ Name = 'clippy'; Args = @('clippy', '--all-targets', '--all-features', '--', '-D', 'warnings') },
        @{ Name = 'test';   Args = @('test', '--quiet') }
    )
    if (-not $SkipAudit) {
        $steps += @{ Name = 'audit'; Args = @('audit', '--ignore', 'RUSTSEC-2023-0071') }
    }

    # Save the parent shell's DATABASE_URL so we can restore it byte-for-byte
    # after every child cargo invocation. Per-process env scope is achieved
    # by setting $env:DATABASE_URL just before Start-Process and restoring
    # it immediately after (Start-Process with -UseNewEnvironment is
    # incompatible with PATH-based exe lookup on Windows PowerShell 5.1).
    $parentDbUrl       = $env:DATABASE_URL
    $parentKushimFlag  = $env:KUSHIM_RUNNER
    try {
        foreach ($step in $steps) {
            Write-Host ""
            Write-Host "[runner] cargo $($step.Args -join ' ')  (db=$dbName)"
            $env:DATABASE_URL   = $childUrl
            $env:KUSHIM_RUNNER  = '1'
            $proc = Start-Process -FilePath 'cargo' `
                                  -ArgumentList $step.Args `
                                  -WorkingDirectory $serviceDir `
                                  -NoNewWindow `
                                  -PassThru -Wait
            $rc = $proc.ExitCode
            if ($rc -ne 0) {
                throw "Step '$($step.Name)' failed with exit code $rc."
            }
        }
    }
    finally {
        # Restore the parent shell's environment exactly. Setting to $null
        # via Set-Item -Force unsets the variable when the original was
        # absent.
        if ($null -eq $parentDbUrl) {
            Remove-Item Env:DATABASE_URL -ErrorAction SilentlyContinue
        } else {
            $env:DATABASE_URL = $parentDbUrl
        }
        if ($null -eq $parentKushimFlag) {
            Remove-Item Env:KUSHIM_RUNNER -ErrorAction SilentlyContinue
        } else {
            $env:KUSHIM_RUNNER = $parentKushimFlag
        }
    }

    $exitCode = 0
    Write-Host ""
    Write-Host "[runner] All steps PASSED on $dbName."
}
catch {
    Write-Host ""
    Write-Host "[runner] FAILURE: $_"
    $exitCode = 2
    throw
}
finally {
    # Always attempt to drop the disposable database — even on failure —
    # unless the operator explicitly asked us to keep it.
    if ($created -and (Test-DatabaseExists -Name $dbName)) {
        if ($exitCode -ne 0 -and $KeepDatabaseOnFailure) {
            Write-Host ""
            Write-Host "[runner] -KeepDatabaseOnFailure set: leaving $dbName in place for inspection."
            Write-Host "[runner] Drop it manually with: docker exec -i $container psql -U $adminUser -d $adminDb -c 'DROP DATABASE `"$dbName`"'"
        }
        else {
            try {
                Remove-Database -Name $dbName
            }
            catch {
                Write-Host ""
                Write-Host "[runner] Cleanup error while dropping $dbName : $_"
                # Make sure the exit code reflects the cleanup failure if
                # the suite itself succeeded — otherwise preserve the
                # suite-level non-zero exit code.
                if ($exitCode -eq 0) { $exitCode = 3 }
            }
        }
    }
    Write-Host "[runner] exit $exitCode"
}

exit $exitCode
