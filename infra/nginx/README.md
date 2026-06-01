# infra/nginx

Local nginx reverse proxy.

Routes:

- `kushim.localhost` -> `kushim-website:3000`
- `app.kushim.localhost` -> `kushim-app:5173`
- `auth.kushim.localhost` -> `kushim-auth-front:3001`
- `api.kushim.localhost` -> `kushim-api:8080`

Only nginx is exposed publicly by `docker-compose.yml`.
