<#
.SYNOPSIS
    Dry-run-by-default cleanup of legacy integration-test equity fixtures
    from the local Kushim asset catalogue.

.DESCRIPTION
    Identifies and (optionally) removes legacy test/fixture rows that were
    inserted by historical kushim-api and kushim-worker integration tests
    without subsequent cleanup. The selection rule mirrors the runtime
    catalogue discoverability policy enforced by the `kushim-api`
    `list_assets_page` repository:

        asset_class = 'equity' AND exchange IS NULL

    Rationale.
      - The canonical catalogue seed (002_seed_canonical_assets.sql)
        states that an equity's natural identity is (ticker, exchange).
        Canonical equities (AAPL, MSFT, NVDA) all have NASDAQ as their
        exchange and are therefore NEVER candidates.
      - Crypto, ETF, fund, bond, forex and other asset classes have a
        different natural identity (symbol/network/ISIN) and are NEVER
        candidates, regardless of exchange.
      - The same rule perfectly separates the 490 currently-leaked
        integration-test fixtures from the 3 canonical rows.

    Safety guarantees.
      - Default mode is read-only (-DryRun explicit or implicit).
      - `-Apply` only deletes candidates with ZERO references in any
        RESTRICT-FK dependent table:
            portfolio_operations.id_asset
            portfolio_operations.id_related_asset
            asset_aliases.id_asset
            asset_metadata.id_asset
            portfolio_holding_snapshot_daily.id_asset
        CASCADE-FK rows (asset_market_data, asset_price_history_cache,
        rm_portfolio_holdings) are removed automatically with the asset.
      - Apply mode wraps all deletes in a single transaction so a failure
        leaves the catalogue unchanged.
      - The script is idempotent: re-running -Apply after a successful
        apply finds zero candidates and makes no changes.
      - It never targets canonical rows (their (ticker, exchange) IS NOT
        NULL).
      - It never deletes referenced rows; those are reported as
        "skipped — referenced" so an operator can inspect them.

.PARAMETER DryRun
    Default. List candidates and per-row reference counts without
    mutating anything.

.PARAMETER Apply
    Actually delete unreferenced candidates inside a transaction.

.EXAMPLE
    .\scripts\dev\clean-asset-catalog.ps1
    .\scripts\dev\clean-asset-catalog.ps1 -DryRun
    .\scripts\dev\clean-asset-catalog.ps1 -Apply
#>

[CmdletBinding(DefaultParameterSetName = 'DryRun')]
param(
    [Parameter(ParameterSetName = 'DryRun')]
    [switch]$DryRun,

    [Parameter(ParameterSetName = 'Apply')]
    [switch]$Apply
)

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
Write-Host ' Kushim asset catalogue cleanup'
if ($Apply) {
    Write-Host ' Mode: APPLY (will delete unreferenced fixtures)'
} else {
    Write-Host ' Mode: DRY-RUN (no database mutation)'
}
Write-Host '===================================================='
Write-Host ''

Write-Host '1. Catalogue snapshot (before)'
Write-Host '-------------------------------'
Invoke-Psql @'
SELECT
    COUNT(*) AS total_assets,
    COUNT(*) FILTER (
        WHERE asset_class = 'equity' AND exchange IS NOT NULL
    ) AS canonical_equities,
    COUNT(*) FILTER (
        WHERE asset_class = 'equity' AND exchange IS NULL
    ) AS equity_fixtures,
    COUNT(*) FILTER (
        WHERE asset_class <> 'equity'
    ) AS non_equity_assets
FROM assets;
'@

Write-Host ''
Write-Host '2. Candidate fixtures and per-row reference counts'
Write-Host '---------------------------------------------------'
$candidateSql = @'
WITH candidates AS (
    SELECT id_asset, name, symbol, ticker, created_at
    FROM assets
    WHERE asset_class = 'equity' AND exchange IS NULL
)
SELECT
    c.id_asset,
    c.name,
    COALESCE(c.ticker, c.symbol) AS resolved_symbol,
    (SELECT COUNT(*) FROM portfolio_operations po
        WHERE po.id_asset = c.id_asset OR po.id_related_asset = c.id_asset) AS portfolio_ops,
    (SELECT COUNT(*) FROM asset_aliases aa WHERE aa.id_asset = c.id_asset) AS aliases,
    (SELECT COUNT(*) FROM asset_metadata am WHERE am.id_asset = c.id_asset) AS metadata_refs,
    (SELECT COUNT(*) FROM portfolio_holding_snapshot_daily phs WHERE phs.id_asset = c.id_asset) AS holding_snapshots
FROM candidates c
ORDER BY c.created_at DESC
LIMIT 20;
'@
Invoke-Psql $candidateSql

Write-Host ''
Write-Host '3. Eligible-for-deletion summary'
Write-Host '---------------------------------'
$summarySql = @'
WITH candidates AS (
    SELECT id_asset FROM assets WHERE asset_class = 'equity' AND exchange IS NULL
),
referenced AS (
    SELECT c.id_asset
    FROM candidates c
    WHERE EXISTS (SELECT 1 FROM portfolio_operations po
                   WHERE po.id_asset = c.id_asset OR po.id_related_asset = c.id_asset)
       OR EXISTS (SELECT 1 FROM asset_aliases aa WHERE aa.id_asset = c.id_asset)
       OR EXISTS (SELECT 1 FROM asset_metadata am WHERE am.id_asset = c.id_asset)
       OR EXISTS (SELECT 1 FROM portfolio_holding_snapshot_daily phs WHERE phs.id_asset = c.id_asset)
)
SELECT
    (SELECT COUNT(*) FROM candidates) AS total_candidates,
    (SELECT COUNT(*) FROM referenced) AS skipped_referenced,
    (SELECT COUNT(*) FROM candidates) - (SELECT COUNT(*) FROM referenced) AS eligible_for_deletion;
'@
Invoke-Psql $summarySql

$eligibleCountRaw = (Invoke-PsqlTsv @'
WITH candidates AS (
    SELECT id_asset FROM assets WHERE asset_class = 'equity' AND exchange IS NULL
)
SELECT COUNT(*)
FROM candidates c
WHERE NOT EXISTS (SELECT 1 FROM portfolio_operations po
                   WHERE po.id_asset = c.id_asset OR po.id_related_asset = c.id_asset)
  AND NOT EXISTS (SELECT 1 FROM asset_aliases aa WHERE aa.id_asset = c.id_asset)
  AND NOT EXISTS (SELECT 1 FROM asset_metadata am WHERE am.id_asset = c.id_asset)
  AND NOT EXISTS (SELECT 1 FROM portfolio_holding_snapshot_daily phs WHERE phs.id_asset = c.id_asset);
'@).Trim()
$eligibleCount = [int]$eligibleCountRaw

Write-Host ''
if (-not $Apply) {
    Write-Host "Dry-run complete: $eligibleCount unreferenced fixture(s) eligible for deletion." -ForegroundColor Yellow
    Write-Host 'Re-run with -Apply to delete them inside a transaction. No mutation was performed.'
    exit 0
}

if ($eligibleCount -eq 0) {
    Write-Host 'Apply mode: nothing to delete.' -ForegroundColor Green
    Write-Host 'The catalogue is already free of unreferenced equity fixtures (idempotent).'
    exit 0
}

Write-Host ''
Write-Host "Apply mode: deleting $eligibleCount unreferenced equity fixture(s) in a transaction..."
$applySql = @'
BEGIN;

WITH candidates AS (
    SELECT id_asset FROM assets WHERE asset_class = 'equity' AND exchange IS NULL
),
eligible AS (
    SELECT c.id_asset
    FROM candidates c
    WHERE NOT EXISTS (SELECT 1 FROM portfolio_operations po
                       WHERE po.id_asset = c.id_asset OR po.id_related_asset = c.id_asset)
      AND NOT EXISTS (SELECT 1 FROM asset_aliases aa WHERE aa.id_asset = c.id_asset)
      AND NOT EXISTS (SELECT 1 FROM asset_metadata am WHERE am.id_asset = c.id_asset)
      AND NOT EXISTS (SELECT 1 FROM portfolio_holding_snapshot_daily phs WHERE phs.id_asset = c.id_asset)
)
DELETE FROM assets WHERE id_asset IN (SELECT id_asset FROM eligible);

COMMIT;
'@
Invoke-Psql $applySql

Write-Host ''
Write-Host '4. Catalogue snapshot (after)'
Write-Host '------------------------------'
Invoke-Psql @'
SELECT
    COUNT(*) AS total_assets,
    COUNT(*) FILTER (
        WHERE asset_class = 'equity' AND exchange IS NOT NULL
    ) AS canonical_equities,
    COUNT(*) FILTER (
        WHERE asset_class = 'equity' AND exchange IS NULL
    ) AS equity_fixtures_remaining,
    COUNT(*) FILTER (
        WHERE asset_class <> 'equity'
    ) AS non_equity_assets
FROM assets;
'@

Write-Host ''
Write-Host 'Apply mode complete.' -ForegroundColor Green
exit 0
