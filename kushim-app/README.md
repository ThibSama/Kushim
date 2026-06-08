# kushim-app

Private authenticated frontend for Kushim.

## Stack

- React
- Vite
- TypeScript
- React Router
- Zustand
- Tailwind

## Current status

Status:

- **Implemented visually**
- **Partially wired**

What currently exists:

- dashboard UI
- assets pages
- asset detail page
- transactions page
- settings page
- local token storage in the browser

Current limitation:

- the app still uses local mock portfolio data for key portfolio views
- it is not yet fully wired to `kushim-api`

## Routes

- `/dashboard`
- `/actifs`
- `/actifs/:id`
- `/transactions`
- `/parametres`

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

## MVP note

This frontend should currently be understood as:

- a strong UI shell for the authenticated app
- not yet the final integrated portfolio client

The next important step is wiring it to:

- `kushim-auth/front` for real auth flows
- `kushim-api` for real portfolio, holdings, and snapshot data
