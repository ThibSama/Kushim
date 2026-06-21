#!/usr/bin/env bash
set -Eeuo pipefail

# =============================================================================
# apply-db-upgrades.sh — Bash equivalent of apply-db-upgrades.ps1
#
# Applies idempotent, non-destructive PostgreSQL upgrade scripts to the local
# Kushim database inside the running `kushim_database` container.
#
# - Locates infra/postgres/upgrades/ from the repository root.
# - Selects only *.sql files in deterministic lexical order.
# - Applies each via: docker exec -i kushim_database psql -U kushim -d kushim \
#                       -v ON_ERROR_STOP=1 -q
# - Stops immediately on the first SQL failure.
# - Never resets volumes, drops data, truncates data, or deletes application rows.
# - Does not print credentials.
# - Verifies required P3 idempotency objects after all scripts are applied.
#
# Usage:
#   ./scripts/bash/dev/apply-db-upgrades.sh
# =============================================================================

# Resolve repository root from this script's location (three levels up).
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

# --- Argument parsing --------------------------------------------------------

for arg in "$@"; do
  case "${arg}" in
    -h|--help)
      echo "Usage: $0"
      echo "Applies all *.sql upgrade scripts under infra/postgres/upgrades/ to the local kushim_database container."
      exit 0
      ;;
    *)
      echo "Error: unknown argument '${arg}'" >&2
      echo "Usage: $0" >&2
      exit 1
      ;;
  esac
done

# --- Locate upgrades directory ----------------------------------------------

upgrades_dir="${repo_root}/infra/postgres/upgrades"

if [ ! -d "${upgrades_dir}" ]; then
  echo "Error: upgrades directory not found at ${upgrades_dir}" >&2
  exit 1
fi

# --- Verify the kushim_database container is running ------------------------

container="kushim_database"
running="$(docker ps --filter "name=^/${container}$" --filter 'status=running' --format '{{.Names}}')"

if [ -z "${running}" ]; then
  echo "Error: container '${container}' is not running." >&2
  echo "Start it with: docker compose up -d database" >&2
  exit 1
fi

# --- Collect *.sql files in lexical order ------------------------------------

# Use null-delimited output for safety with filenames containing spaces.
sql_files=()
while IFS= read -r -d '' f; do
  sql_files+=("$f")
done < <(find "${upgrades_dir}" -maxdepth 1 -name '*.sql' -print0 | sort -z)

if [ "${#sql_files[@]}" -eq 0 ]; then
  echo "No upgrade scripts found. Nothing to apply."
  exit 0
fi

# --- Apply each upgrade script -----------------------------------------------
#
# Pipe the file content into psql via `docker exec -i`. ON_ERROR_STOP=1
# ensures psql exits non-zero on the first SQL error, and `set -e` in the
# shell propagates that immediately.

for script in "${sql_files[@]}"; do
  script_name="$(basename "${script}")"
  echo "Applying ${script_name} ..."
  if ! docker exec -i "${container}" psql -U kushim -d kushim -v ON_ERROR_STOP=1 -q < "${script}"; then
    echo "Error: upgrade script ${script_name} failed." >&2
    exit 1
  fi
  echo "  OK: ${script_name}"
done

# --- Verify required P3 objects ----------------------------------------------

echo ""
echo "All upgrade scripts applied. Verifying required objects..."

# Helper: run a SQL query in the container, return trimmed result.
run_sql() {
  local sql="$1"
  docker exec -i "${container}" psql -U kushim -d kushim -t -A <<< "${sql}" | tr -d '[:space:]'
}

# --- Required tables ---------------------------------------------------------

required_tables=(
  "portfolio_refresh_requests"
  "portfolio_operation_idempotency"
)

missing=()

for table in "${required_tables[@]}"; do
  present="$(run_sql "SELECT to_regclass('public.${table}') IS NOT NULL;")"
  if [ "${present}" = "t" ]; then
    echo "  table  OK: ${table}"
  else
    echo "  table  MISSING: ${table}"
    missing+=("table ${table}")
  fi
done

# --- Required indexes -------------------------------------------------------

required_indexes=(
  "uq_portfolio_operation_idempotency_user_key"
  "idx_portfolio_operation_idempotency_portfolio_created"
)

for idx in "${required_indexes[@]}"; do
  present="$(run_sql "SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = '${idx}');")"
  if [ "${present}" = "t" ]; then
    echo "  index  OK: ${idx}"
  else
    echo "  index  MISSING: ${idx}"
    missing+=("index ${idx}")
  fi
done

# --- Required foreign keys (with expected confdeltype) ----------------------
# confdeltype: 'r' = RESTRICT, 'n' = SET NULL, 'c' = CASCADE, 'a' = NO ACTION.

required_fks=(
  "fk_portfolio_operation_idempotency_user|r"
  "fk_portfolio_operation_idempotency_portfolio|r"
  "fk_portfolio_operation_idempotency_operation|r"
  "fk_portfolio_operation_idempotency_corrected_operation|n"
  "fk_portfolio_operation_idempotency_refresh_request|n"
)

for entry in "${required_fks[@]}"; do
  fk_name="${entry%%|*}"
  expected_del="${entry##*|}"
  present="$(run_sql "SELECT EXISTS(SELECT 1 FROM pg_constraint WHERE conname = '${fk_name}' AND contype = 'f' AND confdeltype = '${expected_del}');")"
  if [ "${present}" = "t" ]; then
    echo "  fk     OK: ${fk_name} (ON DELETE ${expected_del})"
  else
    echo "  fk     MISSING or WRONG: ${fk_name} (expected ON DELETE ${expected_del})"
    missing+=("fk ${fk_name}")
  fi
done

# --- Required CHECK constraints ---------------------------------------------

required_checks=(
  "chk_portfolio_operation_idempotency_request_kind"
  "chk_portfolio_operation_idempotency_correction_link"
)

for chk in "${required_checks[@]}"; do
  present="$(run_sql "SELECT EXISTS(SELECT 1 FROM pg_constraint WHERE conname = '${chk}' AND contype = 'c');")"
  if [ "${present}" = "t" ]; then
    echo "  check  OK: ${chk}"
  else
    echo "  check  MISSING: ${chk}"
    missing+=("check ${chk}")
  fi
done

# --- Final report ------------------------------------------------------------

if [ "${#missing[@]}" -gt 0 ]; then
  echo ""
  echo "Error: required objects missing after upgrade: ${missing[*]}" >&2
  exit 1
fi

echo ""
echo "All required tables, indexes and FK constraints verified. Upgrade complete."
