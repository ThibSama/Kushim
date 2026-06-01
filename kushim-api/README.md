# kushim-api

Rust + Axum API scaffold with sqlx and redis-rs dependencies.

Stub routes:

- `GET /health`
- `GET /v1/portfolios`
- `GET /v1/transactions`
- `GET /v1/assets`

Run:

```bash
copy .env.example .env
cargo run
```

No database migrations or Redis calls are implemented yet.
