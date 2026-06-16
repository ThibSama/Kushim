<#
.SYNOPSIS
    Apply idempotent, non-destructive PostgreSQL upgrade scripts to the local
    Kushim database.

.DESCRIPTION
    `infra/postgres/init/001_init.sql` only runs on a fresh PostgreSQL volume.
    Existing local databases need the incremental upgrade scripts under
    `infra/postgres/upgrades/` applied manually. This helper applies them in
    lexical order inside the running `kushim_database` container.

    Every upgrade script is written to be idempotent (IF NOT EXISTS / guarded
    DO blocks), so this helper is safe to run multiple times. It never drops,
    truncates, or deletes application data.

.NOTES
    - Does not reset the volume.
    - Does not print secrets.
#>

$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$upgradesDir = Join-Path $repoRoot 'infra\postgres\upgrades'

if (-not (Test-Path $upgradesDir)) {
    Write-Error "Upgrades directory not found at $upgradesDir"
    exit 1
}

$container = 'kushim_database'
$running = docker ps --filter "name=^/$container$" --filter 'status=running' --format '{{.Names}}'
if (-not $running) {
    Write-Error "Container '$container' is not running. Start it with: docker compose up -d database"
    exit 1
}

$scripts = Get-ChildItem -Path $upgradesDir -Filter '*.sql' | Sort-Object Name
if ($scripts.Count -eq 0) {
    Write-Host 'No upgrade scripts found. Nothing to apply.'
    exit 0
}

foreach ($script in $scripts) {
    Write-Host "Applying $($script.Name) ..."
    Get-Content -Raw -Path $script.FullName |
        docker exec -i $container psql -U kushim -d kushim -v ON_ERROR_STOP=1 -q
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Upgrade script $($script.Name) failed (exit $LASTEXITCODE)."
        exit 1
    }
    Write-Host "  OK: $($script.Name)"
}

Write-Host ''
Write-Host 'All upgrade scripts applied. Verifying required objects...'

# Required relations after every upgrade pass. Add new entries here whenever a
# new upgrade introduces a load-bearing table — checking only one table no
# longer implies the whole schema is in good shape.
$requiredTables = @(
    'portfolio_refresh_requests',
    'portfolio_operation_idempotency'
)

# Required uniqueness / FK / CHECK / index objects for P3 durable idempotency.
# These are what makes ON CONFLICT DO NOTHING actually serialize concurrent
# claims and what keeps the audit history coherent; a missing object silently
# breaks the contract, so we fail loudly. FK delete actions are pinned where
# they matter (RESTRICT for the audit chain).
$requiredObjects = @(
    @{ Kind = 'index';      Name = 'uq_portfolio_operation_idempotency_user_key' },
    @{ Kind = 'index';      Name = 'idx_portfolio_operation_idempotency_portfolio_created' },
    @{ Kind = 'fk';         Name = 'fk_portfolio_operation_idempotency_user';                Delete = 'r' },
    @{ Kind = 'fk';         Name = 'fk_portfolio_operation_idempotency_portfolio';           Delete = 'r' },
    @{ Kind = 'fk';         Name = 'fk_portfolio_operation_idempotency_operation';           Delete = 'r' },
    @{ Kind = 'fk';         Name = 'fk_portfolio_operation_idempotency_corrected_operation'; Delete = 'n' },
    @{ Kind = 'fk';         Name = 'fk_portfolio_operation_idempotency_refresh_request';     Delete = 'n' },
    @{ Kind = 'check';      Name = 'chk_portfolio_operation_idempotency_request_kind' },
    @{ Kind = 'check';      Name = 'chk_portfolio_operation_idempotency_correction_link' }
)

$missing = @()

foreach ($table in $requiredTables) {
    $present = "SELECT to_regclass('public.$table') IS NOT NULL;" |
        docker exec -i $container psql -U kushim -d kushim -t -A
    if ($present.Trim() -eq 't') {
        Write-Host "  table  OK: $table"
    } else {
        Write-Host "  table  MISSING: $table"
        $missing += "table $table"
    }
}

foreach ($obj in $requiredObjects) {
    switch ($obj.Kind) {
        'index' {
            $sql = "SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = '$($obj.Name)');"
        }
        'check' {
            $sql = "SELECT EXISTS(SELECT 1 FROM pg_constraint WHERE conname = '$($obj.Name)' AND contype = 'c');"
        }
        'fk' {
            # confdeltype: 'r' RESTRICT, 'n' SET NULL, 'c' CASCADE, 'a' NO ACTION.
            $sql = "SELECT EXISTS(SELECT 1 FROM pg_constraint WHERE conname = '$($obj.Name)' AND contype = 'f' AND confdeltype = '$($obj.Delete)');"
        }
        default {
            Write-Error "Unknown verification kind: $($obj.Kind)"
            exit 1
        }
    }
    $present = $sql | docker exec -i $container psql -U kushim -d kushim -t -A
    if ($present.Trim() -eq 't') {
        $detail = if ($obj.Kind -eq 'fk') { " (ON DELETE $($obj.Delete))" } else { '' }
        Write-Host "  $($obj.Kind)  OK: $($obj.Name)$detail"
    } else {
        Write-Host "  $($obj.Kind)  MISSING or WRONG: $($obj.Name)"
        $missing += "$($obj.Kind) $($obj.Name)"
    }
}

if ($missing.Count -gt 0) {
    Write-Host ''
    Write-Error ("Required objects missing after upgrade: " + ($missing -join ', '))
    exit 1
}

Write-Host ''
Write-Host 'All required tables, indexes and FK constraints verified. Upgrade complete.'
exit 0
