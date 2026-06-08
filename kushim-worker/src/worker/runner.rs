use crate::{config::WorkerMode, errors::WorkerError, jobs::Job, state::AppState};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub struct JobRunner {
    mode: WorkerMode,
    poll_interval: Duration,
    job: Arc<dyn Job>,
}

impl JobRunner {
    pub fn new(mode: WorkerMode, poll_interval: Duration, job: Arc<dyn Job>) -> Self {
        Self {
            mode,
            poll_interval,
            job,
        }
    }

    pub async fn run(
        &self,
        state: &AppState,
        cancellation_token: CancellationToken,
    ) -> Result<(), WorkerError> {
        match self.mode {
            WorkerMode::Idle => {
                tracing::info!(worker = %state.worker_name, mode = %self.mode.as_str(), "worker started in idle mode");
                cancellation_token.cancelled().await;
                tracing::info!(worker = %state.worker_name, "worker idle mode received shutdown");
                Ok(())
            }
            WorkerMode::Once => {
                tracing::info!(worker = %state.worker_name, mode = %self.mode.as_str(), job = self.job.name(), "worker started in once mode");
                self.job.run(state).await?;
                tracing::info!(worker = %state.worker_name, mode = %self.mode.as_str(), job = self.job.name(), "worker once mode completed");
                Ok(())
            }
            WorkerMode::Loop => {
                tracing::info!(
                    worker = %state.worker_name,
                    mode = %self.mode.as_str(),
                    interval_seconds = self.poll_interval.as_secs(),
                    job = self.job.name(),
                    "worker started in loop mode"
                );

                loop {
                    tokio::select! {
                        _ = cancellation_token.cancelled() => {
                            tracing::info!(worker = %state.worker_name, mode = %self.mode.as_str(), "worker loop mode received shutdown");
                            break;
                        }
                        result = self.job.run(state) => {
                            result?;
                        }
                    }

                    tokio::select! {
                        _ = cancellation_token.cancelled() => {
                            tracing::info!(worker = %state.worker_name, mode = %self.mode.as_str(), "worker loop mode stopped before next cycle");
                            break;
                        }
                        _ = tokio::time::sleep(self.poll_interval) => {}
                    }
                }

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JobRunner;
    use crate::{config::WorkerMode, errors::WorkerError, jobs::Job, state::AppState};
    use async_trait::async_trait;
    use sqlx::postgres::PgPoolOptions;
    use std::{
        sync::Arc,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };
    use tokio_util::sync::CancellationToken;

    #[derive(Clone)]
    struct CountingJob {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Job for CountingJob {
        fn name(&self) -> &'static str {
            "counting_job"
        }

        async fn run(&self, _state: &AppState) -> Result<(), WorkerError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn test_state() -> AppState {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://postgres:postgres@localhost:5432/kushim")
            .expect("lazy pool should build");

        AppState {
            pg_pool: pool,
            worker_name: "test-worker".to_string(),
        }
    }

    #[tokio::test]
    async fn runner_once_mode_runs_job_once() {
        let count = Arc::new(AtomicUsize::new(0));
        let runner = JobRunner::new(
            WorkerMode::Once,
            Duration::from_secs(1),
            Arc::new(CountingJob {
                count: count.clone(),
            }),
        );

        runner
            .run(&test_state(), CancellationToken::new())
            .await
            .expect("runner once mode should succeed");

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn runner_idle_mode_does_not_run_job() {
        let count = Arc::new(AtomicUsize::new(0));
        let runner = JobRunner::new(
            WorkerMode::Idle,
            Duration::from_secs(1),
            Arc::new(CountingJob {
                count: count.clone(),
            }),
        );
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let handle = tokio::spawn(async move { runner.run(&test_state(), token_clone).await });
        tokio::task::yield_now().await;
        token.cancel();

        handle
            .await
            .expect("idle runner join should succeed")
            .expect("idle runner should stop cleanly");

        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
