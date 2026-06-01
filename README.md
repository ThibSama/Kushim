# Kushim

Kushim is scaffolded as independent services under `E:/Kushim/`. There is no monorepo tool; each frontend and Rust service owns its own dependencies, build files, README, and environment example.

```txt
E:/Kushim/
├── kushim-website/       # Next.js public marketing site
├── kushim-auth/
│   ├── front/            # Next.js auth UI
│   └── api/              # Rust + Axum auth stubs
├── kushim-app/           # React + Vite private app
├── kushim-api/           # Rust + Axum + sqlx + redis-rs API stubs
├── kushim-market-data/   # Rust internal service scaffold
├── kushim-worker/        # Rust internal worker scaffold
└── infra/
    ├── nginx/
    ├── postgres/
    └── redis/
```

## Source Notes

- Mockup preview inspected: `https://kushimmaquette.vercel.app/`.
- Mockup repository inspected: `https://github.com/ThibSama/Kushimmake`.
- Visual surfaces reproduced from the mockup: auth screens and private app screens. The public website is a clean scaffold as requested.

## Services

### `kushim-website`

Next.js + TypeScript public marketing scaffold on port `3000`.

```powershell
cd E:/Kushim/kushim-website
copy .env.local.example .env.local
npm install
npm run dev
```

### `kushim-auth/front`

Next.js + TypeScript auth UI on port `3001`.

Routes:

- `/connexion`
- `/inscription`
- `/recuperation`
- `/recuperation/confirmation`

```powershell
cd E:/Kushim/kushim-auth/front
copy .env.local.example .env.local
npm install
npm run dev
```

### `kushim-auth/api`

Rust + Axum auth API scaffold on port `8090`.

Stub routes:

- `POST /login`
- `POST /register`
- `POST /forgot-password`

```powershell
cd E:/Kushim/kushim-auth/api
copy .env.example .env
cargo run
```

### `kushim-app`

React + Vite + TypeScript private app on port `5173`.

Routes:

- `/dashboard`
- `/actifs`
- `/actifs/:id`
- `/transactions`
- `/parametres`

```powershell
cd E:/Kushim/kushim-app
copy .env.example .env
npm install
npm run dev
```

### `kushim-api`

Rust + Axum + sqlx + redis-rs API scaffold on port `8080`.

Stub routes:

- `GET /v1/portfolios`
- `GET /v1/transactions`
- `GET /v1/assets`

```powershell
cd E:/Kushim/kushim-api
copy .env.example .env
cargo run
```

### Internal Rust Services

`kushim-market-data` and `kushim-worker` are Rust scaffolds intended for the internal Docker network only.

```powershell
cd E:/Kushim/kushim-market-data
copy .env.example .env
cargo run

cd E:/Kushim/kushim-worker
copy .env.example .env
cargo run
```

## Docker

The root `docker-compose.yml` defines all services on one internal network. Only nginx exposes a host port.

Local nginx routing:

- `kushim.localhost` -> `kushim-website:3000`
- `app.kushim.localhost` -> `kushim-app:5173`
- `auth.kushim.localhost` -> `kushim-auth-front:3001`
- `api.kushim.localhost` -> `kushim-api:8080`

```powershell
cd E:/Kushim
copy .env.example .env
docker compose up --build
```

## Verification

Frontend checks:

```powershell
npm run lint
npm run build
npm audit
```

Rust checks:

```powershell
cargo check
```

Docker config check:

```powershell
docker compose config --quiet
```
