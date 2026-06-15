<#
.SYNOPSIS
    Kushim Backend MVP E2E Demo Smoke Test

.DESCRIPTION
    Executes the full backend-only MVP scenario automatically:
    signup -> portfolio -> operations -> market-data jobs -> worker jobs -> API verification

    This script is for local development and demo validation only.
    It does not modify application code, DDL, or Docker configuration.
    It does not delete or truncate any data.

    See: documentation/operations/backend-demo-e2e.md

.PARAMETER BaseUrlAuth
    Base URL for kushim-auth-api. Default: http://localhost:3002

.PARAMETER BaseUrlApi
    Base URL for kushim-api. Default: http://localhost:8080

.PARAMETER DemoPrefix
    Prefix for demo user/portfolio names. Default: demo_e2e

.PARAMETER DepositDate
    Execution date for the deposit operation (RFC3339). Default: 2026-06-01T10:00:00Z

.PARAMETER BuyDate
    Execution date for the buy operation (RFC3339). Default: 2026-06-02T14:00:00Z

.PARAMETER SnapshotDate
    Date for daily snapshot generation (YYYY-MM-DD). Default: 2026-06-09

.PARAMETER HistoryDateFrom
    Start date for historical price fill and backfill (YYYY-MM-DD). Default: 2026-06-01

.PARAMETER HistoryDateTo
    End date for historical price fill and backfill (YYYY-MM-DD). Default: 2026-06-09

.PARAMETER BackfillDateTo
    End date for backfill snapshots. Defaults to one day before SnapshotDate.

.PARAMETER SkipDockerJobs
    Skip all Docker Compose job steps (market-data and worker). Useful when jobs were already run.

.PARAMETER VerboseJson
    Print full JSON responses for verification endpoints.

.PARAMETER DryRun
    Only verify health endpoints and print what would be done, without executing.

.EXAMPLE
    .\backend-e2e.ps1

.EXAMPLE
    .\backend-e2e.ps1 -VerboseJson

.EXAMPLE
    .\backend-e2e.ps1 -SkipDockerJobs -VerboseJson

.EXAMPLE
    .\backend-e2e.ps1 -DemoPrefix "jury_demo" -SnapshotDate "2026-06-09"
#>

param(
    [string]$BaseUrlAuth     = "http://localhost:3002",
    [string]$BaseUrlApi      = "http://localhost:8080",
    [string]$DemoPrefix      = "demo_e2e",
    [string]$DepositDate     = "2026-06-01T10:00:00Z",
    [string]$BuyDate         = "2026-06-02T14:00:00Z",
    [string]$SnapshotDate    = "2026-06-09",
    [string]$HistoryDateFrom = "2026-06-01",
    [string]$HistoryDateTo   = "2026-06-09",
    [string]$BackfillDateTo  = "",
    # Optional explicit demo password override. When empty, a per-run strong
    # password is generated in memory. The chosen value is never logged or
    # written to disk. Override only when wiring this script into a flow
    # that needs to reuse the same user across runs.
    [string]$DemoPassword    = "",
    [switch]$SkipDockerJobs,
    [switch]$VerboseJson,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# Logging helpers
# ---------------------------------------------------------------------------
function Write-Info    { param([string]$Message) Write-Host "[INFO]    $Message" -ForegroundColor Cyan }
function Write-Success { param([string]$Message) Write-Host "[OK]      $Message" -ForegroundColor Green }
function Write-Warn    { param([string]$Message) Write-Host "[WARN]    $Message" -ForegroundColor Yellow }
function Write-Err     { param([string]$Message) Write-Host "[ERROR]   $Message" -ForegroundColor Red }
function Write-Step    { param([string]$Message) Write-Host "`n========= $Message =========" -ForegroundColor Magenta }

# ---------------------------------------------------------------------------
# State
# ---------------------------------------------------------------------------
$script:Warnings  = [System.Collections.Generic.List[string]]::new()
$script:Passed    = [System.Collections.Generic.List[string]]::new()
$script:Failed    = [System.Collections.Generic.List[string]]::new()
$script:DemoState = @{
    Username         = ""
    UserId           = ""
    PortfolioId      = ""
    AssetId          = ""
    DepositOpId      = ""
    BuyOpId          = ""
    AccessToken      = ""
    RefreshRequestId = ""
}

# Compute backfill end date: one day before snapshot date if not provided
if ($BackfillDateTo -eq "") {
    try {
        $snapshotDt = [datetime]::ParseExact($SnapshotDate, "yyyy-MM-dd", $null)
        $BackfillDateTo = $snapshotDt.AddDays(-1).ToString("yyyy-MM-dd")
    } catch {
        $BackfillDateTo = $HistoryDateTo
    }
}

# Unique suffix for this run
$runSuffix = (Get-Date).ToString("yyyyMMdd_HHmmss")

# ---------------------------------------------------------------------------
# HTTP helpers
# ---------------------------------------------------------------------------
function Invoke-ApiGet {
    param(
        [string]$Url,
        [hashtable]$Headers = @{}
    )
    try {
        $response = Invoke-RestMethod -Uri $Url -Method GET -Headers $Headers -ContentType "application/json"
        return $response
    } catch {
        $statusCode = $null
        if ($_.Exception.Response) {
            $statusCode = [int]$_.Exception.Response.StatusCode
        }
        throw "GET $Url failed (status=$statusCode): $($_.Exception.Message)"
    }
}

function Invoke-ApiPost {
    param(
        [string]$Url,
        [hashtable]$Headers = @{},
        [object]$Body = $null
    )
    $params = @{
        Uri         = $Url
        Method      = "POST"
        Headers     = $Headers
        ContentType = "application/json"
    }
    if ($null -ne $Body) {
        $jsonBody = $Body | ConvertTo-Json -Depth 10 -Compress
        $params["Body"] = [System.Text.Encoding]::UTF8.GetBytes($jsonBody)
    }
    try {
        $response = Invoke-RestMethod @params
        return $response
    } catch {
        $statusCode = $null
        $responseBody = ""
        if ($_.Exception.Response) {
            $statusCode = [int]$_.Exception.Response.StatusCode
            try {
                $reader = [System.IO.StreamReader]::new($_.Exception.Response.GetResponseStream())
                $responseBody = $reader.ReadToEnd()
                $reader.Close()
            } catch {}
        }
        throw "POST $Url failed (status=$statusCode): $($_.Exception.Message)`nResponse: $responseBody"
    }
}

function Invoke-ApiPostNoBody {
    param(
        [string]$Url,
        [hashtable]$Headers = @{}
    )
    try {
        $response = Invoke-RestMethod -Uri $Url -Method POST -Headers $Headers -ContentType "application/json"
        return $response
    } catch {
        $statusCode = $null
        $responseBody = ""
        if ($_.Exception.Response) {
            $statusCode = [int]$_.Exception.Response.StatusCode
            try {
                $reader = [System.IO.StreamReader]::new($_.Exception.Response.GetResponseStream())
                $responseBody = $reader.ReadToEnd()
                $reader.Close()
            } catch {}
        }
        throw "POST $Url failed (status=$statusCode): $($_.Exception.Message)`nResponse: $responseBody"
    }
}

function Get-AuthHeaders {
    return @{ Authorization = "Bearer $($script:DemoState.AccessToken)" }
}

function Assert-True {
    param(
        [string]$Name,
        [bool]$Condition,
        [string]$FailMessage = ""
    )
    if ($Condition) {
        $script:Passed.Add($Name)
        Write-Success "PASS: $Name"
    } else {
        $script:Failed.Add($Name)
        $msg = "FAIL: $Name"
        if ($FailMessage) { $msg += " -- $FailMessage" }
        Write-Err $msg
    }
}

# ---------------------------------------------------------------------------
# Step A: Verify infrastructure
# ---------------------------------------------------------------------------
function Test-ServiceHealth {
    param([string]$Name, [string]$Url)
    try {
        $response = Invoke-RestMethod -Uri "$Url/health" -Method GET -TimeoutSec 5
        if ($response.status -eq "ok") {
            Write-Success "$Name is healthy"
            return $true
        } else {
            Write-Err "$Name health check returned unexpected status: $($response.status)"
            return $false
        }
    } catch {
        Write-Err "$Name is not reachable at $Url/health: $($_.Exception.Message)"
        return $false
    }
}

Write-Step "A. Verify infrastructure"

$healthOk = $true
$healthOk = (Test-ServiceHealth "kushim-auth-api"  $BaseUrlAuth) -and $healthOk
$healthOk = (Test-ServiceHealth "kushim-api"       $BaseUrlApi) -and $healthOk
$healthOk = (Test-ServiceHealth "kushim-worker"    "http://localhost:8081") -and $healthOk
$healthOk = (Test-ServiceHealth "kushim-market-data" "http://localhost:8082") -and $healthOk

if (-not $healthOk) {
    Write-Err "One or more services are not healthy. Cannot proceed."
    Write-Err "Start services with: docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data"
    exit 1
}

# Verify docker compose is available for job steps
if (-not $SkipDockerJobs) {
    try {
        $dcVersion = docker compose version 2>&1
        Write-Info "Docker Compose: $dcVersion"
    } catch {
        Write-Err "docker compose is not available. Use -SkipDockerJobs to skip job steps."
        exit 1
    }
}

if ($DryRun) {
    Write-Info "DryRun mode: all services are healthy. Exiting without executing demo steps."
    exit 0
}

# ---------------------------------------------------------------------------
# Step B: Signup demo user
# ---------------------------------------------------------------------------
Write-Step "B. Signup demo user"

# ---------------------------------------------------------------------------
# Username contract (kushim-auth/api): ^[a-z0-9_][a-z0-9_-]{2,39}$
#   - 3 to 40 characters total
#   - first char: lowercase letter, digit, or underscore
#   - remaining chars: lowercase letters, digits, underscores, hyphens
# The final username is "<DemoPrefix>_<runSuffix>". We validate it locally
# and fail fast BEFORE calling /auth/signup so an over-long or malformed
# prefix produces an actionable error instead of an opaque HTTP 4xx.
# ---------------------------------------------------------------------------
$AUTH_USERNAME_MAX = 40
$AUTH_USERNAME_REGEX = '^[a-z0-9_][a-z0-9_-]{2,39}$'

$username = "${DemoPrefix}_${runSuffix}"

# Maximum prefix length for the current suffix: total max minus the
# "_<runSuffix>" tail that this script always appends.
$suffixTailLength = ("_" + $runSuffix).Length
$maxPrefixLength = $AUTH_USERNAME_MAX - $suffixTailLength

function Stop-WithPrefixError {
    Write-Err "DemoPrefix is invalid."
    Write-Err "Generated username: $username"
    Write-Err "Maximum username length: $AUTH_USERNAME_MAX"
    Write-Err "Maximum DemoPrefix length for this run: $maxPrefixLength"
    Write-Err "Allowed DemoPrefix characters: lowercase a-z, digits, underscore and hyphen."
    exit 1
}

if ([string]::IsNullOrWhiteSpace($DemoPrefix)) {
    Write-Err "DemoPrefix must not be empty or whitespace."
    Stop-WithPrefixError
}

if ($username.Length -gt $AUTH_USERNAME_MAX) {
    Write-Err "Generated username is $($username.Length) characters; the auth API allows at most $AUTH_USERNAME_MAX."
    Stop-WithPrefixError
}

if ($username -cnotmatch $AUTH_USERNAME_REGEX) {
    Write-Err "Generated username does not match the auth username contract $AUTH_USERNAME_REGEX."
    Stop-WithPrefixError
}

# Per-run strong password. Generated in memory via a cryptographic RNG.
# Never logged, never written to disk, never echoed.
# The password policy (kushim-auth-api) requires 12-128 chars; this gives 32.
$AUTH_PASSWORD_MIN = 12
$AUTH_PASSWORD_MAX = 128

function New-DemoPassword {
    $alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789!@#%^*-_=+'
    $bytes = New-Object byte[] 32
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try {
        $rng.GetBytes($bytes)
    } finally {
        $rng.Dispose()
    }
    $chars = New-Object char[] 32
    for ($i = 0; $i -lt 32; $i++) {
        $chars[$i] = $alphabet[$bytes[$i] % $alphabet.Length]
    }
    return -join $chars
}

$passwordIsOverride = -not [string]::IsNullOrEmpty($DemoPassword)
if ($passwordIsOverride) {
    # Validate the explicit override against the current auth policy
    # (length + nonblank only; no complexity rules are enforced by the API).
    if ([string]::IsNullOrWhiteSpace($DemoPassword)) {
        Write-Err "DemoPassword override must not be blank."
        exit 1
    }
    if ($DemoPassword.Length -lt $AUTH_PASSWORD_MIN) {
        Write-Err "DemoPassword override is too short: minimum $AUTH_PASSWORD_MIN characters."
        exit 1
    }
    if ($DemoPassword.Length -gt $AUTH_PASSWORD_MAX) {
        Write-Err "DemoPassword override is too long: maximum $AUTH_PASSWORD_MAX characters."
        exit 1
    }
    $password = $DemoPassword
} else {
    $password = New-DemoPassword
}

Write-Info "username: $username"
if ($passwordIsOverride) {
    Write-Info "password: explicit override provided (value redacted)"
} else {
    Write-Info "password: generated in memory (length=$($password.Length))"
}

$signupBody = @{
    username = $username
    password = $password
}

try {
    $signupResponse = Invoke-ApiPost -Url "$BaseUrlAuth/auth/signup" -Body $signupBody
    $script:DemoState.AccessToken  = $signupResponse.access_token
    $script:DemoState.UserId       = $signupResponse.user.id_user
    $script:DemoState.Username     = $username
    Write-Success "User created: id=$($script:DemoState.UserId)"
} catch {
    # The username/password were validated locally above, so a failure here is
    # most likely a server-side condition. Surface the sanitized error and the
    # likely categories without asserting a single cause. The error text from
    # Invoke-ApiPost already includes HTTP status and the response body; it does
    # not include the request password (only the response is echoed).
    Write-Err "Signup failed: $_"
    Write-Err "Likely causes (not exhaustive): duplicate username; missing auth reference data (the 'user' role seed); username/password rejected by the auth policy; or an auth API internal error."
    Write-Err "If the 'user' role is missing on a fresh database, apply infra/postgres/init/003_seed_auth_roles.sql (loaded automatically on fresh volumes)."
    exit 1
}

# ---------------------------------------------------------------------------
# Step C: Verify token
# ---------------------------------------------------------------------------
Write-Step "C. Verify access token"

try {
    $meResponse = Invoke-ApiGet -Url "$BaseUrlApi/v1/me" -Headers (Get-AuthHeaders)
    Write-Success "Token verified via /v1/me (user=$($meResponse.id_user))"
} catch {
    Write-Err "Token verification failed: $_"
    exit 1
}

# ---------------------------------------------------------------------------
# Step D: Create USD portfolio
# ---------------------------------------------------------------------------
Write-Step "D. Create USD portfolio"

$portfolioName = "E2E Demo Portfolio $runSuffix"
Write-Info "Portfolio name: $portfolioName"

$portfolioBody = @{
    name          = $portfolioName
    base_currency = "USD"
}

try {
    $portfolioResponse = Invoke-ApiPost -Url "$BaseUrlApi/v1/portfolios" -Headers (Get-AuthHeaders) -Body $portfolioBody
    $script:DemoState.PortfolioId = $portfolioResponse.portfolio.id_portfolio
    Write-Success "Portfolio created: id=$($script:DemoState.PortfolioId)"
} catch {
    Write-Err "Portfolio creation failed: $_"
    exit 1
}

# ---------------------------------------------------------------------------
# Step E: Resolve canonical AAPL asset
#
# This script never creates catalogue assets. It expects the canonical
# (AAPL, NASDAQ) row to already exist, seeded by
# infra/postgres/init/002_seed_canonical_assets.sql. The seed is loaded
# automatically on fresh PostgreSQL volumes via docker-compose, and can
# be re-applied manually with:
#   powershell -ExecutionPolicy Bypass -File scripts/dev/seed-canonical-assets.ps1
# ---------------------------------------------------------------------------
Write-Step "E. Resolve canonical AAPL asset"

$resolveSql = "SELECT id_asset FROM assets WHERE ticker = 'AAPL' AND symbol = 'AAPL' AND exchange = 'NASDAQ' AND native_currency = 'USD' AND status = 'active'"
try {
    $resolveResult = docker exec kushim_database psql -U kushim -d kushim -t -A -c $resolveSql 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "psql exited with code $LASTEXITCODE : $resolveResult"
    }
} catch {
    Write-Err "Canonical AAPL resolution failed: $_"
    exit 1
}

$rows = @($resolveResult | Where-Object { $_ -and ($_ -match '^[0-9a-f-]{36}$') })
if ($rows.Count -eq 0) {
    Write-Err "Canonical (AAPL, NASDAQ, USD, active) row is missing from the assets catalogue."
    Write-Err "Run: powershell -ExecutionPolicy Bypass -File scripts/dev/seed-canonical-assets.ps1"
    exit 1
}
if ($rows.Count -gt 1) {
    Write-Err "Catalogue is ambiguous: $($rows.Count) exact canonical AAPL rows match (ticker='AAPL', exchange='NASDAQ', native_currency='USD', status='active')."
    Write-Err "Run: powershell -ExecutionPolicy Bypass -File scripts/dev/audit-asset-catalog.ps1 to inspect duplicates."
    exit 1
}

$script:DemoState.AssetId = $rows[0].Trim()
Write-Success "Canonical AAPL resolved: id=$($script:DemoState.AssetId) (reused, not created)"

# ---------------------------------------------------------------------------
# Docker job helper (defined before first use)
# ---------------------------------------------------------------------------
function Invoke-DockerJob {
    param(
        [string]$ServiceName,
        [string]$JobDescription,
        [string[]]$EnvArgs
    )

    Write-Info "Running: $JobDescription"

    $dockerArgs = @("compose", "run", "--rm")
    foreach ($envArg in $EnvArgs) {
        $dockerArgs += "-e"
        $dockerArgs += $envArg
    }
    $dockerArgs += $ServiceName

    Write-Info "Command: docker $($dockerArgs -join ' ')"

    $prevEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $stderrFile = [System.IO.Path]::GetTempFileName()
    try {
        $stdoutLines = & docker @dockerArgs 2>$stderrFile
        $exitCode = $LASTEXITCODE
        $stderrContent = Get-Content $stderrFile -Raw -ErrorAction SilentlyContinue
    } finally {
        Remove-Item $stderrFile -Force -ErrorAction SilentlyContinue
        $ErrorActionPreference = $prevEAP
    }

    if ($VerboseJson) {
        if ($stdoutLines) {
            $stdoutLines | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
        }
        if ($stderrContent) {
            $stderrContent -split "`n" | ForEach-Object {
                $line = $_.Trim()
                if ($line) { Write-Host "  [stderr] $line" -ForegroundColor DarkGray }
            }
        }
    }

    if ($exitCode -ne 0) {
        Write-Err "Docker job failed with exit code $exitCode"
        if ($stdoutLines) { $stdoutLines | ForEach-Object { Write-Host "  $_" -ForegroundColor Red } }
        if ($stderrContent) {
            $stderrContent -split "`n" | ForEach-Object {
                $line = $_.Trim()
                if ($line) { Write-Host "  [stderr] $line" -ForegroundColor Red }
            }
        }
        throw "$JobDescription failed (exit code $exitCode)"
    }

    Write-Success "$JobDescription completed"
}

# ---------------------------------------------------------------------------
# Step E2: Prepare market data BEFORE posting operations
#
# Current market data must exist before the automatic refresh runs, so the
# worker prices holdings deterministically (no race). This is legitimate
# market-data preparation and is intentionally kept. It is NOT the manual
# portfolio rebuild/snapshot orchestration (which has been removed).
# ---------------------------------------------------------------------------
if (-not $SkipDockerJobs) {
    Write-Step "E2. Market-data: refresh current + fill history (mock provider)"
    try {
        Invoke-DockerJob -ServiceName "kushim-market-data" -JobDescription "refresh_current_market_data" -EnvArgs @(
            "MARKET_DATA_MODE=once",
            "MARKET_DATA_JOB=refresh_current_market_data",
            "MARKET_DATA_PROVIDER=mock"
        )
        Invoke-DockerJob -ServiceName "kushim-market-data" -JobDescription "fill_missing_price_history_cache" -EnvArgs @(
            "MARKET_DATA_MODE=once",
            "MARKET_DATA_JOB=fill_missing_price_history_cache",
            "MARKET_DATA_PROVIDER=mock",
            "MARKET_DATA_HISTORY_DATE_FROM=$HistoryDateFrom",
            "MARKET_DATA_HISTORY_DATE_TO=$HistoryDateTo"
        )
    } catch {
        Write-Err "$_"
        exit 1
    }
} else {
    Write-Step "E2. Market-data preparation SKIPPED (-SkipDockerJobs)"
    $script:Warnings.Add("Market-data preparation skipped; holdings may be estimated.")
}

# ---------------------------------------------------------------------------
# Step F: Create and post deposit
# ---------------------------------------------------------------------------
Write-Step "F. Create and post deposit (10,000.00 USD)"

$depositBody = @{
    operation_type    = "deposit"
    executed_at       = $DepositDate
    gross_amount_minor = 1000000
    cash_amount_minor  = 1000000
    currency          = "USD"
    metadata          = @{}
}

try {
    $depositResponse = Invoke-ApiPost -Url "$BaseUrlApi/v1/portfolios/$($script:DemoState.PortfolioId)/operations" -Headers (Get-AuthHeaders) -Body $depositBody
    $script:DemoState.DepositOpId = $depositResponse.operation.id_portfolio_operation
    Write-Success "Deposit created: id=$($script:DemoState.DepositOpId) (status=$($depositResponse.operation.operation_status))"
} catch {
    Write-Err "Deposit creation failed: $_"
    exit 1
}

try {
    $postResponse = Invoke-ApiPostNoBody -Url "$BaseUrlApi/v1/portfolios/$($script:DemoState.PortfolioId)/operations/$($script:DemoState.DepositOpId)/post" -Headers (Get-AuthHeaders)
    Write-Success "Deposit posted: status=$($postResponse.operation.operation_status)"
} catch {
    Write-Err "Deposit post failed: $_"
    exit 1
}

# ---------------------------------------------------------------------------
# Step G: Create and post buy
# ---------------------------------------------------------------------------
Write-Step "G. Create and post buy (10 AAPL at 195.23 USD)"

$buyBody = @{
    id_asset           = $script:DemoState.AssetId
    operation_type     = "buy"
    executed_at        = $BuyDate
    quantity           = "10.0000000000"
    price_minor        = 19523
    gross_amount_minor = 195230
    cash_amount_minor  = 195230
    currency           = "USD"
    metadata           = @{}
}

try {
    $buyResponse = Invoke-ApiPost -Url "$BaseUrlApi/v1/portfolios/$($script:DemoState.PortfolioId)/operations" -Headers (Get-AuthHeaders) -Body $buyBody
    $script:DemoState.BuyOpId = $buyResponse.operation.id_portfolio_operation
    Write-Success "Buy created: id=$($script:DemoState.BuyOpId) (status=$($buyResponse.operation.operation_status))"
} catch {
    Write-Err "Buy creation failed: $_"
    exit 1
}

try {
    $postResponse = Invoke-ApiPostNoBody -Url "$BaseUrlApi/v1/portfolios/$($script:DemoState.PortfolioId)/operations/$($script:DemoState.BuyOpId)/post" -Headers (Get-AuthHeaders)
    Write-Success "Buy posted: status=$($postResponse.operation.operation_status)"
    if ($postResponse.refresh_request -and $postResponse.refresh_request.id_portfolio_refresh_request) {
        $script:DemoState.RefreshRequestId = $postResponse.refresh_request.id_portfolio_refresh_request
        Write-Success "Refresh request enqueued by API: id=$($script:DemoState.RefreshRequestId) (status=$($postResponse.refresh_request.status))"
    } else {
        Write-Err "Posting the buy did not return a refresh_request - automatic refresh contract violated."
        exit 1
    }
} catch {
    Write-Err "Buy post failed: $_"
    exit 1
}

# ---------------------------------------------------------------------------
# Step H: Automatic refresh — poll the durable refresh request until completed
#
# The kushim-worker service runs in loop mode with
# WORKER_JOB=process_portfolio_refresh_requests and consumes the request the
# API enqueued atomically when the operation was posted. There is NO manual
# rebuild_current_read_models or generate_daily_snapshots invocation here.
# ---------------------------------------------------------------------------
if ($SkipDockerJobs) {
    Write-Step "H. Automatic refresh polling SKIPPED (-SkipDockerJobs)"
    $script:Warnings.Add("Automatic refresh polling skipped; read models/snapshots may not be available.")
} else {
    Write-Step "H. Automatic refresh: poll refresh request until completed"

    $refreshId = $script:DemoState.RefreshRequestId
    $deadline = (Get-Date).AddSeconds(90)
    $lastStatus = "unknown"
    $refreshCompleted = $false

    while ((Get-Date) -lt $deadline) {
        try {
            $statusResponse = Invoke-ApiGet -Url "$BaseUrlApi/v1/portfolios/$($script:DemoState.PortfolioId)/refresh-requests/$refreshId" -Headers (Get-AuthHeaders)
            $lastStatus = $statusResponse.refresh_request.status
        } catch {
            $lastStatus = "poll_error"
        }

        Write-Info "refresh request status: $lastStatus"

        if ($lastStatus -eq "completed") {
            $refreshCompleted = $true
            break
        }
        if ($lastStatus -eq "failed") {
            Write-Err "Refresh request reached terminal 'failed' status (error_code=$($statusResponse.refresh_request.error_code))."
            exit 1
        }

        Start-Sleep -Seconds 3
    }

    if (-not $refreshCompleted) {
        Write-Err "Refresh request did not complete within the timeout. Last observed status: $lastStatus"
        exit 1
    }

    Write-Success "Automatic refresh completed (no manual rebuild/snapshot invocation was used)"

    # Optional: historical backfill remains a separate worker capability and is
    # NOT part of the per-operation automatic refresh. Kept to demonstrate
    # historical snapshots.
    Write-Step "I. Worker: backfill historical daily snapshots ($HistoryDateFrom to $BackfillDateTo)"
    try {
        Invoke-DockerJob -ServiceName "kushim-worker" -JobDescription "backfill_daily_snapshots" -EnvArgs @(
            "WORKER_MODE=once",
            "WORKER_JOB=backfill_daily_snapshots",
            "WORKER_TARGET_PORTFOLIO_ID=$($script:DemoState.PortfolioId)",
            "WORKER_BACKFILL_DATE_FROM=$HistoryDateFrom",
            "WORKER_BACKFILL_DATE_TO=$BackfillDateTo"
        )
    } catch {
        Write-Err "$_"
        exit 1
    }
}

# ---------------------------------------------------------------------------
# Step M: API verification
# ---------------------------------------------------------------------------
Write-Step "M. API verification"

$portfolioId = $script:DemoState.PortfolioId
$authHeaders = Get-AuthHeaders

# Re-authenticate if needed (token may have expired during Docker jobs)
try {
    $null = Invoke-ApiGet -Url "$BaseUrlApi/v1/me" -Headers $authHeaders
} catch {
    Write-Warn "Token may have expired. Re-authenticating..."
    try {
        $loginBody = @{
            username = $script:DemoState.Username
            password = $password
        }
        $loginResponse = Invoke-ApiPost -Url "$BaseUrlAuth/auth/login" -Body $loginBody
        $script:DemoState.AccessToken = $loginResponse.access_token
        $authHeaders = Get-AuthHeaders
        Write-Success "Re-authenticated successfully"
    } catch {
        Write-Err "Re-authentication failed: $_"
        exit 1
    }
}

# --- M.1: Portfolio summary ---
Write-Info "Verifying: GET /v1/portfolios/$portfolioId/summary"
try {
    $summary = Invoke-ApiGet -Url "$BaseUrlApi/v1/portfolios/$portfolioId/summary" -Headers $authHeaders
    if ($VerboseJson) { Write-Host ($summary | ConvertTo-Json -Depth 10) -ForegroundColor DarkGray }

    Assert-True "summary.data_available = true" ($summary.data_available -eq $true) "got: $($summary.data_available)"

    if ($summary.data_available -and $null -ne $summary.summary) {
        $s = $summary.summary
        Assert-True "summary.cash_balance_minor = 804770"     ($s.cash_balance_minor -eq 804770)     "got: $($s.cash_balance_minor)"
        Assert-True "summary.total_value_minor = 1000000"     ($s.total_value_minor -eq 1000000)     "got: $($s.total_value_minor)"
        Assert-True "summary.total_invested_minor = 1000000"  ($s.total_invested_minor -eq 1000000)  "got: $($s.total_invested_minor)"
        Assert-True "summary.total_pnl_minor = 0"             ($s.total_pnl_minor -eq 0)             "got: $($s.total_pnl_minor)"
        Assert-True "summary.is_estimated = false"            ($s.is_estimated -eq $false)           "got: $($s.is_estimated)"
        Assert-True "summary.portfolio_status = active"       ($s.portfolio_status -eq "active")     "got: $($s.portfolio_status)"
    } else {
        $script:Warnings.Add("Summary not available (data_available=false). Worker rebuild may not have run.")
    }
} catch {
    Write-Err "Summary verification failed: $_"
    $script:Failed.Add("summary endpoint")
}

# --- M.2: Portfolio holdings ---
Write-Info "Verifying: GET /v1/portfolios/$portfolioId/holdings"
try {
    $holdings = Invoke-ApiGet -Url "$BaseUrlApi/v1/portfolios/$portfolioId/holdings" -Headers $authHeaders
    if ($VerboseJson) { Write-Host ($holdings | ConvertTo-Json -Depth 10) -ForegroundColor DarkGray }

    Assert-True "holdings.data_available = true" ($holdings.data_available -eq $true) "got: $($holdings.data_available)"

    if ($holdings.data_available -and $holdings.holdings.Count -gt 0) {
        $h = $holdings.holdings[0]
        Assert-True "holdings[0].market_value_minor = 195230"   ($h.market_value_minor -eq 195230)  "got: $($h.market_value_minor)"
        Assert-True "holdings[0].quantity = 10.0000000000"       ($h.quantity -eq "10.0000000000")    "got: $($h.quantity)"
        Assert-True "holdings[0].is_estimated = false"           ($h.is_estimated -eq $false)        "got: $($h.is_estimated)"
        Assert-True "holdings count = 1"                         ($holdings.holdings.Count -eq 1)    "got: $($holdings.holdings.Count)"
    } else {
        $script:Warnings.Add("Holdings not available. Worker rebuild may not have run.")
    }
} catch {
    Write-Err "Holdings verification failed: $_"
    $script:Failed.Add("holdings endpoint")
}

# --- M.3: Daily snapshots ---
Write-Info "Verifying: GET /v1/portfolios/$portfolioId/snapshots/daily"
try {
    $snapshots = Invoke-ApiGet -Url "$BaseUrlApi/v1/portfolios/$portfolioId/snapshots/daily" -Headers $authHeaders
    if ($VerboseJson) { Write-Host ($snapshots | ConvertTo-Json -Depth 10) -ForegroundColor DarkGray }

    Assert-True "snapshots.data_available = true" ($snapshots.data_available -eq $true) "got: $($snapshots.data_available)"

    if ($snapshots.data_available -and $snapshots.snapshots.Count -gt 0) {
        $snapshotCount = $snapshots.snapshots.Count
        # At minimum, generate_daily_snapshots creates 1 snapshot for the snapshot date.
        # Backfill creates additional snapshots only for dates >= portfolio creation date.
        # A freshly created portfolio may only have 1 snapshot (today's date).
        Assert-True "snapshots count >= 1" ($snapshotCount -ge 1) "got: $snapshotCount"
        Write-Info "Snapshot count: $snapshotCount (backfill covers dates >= portfolio creation only)"
    } else {
        $script:Warnings.Add("Snapshots not available. Worker jobs may not have run.")
    }
} catch {
    Write-Err "Snapshots verification failed: $_"
    $script:Failed.Add("snapshots/daily endpoint")
}

# --- M.4: Snapshot holdings for the automatic-refresh snapshot date (today) ---
# The automatic refresh generates the CURRENT daily snapshot, dated today (UTC),
# not the historical $SnapshotDate (which is covered by backfill).
$AutoSnapshotDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd")
Write-Info "Verifying: GET /v1/portfolios/$portfolioId/snapshots/daily/$AutoSnapshotDate/holdings"
try {
    $snapshotHoldings = Invoke-ApiGet -Url "$BaseUrlApi/v1/portfolios/$portfolioId/snapshots/daily/$AutoSnapshotDate/holdings" -Headers $authHeaders
    if ($VerboseJson) { Write-Host ($snapshotHoldings | ConvertTo-Json -Depth 10) -ForegroundColor DarkGray }

    Assert-True "snapshot_holdings.data_available = true" ($snapshotHoldings.data_available -eq $true) "got: $($snapshotHoldings.data_available)"

    if ($snapshotHoldings.data_available -and $snapshotHoldings.holdings.Count -gt 0) {
        Assert-True "snapshot holdings count >= 1" ($snapshotHoldings.holdings.Count -ge 1) "got: $($snapshotHoldings.holdings.Count)"
    }
} catch {
    Write-Err "Snapshot holdings verification failed: $_"
    $script:Failed.Add("snapshots/daily/$AutoSnapshotDate/holdings endpoint")
}

# --- M.5: Operations list ---
Write-Info "Verifying: GET /v1/portfolios/$portfolioId/operations"
try {
    $operations = Invoke-ApiGet -Url "$BaseUrlApi/v1/portfolios/$portfolioId/operations" -Headers $authHeaders
    if ($VerboseJson) { Write-Host ($operations | ConvertTo-Json -Depth 10) -ForegroundColor DarkGray }

    $opCount = $operations.operations.Count
    Assert-True "operations count >= 2" ($opCount -ge 2) "got: $opCount"

    $postedOps = $operations.operations | Where-Object { $_.operation_status -eq "posted" }
    Assert-True "posted operations count >= 2" ($postedOps.Count -ge 2) "got: $($postedOps.Count)"
} catch {
    Write-Err "Operations verification failed: $_"
    $script:Failed.Add("operations endpoint")
}

# ---------------------------------------------------------------------------
# Final summary
# ---------------------------------------------------------------------------
Write-Step "SUMMARY"

Write-Host ""
Write-Host "  Demo identifiers:" -ForegroundColor White
Write-Host "    username:          $($script:DemoState.Username)"
Write-Host "    user_id:           $($script:DemoState.UserId)"
Write-Host "    portfolio_id:      $($script:DemoState.PortfolioId)"
Write-Host "    asset_id:          $($script:DemoState.AssetId)"
Write-Host "    deposit_op_id:     $($script:DemoState.DepositOpId)"
Write-Host "    buy_op_id:         $($script:DemoState.BuyOpId)"
Write-Host ""

if ($script:Passed.Count -gt 0) {
    Write-Host "  Assertions passed: $($script:Passed.Count)" -ForegroundColor Green
    foreach ($p in $script:Passed) {
        Write-Host "    [PASS] $p" -ForegroundColor Green
    }
}

if ($script:Warnings.Count -gt 0) {
    Write-Host ""
    Write-Host "  Warnings: $($script:Warnings.Count)" -ForegroundColor Yellow
    foreach ($w in $script:Warnings) {
        Write-Host "    [WARN] $w" -ForegroundColor Yellow
    }
}

if ($script:Failed.Count -gt 0) {
    Write-Host ""
    Write-Host "  Assertions failed: $($script:Failed.Count)" -ForegroundColor Red
    foreach ($f in $script:Failed) {
        Write-Host "    [FAIL] $f" -ForegroundColor Red
    }
    Write-Host ""
    Write-Err "RESULT: FAIL ($($script:Failed.Count) assertion(s) failed)"
    exit 1
} else {
    Write-Host ""
    Write-Success "RESULT: PASS ($($script:Passed.Count) assertion(s) passed)"
    exit 0
}
