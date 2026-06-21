#!/usr/bin/env bash
set -Eeuo pipefail

# =============================================================================
# check-local-services.sh — Bash equivalent of check-local-services.ps1
#
# Verifies that the four local Kushim backend services are healthy.
# With --start, brings up the required Docker Compose services first.
#
# Usage:
#   ./scripts/bash/validation/check-local-services.sh
#   ./scripts/bash/validation/check-local-services.sh --start
# =============================================================================

# Resolve repository root from this script's location (three levels up).
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

# --- Argument parsing --------------------------------------------------------

start_services=false

for arg in "$@"; do
  case "${arg}" in
    --start)
      start_services=true
      ;;
    -h|--help)
      echo "Usage: $0 [--start]"
      echo "  --start  Bring up Docker Compose backend services before checking."
      exit 0
      ;;
    *)
      echo "Error: unknown argument '${arg}'" >&2
      echo "Usage: $0 [--start]" >&2
      exit 1
      ;;
  esac
done

# --- Preflight: docker must be on PATH --------------------------------------

if ! command -v docker >/dev/null 2>&1; then
  echo "Error: docker is not available on PATH." >&2
  exit 1
fi

if ! docker compose version >/dev/null 2>&1; then
  echo "Error: 'docker compose' is not available or Docker daemon is not running." >&2
  exit 1
fi

# --- Optional: start backend services ----------------------------------------

if [ "${start_services}" = true ]; then
  echo "Starting backend services..."
  docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data
fi

# --- Health check helper -----------------------------------------------------
#
# Uses curl with a 5-second timeout. Requires valid JSON with status == "ok".
# On failure, prints a concise redacted error — never the full payload — to
# avoid disclosing sensitive configuration or stack traces.

check_health() {
  local name="$1"
  local url="$2"

  local response
  local http_code

  # Capture both body and HTTP status code. 5-second connect+max timeout.
  response="$(curl -fsS --connect-timeout 5 --max-time 5 "${url}" 2>/dev/null)" || {
    echo "Error: ${name} is not reachable at ${url} (curl failed or timed out)" >&2
    return 1
  }

  # Require valid JSON and check .status == "ok" using jq.
  if ! echo "${response}" | jq -e '.status == "ok"' >/dev/null 2>&1; then
    echo "Error: ${name} returned an unexpected or invalid health payload at ${url}" >&2
    return 1
  fi

  echo "[ok] ${name} ${url}"
}

# --- Check all four backend services -----------------------------------------

check_health "kushim-auth-api"    "http://127.0.0.1:3002/health"
check_health "kushim-api"         "http://127.0.0.1:8080/health"
check_health "kushim-worker"      "http://127.0.0.1:8081/health"
check_health "kushim-market-data" "http://127.0.0.1:8082/health"

echo "Local MVP backend prerequisites are healthy."
