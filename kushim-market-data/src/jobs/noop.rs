use crate::{errors::MarketDataError, jobs::Job, state::AppState};

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopJob;

impl Job for NoopJob {
    fn name(&self) -> &'static str {
        "noop"
    }

    async fn run(&self, _state: &AppState) -> Result<(), MarketDataError> {
        tracing::info!(job = self.name(), "noop job executed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::NoopJob;
    use crate::{jobs::Job, state::AppState};
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn noop_job_returns_success() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://localhost/fake")
            .expect("lazy pool should build");
        let state = AppState { pg_pool: pool };

        NoopJob.run(&state).await.expect("noop job should succeed");
    }
}
