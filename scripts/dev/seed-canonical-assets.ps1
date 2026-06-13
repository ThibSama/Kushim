<#
.SYNOPSIS
    Apply the canonical Kushim MVP asset seed to the local PostgreSQL container.

.DESCRIPTION
    Loads infra/postgres/init/002_seed_canonical_assets.sql into the running
    `kushim_database` container, then asserts that exactly one row exists for
    each canonical (ticker, exchange) tuple:

        (AAPL, NASDAQ)
        (MSFT, NASDAQ)
        (NVDA, NASDAQ)

    The script is idempotent: it relies on the partial unique index
    uq_assets_ticker_exchange and ON CONFLICT DO UPDATE in the seed file.
    Legacy rows with ticker IS NULL are reported but never deleted.

.NOTES
    - Read-only against any non-canonical rows.
    - Does not touch market data, history cache, portfolios or operations.
    - No secrets are read or printed.
    - Safe to run multiple times.
#>

$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$seedPath = Join-Path $repoRoot 'infra\postgres\init\002_seed_canonical_assets.sql'

if (-not (Test-Path $seedPath)) {
    Write-Error "Canonical seed file not found at $seedPath"
    exit 1
}

# 1. Verify the database container is running.
$container = 'kushim_database'
$running = docker ps --filter "name=^/$container$" --filter 'status=running' --format '{{.Names}}'
if (-not $running) {
    Write-Error "Container '$container' is not running. Start the stack first with: docker compose up -d database"
    exit 1
}

# 2. Apply the seed via psql.
Write-Host "Applying canonical seed from $seedPath ..."
$sqlContent = Get-Content -Raw -Path $seedPath
$sqlContent | docker exec -i $container psql -U kushim -d kushim -v ON_ERROR_STOP=1
if ($LASTEXITCODE -ne 0) {
    Write-Error "psql failed while applying the canonical seed (exit $LASTEXITCODE)"
    exit 1
}

# 3. Resolve canonical IDs and print them (IDs are not secrets).
Write-Host ''
Write-Host 'Canonical catalogue entries:'
$resolveQuery = @'
SELECT id_asset, ticker, exchange, name, status, native_currency
FROM assets
WHERE (ticker, exchange) IN (('AAPL','NASDAQ'),('MSFT','NASDAQ'),('NVDA','NASDAQ'))
ORDER BY ticker;
'@
$resolveQuery | docker exec -i $container psql -U kushim -d kushim
if ($LASTEXITCODE -ne 0) {
    Write-Error 'Failed to resolve canonical rows.'
    exit 1
}

# 4. Strict exact-match assertion: exactly one canonical row per (ticker, exchange).
$assertQuery = @'
SELECT ticker, COUNT(*) AS canonical_row_count
FROM assets
WHERE (ticker, exchange) IN (('AAPL','NASDAQ'),('MSFT','NASDAQ'),('NVDA','NASDAQ'))
GROUP BY ticker
ORDER BY ticker;
'@
$assertCsv = $assertQuery | docker exec -i $container psql -U kushim -d kushim -t -A -F ',' 2>$null
if ($LASTEXITCODE -ne 0) {
    Write-Error 'Failed to count canonical rows.'
    exit 1
}

$expected = @{ 'AAPL' = $false; 'MSFT' = $false; 'NVDA' = $false }
foreach ($line in $assertCsv -split "`n") {
    $line = $line.Trim()
    if (-not $line) { continue }
    $parts = $line -split ','
    if ($parts.Count -ne 2) { continue }
    $ticker = $parts[0].Trim()
    $count  = [int]$parts[1].Trim()
    if ($expected.ContainsKey($ticker)) {
        if ($count -ne 1) {
            Write-Error "Canonical assertion failed: expected exactly 1 row for ($ticker, NASDAQ), found $count."
            exit 1
        }
        $expected[$ticker] = $true
    }
}
foreach ($key in $expected.Keys) {
    if (-not $expected[$key]) {
        Write-Error "Canonical assertion failed: no row resolved for ($key, NASDAQ)."
        exit 1
    }
}

# 5. Report legacy rows separately (informational only).
$legacyQuery = @'
SELECT id_asset, name, ticker, symbol, exchange, status, native_currency, created_at
FROM assets
WHERE COALESCE(ticker, symbol) IN ('AAPL','MSFT','NVDA')
  AND (ticker IS NULL OR exchange IS NULL OR exchange <> 'NASDAQ')
ORDER BY COALESCE(ticker, symbol), created_at;
'@
$legacyOutput = $legacyQuery | docker exec -i $container psql -U kushim -d kushim 2>$null

Write-Host ''
if ($legacyOutput -match '\(0 rows\)') {
    Write-Host 'Legacy non-canonical rows for AAPL/MSFT/NVDA: none.'
} else {
    Write-Host 'Legacy non-canonical rows for AAPL/MSFT/NVDA (NOT deleted by this script):'
    Write-Host $legacyOutput
    Write-Host 'NOTE: these rows still resolve to the same symbol via COALESCE(ticker, symbol).'
    Write-Host '      Run scripts\dev\audit-asset-catalog.ps1 for a full safety assessment.'
}

Write-Host ''
Write-Host 'Canonical seed applied. Exactly one canonical row per (ticker, NASDAQ) for AAPL, MSFT, NVDA.'
exit 0
