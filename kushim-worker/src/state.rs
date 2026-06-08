use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pg_pool: PgPool,
    pub worker_name: String,
}
