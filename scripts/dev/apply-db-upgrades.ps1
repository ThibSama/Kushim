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
Write-Host 'All upgrade scripts applied. Verifying portfolio_refresh_requests table...'
$tableCheck = 'SELECT to_regclass(''public.portfolio_refresh_requests'') IS NOT NULL;' |
    docker exec -i $container psql -U kushim -d kushim -t -A
if ($tableCheck.Trim() -ne 't') {
    Write-Error 'portfolio_refresh_requests table is missing after upgrade.'
    exit 1
}

Write-Host 'portfolio_refresh_requests table present. Upgrade complete.'
exit 0
