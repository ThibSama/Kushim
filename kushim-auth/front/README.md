# kushim-auth/front

Frontend authentication UI for Kushim.

## Stack

- Next.js
- TypeScript
- Tailwind

## Current status

Status:

- **Interactive frontend scaffold**
- **Not fully wired to the backend yet**

What currently exists:

- login page
- signup page
- recovery pages
- UX and visual shell aligned with the Kushim auth experience

What is still missing:

- real integration with `kushim-auth/api`
- real token storage/session flow
- real recovery workflow against the backend

Important current behavior:

- some flows still simulate success and redirect toward the app with demo behavior

## Routes

- `/connexion`
- `/inscription`
- `/recuperation`
- `/recuperation/confirmation`

## Local run

```powershell
cd E:\Kushim\kushim-auth\front
copy .env.local.example .env.local
npm install
npm run dev
```

## Validation

```powershell
npm run lint
npm run build
```

## MVP note

This frontend should not yet be described as a complete auth client.

It is currently:

- a visually usable auth shell
- awaiting full integration with `kushim-auth/api`
