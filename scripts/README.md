# Kushim Scripts

This directory contains helper scripts for local development, validation, and
demo scenarios. Scripts are organized by shell so that each platform has a
clear, self-contained command set.

## Directory layout

```
scripts/
├── bash/                  # Bash scripts for Linux / macOS / WSL
│   ├── validation/
│   │   └── check-local-services.sh
│   ├── dev/
│   │   └── apply-db-upgrades.sh
│   └── demo/
│       └── backend-e2e.sh
├── powershell/            # PowerShell scripts for Windows or pwsh users
│   ├── validation/
│   │   └── check-local-services.ps1
│   ├── dev/
│   │   └── apply-db-upgrades.ps1
│   └── demo/
│       └── backend-e2e.ps1
├── dev/                   # Other PowerShell dev utilities (not yet reorganized)
├── test/                  # Migration validators used by CI
└── README.md              # This file
```

## Why separated by shell?

Bash and PowerShell have different syntax, quoting rules, and platform
availability. Keeping them in separate subtrees avoids cross-platform
confusion and makes it obvious which command to run on each OS.

Both implementations of the same utility **must preserve functional parity**.
When a behavior changes in one, the other must be updated accordingly.

## Bash (Linux / macOS / WSL)

Prerequisites: `bash`, `curl`, `jq`, `docker`, `docker compose`, `openssl`.

| Action | Command |
| --- | --- |
| Check local services | `./scripts/bash/validation/check-local-services.sh` |
| Check + start services | `./scripts/bash/validation/check-local-services.sh --start` |
| Apply DB upgrades | `./scripts/bash/dev/apply-db-upgrades.sh` |
| Backend E2E demo | `./scripts/bash/demo/backend-e2e.sh` |
| Backend E2E (verbose) | `./scripts/bash/demo/backend-e2e.sh --verbose-json` |
| Backend E2E (skip Docker jobs) | `./scripts/bash/demo/backend-e2e.sh --skip-docker-jobs --verbose-json` |
| Backend E2E (custom prefix) | `./scripts/bash/demo/backend-e2e.sh --demo-prefix jury_demo --snapshot-date 2026-06-09` |

## PowerShell (Windows or `pwsh` users)

Prerequisites: `docker`, `docker compose`, Docker Desktop running.

| Action | Command |
| --- | --- |
| Check local services | `./scripts/powershell/validation/check-local-services.ps1` |
| Check + start services | `./scripts/powershell/validation/check-local-services.ps1 -Start` |
| Apply DB upgrades | `./scripts/powershell/dev/apply-db-upgrades.ps1` |
| Backend E2E demo | `./scripts/powershell/demo/backend-e2e.ps1` |

## Equivalent command mapping

| Bash | PowerShell |
| --- | --- |
| `check-local-services.sh` | `check-local-services.ps1` |
| `check-local-services.sh --start` | `check-local-services.ps1 -Start` |
| `apply-db-upgrades.sh` | `apply-db-upgrades.ps1` |
| `backend-e2e.sh` | `backend-e2e.ps1` |
| `backend-e2e.sh --verbose-json` | `backend-e2e.ps1 -VerboseJson` |
| `backend-e2e.sh --skip-docker-jobs` | `backend-e2e.ps1 -SkipDockerJobs` |
| `backend-e2e.sh --dry-run` | `backend-e2e.ps1 -DryRun` |

## Backend E2E

Both Bash and PowerShell implementations of the backend E2E smoke test are
available. Both use the deterministic mock provider and cover the same 18
assertions. They **must preserve functional parity**.

- Bash: `./scripts/bash/demo/backend-e2e.sh`
- PowerShell: `./scripts/powershell/demo/backend-e2e.ps1`
- See `scripts/powershell/demo/README.md` for PowerShell-specific details.

## Safety warning: database upgrades

`apply-db-upgrades.*` applies idempotent, non-destructive SQL upgrade scripts
to the running `kushim_database` container. It:

- never resets the volume (`docker compose down -v`);
- never drops, truncates, or deletes application data;
- stops immediately on the first SQL failure;
- verifies required P3 idempotency objects after all scripts are applied.

Despite these safeguards, always inspect the upgrade scripts before running
them against a database you care about. Upgrade scripts live in
`infra/postgres/upgrades/`.
