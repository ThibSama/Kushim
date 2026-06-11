# Kushim Documentation

This folder is the documentation entry point for the current Kushim MVP.

It is organized to help:

- a developer joining the project;
- Codex or other AI sessions operating in the repo;
- a teacher, reviewer, or jury evaluating the MVP;
- a future maintainer;
- a product person trying to understand what Kushim is and is not.

## Read this first

Core onboarding documents:

- [Project root README](../README.md)
- [Architecture overview](architecture/overview.md)
- [Service boundaries](architecture/service-boundaries.md)
- [Data flow](architecture/data-flow.md)
- [Database architecture](database/database-architecture.md)
- [Portfolio reconstruction](database/portfolio-reconstruction.md)
- [MVP scope](mvp/mvp-scope.md)
- [Deferred TODOs](mvp/deferred-todos.md)
- [Docker local development](operations/docker-local-dev.md)
- [Validation commands](operations/validation-commands.md)
- [Backend MVP demo runbook](operations/backend-demo-e2e.md)

## Current documentation structure

### Architecture

- [architecture/overview.md](architecture/overview.md)
- [architecture/service-boundaries.md](architecture/service-boundaries.md)
- [architecture/data-flow.md](architecture/data-flow.md)

### Database

- [database/database-architecture.md](database/database-architecture.md)
- [database/portfolio-reconstruction.md](database/portfolio-reconstruction.md)

### MVP

- [mvp/mvp-scope.md](mvp/mvp-scope.md)
- [mvp/deferred-todos.md](mvp/deferred-todos.md)

### Operations

- [operations/docker-local-dev.md](operations/docker-local-dev.md)
- [operations/validation-commands.md](operations/validation-commands.md)
- [operations/backend-demo-e2e.md](operations/backend-demo-e2e.md)

### Reports

- [reports/kushim-mvp-progress-report.fr.md](reports/kushim-mvp-progress-report.fr.md)
- [reports/kushim-mvp-progress-report.en.md](reports/kushim-mvp-progress-report.en.md)

### Archive

- [documentation/_archive/README.md](_archive/README.md)

## Archived legacy documentation

The repository also contains earlier or broader product, technical, UI, and review material that remains useful as historical reference, but should not be mistaken for the shortest path to understand the current MVP state.

Those files are now grouped under:

- `documentation/_archive/legacy-docs/`

Important:

- some of these files describe planned, deferred, or broader long-term architecture;
- when there is tension between an older document and the current service READMEs or the current DDL, the code and DDL win.
- archived docs are preserved, not deleted, because they can still be useful to understand the evolution of the project.

## Documentation rules

Use these wording rules consistently:

- say `portfolio_operations`, not “transactions”, unless you explicitly mean database transactions or legacy product vocabulary;
- say read models and snapshots are **worker-generated**;
- say those data are **read-only in `kushim-api`**;
- say `kushim-market-data` is **implemented with mock provider** (not "fully implemented" until a real provider is integrated);
- distinguish clearly between:
  - Implemented
  - Implemented and validated
  - Scaffolded
  - Planned
  - Deferred
  - Not started
  - Known limitation
  - Accepted risk

## Source of truth hierarchy

For the current project state:

1. PostgreSQL DDL is the schema source of truth:
   - `infra/postgres/init/001_init.sql`
2. Rust and frontend source code define implemented behavior.
3. Service READMEs describe intended validated behavior per service.
4. The new architecture/MVP docs in this folder explain the project state at repository level.
5. Older design or review documents are reference material only.
