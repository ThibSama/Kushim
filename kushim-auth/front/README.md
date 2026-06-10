# kushim-auth/front

Frontend authentication UI for Kushim.

## Stack

- Next.js 16
- React 19
- TypeScript
- Tailwind CSS 4

## Current status

- **Wired to `kushim-auth/api`** for signup, login, logout, and password recovery
- Token storage in localStorage (local MVP only)
- **i18n**: French and English, client-side dictionary system

### Implemented

- Login form (`username` + `password`) calls `POST /auth/login`
- Signup form (`username` + `password`) calls `POST /auth/signup` (`username` = unique handle-formatted identifier)
- Recovery phrase setup after signup: frontend generates a 12-word phrase, displays it, and registers it via `POST /auth/recovery/setup`
- Recovery/reset form (`username` + `recovery_phrase` + `new_password`) calls `POST /auth/recovery/reset-password` with automatic phrase rotation
- New recovery phrase displayed after successful password reset (old phrase invalidated)
- Tokens stored via `src/lib/auth-storage.ts`
- API client centralized in `src/lib/auth-api.ts`
- Client-side validation aligned with backend rules (username handle format 3-40 chars, password 12-128 chars)
- API error display (invalid credentials, conflict, rate limiting, network errors)
- Loading states on form submission
- Redirect to `kushim-app` after successful login/signup
- i18n system with French/English dictionaries and language toggle
- Recovery phrase textarea with 12-word client-side validation and word counter
- 12-word recovery phrase generation using a curated English word list with `crypto.getRandomValues`

### Not yet implemented

- Automatic token refresh on expiry
- `/auth/me` session check on page load
- Logout UI flow (no logout button in the auth frontend shell)
- httpOnly cookie-based token storage (production security)

## i18n

The auth frontend uses a lightweight client-side i18n system (no external dependency).

| Detail | Value |
|---|---|
| Supported languages | French (`fr`), English (`en`) |
| Default language | French (`fr`) |
| Storage key | `kushim.locale` (localStorage) |
| Dictionary location | `src/i18n/fr.ts`, `src/i18n/en.ts` |
| Context provider | `src/i18n/context.tsx` |
| Hook | `useI18n()` returns `{ locale, t, setLocale, toggleLocale }` |

The language toggle is the button in the navbar showing the current locale code (FR/EN). Clicking it switches between French and English. The preference persists across page reloads via localStorage.

All user-facing text (labels, placeholders, errors, helper text, nav, footer) is served from the dictionaries. Static metadata (page titles, OpenGraph) remains in French as it is server-rendered.

## Recovery phrase UX

The recovery page uses a textarea instead of a one-line input for the 12-word recovery phrase:

- Textarea with multi-line support
- Live word counter (e.g. `8/12`) visible when typing
- Client-side validation: blocks submission if word count is not exactly 12
- Whitespace is normalized before sending (trim + collapse multiple spaces)
- Helper text explains the expected format
- User casing is preserved (no automatic lowercasing)

## Routes

- `/connexion` — login
- `/inscription` — signup
- `/recuperation` — password reset via recovery phrase
- `/recuperation/confirmation` — static confirmation page

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `NEXT_PUBLIC_APP_URL` | `http://localhost:5173` | URL of the main Kushim app (redirect after auth) |
| `NEXT_PUBLIC_WEBSITE_URL` | `http://localhost:3000` | URL of the Kushim landing website |
| `NEXT_PUBLIC_AUTH_API_URL` | `http://localhost:3002` | URL of `kushim-auth/api` |

## Backend dependency

Requires `kushim-auth/api` running on port 3002 (or the URL set in `NEXT_PUBLIC_AUTH_API_URL`).

Start the backend:

```powershell
docker compose up -d kushim-auth-api database redis
```

## Local run

```powershell
cd kushim-auth/front
copy .env.local.example .env.local
npm install
npm run dev
```

## Validation

```powershell
npm run lint
npm run build
```

## Token storage limitation

Tokens are stored in `localStorage` for local MVP convenience. This is **not production-grade**. A production implementation should use httpOnly cookies set by the backend, or an equivalent secure mechanism.
