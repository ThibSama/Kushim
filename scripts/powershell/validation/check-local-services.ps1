param(
    [switch]$Start
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Set-Location $root

function Test-CommandAvailable {
    param([string]$Name)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name is not available on PATH."
    }
}

function Test-HealthEndpoint {
    param(
        [string]$Name,
        [string]$Url
    )

    try {
        $response = Invoke-RestMethod -Uri $Url -Method GET -TimeoutSec 5
    } catch {
        throw "$Name is not reachable at ${Url}: $($_.Exception.Message)"
    }

    if ($response.status -ne "ok") {
        throw "$Name returned unexpected health payload: $($response | ConvertTo-Json -Compress)"
    }

    Write-Host "[ok] $Name $Url"
}

Test-CommandAvailable "docker"
$null = docker compose version

if ($Start) {
    docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data
}

Test-HealthEndpoint "kushim-auth-api" "http://127.0.0.1:3002/health"
Test-HealthEndpoint "kushim-api" "http://127.0.0.1:8080/health"
Test-HealthEndpoint "kushim-worker" "http://127.0.0.1:8081/health"
Test-HealthEndpoint "kushim-market-data" "http://127.0.0.1:8082/health"

Write-Host "Local MVP backend prerequisites are healthy."
