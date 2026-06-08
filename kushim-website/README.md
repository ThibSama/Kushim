# kushim-website

Public marketing website for Kushim.

## Stack

- Next.js
- TypeScript
- Tailwind

## Current status

Status:

- **Implemented for marketing/landing use**

What currently exists:

- landing page
- product presentation
- navigation toward auth

This service does not own business logic.

## Local run

```powershell
cd E:\Kushim\kushim-website
copy .env.local.example .env.local
npm install
npm run dev
```

## Validation

```powershell
npm run lint
npm run build
```

## Scope note

This website is not the authenticated application.

It presents Kushim publicly and routes users toward the auth and app surfaces.
