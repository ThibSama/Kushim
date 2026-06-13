<#
.SYNOPSIS
    Read-only Kushim asset catalogue audit.

.DESCRIPTION
    Reports the current state of the local Kushim asset catalogue and
    decides whether it is SAFE for a controlled Finnhub allowlist
    (AAPL/MSFT/NVDA).

    The script never deletes, updates, deactivates, remaps, or otherwise
    mutates catalogue rows. It is read-only.

    SAFE means:
        - exactly one canonical row exists for each of
              (AAPL, NASDAQ), (MSFT, NASDAQ), (NVDA, NASDAQ)
        - no other active row resolves to AAPL/MSFT/NVDA via
              COALESCE(ticker, symbol)

    Otherwise the script reports UNSAFE and explains why. Pre-existing
    legacy duplicates count as UNSAFE here; a code change cannot delete
    them, that is an explicit operator decision (see deferred-todos and
    backend-demo-e2e documentation).
#>

$ErrorActionPreference = 'Stop'

$container = 'kushim_database'
$running = docker ps --filter "name=^/$container$" --filter 'status=running' --format '{{.Names}}'
if (-not $running) {
    Write-Error "Container '$container' is not running. Start with: docker compose up -d database"
    exit 1
}

function Invoke-Psql {
    param([string]$Sql)
    return $Sql | docker exec -i $container psql -U kushim -d kushim
}

function Invoke-PsqlTsv {
    param([string]$Sql)
    return $Sql | docker exec -i $container psql -U kushim -d kushim -t -A -F "`t"
}

Write-Host '===================================================='
Write-Host ' Kushim asset catalogue audit (read-only)'
Write-Host '===================================================='

Write-Host ''
Write-Host '1. Active asset count by resolved symbol (AAPL, MSFT, NVDA)'
Write-Host '-----------------------------------------------------------'
Invoke-Psql @'
SELECT COALESCE(ticker, symbol) AS resolved_symbol,
       COUNT(*)                                       AS asset_count,
       COUNT(*) FILTER (WHERE status = 'active')      AS active_count,
       COUNT(*) FILTER (WHERE ticker IS NOT NULL
                          AND exchange = 'NASDAQ')    AS canonical_count
FROM assets
WHERE COALESCE(ticker, symbol) IN ('AAPL','MSFT','NVDA')
GROUP BY COALESCE(ticker, symbol)
ORDER BY resolved_symbol;
'@

Write-Host ''
Write-Host '2. Exact canonical rows (ticker, exchange = NASDAQ)'
Write-Host '----------------------------------------------------'
Invoke-Psql @'
SELECT id_asset, ticker, exchange, name, status, native_currency, created_at
FROM assets
WHERE (ticker, exchange) IN (('AAPL','NASDAQ'),('MSFT','NASDAQ'),('NVDA','NASDAQ'))
ORDER BY ticker;
'@

Write-Host ''
Write-Host '3. Legacy / non-canonical rows resolving to AAPL/MSFT/NVDA'
Write-Host '-----------------------------------------------------------'
Invoke-Psql @'
SELECT id_asset, name, ticker, symbol, exchange, status, native_currency, created_at
FROM assets
WHERE COALESCE(ticker, symbol) IN ('AAPL','MSFT','NVDA')
  AND (ticker IS NULL OR exchange IS NULL OR exchange <> 'NASDAQ')
ORDER BY COALESCE(ticker, symbol), created_at;
'@

Write-Host ''
Write-Host '4. Rows matching legacy test/demo name patterns'
Write-Host '------------------------------------------------'
Invoke-Psql @'
SELECT id_asset, name, ticker, symbol, exchange, status, created_at
FROM assets
WHERE name LIKE 'test\_%' ESCAPE '\'
   OR name LIKE 'test\_hist\_%' ESCAPE '\'
   OR name LIKE 'test\_history\_%' ESCAPE '\'
   OR name LIKE 'test\_current\_%' ESCAPE '\'
   OR name LIKE '%E2E Demo%'
ORDER BY name, created_at;
'@

Write-Host ''
Write-Host '5. Duplicate resolved symbols across the whole catalogue'
Write-Host '---------------------------------------------------------'
Invoke-Psql @'
SELECT COALESCE(ticker, symbol) AS resolved_symbol, COUNT(*) AS active_count
FROM assets
WHERE status = 'active'
  AND COALESCE(ticker, symbol) IS NOT NULL
GROUP BY COALESCE(ticker, symbol)
HAVING COUNT(*) > 1
ORDER BY active_count DESC, resolved_symbol;
'@

Write-Host ''
Write-Host '6. Reference counts from asset-linked tables (canonical 3 only)'
Write-Host '----------------------------------------------------------------'
Invoke-Psql @'
WITH canonical AS (
    SELECT id_asset, ticker
    FROM assets
    WHERE (ticker, exchange) IN (('AAPL','NASDAQ'),('MSFT','NASDAQ'),('NVDA','NASDAQ'))
)
SELECT c.ticker,
       (SELECT COUNT(*) FROM portfolio_operations            WHERE id_asset = c.id_asset) AS portfolio_operations,
       (SELECT COUNT(*) FROM rm_portfolio_holdings           WHERE id_asset = c.id_asset) AS rm_holdings,
       (SELECT COUNT(*) FROM asset_market_data               WHERE id_asset = c.id_asset) AS market_data,
       (SELECT COUNT(*) FROM asset_price_history_cache       WHERE id_asset = c.id_asset) AS price_history,
       (SELECT COUNT(*) FROM portfolio_holding_snapshot_daily WHERE id_asset = c.id_asset) AS holding_snapshots
FROM canonical c
ORDER BY c.ticker;
'@

Write-Host ''
Write-Host '===================================================='
Write-Host ' Safety verdict'
Write-Host '===================================================='

# Decision logic.
# A catalogue is SAFE when, for each canonical symbol S in (AAPL, MSFT, NVDA):
#   - exactly one row has (ticker = S, exchange = 'NASDAQ', status = 'active')
#   - no other active row resolves to S via COALESCE(ticker, symbol).
$decisionSql = @'
WITH counters AS (
    SELECT s.symbol AS canonical_symbol,
           (SELECT COUNT(*) FROM assets a
             WHERE a.ticker = s.symbol AND a.exchange = 'NASDAQ' AND a.status = 'active') AS canonical_active,
           (SELECT COUNT(*) FROM assets a
             WHERE a.status = 'active'
               AND COALESCE(a.ticker, a.symbol) = s.symbol
               AND (a.ticker IS DISTINCT FROM s.symbol
                    OR a.exchange IS DISTINCT FROM 'NASDAQ')) AS extra_active
    FROM (VALUES ('AAPL'),('MSFT'),('NVDA')) AS s(symbol)
)
SELECT canonical_symbol, canonical_active, extra_active FROM counters ORDER BY canonical_symbol;
'@

$rows = Invoke-PsqlTsv $decisionSql
$problems = New-Object System.Collections.Generic.List[string]
foreach ($line in $rows -split "`n") {
    $line = $line.Trim()
    if (-not $line) { continue }
    $parts = $line -split "`t"
    if ($parts.Count -ne 3) { continue }
    $sym = $parts[0]
    $canonical = [int]$parts[1]
    $extra     = [int]$parts[2]
    if ($canonical -ne 1) {
        $problems.Add("($sym, NASDAQ) canonical active rows = $canonical (expected 1)")
    }
    if ($extra -gt 0) {
        $problems.Add("$sym has $extra other active row(s) resolving to it via COALESCE(ticker, symbol)")
    }
}

if ($problems.Count -eq 0) {
    Write-Host ''
    Write-Host 'SAFE' -ForegroundColor Green
    Write-Host 'AAPL, MSFT, NVDA each resolve to exactly one active catalogue asset.'
    Write-Host 'A controlled Finnhub allowlist will not refresh duplicate rows.'
    exit 0
} else {
    Write-Host ''
    Write-Host 'UNSAFE -- duplicate active provider symbols detected' -ForegroundColor Yellow
    foreach ($p in $problems) {
        Write-Host "  - $p"
    }
    Write-Host ''
    Write-Host 'This script does NOT delete catalogue rows. To resolve:'
    Write-Host '  - Path A (disposable local data) : docker compose down -v; docker compose up -d ...'
    Write-Host '  - Path B (preserve local data)   : design a reviewed migration; do not blindly drop'
    Write-Host '  - Code prevention is already in place: demo scripts and tests no longer create'
    Write-Host '    AAPL/MSFT/NVDA fixtures. Existing duplicates are local-data debt only.'
    exit 0
}
