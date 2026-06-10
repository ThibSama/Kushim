# Backend E2E Demo Script

## What it does

`backend-e2e.ps1` executes the full backend MVP scenario automatically:

1. Verifies all 4 backend services are healthy.
2. Signs up a unique demo user via `kushim-auth-api`.
3. Creates a USD portfolio via `kushim-api`.
4. Seeds a demo AAPL asset in PostgreSQL (or finds an existing one).
5. Creates and posts a deposit operation (10,000.00 USD).
6. Creates and posts a buy operation (10 AAPL at 195.23 USD).
7. Runs `kushim-market-data` to refresh current prices (mock provider).
8. Runs `kushim-market-data` to fill historical price cache.
9. Runs `kushim-worker` to rebuild read models.
10. Runs `kushim-worker` to generate daily snapshots.
11. Runs `kushim-worker` to backfill historical snapshots.
12. Verifies all API read endpoints (summary, holdings, snapshots, operations).
13. Prints a PASS/FAIL summary with all generated IDs.

## Prerequisites

- Docker Desktop running.
- All backend services started: `docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data`
- PostgreSQL schema initialized (DDL V3).
- PowerShell 5.1+ or PowerShell 7+.

## How to run

```powershell
cd E:\Kushim
.\scripts\demo\backend-e2e.ps1
```

With verbose JSON output:

```powershell
.\scripts\demo\backend-e2e.ps1 -VerboseJson
```

Skip Docker jobs (useful when jobs were already run):

```powershell
.\scripts\demo\backend-e2e.ps1 -SkipDockerJobs -VerboseJson
```

Custom prefix for demo data:

```powershell
.\scripts\demo\backend-e2e.ps1 -DemoPrefix "jury_demo"
```

Dry run (health check only):

```powershell
.\scripts\demo\backend-e2e.ps1 -DryRun
```

## Safety policy

- Each run creates a **unique** demo user (timestamp suffix).
- Each run creates a **new** portfolio with a unique name.
- Each run seeds a **new** AAPL asset (or finds an existing one).
- The script **never deletes, truncates, or drops** any data.
- The script **never modifies** application code, DDL, or Docker configuration.
- Demo data remains in the database after the script finishes.

## What it does not do

- Does not test frontend behavior.
- Does not test real market-data providers.
- Does not test FX conversion.
- Does not test production deployment.
- Does not clean up after itself.
- Does not modify any code or configuration.

## Common troubleshooting

| Problem | Solution |
|---|---|
| Service not healthy | Run `docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data` |
| Signup 409 conflict | Re-run the script (auto-generates a new suffix) or use `-DemoPrefix` |
| Token expired during jobs | The script re-authenticates automatically before verification |
| Docker job fails | Check `docker compose logs <service>` for details |
| Holdings `is_estimated = true` | Ensure portfolio uses USD and asset uses USD |
| `data_available = false` | Docker jobs may not have run; check `-SkipDockerJobs` was not set |

## Reference

Full manual runbook: `documentation/operations/backend-demo-e2e.md`
