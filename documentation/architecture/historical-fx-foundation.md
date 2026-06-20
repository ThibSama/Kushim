# Historical FX Foundation (PR004)

Status: **Implemented and validated locally** (schema, mock provider,
repository, job, validator) — see "Limitations" for what is intentionally
deferred. This document describes the PR004 deliverable.

This foundation is paired with:

- [portfolio-performance-contract.md](portfolio-performance-contract.md) —
  defines `fx_rate_missing`, `partial_currency`, the 7-day carry-forward
  tolerance, and the transactional rebuild contract that future portfolio
  history consumers must satisfy.
- [historical-valuation-provenance.md](historical-valuation-provenance.md) —
  defines how a per-position day records its FX-related provenance.

## 1. Scope

In scope of PR004:

- a generic, additive PostgreSQL table `fx_rate_history_cache` (migration
  `infra/postgres/upgrades/004_fx_rate_history_cache.sql`);
- both a fresh-bootstrap and an existing-database upgrade path, converging
  on the same final structure;
- a deterministic mock FX history provider with a frozen ECB-anchored
  fixture for ten product-selected currencies;
- a repository with insert / bulk-upsert / latest-with-carry-forward
  lookup / gap detection;
- a fill / repair job that requests rates from a provider and persists
  them atomically per pair;
- new environment variables (CLI integration);
- a migration validator pinned to an immutable pre-migration baseline;
- a CI job that runs the validator on every push/PR;
- this document.

Out of scope of PR004 (deferred):

- selection or integration of a **real** FX provider;
- automatic "first conversion need" cross-service trigger;
- portfolio-history rebuild after FX correction (the worker is untouched);
- the portfolio-side `partial_currency` fallback implementation;
- the future historical-performance read model and API;
- corporate-action-aware adjusted FX (none of the providers expose it);
- frontend integration.

## 2. Schema

Table `fx_rate_history_cache` (added by migration 004 and present in
`infra/postgres/init/001_init.sql` for fresh bootstraps):

| Column | Type | Notes |
| --- | --- | --- |
| `id_fx_rate_history_cache` | `uuid` PRIMARY KEY DEFAULT `gen_random_uuid()` | |
| `rate_date` | `date` NOT NULL | UTC calendar date the rate applies to. |
| `canonical_base_currency` | `char(3)` NOT NULL | Uppercase ISO-style code; must lexicographically precede `canonical_quote_currency`. |
| `canonical_quote_currency` | `char(3)` NOT NULL | Uppercase ISO-style code. |
| `canonical_rate` | `numeric(28, 12)` NOT NULL | `quote_per_base`. Strictly positive. |
| `inverse_rate` | `numeric(28, 12)` STORED GENERATED | `ROUND((1::numeric / canonical_rate)::numeric, 12)`. Cannot diverge from `canonical_rate` by construction. |
| `provider` | `varchar(50)` NOT NULL | Provider identifier. |
| `provider_as_of` | `timestamptz` NULL | Provider-reported publication instant. |
| `dataset_version` | `varchar(64)` NOT NULL | Identifier of the source dataset / fixture version. Bumping this is the signal that previously-persisted rows are out of date. |
| `created_at` | `timestamptz` NOT NULL DEFAULT `now()` | |
| `updated_at` | `timestamptz` NOT NULL DEFAULT `now()` | Maintained by `set_updated_at()` trigger. |

CHECK constraints:

- `chk_fx_rate_history_cache_base_currency_format`: `^[A-Z]{3}$`.
- `chk_fx_rate_history_cache_quote_currency_format`: `^[A-Z]{3}$`.
- `chk_fx_rate_history_cache_pair_canonical_ordering`:
  `canonical_base_currency < canonical_quote_currency`.
- `chk_fx_rate_history_cache_canonical_rate_positive`:
  `canonical_rate > 0`.
- `chk_fx_rate_history_cache_provider_not_blank`.
- `chk_fx_rate_history_cache_dataset_version_not_blank`.

Indexes:

- `uq_fx_rate_history_cache_pair_date_provider` — UNIQUE on
  `(canonical_base_currency, canonical_quote_currency, rate_date,
  provider)`.
- `idx_fx_rate_history_cache_pair_date_desc` — supports
  latest-on-or-before lookups per pair.
- `idx_fx_rate_history_cache_date_desc` — supports date-bounded scans.
- `idx_fx_rate_history_cache_provider_date` — supports provider-scoped
  repair scans.

The supported currency set is **not** pinned in the schema. Adding a new
currency requires no migration — only updating the provider's anchor
table.

## 3. Canonical pair convention

A canonical pair stores the lexicographically smaller currency as
`canonical_base_currency` and the larger as `canonical_quote_currency`.
Direction handling:

- Storage and database CHECK enforce `base < quote`.
- Application code (`CanonicalPair::new(a, b)`) accepts the two
  currencies in any order and orders them automatically.
- Lookup requests source/target in their natural order and the repository
  returns the rate in the requested direction (`PairDirection::Direct` or
  `Inverse`).
- The inverse rate is the database-computed `inverse_rate` column, which
  is a STORED GENERATED column derived from `canonical_rate`. The two
  directions cannot diverge.
- Identity conversions (source == target) return `rate = 1` via
  `domain::fx_rate::identity_lookup` and **never** touch the database.

## 4. Decimal precision

- `canonical_rate` and `inverse_rate` use PostgreSQL `numeric(28, 12)`.
- Rust code uses `rust_decimal::Decimal` end-to-end (already a workspace
  dependency). `f32` / `f64` are forbidden for persisted or calculated
  rates.
- The deterministic mock provider's per-currency variation is computed
  **without** any floating-point arithmetic: an integer FNV-1a hash of
  the currency code yields amplitude (basis points) and period (days);
  a deterministic **integer triangle wave** evaluates the signed
  basis-point offset for the requested date; the factor is built by
  `Decimal::ONE + Decimal::from(signed_bps) / Decimal::from(10_000)`.
- A test in the provider source (`no_floating_point_rate_generation_path`)
  rejects any reintroduction of `f32`, `f64`, `.sin(`, `.cos(`, `.tan(`,
  `.powf(`, `as f32`, `as f64`, `f64::consts`, `: f64`, `-> f64` in the
  production portion of `mock_fx_history.rs`.

## 5. Supported mock currencies

The mock provider supports exactly these ten product-selected currencies
(uppercase, ISO 4217 style):

```
AUD CAD CHF CNY EUR GBP HKD JPY SGD USD
```

10 currencies produce **45 canonical unordered pairs** (`10 × 9 / 2`).
Every pair is supported by the mock provider.

## 6. Frozen fixture provenance

- **Source**: European Central Bank, "Euro foreign exchange reference
  rates", daily reference rates published at
  <https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml>.
- **Effective rate date** (the `time=` attribute of the ECB XML
  envelope): **2026-06-18**.
- **Retrieved on**: 2026-06-19 (during PR004 implementation).
- **Anchor currency**: `EUR` (the ECB feed publishes "1 EUR = X" for
  every quoted currency).
- **Frozen anchor values** (1 EUR = X currency on 2026-06-18):

  | Currency | Anchor (units per EUR) |
  | --- | --- |
  | EUR | 1 |
  | USD | 1.1461 |
  | JPY | 184.44 |
  | GBP | 0.86638 |
  | CHF | 0.9218 |
  | AUD | 1.6362 |
  | CAD | 1.6189 |
  | HKD | 8.9827 |
  | SGD | 1.4795 |
  | CNY | 7.7609 |

- **Dataset version** (`fx_rate_history_cache.dataset_version`):
  `mock-ecb-2026-06-18-v2`. The bump from `v1` to `v2` represents a
  **generation-algorithm correction** (floating-point `sin()` replaced
  by an integer triangle wave); the frozen ECB anchor values themselves
  are unchanged. Applying `v2` over existing `v1` rows surfaces them as
  `Updated` (validated end-to-end against the dev database).

The anchor table is inlined as Rust constants in
`kushim-market-data/src/providers/mock_fx_history.rs`. The fixture is
never re-fetched at runtime (no network call). Updating the fixture is
an explicit code review: change the constants **and** bump
`MOCK_FX_DATASET_VERSION` so the repository surfaces `Updated` outcomes
and downstream rebuilds can be triggered.

ECB is used here **only as a one-time human-reviewed snapshot source**
for the deterministic mock. It is **not** a registered runtime provider.
Real provider selection is deferred — see "Limitations" below.

## 7. Deterministic historical algorithm

For business days other than the anchor date, the mock provider derives
a small deterministic, bounded **per-currency** factor entirely from
integer arithmetic and `rust_decimal::Decimal`:

```text
hash(ccy)               = FNV-1a over the three uppercase ASCII bytes
amp_bps(ccy)            = 10 + (hash(ccy) % 9) * 5         (10..50 bps; 0 for EUR)
period_days(ccy)        = 40 + ((hash(ccy)/7) % 11) * 6    (40..100 days; even)
offset(D)               = jdn(D) - jdn(anchor_rate_date())  (integer)
t(D, ccy)               = offset(D) rem_euclid period_days(ccy)   (in [0, period))

# Integer triangle wave anchored at 0 at t = 0:
#   [0,           P/4]            : linear 0       → +amp_bps
#   (P/4,         P/2]            : linear +amp_bps → 0
#   (P/2,         P/2 + P/4]      : linear 0       → -amp_bps
#   (P/2 + P/4,   P)              : linear -amp_bps → 0
signed_amp_bps(ccy, D)  = triangle_wave(t, period_days(ccy), amp_bps(ccy))

factor(ccy, D)          = Decimal::ONE + Decimal::from(signed_amp_bps) / Decimal::from(10_000)
daily_anchor_value(ccy, D)
                        = anchor[ccy] * factor(ccy, D)
rate(A → B, D)          = daily_anchor_value(B, D) / daily_anchor_value(A, D)
```

Properties — all guaranteed by construction:

- **No floating point** in any line that participates in a rate; only
  integer hashing, integer date arithmetic, and `rust_decimal::Decimal`.
- On the anchor date the offset is exactly `0` ⇒ triangle value is `0`
  ⇒ `factor = Decimal::ONE` ⇒ the frozen ECB anchor vector is
  recovered byte-for-byte.
- `factor ∈ [Decimal("0.9950"), Decimal("1.0050")]` for every non-EUR
  currency (≤ ±0.5 % drift); for EUR `factor = Decimal::ONE` always.
- Because each pair is derived from the **same** per-currency daily
  vector, the triangular identity
  `rate(A→B) × rate(B→C) = rate(A→C)` holds exactly (subject to the
  12-dp persistence rounding). Reciprocity is the special case `A = C`.
- Different currencies use different `amp_bps` and `period_days`, so
  the per-currency factors differ across dates ⇒ every cross-rate
  genuinely varies day-over-day.

Weekends (Saturday, Sunday) return `ProviderDailyRate::NoQuoteForDate`.
Repository carry-forward (section 9) covers weekends and short holiday
gaps.

### History — what changed across versions

- `v1` used `f64` `sin()` for the per-currency factor. The output was
  deterministic on any IEEE-754 platform, but the contract requires
  **no `f32` or `f64` in rate calculation**.
- `v2` (current) replaces `sin()` with the integer triangle wave
  described above. Anchor values unchanged; algorithm corrected.

## 8. Multiple providers

The schema supports multiple providers for the same pair and date via
the unique `(pair, date, provider)` index. Lookups require an explicit
provider — there is no implicit priority mixing. Only the mock provider
is registered in PR004; real-provider selection is deferred.

## 9. Carry-forward and stale detection

Repository function `lookup_latest(source, target, requested_date,
max_age_days, provider)`:

1. Identity (`source == target`) is handled by the domain layer with
   `rate = 1` and `age_days = 0` — no DB access.
2. Build the canonical pair and remember the requested direction
   (`Direct` or `Inverse`).
3. Select the most recent row for `(pair, provider, rate_date <=
   requested_date)`.
4. If no row exists: `FxLookup::Unavailable { reason: RateMissing }`.
5. Compute `age_days = requested_date - rate_date`.
6. If `age_days > max_age_days`: `FxLookup::Unavailable { reason:
   RateStale, candidate_age_days: Some(age_days) }`.
7. Otherwise: return the canonical or inverse rate per the direction,
   with full provenance.

Default `max_age_days = 7` (constant
`DEFAULT_MAX_CARRY_DAYS`). This matches the MVP performance contract.

## 10. Lookup result vocabulary

Reasons for `FxLookup::Unavailable`:

- `rate_missing` — no row at all for this pair/provider on or before
  `requested_date`.
- `rate_stale` — a row exists but its `rate_date` is older than
  `max_age_days`.
- `provider_not_configured` — the requested provider is not registered.
- `unsupported_mock_currency` — the mock provider does not know one of
  the requested currencies (15+ currencies fall here in this PR; the
  10-currency set is the supported subset).

The future portfolio worker maps these to the
`daily_return_status = 'unavailable'` reason vocabulary of the
performance contract (`fx_rate_missing` for valuation-time FX failures
and for unsupported currencies; the carry-forward window is then the
single tuning knob).

## 11. Idempotence and correction behavior

`upsert_canonical_rate` and `upsert_bulk` distinguish three outcomes:

- `Inserted` — a new row was created.
- `Updated` — an existing `(pair, date, provider)` row had a different
  `canonical_rate`, `provider_as_of` or `dataset_version` and was
  updated. The future integration treats this as the signal that a full
  portfolio-history rebuild is required for every portfolio that uses
  the affected currency.
- `Unchanged` — the row matched byte-for-byte; no write occurred and
  `updated_at` was **not** touched.

A second run of the fill job over the same range with the same fixture
version therefore reports `inserted = 0, updated = 0` and only
`unchanged` counters change.

## 12. Job: fill and repair

`FillMissingFxHistoryCacheJob` (`kushim-market-data/src/jobs/fill_missing_fx_history_cache.rs`).

Inputs (via `Config` / environment, see section 14):

- `FX_HISTORY_PROVIDER` — only `mock` is registered.
- `FX_HISTORY_CURRENCIES` — optional comma-separated currency list. When
  unset, the default is the 10 supported mock currencies. Duplicates are
  removed; identity pairs are not persisted.
- `FX_HISTORY_DATE_FROM`, `FX_HISTORY_DATE_TO` — required for the fill
  job. Range ≤ 366 days.
- `FX_HISTORY_MAX_CARRY_DAYS` — default 7. Carried by the runtime config
  for future lookup-side reuse.
- `FX_HISTORY_CHUNK_DAYS` — default 366. Currently used only for batching
  guard rails.

Behavior:

1. Validate the date range and currency set.
2. For each canonical pair (45 pairs for the 10 default currencies):
   1. detect missing dates inside the range;
   2. ask the provider for each missing date — `NoQuoteForDate` (weekend)
      is silently skipped;
   3. re-fetch the rate for already-present dates to surface
      `Updated` outcomes when the dataset version or canonical rate has
      changed;
   4. atomically upsert the batch in a single PostgreSQL transaction
      (so either every rate for this pair lands or none does).
3. Report aggregated counters: `pairs_total`, `pairs_failed`,
   `dates_in_range`, `inserted`, `updated`, `unchanged`,
   `provider_no_quote`, `provider_errors`.
4. A non-zero exit is returned only when every pair fails (full
   provider outage). Partial pair failures isolate and are logged.

`MARKET_DATA_MODE = once | loop` is honored exactly like the existing
equity-price fill job. `loop` mode performs the same fill on each
interval, which doubles as the repair mode: gaps that appear later (for
any reason) are filled on the next pass without rewriting healthy rows.

## 13. CLI integration

`MARKET_DATA_JOB` accepts the new value
`fill_missing_fx_history_cache`. The job uses
`FX_HISTORY_PROVIDER`, `FX_HISTORY_CURRENCIES`, `FX_HISTORY_DATE_FROM`,
`FX_HISTORY_DATE_TO`, `FX_HISTORY_CHUNK_DAYS`, ignores Finnhub-specific
variables, and does not require secrets when running the mock provider.

Other equity-price jobs and Finnhub behavior are unchanged.

## 14. Environment variables

| Variable | Default | Notes |
| --- | --- | --- |
| `FX_HISTORY_PROVIDER` | `mock` | Only value registered in PR004. |
| `FX_HISTORY_CURRENCIES` | (unset → 10 mock currencies) | Comma-separated uppercase ISO codes. Invalid codes rejected at startup. |
| `FX_HISTORY_DATE_FROM` | (unset) | Required for `fill_missing_fx_history_cache`. `YYYY-MM-DD`. |
| `FX_HISTORY_DATE_TO` | (unset) | Required for `fill_missing_fx_history_cache`. Must be ≥ `FX_HISTORY_DATE_FROM`. |
| `FX_HISTORY_MAX_CARRY_DAYS` | `7` | Carry-forward tolerance for future lookup callers. |
| `FX_HISTORY_CHUNK_DAYS` | `366` | Internal batching guard. |

## 15. Migration and validator

- Upgrade file: `infra/postgres/upgrades/004_fx_rate_history_cache.sql`.
- Fresh bootstrap path: the same table is declared in
  `infra/postgres/init/001_init.sql` — a brand-new database starts
  already containing the table and applying migration 004 afterwards is
  a no-op.
- Validator: `scripts/test/validate-fx-rate-history-migration.ps1`.
  Pinned to immutable baseline
  `4d674d409a6bf560ec056fe8efcdf89741a83e13` (the head of `main`
  immediately before PR004 introduced migration 004).
- The validator: bootstraps the disposable database from baseline init
  SQL files, applies migration 004, verifies columns, CHECKs and indexes,
  verifies the STORED GENERATED inverse, applies migration 004 a second
  time to prove idempotence, tests negative cases (canonical ordering,
  non-positive rate, duplicate `(pair, date, provider)`), and tests
  positive cases (two providers coexisting), then drops the disposable
  database.

## 15bis. Local validation commands

Run from `kushim-market-data/`:

```powershell
cargo fmt --all --check
cargo clippy -p kushim-market-data --all-targets --all-features -- -D warnings

# DB-backed integration tests (FX repository, FX job, and pre-existing
# market-data tests) each open their own `PgPool`. The Postgres dev
# container can refuse new connections under the default cargo test
# parallelism (one pool per concurrent test × ~16-30 cores) and surface
# `PoolTimedOut`. A bounded thread count is required by the current
# local infrastructure:
cargo test `
  -p kushim-market-data `
  --all-features `
  --no-fail-fast `
  -- `
  --test-threads=4

# This bound is purely a test-infrastructure detail. It is NOT part of
# financial behaviour, it does NOT weaken coverage (every test still
# runs), and it does NOT affect production. Upgrading the shared
# test-pool architecture (e.g. a process-wide static pool with a higher
# `max_connections` against a service-scoped Postgres) is independent
# technical debt and is not in PR004 scope.

# CI uses its own dedicated Postgres service and is not constrained by
# the local pool ceiling; the GitHub Actions workflow does not need
# `--test-threads=4`.

cargo audit                         # plain — fails on pre-existing RUSTSEC-2023-0071 (see below)
cargo audit --ignore RUSTSEC-2023-0071   # repository-approved per AGENTS.md
```

`RUSTSEC-2023-0071` (Marvin attack on `rsa 0.9.10`) is reached through
`sqlx → sqlx-macros → sqlx-macros-core → sqlx-mysql → rsa`. It was
already present at the PR004 baseline and PR004 did not add the `mysql`
sqlx feature — only `rust_decimal`. The advisory remains the workspace's
documented accepted risk while sqlx 0.8.x's transitive
`sqlx-mysql` pulls `rsa 0.9.x`.

## 15ter. Test coverage breakdown

Counts produced by `cargo test -p kushim-market-data --all-features -- --list`:

| Category | Count |
| --- | --- |
| Domain FX tests (`domain::fx_rate::tests`) | 9 |
| Provider FX tests (`providers::mock_fx_history::tests`) | 18 |
| Repository FX tests (`repositories::fx_rate_history_cache::tests`) | 16 |
| FX job tests (`jobs::fill_missing_fx_history_cache::tests`) | 8 |
| **Total FX-related tests** | **51** |
| Total package tests (all modules) | 123 |

These counts are produced by `--list` and should be re-measured before
any commit message references them.

## 16. CI

A new job `fx-rate-history-migration` (display name "FX rate history
migration validation") is added to `.github/workflows/mvp-smoke.yml`.
It runs on every push / PR with PostgreSQL 16 as a service, invokes the
validator in CI mode, and uses the immutable baseline env var
`FX_RATE_HISTORY_BASE_REF`. The migration-003 job and every other
existing CI job are untouched.

## 17. Future integration boundary

The portfolio worker is **untouched** by PR004. Future integration must:

1. Replace the future portfolio-history rebuild's price lookup with an
   FX-aware lookup that calls `repositories::fx_rate_history_cache::lookup_latest`
   for every cross-currency position-day.
2. Translate the lookup outcome into the historical performance contract
   vocabulary:
   - `FxLookup::Available` → continue to compute the position's market
     value.
   - `FxLookup::Unavailable { reason: rate_missing | rate_stale | unsupported_mock_currency }`
     → set the per-position
     `(valuation_source = invested_cost_fallback, market_data_status = unsupported_currency)`
     and aggregate the day to `aggregate_valuation_status = partial_currency`,
     `daily_return_status = unavailable`,
     `daily_return_unavailable_reason = fx_rate_missing`.
3. React to `FxUpsertOutcome::Updated` by enqueueing a full
   portfolio-history rebuild for every portfolio that touches the
   affected currency. The exact mechanism is deferred (the existing
   `portfolio_refresh_requests` table does not yet carry a
   currency-targeted invalidation payload).
4. Eventually replace the mock provider with a real provider
   integration. The trait `providers::fx_history_provider::FxHistoryProvider`
   defines the contract any future provider must satisfy.

The "first conversion need" trigger (auto-fill the cache when a
portfolio first needs a currency pair) is deferred: it requires either a
worker→market-data signal or an API-side hook, both of which are out of
PR004's scope. PR004 only delivers the explicit-range fill capability;
the caller integration is documented here and tracked in
[../mvp/deferred-todos.md](../mvp/deferred-todos.md).

## 18. Limitations

- **No real FX provider** is integrated. Only the deterministic mock is
  registered.
- **No real provider has been selected.** Provider selection is a
  separate decision and is tracked under "Market-data" /
  "FX rate provider selection and integration" in
  [../mvp/deferred-todos.md](../mvp/deferred-todos.md).
- **No first-conversion cross-service trigger.** The portfolio worker
  does not yet ask market-data to fill missing pairs; FX fills are
  explicit-range jobs run via configuration.
- **No automatic portfolio rebuild after FX correction.** PR004 exposes
  the `FxUpsertOutcome::Updated` signal but does not route it.
- **No historical performance read model, API or frontend.** Those are
  later PRs.
- **No adjusted/corporate-action FX concept** — FX feeds do not need
  one, but if any future provider exposes adjusted vs raw rates this
  contract may need to record which kind is persisted.
- **No Notion dependency** is introduced anywhere.

These limitations are deliberate. They do not invalidate the foundation;
they are the boundaries between PR004 and later PRs.
