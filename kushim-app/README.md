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

1. User fills name + base currency (3-letter uppercase, e.g. EUR).
2. `POST /v1/portfolios` with `{ name, base_currency, visibility: "private" }`.
3. On success: portfolio is added to state and set as active.
4. Validation errors from the backend are displayed in the modal.

**Logout:** Portfolio state is reset when the user logs out.

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

**Asset display in Transactions table:** Operations show asset ticker when available via local display cache. Falls back to truncated UUID if asset was created in a prior session.

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
- Operation DTO mapping + French labels + asset display cache: **implemented** (Pass 3+4)
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
