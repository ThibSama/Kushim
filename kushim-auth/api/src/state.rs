use crate::services::auth::AuthService;
use crate::services::handoff::HandoffService;
use crate::services::rate_limit::RateLimitService;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub auth_service: AuthService,
    pub rate_limiter: Option<RateLimitService>,
    pub handoff_service: Option<HandoffService>,
    pub rate_limit_enabled: bool,
    pub service_name: &'static str,
    pub service_version: &'static str,
    pub routes_version: &'static str,
    pub environment: String,
}
