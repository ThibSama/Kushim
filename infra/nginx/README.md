# infra/nginx

Local nginx reverse proxy.

Routes:

- `kushim.localhost` -> `kushim-website:3000`
- `app.kushim.localhost` -> `kushim-app:5173`
- `auth.kushim.localhost` -> `kushim-auth-front:3001`
- `auth-api.kushim.localhost` -> `kushim-auth-api:3002`
- `api.kushim.localhost` -> `kushim-api:8080`

Notes:

- The `app.kushim.localhost` host forwards the websocket upgrade so the Vite dev
  server's HMR works through nginx. The Vite dev server serves the SPA history
  fallback itself, so client-side routes (`/dashboard`, `/assets/{id}`, …) work
  on direct refresh without extra `try_files`.
- The Next.js hosts (`kushim.localhost`, `auth.kushim.localhost`) serve their
  own routes (`/connexion`, `/inscription`, `/recuperation`) and handle refresh.
- Browser CORS origins are the nginx hosts; each API's allowlist is set in
  `docker-compose.yml` (`CORS_ALLOWED_ORIGINS`).

Only nginx is exposed publicly by `docker-compose.yml` (port 80). The auth API
also publishes `3002` for direct-dev convenience; the browser uses
`auth-api.kushim.localhost` in Docker.
