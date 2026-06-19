# Local reset and diagnostics

## Scope and safety

This runbook is for **local development only**. Run commands from the repository root in PowerShell.

The labels below are deliberate:

- **Safe / non-destructive**: inspects or restarts services without removing local data.
- **Recreates containers, preserves data**: replaces containers or images while retaining the named PostgreSQL and Redis volumes.
- **Destructive / removes local data**: deletes the Compose-managed named volumes. Use only when local data is disposable.

Do not make a destructive reset the first troubleshooting step.

## Fast decision table

| Need | Appropriate action | Data preserved |
|---|---|---|
| Inspect the current state | `docker compose ps -a` | Yes |
| Restart an unchanged service | `docker compose restart <service>` | Yes |
| Recreate a service with its current image and configuration | `docker compose up -d --force-recreate <service>` | Yes |
| Rebuild source changes | `docker compose build <service>` then `docker compose up -d --force-recreate <service>` | Yes |
| Remove Compose containers and network | `docker compose down` | Yes; named volumes remain |
| Completely reset PostgreSQL and Redis | `docker compose down -v` | **No** |

`docker compose down` does not remove declared named volumes unless `-v`/`--volumes` is supplied.

## Normal non-destructive workflows

### Check current service state

```powershell
docker compose ps -a
docker compose config --services
docker compose config --volumes
```

`docker compose config` may interpolate environment values. Do not paste its full output into an issue or report without reviewing it for secrets.

### Start backend prerequisites

```powershell
docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data
docker compose ps
```

### Restart one unchanged service

```powershell
docker compose restart kushim-api
docker compose logs --tail=100 kushim-api
```

This restarts the existing container. It does not rebuild an image or remove named volumes.

### Rebuild and recreate one changed service

```powershell
docker compose build kushim-api
docker compose up -d --force-recreate kushim-api
docker compose logs --tail=100 kushim-api
```

### Recreate a frontend and nginx

```powershell
docker compose build kushim-website
docker compose up -d --force-recreate kushim-website nginx
docker compose logs --tail=100 kushim-website
docker compose logs --tail=100 nginx
```

Nginx declares dependencies on all routed frontends and APIs. If another required upstream is absent, nginx can fail at startup even when the website itself is healthy.

### Restart the full stack without deleting data

```powershell
docker compose down
docker compose up -d
docker compose ps
```

This removes and recreates containers and the Compose network but retains `kushim_postgres_data` and `redis-data`.

### Inspect recent logs

```powershell
docker compose logs --tail=100 kushim-auth-api
docker compose logs --tail=100 kushim-api
docker compose logs --tail=100 nginx
```

Review logs before sharing them. Do not disclose JWT secrets, database passwords, provider keys, access tokens, refresh tokens, recovery phrases, or hashes.

### Check health and readiness

`/health` confirms that the process responds. `/ready` additionally checks whether the service can serve dependency-backed work.

```powershell
curl.exe -fsS http://127.0.0.1:3002/health
curl.exe -fsS http://127.0.0.1:3002/ready
curl.exe -fsS http://127.0.0.1:8080/health
curl.exe -fsS http://127.0.0.1:8080/ready
curl.exe -fsS http://127.0.0.1:8081/health
curl.exe -fsS http://127.0.0.1:8081/ready
curl.exe -fsS http://127.0.0.1:8082/health
curl.exe -fsS http://127.0.0.1:8082/ready
```

Use `curl.exe`, not the ambiguous PowerShell `curl` alias. `Invoke-RestMethod` is also suitable.

### Check canonical nginx routing

```powershell
curl.exe -I -H "Host: kushim.localhost" http://127.0.0.1/
curl.exe -I -H "Host: auth.kushim.localhost" http://127.0.0.1/
curl.exe -I -H "Host: app.kushim.localhost" http://127.0.0.1/
curl.exe -I -H "Host: api.kushim.localhost" http://127.0.0.1/health
curl.exe -I -H "Host: auth-api.kushim.localhost" http://127.0.0.1/health
```

## Destructive complete reset

A complete reset is appropriate only when the local PostgreSQL and Redis data may be discarded and a fresh initialization is explicitly required.

**WARNING — destructive, local development only:** the following command removes the Compose-managed `kushim_postgres_data` and `redis-data` volumes. All local database data is lost. Redis handoff and rate-limit state is also lost.

```powershell
docker compose down -v
docker compose up -d
```

PostgreSQL executes `infra/postgres/init/001_init.sql`, `002_seed_canonical_assets.sql`, and `003_seed_auth_roles.sql` only when the database starts with a new empty data volume. A fresh initialization therefore recreates the schema and reapplies the canonical asset and auth-role seeds.

This is not equivalent to upgrading an existing database. Never use this procedure against shared or production data.

## Existing-database upgrades

Files under `infra/postgres/init/` initialize a fresh, empty PostgreSQL volume. They do not automatically rerun against an existing volume.

For an existing local database, use the repository helper:

```powershell
docker compose up -d database
./scripts/dev/apply-db-upgrades.ps1
```

The helper:

- requires the `kushim_database` container to be running;
- applies `infra/postgres/upgrades/*.sql` in lexical order with `ON_ERROR_STOP=1`;
- stops on the first failed script;
- does not reset the volume or delete application rows;
- currently applies idempotent additive scripts, although one script may drop and recreate constraints to correct their definitions;
- verifies a defined subset of required tables, indexes, foreign keys, and checks after applying scripts.

The verification list is not a complete schema diff and currently does not prove every object introduced by every upgrade. Inspect the script output and the relevant SQL when diagnosing an older schema.

## Diagnostic decision tree

| Symptom | Smallest safe inspection | Likely interpretation | Next safe action | Data preserved |
|---|---|---|---|---|
| Docker Desktop or `docker` unavailable | `docker compose version` | Docker is stopped or unavailable on `PATH` | Start Docker Desktop; retry the version command | Yes |
| Service container not running | `docker compose ps -a <service>` | Container was never created or exited | Read `docker compose logs --tail=100 <service>`, then `docker compose up -d <service>` | Yes |
| Process healthy but dependency not ready | Call both `/health` and `/ready` | Process is alive; database or Redis is unavailable | Inspect dependency status and logs | Yes |
| `/health` succeeds but `/ready` fails | `docker compose ps database redis` | Dependency-backed readiness failed | Start/fix the named dependency; do not reset data | Yes |
| `PoolTimedOut` | `docker compose ps database`; then database logs | PostgreSQL is unreachable, unready, or connection settings are wrong | Start PostgreSQL and verify `DATABASE_URL` without printing credentials | Yes |
| Container does not match current source | `docker compose images <service>` and recent service logs | Stale image/container | Build, then `up -d --force-recreate <service>` | Yes |
| Canonical `*.kushim.localhost` route fails | Direct health check, then `docker compose ps nginx` | Service or proxy path is down | Inspect nginx and upstream logs | Yes |
| Nginx returns 502/upstream error | `docker compose logs --tail=100 nginx` | Upstream name is absent, exited, or not listening | Start/recreate only the affected upstream and nginx | Yes |
| Host port already allocated | `docker ps --format "table {{.Names}}\t{{.Ports}}"` | Another container owns the fixed port | Stop the conflicting local stack or use an explicit isolated override | Yes |
| Two worktrees expose the same ports | `git worktree list`; then the port inspection above | Separate Compose projects still share host ports | Keep one stack active or supply an explicit local override | Yes |
| Environment value did not reach a container | `docker compose exec <service> printenv NON_SENSITIVE_NAME` | Container was not recreated or interpolation differs | Recreate the service with the intended non-sensitive override | Yes |
| Database volume has an older schema | Run `./scripts/dev/apply-db-upgrades.ps1` and inspect its result | Init scripts did not rerun on the existing volume | Apply upgrades; reserve reset for disposable data | Yes |
| Frontend works directly but not through nginx | Direct frontend check, then canonical Host-header check | Nginx route/upstream/HMR path is failing | Inspect nginx config-mounted container and logs | Yes |

Only use `printenv` for explicitly non-sensitive variables. Never print `AUTH_JWT_SECRET`, database passwords, provider keys, or tokens.

## Worktree and Compose collisions

Multiple Git worktrees can produce distinct Compose project names, networks, and container sets. They still bind the same host interfaces by default.

The repository currently publishes these fixed/default host ports:

| Service | Host port |
|---|---:|
| nginx | 80 |
| PostgreSQL | 5432 by default (`POSTGRES_PORT` can override it) |
| `kushim-auth-api` | 3002 |
| `kushim-api` | 8080 |
| `kushim-worker` | 8081 |
| `kushim-market-data` | 8082 |

The browser frontends are exposed through nginx rather than published directly. A `port is already allocated` error usually means another worktree or Compose project owns a host port; it does not by itself indicate broken application code.

Stop the conflicting local stack, or use an explicit local Compose override maintained outside the repository. No isolated-port override file is currently provided by the repository.

## Scope of `check-local-services.ps1`

```powershell
./scripts/validation/check-local-services.ps1 -Start
```

With `-Start`, the script:

1. verifies that `docker` and Compose are available;
2. starts `database`, `redis`, `kushim-auth-api`, `kushim-api`, `kushim-worker`, and `kushim-market-data`;
3. calls only the four service `/health` endpoints on ports 3002, 8080, 8081, and 8082;
4. requires each JSON payload to contain `status: "ok"`.

It does not call `/ready`, validate nginx, start the three browser frontends, run database upgrades, run smoke scenarios, or prove that every dependency-backed operation works. Use the manual readiness and routing checks above when those guarantees matter.
