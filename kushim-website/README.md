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

## Public URL and temporary routes

`NEXT_PUBLIC_SITE_URL` defines the canonical public origin used for metadata,
`robots.txt`, and `sitemap.xml`. It falls back to `http://localhost:3000` for
local development and must be set to the deployed HTTPS origin in production.
Malformed configured values fail the build.

`/sitemap` is the temporary, human-facing “Plan du site” page. `/sitemap.xml`
is the machine-readable SEO sitemap and currently lists only the homepage.
Footer content routes are temporary placeholders and are marked `noindex` until
their final content is available.

## Scope note

This website is not the authenticated application.

It presents Kushim publicly and routes users toward the auth and app surfaces.
