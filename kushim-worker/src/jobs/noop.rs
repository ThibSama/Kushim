use crate::{errors::WorkerError, jobs::Job, state::AppState};
use async_trait::async_trait;

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopJob;

#[async_trait]
impl Job for NoopJob {
    fn name(&self) -> &'static str {
        "noop_job"
    }

    async fn run(&self, state: &AppState) -> Result<(), WorkerError> {
        tracing::info!(worker = %state.worker_name, job = self.name(), "starting noop worker job");
        tracing::info!(worker = %state.worker_name, job = self.name(), "completed noop worker job");
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
            .connect_lazy("postgresql://postgres:postgres@localhost:5432/kushim")
            .expect("lazy pool should build");
        let state = AppState {
            pg_pool: pool,
            worker_name: "test-worker".to_string(),
        };

        NoopJob.run(&state).await.expect("noop job should succeed");
    }
}
