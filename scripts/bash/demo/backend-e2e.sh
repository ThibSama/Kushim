#!/usr/bin/env bash
set -Eeuo pipefail

# =============================================================================
# backend-e2e.sh — Bash equivalent of backend-e2e.ps1
#
# Kushim Backend MVP E2E Demo Smoke Test.
#
# Executes the full backend-only MVP scenario automatically:
#   signup → access-token verification → portfolio creation → canonical AAPL
#   resolution → mock market-data preparation → deposit + buy (create/post) →
#   automatic refresh-request polling → historical snapshot backfill →
#   API verification (18 assertions) → final PASS/FAIL report.
#
# This script is for local development and demo validation only.
# It does not modify application code, DDL, or Docker configuration.
# It never deletes, truncates, resets, or cleans application data.
#
# See: documentation/operations/backend-demo-e2e.md
# =============================================================================

# --- Resolve repository root from script location (three levels up) ---------
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

# =============================================================================
# CLI parameters with defaults (parity with PowerShell)
# =============================================================================

base_url_auth="http://localhost:3002"
base_url_api="http://localhost:8080"
demo_prefix="demo_e2e"
deposit_date="2026-06-01T10:00:00Z"
buy_date="2026-06-02T14:00:00Z"
snapshot_date="2026-06-09"
history_date_from="2026-06-01"
history_date_to="2026-06-09"
backfill_date_to=""
demo_password=""
skip_docker_jobs=false
verbose_json=false
dry_run=false

# --- Usage ------------------------------------------------------------------

usage() {
  cat <<'EOF'
Usage: backend-e2e.sh [OPTIONS]

Backend MVP E2E smoke test (Bash port of backend-e2e.ps1).

Options:
  --base-url-auth VALUE      Base URL for kushim-auth-api (default: http://localhost:3002)
  --base-url-api VALUE       Base URL for kushim-api (default: http://localhost:8080)
  --demo-prefix VALUE        Prefix for demo user/portfolio names (default: demo_e2e)
  --deposit-date VALUE       RFC3339 execution date for deposit (default: 2026-06-01T10:00:00Z)
  --buy-date VALUE           RFC3339 execution date for buy (default: 2026-06-02T14:00:00Z)
  --snapshot-date VALUE      YYYY-MM-DD snapshot date (default: 2026-06-09)
  --history-date-from VALUE  YYYY-MM-DD history start (default: 2026-06-01)
  --history-date-to VALUE    YYYY-MM-DD history end (default: 2026-06-09)
  --backfill-date-to VALUE  YYYY-MM-DD backfill end (default: snapshot_date - 1 day)
  --demo-password VALUE     Explicit demo password (never printed; default: generated)
  --skip-docker-jobs        Skip Docker Compose job steps
  --verbose-json            Print full JSON responses for verification endpoints
  --dry-run                 Health-check only; create no demo data
  --help                     Show this help and exit
EOF
}

# --- Argument parsing -------------------------------------------------------

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base-url-auth)       shift; [[ $# -eq 0 ]] && { echo "Error: --base-url-auth requires a value." >&2; exit 1; }; base_url_auth="$1";;
    --base-url-api)        shift; [[ $# -eq 0 ]] && { echo "Error: --base-url-api requires a value." >&2; exit 1; }; base_url_api="$1";;
    --demo-prefix)         shift; [[ $# -eq 0 ]] && { echo "Error: --demo-prefix requires a value." >&2; exit 1; }; demo_prefix="$1";;
    --deposit-date)        shift; [[ $# -eq 0 ]] && { echo "Error: --deposit-date requires a value." >&2; exit 1; }; deposit_date="$1";;
    --buy-date)             shift; [[ $# -eq 0 ]] && { echo "Error: --buy-date requires a value." >&2; exit 1; }; buy_date="$1";;
    --snapshot-date)       shift; [[ $# -eq 0 ]] && { echo "Error: --snapshot-date requires a value." >&2; exit 1; }; snapshot_date="$1";;
    --history-date-from)   shift; [[ $# -eq 0 ]] && { echo "Error: --history-date-from requires a value." >&2; exit 1; }; history_date_from="$1";;
    --history-date-to)     shift; [[ $# -eq 0 ]] && { echo "Error: --history-date-to requires a value." >&2; exit 1; }; history_date_to="$1";;
    --backfill-date-to)    shift; [[ $# -eq 0 ]] && { echo "Error: --backfill-date-to requires a value." >&2; exit 1; }; backfill_date_to="$1";;
    --demo-password)       shift; [[ $# -eq 0 ]] && { echo "Error: --demo-password requires a value." >&2; exit 1; }; demo_password="$1";;
    --skip-docker-jobs)    skip_docker_jobs=true;;
    --verbose-json)       verbose_json=true;;
    --dry-run)            dry_run=true;;
    --help)               usage; exit 0;;
    --*=*)                echo "Error: unknown option '$1'. Use '--option VALUE' form." >&2; exit 1;;
    *)                    echo "Error: unknown argument '$1'" >&2; usage >&2; exit 1;;
  esac
  shift
done

# =============================================================================
# Logging helpers
# =============================================================================

log_info()    { printf '[INFO]    %s\n' "$*" >&2; }
log_success() { printf '[OK]      %s\n' "$*" >&2; }
log_warn()    { printf '[WARN]    %s\n' "$*" >&2; }
log_error()   { printf '[ERROR]   %s\n' "$*" >&2; }
log_step()    { printf '\n========= %s =========\n' "$*" >&2; }

# =============================================================================
# Assertion framework
# =============================================================================

passed_list=()
failed_list=()
warnings=()

assert_true() {
  local name="$1"
  local condition="$2"
  local fail_message="${3:-}"
  if [[ "$condition" == "true" ]]; then
    passed_list+=("$name")
    log_success "PASS: $name"
  else
    failed_list+=("$name")
    local msg="FAIL: $name"
    [[ -n "$fail_message" ]] && msg="$msg -- $fail_message"
    log_error "$msg"
  fi
}

# =============================================================================
# Required command verification
# =============================================================================

for cmd in curl jq docker openssl date; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    log_error "Required command '$cmd' is not on PATH."
    exit 1
  fi
done

# =============================================================================
# State
# =============================================================================

state_username=""
state_user_id=""
state_portfolio_id=""
state_asset_id=""
state_deposit_op_id=""
state_buy_op_id=""
state_access_token=""
state_refresh_request_id=""

# --- UUID generation --------------------------------------------------------
# Try uuidgen, then /proc/sys/kernel/random/uuid, then openssl.

generate_uuid() {
  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen
  elif [[ -r /proc/sys/kernel/random/uuid ]]; then
    cat /proc/sys/kernel/random/uuid
  else
    openssl rand -hex 16 | sed 's/\(.\{8\}\)\(.\{4\}\)\(.\{4\}\)\(.\{4\}\)/\1-\2-\3-\4-/'
  fi
}

# --- Password generation ---------------------------------------------------
# 32-char hex password from OpenSSL. Never printed, never written to disk.

generate_password() {
  openssl rand -hex 16
}

# Generate idempotency keys (one per logical write, stable for this invocation).
deposit_idempotency_key="$(generate_uuid)"
buy_idempotency_key="$(generate_uuid)"

# Validate UUID shape.
validate_uuid() {
  local value="$1"
  [[ "$value" =~ ^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$ ]]
}

validate_uuid "$deposit_idempotency_key" || { log_error "Generated deposit idempotency key is not a valid UUID." >&2; exit 1; }
validate_uuid "$buy_idempotency_key" || { log_error "Generated buy idempotency key is not a valid UUID." >&2; exit 1; }

# --- Backfill date computation (parity with PowerShell) ---------------------
# Default: one calendar day before snapshot_date. Fallback: history_date_to.

if [[ -z "$backfill_date_to" ]]; then
  backfill_date_to="$(date -d "$snapshot_date - 1 day" +%Y-%m-%d 2>/dev/null || true)"
  if [[ -z "$backfill_date_to" ]]; then
    backfill_date_to="$history_date_to"
  fi
fi

# --- Run suffix -------------------------------------------------------------

run_suffix="$(date -u +%Y%m%d_%H%M%S)"

# =============================================================================
# HTTP helpers (curl + jq)
# =============================================================================
# Captures HTTP status separately from body. Non-2xx -> failure.
#
# Two modes:
#   - File-based (http_get, http_post, http_post_no_body): response body is
#     written to a temp file, cleaned up by trap. Used for non-sensitive
#     responses (portfolio data, operations, snapshots, health).
#   - In-memory (http_post_inmem): response body is captured in a Bash variable
#     via command substitution — NEVER written to any file, temp file, /tmp,
#     /dev/shm, named pipe, or other filesystem location. Used exclusively for
#     token-bearing responses (signup, login).
#
# Global flag http_body_is_file tracks which mode was last used:
#   true  → http_body is a file path
#   false → http_body is an in-memory string

tmp_files=()
http_body_is_file=true
cleanup_tmp() {
  for f in "${tmp_files[@]}"; do
    rm -f "$f" 2>/dev/null || true
  done
}
trap cleanup_tmp EXIT ERR

# GET with optional headers (array passed by name).
# Sets global: http_body (file path), http_status (integer), http_body_is_file (true).
http_get() {
  http_body_is_file=true
  local url="$1"
  shift
  local -a header_args=()
  while [[ $# -gt 0 ]]; do
    header_args+=(-H "$1")
    shift
  done

  local body_file
  body_file="$(mktemp)"
  tmp_files+=("$body_file")

  local status
  status=$(curl -sS -o "$body_file" -w '%{http_code}' --connect-timeout 5 --max-time 30 \
    -H 'Accept: application/json' \
    "${header_args[@]}" \
    "$url" 2>/dev/null) || {
    http_body="$body_file"
    http_status="0"
    return 1
  }

  http_body="$body_file"
  http_status="$status"

  if [[ "$http_status" -lt 200 || "$http_status" -ge 300 ]]; then
    return 1
  fi
  return 0
}

# POST with JSON body. $2 = JSON string.
# Sets global: http_body (file path), http_status (integer), http_body_is_file (true).
http_post() {
  http_body_is_file=true
  local url="$1"
  local body="$2"
  shift 2
  local -a header_args=()
  while [[ $# -gt 0 ]]; do
    header_args+=(-H "$1")
    shift
  done

  local body_file
  body_file="$(mktemp)"
  tmp_files+=("$body_file")

  local status
  status=$(curl -sS -o "$body_file" -w '%{http_code}' --connect-timeout 5 --max-time 30 \
    -X POST \
    -H 'Content-Type: application/json; charset=utf-8' \
    -H 'Accept: application/json' \
    "${header_args[@]}" \
    -d "$body" \
    "$url" 2>/dev/null) || {
    http_body="$body_file"
    http_status="0"
    return 1
  }

  http_body="$body_file"
  http_status="$status"

  if [[ "$http_status" -lt 200 || "$http_status" -ge 300 ]]; then
    return 1
  fi
  return 0
}

# POST without a body.
# Sets global: http_body (file path), http_status (integer), http_body_is_file (true).
http_post_no_body() {
  http_body_is_file=true
  local url="$1"
  shift
  local -a header_args=()
  while [[ $# -gt 0 ]]; do
    header_args+=(-H "$1")
    shift
  done

  local body_file
  body_file="$(mktemp)"
  tmp_files+=("$body_file")

  local status
  status=$(curl -sS -o "$body_file" -w '%{http_code}' --connect-timeout 5 --max-time 30 \
    -X POST \
    -H 'Content-Type: application/json; charset=utf-8' \
    -H 'Accept: application/json' \
    "${header_args[@]}" \
    "$url" 2>/dev/null) || {
    http_body="$body_file"
    http_status="0"
    return 1
  }

  http_body="$body_file"
  http_status="$status"

  if [[ "$http_status" -lt 200 || "$http_status" -ge 300 ]]; then
    return 1
  fi
  return 0
}

# POST with JSON body — IN-MEMORY ONLY (no filesystem persistence).
#
# Captures the HTTP response body in a Bash variable via command substitution.
# The HTTP status code is appended by curl's -w as a final line and then split
# off. No temp file, /tmp path, /dev/shm, named pipe, or any other filesystem
# location is used for the response body. This is mandatory for token-bearing
# responses (signup, login) which contain access and refresh tokens.
#
# Sets global: http_body (string), http_status (integer), http_body_is_file (false).
http_post_inmem() {
  http_body_is_file=false
  local url="$1"
  local body="$2"
  shift 2
  local -a header_args=()
  while [[ $# -gt 0 ]]; do
    header_args+=(-H "$1")
    shift
  done

  local full_response
  full_response="$(curl -sS -w '\n%{http_code}' --connect-timeout 5 --max-time 30 \
    -X POST \
    -H 'Content-Type: application/json; charset=utf-8' \
    -H 'Accept: application/json' \
    "${header_args[@]}" \
    -d "$body" \
    "$url" 2>/dev/null)" || {
    http_body=""
    http_status="0"
    return 1
  }

  # Split: last line is the status code, everything before is the body.
  http_status="${full_response##*$'\n'}"
  http_body="${full_response%$'\n'*}"

  if [[ "$http_status" -lt 200 || "$http_status" -ge 300 ]]; then
    return 1
  fi
  return 0
}

# Helper: extract a field from the last response body using jq.
# Works with both file-based (http_body is a path) and in-memory (http_body is a string) modes.
jq_field() {
  if $http_body_is_file; then
    jq -r "$1" "$http_body" 2>/dev/null
  else
    jq -r "$1" <<< "$http_body" 2>/dev/null
  fi
}

# Helper: check a jq boolean condition against the last response body.
# Works with both file-based and in-memory modes.
jq_check() {
  if $http_body_is_file; then
    jq -e "$1" "$http_body" >/dev/null 2>&1
  else
    jq -e "$1" <<< "$http_body" >/dev/null 2>&1
  fi
}

# Helper: pretty-print the last response body for --verbose-json (to stderr).
# Works with both file-based and in-memory modes.
jq_pretty() {
  if $http_body_is_file; then
    jq . "$http_body" >&2 || true
  else
    jq . <<< "$http_body" >&2 || true
  fi
}

# Auth headers: Authorization: Bearer *** Prints one header per line.
# Never logs the token value — the header line is consumed by curl only.
auth_headers() {
  printf 'Authorization: Bearer %s\n' "$state_access_token"
}

# Write headers: Authorization + Idempotency-Key (for operation creation only).
# Prints one header per line. Validates the UUID shape locally.
write_headers() {
  local idempotency_key="$1"
  local logical_op="${2:-operation-write}"
  if [[ -z "$idempotency_key" ]]; then
    log_error "Idempotency-Key is empty (logical op: $logical_op)"
    exit 1
  fi
  if ! validate_uuid "$idempotency_key"; then
    log_error "Idempotency-Key '$idempotency_key' is not a valid UUID"
    exit 1
  fi
  log_info "Using Idempotency-Key for ${logical_op}: $idempotency_key"
  printf 'Authorization: Bearer %s\n' "$state_access_token"
  printf 'Idempotency-Key: %s\n' "$idempotency_key"
}

# =============================================================================
# Docker job helper
# =============================================================================

# Runs a docker compose run --rm command with -e env vars.
# $1 = service name, $2 = job description, remaining = ENV=value strings.
invoke_docker_job() {
  local service_name="$1"
  local job_description="$2"
  shift 2

  local -a docker_args=(compose run --rm)
  for env_arg in "$@"; do
    docker_args+=(-e "$env_arg")
  done
  docker_args+=("$service_name")

  log_info "Running: $job_description"
  log_info "Command: docker ${docker_args[*]}"

  local stdout_file stderr_file
  stdout_file="$(mktemp)"
  stderr_file="$(mktemp)"
  tmp_files+=("$stdout_file" "$stderr_file")

  local exit_code
  docker "${docker_args[@]}" >"$stdout_file" 2>"$stderr_file" || exit_code=$?
  exit_code=${exit_code:-0}

  if $verbose_json; then
    if [[ -s "$stdout_file" ]]; then
      while IFS= read -r line; do
        printf '  %s\n' "$line" >&2
      done < "$stdout_file"
    fi
    if [[ -s "$stderr_file" ]]; then
      while IFS= read -r line; do
        [[ -n "$(echo "$line" | tr -d '[:space:]')" ]] && printf '  [stderr] %s\n' "$line" >&2
      done < "$stderr_file"
    fi
  fi

  if [[ "$exit_code" -ne 0 ]]; then
    log_error "Docker job failed with exit code $exit_code"
    if [[ -s "$stdout_file" ]]; then
      while IFS= read -r line; do
        printf '  %s\n' "$line" >&2
      done < "$stdout_file"
    fi
    if [[ -s "$stderr_file" ]]; then
      while IFS= read -r line; do
        [[ -n "$(echo "$line" | tr -d '[:space:]')" ]] && printf '  [stderr] %s\n' "$line" >&2
      done < "$stderr_file"
    fi
    log_error "$job_description failed (exit code $exit_code)"
    exit 1
  fi

  log_success "$job_description completed"
}

# =============================================================================
# Step A: Verify infrastructure
# =============================================================================

log_step "A. Verify infrastructure"

check_service_health() {
  local name="$1"
  local url="$2"
  local body_file
  body_file="$(mktemp)"
  tmp_files+=("$body_file")

  local status
  status=$(curl -sS -o "$body_file" -w '%{http_code}' --connect-timeout 5 --max-time 5 "$url/health" 2>/dev/null) || {
    log_error "$name is not reachable at $url/health (curl failed)"
    return 1
  }
  if [[ "$status" -lt 200 || "$status" -ge 300 ]]; then
    log_error "$name is not reachable at $url/health (HTTP $status)"
    return 1
  fi
  if ! jq -e '.status == "ok"' "$body_file" >/dev/null 2>&1; then
    log_error "$name health check returned unexpected status"
    return 1
  fi
  log_success "$name is healthy"
  return 0
}

health_ok=true
check_service_health "kushim-auth-api" "$base_url_auth" || health_ok=false
check_service_health "kushim-api" "$base_url_api" || health_ok=false
check_service_health "kushim-worker" "http://localhost:8081" || health_ok=false
check_service_health "kushim-market-data" "http://localhost:8082" || health_ok=false

if ! $health_ok; then
  log_error "One or more services are not healthy. Cannot proceed."
  log_error "Start services with: docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data"
  exit 1
fi

if ! $skip_docker_jobs; then
  if ! docker compose version >/dev/null 2>&1; then
    log_error "docker compose is not available. Use --skip-docker-jobs to skip job steps."
    exit 1
  fi
  log_info "Docker Compose: $(docker compose version 2>&1)"
fi

if $dry_run; then
  log_info "DryRun mode: all services are healthy. Exiting without executing demo steps."
  exit 0
fi

# =============================================================================
# Step B: Signup demo user
# =============================================================================

log_step "B. Signup demo user"

# Username contract: ^[a-z0-9_][a-z0-9_-]{2,39}$ (3–40 chars total)
auth_username_max=40
auth_username_regex='^[a-z0-9_][a-z0-9_-]{2,39}$'

username="${demo_prefix}_${run_suffix}"

suffix_tail_length=$(( ${#run_suffix} + 1 ))
max_prefix_length=$(( auth_username_max - suffix_tail_length ))

if [[ -z "${demo_prefix// /}" ]]; then
  log_error "DemoPrefix must not be empty or whitespace."
  log_error "Generated username: $username"
  log_error "Maximum username length: $auth_username_max"
  log_error "Maximum DemoPrefix length for this run: $max_prefix_length"
  log_error "Allowed DemoPrefix characters: lowercase a-z, digits, underscore and hyphen."
  exit 1
fi

if [[ ${#username} -gt $auth_username_max ]]; then
  log_error "Generated username is ${#username} characters; the auth API allows at most $auth_username_max."
  log_error "Generated username: $username"
  log_error "Maximum username length: $auth_username_max"
  log_error "Maximum DemoPrefix length for this run: $max_prefix_length"
  log_error "Allowed DemoPrefix characters: lowercase a-z, digits, underscore and hyphen."
  exit 1
fi

if [[ ! "$username" =~ $auth_username_regex ]]; then
  log_error "Generated username does not match the auth username contract $auth_username_regex."
  log_error "Generated username: $username"
  exit 1
fi

# Password: 32 chars from OpenSSL hex, or explicit override.
auth_password_min=12
auth_password_max=128

password_is_override=false
if [[ -n "$demo_password" ]]; then
  password_is_override=true
  if [[ -z "${demo_password// /}" ]]; then
    log_error "DemoPassword override must not be blank."
    exit 1
  fi
  if [[ ${#demo_password} -lt $auth_password_min ]]; then
    log_error "DemoPassword override is too short: minimum $auth_password_min characters."
    exit 1
  fi
  if [[ ${#demo_password} -gt $auth_password_max ]]; then
    log_error "DemoPassword override is too long: maximum $auth_password_max characters."
    exit 1
  fi
  password="$demo_password"
else
  password="$(generate_password)"
fi

log_info "username: $username"
if $password_is_override; then
  log_info "password: explicit override provided (value redacted)"
else
  log_info "password: generated in memory (length=${#password})"
fi

signup_body="$(jq -nc --arg u "$username" --arg p "$password" '{username:$u, password:$p}')"

if ! http_post_inmem "$base_url_auth/auth/signup" "$signup_body"; then
  log_error "Signup failed: POST $base_url_auth/auth/signup returned HTTP $http_status"
  log_error "Likely causes (not exhaustive): duplicate username; missing auth reference data (the 'user' role seed); username/password rejected by the auth policy; or an auth API internal error."
  log_error "If the 'user' role is missing on a fresh database, apply infra/postgres/init/003_seed_auth_roles.sql (loaded automatically on fresh volumes)."
  exit 1
fi

state_access_token="$(jq_field '.access_token // empty')"
state_user_id="$(jq_field '.user.id_user // empty')"

if [[ -z "$state_access_token" || -z "$state_user_id" ]]; then
  log_error "Signup response missing required fields (.access_token or .user.id_user)."
  exit 1
fi

state_username="$username"
log_success "User created: id=$state_user_id"

# =============================================================================
# Step C: Verify access token
# =============================================================================

log_step "C. Verify access token"

auth_hdr="$(auth_headers)"
if ! http_get "$base_url_api/v1/me" "$auth_hdr"; then
  log_error "Token verification failed: GET /v1/me returned HTTP $http_status"
  exit 1
fi

me_response_id_user="$(jq_field '.id_user // empty')"
log_success "Token verified via /v1/me (user=$me_response_id_user)"

# =============================================================================
# Step D: Create USD portfolio
# =============================================================================

log_step "D. Create USD portfolio"

portfolio_name="E2E Demo Portfolio $run_suffix"
log_info "Portfolio name: $portfolio_name"

portfolio_body="$(jq -nc --arg n "$portfolio_name" '{name:$n, base_currency:"USD"}')"

auth_hdr="$(auth_headers)"
if ! http_post "$base_url_api/v1/portfolios" "$portfolio_body" "$auth_hdr"; then
  log_error "Portfolio creation failed: HTTP $http_status"
  exit 1
fi

state_portfolio_id="$(jq_field '.portfolio.id_portfolio // empty')"
if [[ -z "$state_portfolio_id" ]]; then
  log_error "Portfolio creation response missing .portfolio.id_portfolio."
  exit 1
fi
log_success "Portfolio created: id=$state_portfolio_id"

# =============================================================================
# Step E: Resolve canonical AAPL asset
# =============================================================================
# This script never creates catalogue assets. It expects the canonical
# (AAPL, NASDAQ, USD, active) row to already exist, seeded by
# infra/postgres/init/002_seed_canonical_assets.sql.
# =============================================================================

log_step "E. Resolve canonical AAPL asset"

resolve_sql="SELECT id_asset FROM assets WHERE ticker = 'AAPL' AND symbol = 'AAPL' AND exchange = 'NASDAQ' AND native_currency = 'USD' AND status = 'active'"

resolve_result_file="$(mktemp)"
tmp_files+=("$resolve_result_file")

if ! docker exec kushim_database psql -U kushim -d kushim -t -A -c "$resolve_sql" > "$resolve_result_file" 2>&1; then
  log_error "Canonical AAPL resolution failed: psql exited non-zero."
  cat "$resolve_result_file" >&2
  exit 1
fi

# Filter for valid UUID lines only (matches PowerShell's simpler pattern).
mapfile -t resolve_rows < <(grep -E '^[0-9a-f-]{36}$' "$resolve_result_file" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')

if [[ ${#resolve_rows[@]} -eq 0 ]]; then
  log_error "Canonical (AAPL, NASDAQ, USD, active) row is missing from the assets catalogue."
  exit 1
fi

if [[ ${#resolve_rows[@]} -gt 1 ]]; then
  log_error "Catalogue is ambiguous: ${#resolve_rows[@]} exact canonical AAPL rows match."
  exit 1
fi

state_asset_id="${resolve_rows[0]}"
log_success "Canonical AAPL resolved: id=$state_asset_id (reused, not created)"

# =============================================================================
# Step E2: Prepare market data BEFORE posting operations
# =============================================================================
# Current market data must exist before the automatic refresh runs so the
# worker prices holdings deterministically (no race). This is legitimate
# market-data preparation. It is NOT the manual portfolio rebuild/snapshot.
# =============================================================================

if ! $skip_docker_jobs; then
  log_step "E2. Market-data: refresh current + fill history (mock provider)"

  invoke_docker_job "kushim-market-data" "refresh_current_market_data" \
    "MARKET_DATA_MODE=once" \
    "MARKET_DATA_JOB=refresh_current_market_data" \
    "MARKET_DATA_PROVIDER=mock"

  invoke_docker_job "kushim-market-data" "fill_missing_price_history_cache" \
    "MARKET_DATA_MODE=once" \
    "MARKET_DATA_JOB=fill_missing_price_history_cache" \
    "MARKET_DATA_PROVIDER=mock" \
    "MARKET_DATA_HISTORY_DATE_FROM=$history_date_from" \
    "MARKET_DATA_HISTORY_DATE_TO=$history_date_to"
else
  log_step "E2. Market-data preparation SKIPPED (--skip-docker-jobs)"
  warnings+=("Market-data preparation skipped; holdings may be estimated.")
fi

# =============================================================================
# Step F: Create and post deposit
# =============================================================================

log_step "F. Create and post deposit (10,000.00 USD)"

deposit_body="$(jq -nc \
  --arg d "$deposit_date" \
  '{operation_type:"deposit", executed_at:$d, gross_amount_minor:1000000, cash_amount_minor:1000000, currency:"USD", metadata:{}}')"

write_hdrs="$(write_headers "$deposit_idempotency_key" "deposit-create")"
mapfile -t write_hdr_arr <<< "$write_hdrs"

if ! http_post "$base_url_api/v1/portfolios/$state_portfolio_id/operations" "$deposit_body" "${write_hdr_arr[@]}"; then
  log_error "Deposit creation failed: HTTP $http_status"
  exit 1
fi

state_deposit_op_id="$(jq_field '.operation.id_portfolio_operation // empty')"
if [[ -z "$state_deposit_op_id" ]]; then
  log_error "Deposit creation response missing .operation.id_portfolio_operation."
  exit 1
fi
deposit_status="$(jq_field '.operation.operation_status // empty')"
log_success "Deposit created: id=$state_deposit_op_id (status=$deposit_status)"

auth_hdr="$(auth_headers)"
if ! http_post_no_body "$base_url_api/v1/portfolios/$state_portfolio_id/operations/$state_deposit_op_id/post" "$auth_hdr"; then
  log_error "Deposit post failed: HTTP $http_status"
  exit 1
fi
post_deposit_status="$(jq_field '.operation.operation_status // empty')"
log_success "Deposit posted: status=$post_deposit_status"

# =============================================================================
# Step G: Create and post buy
# =============================================================================

log_step "G. Create and post buy (10 AAPL at 195.23 USD)"

buy_body="$(jq -nc \
  --arg a "$state_asset_id" \
  --arg d "$buy_date" \
  --arg q "10.0000000000" \
  '{id_asset:$a, operation_type:"buy", executed_at:$d, quantity:$q, price_minor:19523, gross_amount_minor:195230, cash_amount_minor:195230, currency:"USD", metadata:{}}')"

write_hdrs="$(write_headers "$buy_idempotency_key" "buy-create")"
mapfile -t write_hdr_arr <<< "$write_hdrs"

if ! http_post "$base_url_api/v1/portfolios/$state_portfolio_id/operations" "$buy_body" "${write_hdr_arr[@]}"; then
  log_error "Buy creation failed: HTTP $http_status"
  exit 1
fi

state_buy_op_id="$(jq_field '.operation.id_portfolio_operation // empty')"
if [[ -z "$state_buy_op_id" ]]; then
  log_error "Buy creation response missing .operation.id_portfolio_operation."
  exit 1
fi
buy_status="$(jq_field '.operation.operation_status // empty')"
log_success "Buy created: id=$state_buy_op_id (status=$buy_status)"

auth_hdr="$(auth_headers)"
if ! http_post_no_body "$base_url_api/v1/portfolios/$state_portfolio_id/operations/$state_buy_op_id/post" "$auth_hdr"; then
  log_error "Buy post failed: HTTP $http_status"
  exit 1
fi
post_buy_status="$(jq_field '.operation.operation_status // empty')"
log_success "Buy posted: status=$post_buy_status"

state_refresh_request_id="$(jq_field '.refresh_request.id_portfolio_refresh_request // empty')"
if [[ -z "$state_refresh_request_id" ]]; then
  log_error "Posting the buy did not return a refresh_request — automatic refresh contract violated."
  exit 1
fi
refresh_req_status="$(jq_field '.refresh_request.status // empty')"
log_success "Refresh request enqueued by API: id=$state_refresh_request_id (status=$refresh_req_status)"

# =============================================================================
# Step H: Automatic refresh — poll the durable refresh request until completed
# =============================================================================

if $skip_docker_jobs; then
  log_step "H. Automatic refresh polling SKIPPED (--skip-docker-jobs)"
  warnings+=("Automatic refresh polling skipped; read models/snapshots may not be available.")
else
  log_step "H. Automatic refresh: poll refresh request until completed"

  refresh_id="$state_refresh_request_id"
  deadline=$(( $(date +%s) + 90 ))
  last_status="unknown"
  refresh_completed=false

  while [[ $(date +%s) -lt $deadline ]]; do
    auth_hdr="$(auth_headers)"
    if http_get "$base_url_api/v1/portfolios/$state_portfolio_id/refresh-requests/$refresh_id" "$auth_hdr"; then
      last_status="$(jq_field '.refresh_request.status // empty')"
    else
      last_status="poll_error"
    fi

    log_info "refresh request status: $last_status"

    if [[ "$last_status" == "completed" ]]; then
      refresh_completed=true
      break
    fi
    if [[ "$last_status" == "failed" ]]; then
      error_code="$(jq_field '.refresh_request.error_code // empty')"
      log_error "Refresh request reached terminal 'failed' status (error_code=$error_code)."
      exit 1
    fi

    sleep 3
  done

  if ! $refresh_completed; then
    log_error "Refresh request did not complete within the timeout. Last observed status: $last_status"
    exit 1
  fi

  log_success "Automatic refresh completed (no manual rebuild/snapshot invocation was used)"

  # Step I: Historical daily snapshot backfill
  log_step "I. Worker: backfill historical daily snapshots ($history_date_from to $backfill_date_to)"

  invoke_docker_job "kushim-worker" "backfill_daily_snapshots" \
    "WORKER_MODE=once" \
    "WORKER_JOB=backfill_daily_snapshots" \
    "WORKER_TARGET_PORTFOLIO_ID=$state_portfolio_id" \
    "WORKER_BACKFILL_DATE_FROM=$history_date_from" \
    "WORKER_BACKFILL_DATE_TO=$backfill_date_to"
fi

# =============================================================================
# Step M: API verification
# =============================================================================

log_step "M. API verification"

portfolio_id="$state_portfolio_id"

# Re-authenticate if needed (token may have expired during Docker jobs)
auth_hdr="$(auth_headers)"
if ! http_get "$base_url_api/v1/me" "$auth_hdr"; then
  log_warn "Token may have expired. Re-authenticating..."
  login_body="$(jq -nc --arg u "$state_username" --arg p "$password" '{username:$u, password:$p}')"
  if ! http_post_inmem "$base_url_auth/auth/login" "$login_body"; then
    log_error "Re-authentication failed: HTTP $http_status"
    exit 1
  fi
  state_access_token="$(jq_field '.access_token // empty')"
  if [[ -z "$state_access_token" ]]; then
    log_error "Re-authentication response missing .access_token."
    exit 1
  fi
  auth_hdr="$(auth_headers)"
  log_success "Re-authenticated successfully"
fi

# --- M.1: Portfolio summary -------------------------------------------------

log_info "Verifying: GET /v1/portfolios/$portfolio_id/summary"

if http_get "$base_url_api/v1/portfolios/$portfolio_id/summary" "$auth_hdr"; then
  if $verbose_json; then
    jq_pretty
  fi

  assert_true "summary.data_available = true" \
    "$(jq_check '.data_available == true' && echo true || echo false)" \
    "got: $(jq_field '.data_available')"

  if jq_check '.data_available == true and .summary != null'; then
    assert_true "summary.cash_balance_minor = 804770" \
      "$(jq_check '.summary.cash_balance_minor == 804770' && echo true || echo false)" \
      "got: $(jq_field '.summary.cash_balance_minor')"
    assert_true "summary.total_value_minor = 1000000" \
      "$(jq_check '.summary.total_value_minor == 1000000' && echo true || echo false)" \
      "got: $(jq_field '.summary.total_value_minor')"
    assert_true "summary.total_invested_minor = 1000000" \
      "$(jq_check '.summary.total_invested_minor == 1000000' && echo true || echo false)" \
      "got: $(jq_field '.summary.total_invested_minor')"
    assert_true "summary.total_pnl_minor = 0" \
      "$(jq_check '.summary.total_pnl_minor == 0' && echo true || echo false)" \
      "got: $(jq_field '.summary.total_pnl_minor')"
    assert_true "summary.is_estimated = false" \
      "$(jq_check '.summary.is_estimated == false' && echo true || echo false)" \
      "got: $(jq_field '.summary.is_estimated')"
    assert_true "summary.portfolio_status = active" \
      "$(jq_check '.summary.portfolio_status == "active"' && echo true || echo false)" \
      "got: $(jq_field '.summary.portfolio_status')"
  else
    warnings+=("Summary not available (data_available=false). Worker rebuild may not have run.")
  fi
else
  log_error "Summary verification failed: HTTP $http_status"
  failed_list+=("summary endpoint")
fi

# --- M.2: Portfolio holdings -----------------------------------------------

log_info "Verifying: GET /v1/portfolios/$portfolio_id/holdings"

if http_get "$base_url_api/v1/portfolios/$portfolio_id/holdings" "$auth_hdr"; then
  if $verbose_json; then
    jq_pretty
  fi

  assert_true "holdings.data_available = true" \
    "$(jq_check '.data_available == true' && echo true || echo false)" \
    "got: $(jq_field '.data_available')"

  if jq_check '.data_available == true and (.holdings | length) > 0'; then
    assert_true "holdings[0].market_value_minor = 195230" \
      "$(jq_check '.holdings[0].market_value_minor == 195230' && echo true || echo false)" \
      "got: $(jq_field '.holdings[0].market_value_minor')"
    assert_true "holdings[0].quantity = 10.0000000000" \
      "$(jq_check '.holdings[0].quantity == "10.0000000000"' && echo true || echo false)" \
      "got: $(jq_field '.holdings[0].quantity')"
    assert_true "holdings[0].is_estimated = false" \
      "$(jq_check '.holdings[0].is_estimated == false' && echo true || echo false)" \
      "got: $(jq_field '.holdings[0].is_estimated')"
    assert_true "holdings count = 1" \
      "$(jq_check '(.holdings | length) == 1' && echo true || echo false)" \
      "got: $(jq_field '.holdings | length')"
  else
    warnings+=("Holdings not available. Worker rebuild may not have run.")
  fi
else
  log_error "Holdings verification failed: HTTP $http_status"
  failed_list+=("holdings endpoint")
fi

# --- M.3: Daily snapshots --------------------------------------------------

log_info "Verifying: GET /v1/portfolios/$portfolio_id/snapshots/daily"

if http_get "$base_url_api/v1/portfolios/$portfolio_id/snapshots/daily" "$auth_hdr"; then
  if $verbose_json; then
    jq_pretty
  fi

  assert_true "snapshots.data_available = true" \
    "$(jq_check '.data_available == true' && echo true || echo false)" \
    "got: $(jq_field '.data_available')"

  if jq_check '.data_available == true and (.snapshots | length) > 0'; then
    snapshot_count="$(jq_field '.snapshots | length')"
    assert_true "snapshots count >= 1" \
      "$(jq_check '(.snapshots | length) >= 1' && echo true || echo false)" \
      "got: $snapshot_count"
    log_info "Snapshot count: $snapshot_count (backfill covers dates >= portfolio creation only)"
  else
    warnings+=("Snapshots not available. Worker jobs may not have run.")
  fi
else
  log_error "Snapshots verification failed: HTTP $http_status"
  failed_list+=("snapshots/daily endpoint")
fi

# --- M.4: Current automatic snapshot holdings (today UTC) ------------------

auto_snapshot_date="$(date -u +%Y-%m-%d)"
log_info "Verifying: GET /v1/portfolios/$portfolio_id/snapshots/daily/$auto_snapshot_date/holdings"

if http_get "$base_url_api/v1/portfolios/$portfolio_id/snapshots/daily/$auto_snapshot_date/holdings" "$auth_hdr"; then
  if $verbose_json; then
    jq_pretty
  fi

  assert_true "snapshot_holdings.data_available = true" \
    "$(jq_check '.data_available == true' && echo true || echo false)" \
    "got: $(jq_field '.data_available')"

  if jq_check '.data_available == true and (.holdings | length) > 0'; then
    assert_true "snapshot holdings count >= 1" \
      "$(jq_check '(.holdings | length) >= 1' && echo true || echo false)" \
      "got: $(jq_field '.holdings | length')"
  fi
else
  log_error "Snapshot holdings verification failed: HTTP $http_status"
  failed_list+=("snapshots/daily/$auto_snapshot_date/holdings endpoint")
fi

# --- M.5: Operations list ---------------------------------------------------

log_info "Verifying: GET /v1/portfolios/$portfolio_id/operations"

if http_get "$base_url_api/v1/portfolios/$portfolio_id/operations" "$auth_hdr"; then
  if $verbose_json; then
    jq_pretty
  fi

  op_count="$(jq_field '.operations | length')"
  assert_true "operations count >= 2" \
    "$(jq_check '(.operations | length) >= 2' && echo true || echo false)" \
    "got: $op_count"

  posted_count="$(jq '[.operations[] | select(.operation_status == "posted")] | length' "$http_body" 2>/dev/null || echo 0)"
  assert_true "posted operations count >= 2" \
    "$(jq_check '([.operations[] | select(.operation_status == "posted")] | length) >= 2' && echo true || echo false)" \
    "got: $posted_count"
else
  log_error "Operations verification failed: HTTP $http_status"
  failed_list+=("operations endpoint")
fi

# =============================================================================
# Final summary
# =============================================================================

log_step "SUMMARY"

printf '\n' >&2
printf '  Demo identifiers:\n' >&2
printf '    username:          %s\n' "$state_username" >&2
printf '    user_id:           %s\n' "$state_user_id" >&2
printf '    portfolio_id:      %s\n' "$state_portfolio_id" >&2
printf '    asset_id:          %s\n' "$state_asset_id" >&2
printf '    deposit_op_id:     %s\n' "$state_deposit_op_id" >&2
printf '    buy_op_id:         %s\n' "$state_buy_op_id" >&2
printf '\n' >&2

if [[ ${#passed_list[@]} -gt 0 ]]; then
  printf '  Assertions passed: %d\n' "${#passed_list[@]}" >&2
  for p in "${passed_list[@]}"; do
    printf '    [PASS] %s\n' "$p" >&2
  done
fi

if [[ ${#warnings[@]} -gt 0 ]]; then
  printf '\n' >&2
  printf '  Warnings: %d\n' "${#warnings[@]}" >&2
  for w in "${warnings[@]}"; do
    printf '    [WARN] %s\n' "$w" >&2
  done
fi

if [[ ${#failed_list[@]} -gt 0 ]]; then
  printf '\n' >&2
  printf '  Assertions failed: %d\n' "${#failed_list[@]}" >&2
  for f in "${failed_list[@]}"; do
    printf '    [FAIL] %s\n' "$f" >&2
  done
  printf '\n' >&2
  log_error "RESULT: FAIL (${#failed_list[@]} assertion(s) failed)"
  exit 1
else
  printf '\n' >&2
  log_success "RESULT: PASS (${#passed_list[@]} assertion(s) passed)"
  exit 0
fi
