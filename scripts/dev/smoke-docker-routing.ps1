<#
.SYNOPSIS
    Smoke-test the Docker nginx routing, CORS preflights and the full auth
    handoff chain for Kushim.

.DESCRIPTION
    Validates, against a running Docker stack, that:
      - each canonical nginx host returns the expected HTTP status;
      - CORS preflights succeed with the exact allowed origin (no wildcard);
      - the signup -> handoff create -> handoff exchange -> /v1/me chain works
        across the auth-api and api origins.

    The `*.kushim.localhost` names are not resolved by the Windows resolver, so
    requests are sent to 127.0.0.1 with an explicit Host header (this is exactly
    how nginx virtual-host routing is selected). Browsers resolve
    `*.localhost` to 127.0.0.1 automatically, so the same routing applies there.

    Exits non-zero on the first failure. No secrets are printed.
#>

$ErrorActionPreference = 'Stop'
$base = 'http://127.0.0.1'
$failures = New-Object System.Collections.Generic.List[string]

function Invoke-Curl {
    param([string[]]$CurlArgs)
    $out = & curl.exe @CurlArgs 2>$null
    return @{ Body = ($out -join "`n"); Exit = $LASTEXITCODE }
}

function Check-Status {
    param([string]$Name, [string]$Host_, [string]$Path, [int]$Expected = 200)
    $code = (& curl.exe -s -o NUL -w "%{http_code}" -H "Host: $Host_" "$base$Path") 2>$null
    if ("$code" -eq "$Expected") {
        Write-Host ("[OK]   {0,-42} {1}" -f "$Host_$Path", $code) -ForegroundColor Green
    } else {
        Write-Host ("[FAIL] {0,-42} {1} (expected {2})" -f "$Host_$Path", $code, $Expected) -ForegroundColor Red
        $failures.Add("$Host_$Path -> $code")
    }
}

function Check-Preflight {
    param([string]$Name, [string]$Host_, [string]$Path, [string]$Origin, [string]$Method)
    $headers = & curl.exe -s -o NUL -D - -X OPTIONS `
        -H "Host: $Host_" -H "Origin: $Origin" `
        -H "Access-Control-Request-Method: $Method" `
        -H "Access-Control-Request-Headers: authorization,content-type" `
        "$base$Path" 2>$null
    if (($headers -join "`n") -match "(?im)^access-control-allow-origin:\s*$([regex]::Escape($Origin))\s*$") {
        Write-Host ("[OK]   preflight {0,-32} <- {1}" -f $Name, $Origin) -ForegroundColor Green
    } else {
        Write-Host ("[FAIL] preflight {0,-32} <- {1}" -f $Name, $Origin) -ForegroundColor Red
        $failures.Add("preflight $Name <- $Origin")
    }
}

Write-Host "== HTTP routes =="
Check-Status "website"  "kushim.localhost"          "/"
Check-Status "auth ui"  "auth.kushim.localhost"     "/connexion"
Check-Status "auth ui"  "auth.kushim.localhost"     "/inscription"
Check-Status "app"      "app.kushim.localhost"      "/"
Check-Status "api"      "api.kushim.localhost"      "/health"
Check-Status "auth-api" "auth-api.kushim.localhost" "/health"

Write-Host "`n== CORS preflights =="
Check-Preflight "auth-api /auth/handoff"          "auth-api.kushim.localhost" "/auth/handoff"          "http://auth.kushim.localhost" "POST"
Check-Preflight "auth-api /auth/handoff/exchange" "auth-api.kushim.localhost" "/auth/handoff/exchange" "http://app.kushim.localhost"  "POST"
Check-Preflight "api /v1/me"                      "api.kushim.localhost"      "/v1/me"                 "http://app.kushim.localhost"  "GET"

Write-Host "`n== Auth handoff chain =="
# Windows PowerShell strips inner quotes when passing inline JSON to native
# curl.exe, so each JSON body is written to a temp file and sent with `-d @file`.
$tmp = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "kushim_smoke_$([guid]::NewGuid().ToString('N'))")
function Post-Json {
    param([string]$Host_, [string]$Origin, [string]$Path, [hashtable]$Body, [string]$Bearer)
    $file = Join-Path $tmp ([guid]::NewGuid().ToString('N') + '.json')
    ($Body | ConvertTo-Json -Compress) | Set-Content -Path $file -Encoding ascii -NoNewline
    $args = @('-s', '-H', "Host: $Host_", '-H', "Origin: $Origin", '-H', 'Content-Type: application/json')
    if ($Bearer) { $args += @('-H', "Authorization: Bearer $Bearer") }
    $args += @('-X', 'POST', "$base$Path", '-d', "@$file")
    $out = & curl.exe @args 2>$null
    return ($out -join "`n")
}

try {
    $user = "smoke_$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())"
    $pwd = "SmokeP0p1!Pass$(Get-Random -Minimum 1000 -Maximum 9999)"
    $signup = Post-Json "auth-api.kushim.localhost" "http://auth.kushim.localhost" "/auth/signup" @{ username = $user; password = $pwd }
    $signupObj = $signup | ConvertFrom-Json
    if (-not $signupObj.access_token) { Write-Host "[FAIL] signup" -ForegroundColor Red; $failures.Add("signup"); }
    else { Write-Host "[OK]   signup (tokens received)" -ForegroundColor Green }

    if ($signupObj.access_token) {
        $ho = Post-Json "auth-api.kushim.localhost" "http://auth.kushim.localhost" "/auth/handoff" @{ refresh_token = $signupObj.refresh_token } $signupObj.access_token
        $code = ($ho | ConvertFrom-Json).handoff_code
        if (-not $code) { Write-Host "[FAIL] handoff create" -ForegroundColor Red; $failures.Add("handoff create") }
        else { Write-Host "[OK]   handoff create (code received)" -ForegroundColor Green }

        if ($code) {
            $ex = Post-Json "auth-api.kushim.localhost" "http://app.kushim.localhost" "/auth/handoff/exchange" @{ handoff_code = $code }
            $access2 = ($ex | ConvertFrom-Json).access_token
            if (-not $access2) { Write-Host "[FAIL] handoff exchange" -ForegroundColor Red; $failures.Add("handoff exchange") }
            else { Write-Host "[OK]   handoff exchange (tokens received)" -ForegroundColor Green }

            if ($access2) {
                $me = & curl.exe -s -o NUL -w "%{http_code}" -H "Host: api.kushim.localhost" `
                    -H "Origin: http://app.kushim.localhost" -H "Authorization: Bearer $access2" "$base/v1/me" 2>$null
                if ("$me" -eq "200") { Write-Host "[OK]   GET /v1/me -> 200" -ForegroundColor Green }
                else { Write-Host "[FAIL] GET /v1/me -> $me" -ForegroundColor Red; $failures.Add("/v1/me -> $me") }
            }
        }
    }
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
if ($failures.Count -gt 0) {
    Write-Host "SMOKE FAILED ($($failures.Count)):" -ForegroundColor Red
    $failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host "SMOKE PASS - Docker routing, CORS and auth handoff all healthy." -ForegroundColor Green
exit 0
