# kushim-app

Private authenticated frontend for Kushim.

## Stack

- React 19
- Vite 8
- TypeScript 6
- React Router 6
- Zustand 5
- Tailwind CSS 4
- Recharts 3

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `VITE_AUTH_URL` | `http://localhost:3001` | Auth frontend (login/signup redirect) |
| `VITE_AUTH_API_URL` | `http://localhost:3002` | Auth API (handoff exchange, `/auth/me`, `/auth/refresh`, `/auth/logout`) |
| `VITE_API_URL` | `http://localhost:8080` | Business API (`kushim-api`) |

## Token storage

Canonical localStorage keys:

- `kushim_access_token` — JWT access token (15 min TTL)
- `kushim_refresh_token` — JWT refresh token (30 day TTL, rotated on each use)

Legacy key `kushim_token` is read for migration fallback and removed on next token write.

**Security caveat:** localStorage is an MVP-only choice. It is readable by any
script that runs in the page (including injected ones) and survives across
tabs. Production-grade browser session security (HttpOnly cookies + CSRF
defence, or worker-isolated storage) is not in scope for this MVP. This
limitation is tracked in `documentation/mvp/deferred-todos.md` (Frontend).

## Session layer (P0.3)

See [Authentication session lifecycle](../documentation/operations/auth-session-lifecycle.md) for the end-to-end backend issuance, reactive refresh, rotation, logout, storage, and limitation contract.

Layering (no circular imports):

1. `src/lib/api/httpClient.ts` — raw `apiRequest`; no interception.
2. `src/lib/api/tokenStorage.ts` — single source of truth for the localStorage
   keys. `readAccessToken / readRefreshToken / writeTokens / clearStoredTokens`.
3. `src/lib/api/sessionGate.ts` — refresh coordinator.
   - Single-flight: N concurrent 401s share **one** `POST /auth/refresh`.
   - Session generation: bumped on `setTokens`, `clearSession`. A refresh
     response that lands on an obsolete generation is discarded — a logout
     mid-refresh cannot silently recreate a session.
   - `onTokensRotated(accessToken)` notifies the auth store so
     `useAuthStore.getState().token` mirrors the rotated value immediately.
4. `src/lib/api/authenticatedRequest.ts` — wraps `apiRequest` with the
   **retry-at-most-once** rule:
   - request once → on 401, await the gate's single-flight refresh → retry
     **exactly once** with the rotated token.
   - second 401 → `clearSession("retry_unauthorized")` and the original error
     propagates. **No infinite loop.**
5. `src/lib/api/authApi.ts` — public endpoints only (login / signup / handoff /
   refresh / logout) using raw `apiRequest`. **Never** routed through the
   authenticated wrapper; the refresh endpoint must not recursively try to
   refresh itself.
6. `src/lib/api/businessApi.ts` — every authenticated business endpoint goes
   through `authenticatedRequest`. The historic `accessToken` parameter is
   accepted and ignored; the wrapper is the single bearer-token authority.

### Logout race protection

The session gate stamps each refresh with the generation that was current
when refresh started. If `clearSession("logout")` runs while a refresh is
in flight:

- the generation is bumped → the late refresh response's
  `if (generation !== startedAtGeneration) return null` guard discards it;
- `writeTokens` and `onTokensRotated` are skipped;
- callers awaiting that refresh receive `null` and propagate the original
  `ApiRequestError`. The user remains logged out.

Test coverage: `src/lib/api/sessionGate.test.ts` —
`logout during in-flight refresh prevents late re-authentication` and
`onTokensRotated does not fire when logout races the refresh response`.

## Refresh-tracking persistence (P0.3)

Active automatic portfolio-refresh requests survive a full-page reload.

- **sessionStorage key:** `kushim_active_portfolio_refresh`
- **Payload (only):**
  ```json
  { "portfolioId": "<uuid>", "refreshRequestId": "<uuid>", "startedAt": 1234567890 }
  ```
  Tokens, raw worker errors (`last_error`), and financial values are
  **never** persisted.
- **Polling budget:** 40 polls × 1.5 s = **60 s** active wait per cycle.
- **Recovery TTL:** **15 minutes** — entries older than this are discarded
  on read, slot cleared. Defined in
  `src/lib/api/refreshTrackingStorage.ts::REFRESH_TRACKING_RECOVERY_TTL_MS`.
- **Cleared on:** `completed` / `failed` / 404 ownership / `logout` /
  TTL expiry.
- **Kept on:** `timed_out` (frontend stopped waiting; the worker may still
  finish — one F5 within the TTL resumes polling).
- **Resume hook:** `routes.tsx::RequireAuth` calls
  `useRefreshTrackingStore.getState().resumeFromStorage()` once after
  successful session validation. Idempotent under React Strict Mode (returns
  early if the store already tracks the persisted ID).

## Test suite

```
cd kushim-app
npm run test
```

Vitest (jsdom) covers the session/refresh critical surface:

| File | Tests |
|---|---|
| `src/lib/api/sessionGate.test.ts` | 13 — single-flight, retry, logout race, header preservation, `onTokensRotated`. |
| `src/lib/api/refreshTrackingStorage.test.ts` | 6 — serialization, TTL boundary, malformed payload discard. |
| `src/stores/refreshTracking.test.ts` | 11 — track persists IDs, completed clears + reloads once, failed clears safely, 404 ownership clears, reset cancels timers, resume restarts + idempotent, stale-generation discard, expired TTL no-op, `timed_out` retains. |
| `src/stores/auth.test.ts` | 2 — Zustand token mirrors rotated value after refresh; failed refresh clears Zustand token. |

## Authentication flow

1. User logs in via `kushim-auth/front` at `VITE_AUTH_URL`.
2. Auth frontend generates a one-time handoff code via Redis.
3. `kushim-app` receives `?handoff_code=...` at root route.
4. App exchanges the code via `POST /auth/handoff/exchange` → receives `access_token` + `refresh_token`.
5. Tokens are stored in localStorage.
6. App calls `GET /auth/me` to validate the session and load user profile.
7. On 401: refresh is attempted via `POST /auth/refresh`; on failure → logout + redirect.

## Session validation

On every app load (including page refresh), the access token is validated by calling `GET /auth/me`.
The app does not rely solely on token presence in localStorage.
A loading state ("Vérification de la session…") is shown during validation.

## Logout

1. `POST /auth/logout` is called with the refresh token to revoke it server-side.
2. All localStorage tokens are cleared regardless of the API call result.
3. User state is reset.
4. Redirect to auth login page.

## Business API smoke test

On successful session validation, the app makes a non-blocking call to `GET /v1/me` on `kushim-api`.
This verifies JWT compatibility between auth and business services.
Result is logged to the browser console.

## Portfolio integration (Pass 2)

After session validation, the dashboard fetches `GET /v1/portfolios`.

**API endpoints used:**

| Endpoint | Method | Purpose |
|---|---|---|
| `/v1/portfolios` | GET | List user portfolios |
| `/v1/portfolios` | POST | Create a new portfolio |
| `/v1/portfolios/{id}` | GET | Get a single portfolio |

**State management:** `src/stores/portfolio.ts` (Zustand)

| Key | Value |
|---|---|
| `portfolios` | Array of portfolio objects |
| `activePortfolioId` | UUID of the selected portfolio |
| `status` | `idle` / `loading` / `success` / `error` |

**localStorage key:** `kushim_active_portfolio_id` — persists the selected portfolio across refreshes.
Falls back to the first portfolio if the persisted ID no longer exists.

**Empty state:** If the user has no portfolios, the dashboard shows a create-portfolio CTA.

**Create portfolio flow:**

1. User picks the base currency from `CurrencySelect` (defaults to EUR).
2. `POST /v1/portfolios` with `{ name, base_currency, visibility: "private" }`.
3. On success: portfolio is added to state and set as active.
4. Validation errors from the backend are mapped to safe French messages
   (`unsupported_currency` → "Cette devise n'est pas prise en charge.").

**Logout:** Portfolio state is reset when the user logs out.

## Automatic portfolio refresh (P0)

The normal `CreateOperationModal` flow creates a **posted** operation
(`operation_status: "posted"`). The API write returns a `refresh_request`
alongside the operation. The frontend then tracks the asynchronous worker
refresh via `src/stores/refreshTracking.ts`:

- bounded, non-overlapping polling of
  `GET /v1/portfolios/{id}/refresh-requests/{id}` (generation guard ignores
  stale responses from a previous request/portfolio);
- a compact non-blocking notice (`src/app/components/RefreshNotice.tsx`) shows
  `pending` / `processing` / `completed` / `failed` / `timed_out` on the
  Dashboard and Transactions pages, following the glassmorphism style;
- on `completed`, summary / holdings / snapshots / operations reload via
  `portfolioReadModels.reloadAll` (which preserves `lastHoldingsQuery` /
  `lastSnapshotsQuery`) and `operations.reloadOperations`, with no full page
  reload and no placeholder flashing;
- a failed refresh is never presented as a failed financial operation (the
  posted operation is recorded regardless); raw worker errors are never shown;
- logout clears refresh tracking and its timers.

## Durable operation idempotency (P3)

Every call to `POST /v1/portfolios/{id}/operations` and to
`POST /v1/portfolios/{id}/operations/{op}/corrections` carries a required
`Idempotency-Key: <UUID>` header. The key lifecycle is owned by the UI
layer (`CreateOperationModal`), not by `businessApi` or
`authenticatedRequest`:

- one logical submission attempt = one UUID generated with
  `crypto.randomUUID()`
- the same UUID is reused when the user retries the SAME payload after an
  ambiguous network/server failure → the backend replays the original
  write instead of creating a duplicate ledger row
- the UUID rotates as soon as the user edits the payload materially
  (amount, asset, currency, FX rate, etc.)
- the UUID is cleared on confirmed success and on modal close/reset, so a
  successful key is never reused for a new operation
- `authenticatedRequest` preserves caller-provided `Idempotency-Key`
  headers across the single 401 → refresh → retry path (regression test:
  `sessionGate.test.ts`)
- backend error codes mapped to safe French messages:
  - `missing_idempotency_key` / `invalid_idempotency_key`
    → "La requête ne peut pas être sécurisée. Veuillez réessayer."
  - `idempotency_key_conflict`
    → "Cette tentative correspond à une opération différente. Vérifiez
       les données puis recommencez."

Browser storage (localStorage / sessionStorage) is intentionally NOT used
for the idempotency key in P3: a key survives only for the duration of an
in-flight submission attempt within the same modal instance.

## Operations integration (Pass 3)

After portfolio selection, the Transactions page and Dashboard load operations via `GET /v1/portfolios/{id}/operations`.

**API endpoints used:**

| Endpoint | Method | Purpose |
|---|---|---|
| `/v1/portfolios/{id}/operations` | GET | List portfolio operations |
| `/v1/portfolios/{id}/operations` | POST | Create a new operation |
| `/v1/portfolios/{id}/operations/{opId}` | GET | Get a single operation |
| `/v1/reference/operation-types` | GET | List valid operation types |
| `/v1/reference/operation-statuses` | GET | List valid operation statuses |
| `/v1/reference/currencies` | GET | Canonical currency catalogue (P1) — single source of truth used by `CurrencySelect` and by backend validation. Access-token only. |

**State management:** `src/stores/operations.ts` (Zustand)

| Key | Value |
|---|---|
| `operations` | Array of `PortfolioOperation` objects |
| `status` | `idle` / `loading` / `success` / `error` |
| `operationTypes` | Reference list from `/v1/reference/operation-types` |
| `operationStatuses` | Reference list from `/v1/reference/operation-statuses` |

**DTO mapping:** `src/lib/operations.ts` — converts backend `PortfolioOperation` to `TransactionRow` for UI display. Includes French labels for all 14 operation types and 3 statuses.

**Create operation flow (cash-only):**

1. User selects type (deposit, withdrawal, fee, tax, interest, transfer_in, transfer_out).
2. Fills amount, date, currency, optional note.
3. `POST /v1/portfolios/{id}/operations` with `{ operation_type, executed_at, currency, gross_amount_minor, ... }`.
4. On success: operation is prepended to state.
5. Asset-based operations (buy, sell, dividend, split) are deferred — they need an asset picker.

**Transactions page features:**

- Search by type/note
- Filter by operation type, status, and period
- Metric cards (purchases, sales, deposits, withdrawals, dividends, fees)
- Client-side pagination (15 rows/page)
- Loading, empty, error, and no-portfolio states

**Logout:** Operations state is reset alongside portfolio state.

## Asset selector and asset-linked operations (Pass 4)

**API endpoints used:**

| Endpoint | Method | Purpose |
|---|---|---|
| `/v1/assets` | GET | Search/list assets (search, asset_class, ticker, isin, exchange, status, limit, offset) |
| `/v1/assets/{id}` | GET | Get single asset detail |

**Asset search component:** `src/app/components/AssetSearchSelect.tsx`

- Debounced text search (300ms) on name, ticker, ISIN, aliases
- Filters to `status=active` only
- Shows ticker, name, asset class, exchange
- Empty state if no assets seeded in backend
- Click-outside to dismiss dropdown

**Shared operation modal:** `src/app/components/CreateOperationModal.tsx`

- Extracted from Transactions page inline modal
- Used by both Transactions page and Dashboard "Ajouter une transaction" action
- Operation types grouped: asset operations (buy, sell, dividend) and cash operations (deposit, withdrawal, fee, tax, interest, transfer_in, transfer_out)

**Supported operation types after Pass 4:**

| Type | Category | Required fields |
|---|---|---|
| buy | Asset | id_asset, quantity, price_minor, gross_amount_minor, cash_amount_minor |
| sell | Asset | id_asset, quantity, price_minor, gross_amount_minor, cash_amount_minor |
| dividend | Asset | id_asset, gross_amount_minor, cash_amount_minor |
| deposit | Cash | gross_amount_minor, cash_amount_minor |
| withdrawal | Cash | gross_amount_minor, cash_amount_minor |
| fee | Cash | gross_amount_minor, cash_amount_minor |
| tax | Cash | gross_amount_minor, cash_amount_minor |
| interest | Cash | gross_amount_minor, cash_amount_minor |
| transfer_in | Cash | — |
| transfer_out | Cash | — |

**Deferred operation types:** split, spin_off, symbol_change, adjustment (require complex UX).

## Currency contract and manual FX (P1)

### `CurrencySelect`

`src/app/components/CurrencySelect.tsx` is the single user-facing currency
picker. It feeds from `GET /v1/reference/currencies` (canonical ISO 4217
catalogue maintained in `kushim-api/src/domain/currency.rs`). No currency list
is duplicated client-side.

- Searchable by code (`EUR`, `USD`, …) and by localized French label
  (`Intl.DisplayNames("fr", "currency")` with a fallback on the backend label,
  then the code itself).
- Keyboard usable: `ArrowDown`/`ArrowUp` navigate, `Enter` selects, `Escape`
  closes, click outside closes.
- Controlled value: always a canonical uppercase three-letter code. The
  component never exposes a free-text input as a value.
- Loading state ("Chargement…"), API error state ("Impossible de charger la
  liste des devises."), no-results state ("Aucune devise trouvée.").
- A small in-memory cache shares the fetched catalogue between every
  instance for the duration of the page session.

### Default currencies

- `CreatePortfolioModal` defaults the base currency to **EUR**.
- `CreateOperationModal` defaults the operation currency to the **active
  portfolio's base currency**.

### Manual FX field (CreateOperationModal)

Direction is fixed and explicit:

> **`1 unit of operation currency = fx_rate_to_portfolio units of portfolio
> base currency`**.

The field is rendered (and the rate is required at submit time) when **all**
of the following are true:

1. the selected operation currency differs from the portfolio's base
   currency;
2. the operation type is not structurally zero-cash (the backend forces
   `cash_amount_minor = 0` for `split` / `spin_off` / `symbol_change`);
3. the previewed submitted monetary leg is positive
   (`cash_amount_minor > 0` or `gross_amount_minor > 0`).

The rule is grounded in the actual monetary leg, **not** in an
operation-type allowlist. This matches the worker contract: the worker
applies `converted_cash` to `cash_amount_minor` for every type, including
`transfer_in` and `transfer_out`. Consequence: a positive-cash
`transfer_in` / `transfer_out` in a foreign currency exposes the FX
field and requires a rate, exactly like a `deposit` / `withdrawal`.

State hygiene:

- switching the selected currency back to the portfolio base currency
  clears the FX field immediately, so a previously typed rate cannot leak
  into a same-currency submission;
- if the monetary leg becomes zero before submit, the FX field stops being
  required and the field's value is excluded from the payload — no stale
  rate is ever sent;
- the rate is submitted as the user's original string
  (`fx_rate_to_portfolio: "0.92"`), with no binary-float reformatting.

### Error mapping

Backend P1 error codes are mapped to safe French user-facing messages
(`mapBackendErrorToFrench`):

| Code | Message |
|---|---|
| `unsupported_cross_currency` | « Le taux de change est requis lorsque la devise de l'opération diffère de la devise de base du portefeuille. » |
| `unsupported_currency` | « Cette devise n'est pas prise en charge. » |
| `invalid_fx_rate_to_portfolio` | « Le taux de change doit être un nombre positif. » |

Backend validation remains authoritative — client-side guards exist only
to give the user immediate feedback.

### No automatic FX provider in P1

P1 explicitly does not integrate any FX provider. Cross-currency posted
monetary operations require a user-supplied rate. Provider selection and
historical-restatement policy remain tracked in
`documentation/mvp/deferred-todos.md` (Market-data / Still deferred).

**Asset display in Transactions table (P2):** Every operation response now
embeds a compact `asset` / `related_asset` reference (`{id_asset, name,
ticker, status}` or `null` for cash-only operations). `operationAssetLabel`
prefers the ticker, then the name, then "—" for cash, and falls back to a
truncated UUID only when the backend returned an `id_asset` it could not
resolve (legacy/corrupt data). The previous module-level
`assetDisplayCache` / `hydrateAssetDisplayCache` path has been deleted, so
labels survive a full reload without any per-row `GET /v1/assets/{id}` call.

**Asset seed dependency:** Asset-linked operations (buy/sell/dividend) require assets to be seeded in the database. If no assets exist, the asset selector shows "Aucun actif disponible."

## Asset catalogue (Pass 7)

**API endpoints used:**

| Endpoint | Method | Purpose |
|---|---|---|
| `/v1/assets` | GET | List/search asset catalogue |
| `/v1/assets/{id}` | GET | Asset detail |

**State management:** `src/stores/assets.ts` (Zustand)

**Assets page (`/assets`):**

- Real data from `GET /v1/assets`
- Search by name, ticker, ISIN
- Filter by asset class and status
- Pagination via "load more" button
- Loading, empty, error states
- This is a catalogue of instruments, not user portfolio positions

**Asset detail page (`/assets/:id`):**

- Real data from `GET /v1/assets/{id}`
- Identity section: name, ticker, symbol, ISIN, class, exchange, currency, status
- Market data section: current price, source, timestamp
- Metadata section: sector, industry, country, website (if present)
- Aliases section (if present)
- Loading, error, not-found states

**Terminology / routing:**

- User-facing routes are in English: `/assets`, `/assets/:id`
- UI labels remain French: "Catalogue d'actifs", "Détail de l'actif"
- `/actifs` and `/actifs/:id` redirect to `/assets` and `/assets/:id`
- `/positions` is reserved for future user portfolio positions page
- `/holding` and `/holdings` are not used as user-facing routes
- The assets catalogue must not be confused with user holdings/positions

## Portfolio positions (Pass 8)

**API endpoint used:**

| Endpoint | Method | Purpose |
|---|---|---|
| `/v1/portfolios/{id}/holdings` | GET | List current portfolio positions |

**State management:** Reuses `src/stores/portfolioReadModels.ts` (Zustand) — `loadHoldings()` method.

**Positions page (`/positions`):**

- Real data from `GET /v1/portfolios/{id}/holdings`
- Summary cards: position count, total market value, total P&L
- Table: asset name/ticker/exchange, class, quantity, avg cost, market value, P&L (% + amount), weight
- Search by asset name/ticker
- Filter by asset class
- Sort: weight desc, value desc, name asc
- Click a position → navigates to `/assets/{id_asset}` (catalogue detail)
- Estimated data badge when `is_estimated=true`
- Pagination: initial load of 25 positions, "Charger plus de positions" button when more available
- Quantity formatting: trailing zeros removed (`8.0000000000` → `8`), French locale
- Currency: derived from holdings data with portfolio fallback — consistent across summary cards and table rows

**Data states:**

| State | Behavior |
|---|---|
| No active portfolio | "Sélectionnez ou créez un portefeuille" |
| Loading | Spinner "Chargement des positions…" |
| API error | Error card with message |
| `data_available=false` + `read_model_missing` | "Positions en préparation" — no fake data |
| Empty holdings | "Aucune position pour le moment" |
| Estimated values | "Est." badge per row + summary card |

**Terminology / routing:**

- User-facing route: `/positions`
- UI label: "Positions" (not "Holdings")
- Backend API uses `holdings` internally — this is intentional
- `/holding` and `/holdings` are NOT user-facing routes
- `/positions` shows assets held in the active portfolio
- `/assets` remains the instrument catalogue

**Data dependency:**

- Requires worker/read-model generation (`kushim-worker`)
- Without read models: `data_available=false`, `reason=read_model_missing`

## Routes

- `/` — Handoff exchange (OAuth callback)
- `/dashboard` — Portfolio dashboard (auth required)
- `/assets` — Asset catalogue (auth required)
- `/assets/:id` — Asset detail (auth required)
- `/positions` — Portfolio positions (auth required)
- `/actifs` — Redirects to `/assets`
- `/actifs/:id` — Redirects to `/assets/:id`
- `/transactions` — Transaction history (auth required)
- `/parametres` — Settings (auth required)

## Local run

```powershell
cd E:\Kushim\kushim-app
copy .env.example .env
npm install
npm run dev
```

## Validation

```powershell
npm run lint
npm run build
```

## Current status

- Session validation via `/auth/me`: **implemented**
- Real user profile in Navbar and Settings: **implemented**
- Backend logout with token revocation: **implemented**
- Token refresh on 401: **implemented**
- Business API `/v1/me` smoke test: **implemented** (non-blocking, console-only)
- `kushim-api` CORS: **implemented** (runtime-validated)
- Portfolio list/create/select: **implemented** (Pass 2)
- Portfolio empty state + create flow: **implemented** (Pass 2)
- Active portfolio persistence: **implemented** (localStorage)
- Operations list/create: **implemented** (Pass 3)
- Transactions page (real operations): **implemented** (Pass 3)
- Dashboard recent transactions (real operations): **implemented** (Pass 3)
- Create operation modal (shared, all types): **implemented** (Pass 4)
- Asset search/select component: **implemented** (Pass 4)
- Asset-linked operations (buy/sell/dividend): **implemented** (Pass 4)
- Dashboard add transaction uses real modal: **implemented** (Pass 4)
- Operation DTO mapping + French labels: **implemented** (Pass 3+4)
- Operation asset identity embedded in every operation response (no per-row asset fetch): **implemented and validated** (P2)
- Dashboard KPIs (valeur nette, investi, gain/perte): **connected** to `/summary` read model (Pass 5)
- Dashboard evolution chart: **connected** to `/snapshots/daily` (Pass 5)
- Dashboard allocation (pie chart): **connected** to `/holdings` read model, derived by asset class (Pass 5)
- Dashboard top 5 assets: **connected** to `/holdings` read model (Pass 5)
- Dashboard benchmark: **mock** (demo data), clearly labeled as simulated
- Dashboard allocation stats (open positions, best/worst perf): **connected** to `/holdings` read model (Pass 5b)
- Dashboard notice banner: **accurate** (states that benchmark remains demo; all other blocks use real data)
- Dashboard quick action "Catalogue d'actifs": **routes to `/assets`** instead of opening the old placeholder add-asset modal
- Asset display hydration in Transactions: **implemented** (Pass 5b) — on load, missing asset labels are resolved via `GET /v1/assets/{id}`, best-effort and non-blocking
- Asset catalogue (`/assets`): **implemented** (real data, search, filters, pagination) (Pass 7)
- Asset detail (`/assets/:id`): **implemented** (real data, identity, market data, metadata, aliases) (Pass 7)
- English routes with French UI labels: **implemented** (`/actifs` redirects to `/assets`) (Pass 7)
- Assets Zustand store: **implemented** (`src/stores/assets.ts`) (Pass 7)
- Portfolio positions (`/positions`): **implemented** (real holdings data, search, filters, sort, estimated badge, pagination, quantity formatting, currency consistency) (Pass 8 + 8b)
- Positions → AssetDetail navigation: **implemented** (click row → `/assets/:id`) (Pass 8)
- Settings preference save, password update, and account deletion actions: **disabled/labeled as not included in the MVP demo**
- Complex operations (split/spin_off/symbol_change/adjustment): **deferred**
- Read model unavailable states (`data_available=false`, `read_model_missing`, `snapshot_missing`): **implemented** (Pass 5)
