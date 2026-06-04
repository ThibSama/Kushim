# infra/postgres

Postgres local development support.

The PostgreSQL V3 schema source of truth is:

- `infra/postgres/init/001_init.sql`

The DDL also defines:

- a reusable `set_updated_at()` trigger function for mutable tables,
- a `prevent_posted_operation_mutation()` trigger function protecting posted `portfolio_operations`,
- explicit database-side protection for the ledger immutability rule.

This file is mounted into the PostgreSQL container and executed automatically by the official Postgres image on first initialization only.

Important:

- Postgres runs init scripts only when the data directory is empty.
- If the named volume already contains data, `001_init.sql` will not run again automatically.
- To re-run initialization from scratch, remove the volume and recreate the container.

Trigger scope:

- Triggers are used only for simple invariants and `updated_at` automation.
- Triggers must not recalculate portfolio values, refresh read models, generate snapshots, or call market APIs.

Useful commands:

Start database:

```powershell
docker compose up -d database
```

Stop database:

```powershell
docker compose down
```

Reset database completely:

```powershell
docker compose down -v
docker compose up -d database
```

View logs:

```powershell
docker compose logs -f database
```

Connect with psql:

```powershell
docker exec -it kushim_database psql -U kushim -d kushim
```
