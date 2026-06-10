use crate::{config::MarketDataMode, errors::MarketDataError, jobs::Job, state::AppState};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub struct JobRunner<J: Job> {
    mode: MarketDataMode,
    run_interval: Duration,
    job: J,
}

impl<J: Job> JobRunner<J> {
    pub fn new(mode: MarketDataMode, run_interval: Duration, job: J) -> Self {
        Self {
            mode,
            run_interval,
            job,
        }
    }

    pub async fn run(
        &self,
        state: &AppState,
        cancel: CancellationToken,
    ) -> Result<(), MarketDataError> {
        match self.mode {
            MarketDataMode::Idle => {
                tracing::info!(mode = "idle", "market-data started in idle mode");
                cancel.cancelled().await;
                tracing::info!(mode = "idle", "market-data idle mode received shutdown");
                Ok(())
            }
            MarketDataMode::Once => {
                tracing::info!(
                    mode = "once",
                    job = self.job.name(),
                    "market-data executing job once"
                );
                self.job.run(state).await?;
                tracing::info!(
                    mode = "once",
                    job = self.job.name(),
                    "market-data once mode completed"
                );
                Ok(())
            }
            MarketDataMode::Loop => {
                tracing::info!(
                    mode = "loop",
                    job = self.job.name(),
                    interval_seconds = self.run_interval.as_secs(),
                    "market-data started in loop mode"
                );

                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            tracing::info!(mode = "loop", "market-data loop mode received shutdown");
                            break;
                        }
                        result = self.job.run(state) => {
                            result?;
                        }
                    }

                    tokio::select! {
                        _ = cancel.cancelled() => {
                            tracing::info!(mode = "loop", "market-data loop mode stopped before next cycle");
                            break;
                        }
                        _ = tokio::time::sleep(self.run_interval) => {}
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
    use crate::{config::MarketDataMode, errors::MarketDataError, jobs::Job, state::AppState};
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

    impl Job for CountingJob {
        fn name(&self) -> &'static str {
            "counting_job"
        }

        async fn run(&self, _state: &AppState) -> Result<(), MarketDataError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn test_state() -> AppState {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://localhost/fake")
            .expect("lazy pool should build");
        AppState { pg_pool: pool }
    }

    #[tokio::test]
    async fn once_mode_runs_job_once() {
        let count = Arc::new(AtomicUsize::new(0));
        let runner = JobRunner::new(
            MarketDataMode::Once,
            Duration::from_secs(1),
            CountingJob {
                count: count.clone(),
            },
        );

        runner
            .run(&test_state(), CancellationToken::new())
            .await
            .expect("once mode should succeed");

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn idle_mode_does_not_run_job() {
        let count = Arc::new(AtomicUsize::new(0));
        let runner = JobRunner::new(
            MarketDataMode::Idle,
            Duration::from_secs(1),
            CountingJob {
                count: count.clone(),
            },
        );
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let state = test_state();
        let handle = tokio::spawn(async move { runner.run(&state, token_clone).await });
        tokio::task::yield_now().await;
        token.cancel();

        handle
            .await
            .expect("idle runner join should succeed")
            .expect("idle runner should stop cleanly");

        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn loop_mode_runs_job_then_stops_on_cancel() {
        let count = Arc::new(AtomicUsize::new(0));
        let runner = JobRunner::new(
            MarketDataMode::Loop,
            Duration::from_secs(300),
            CountingJob {
                count: count.clone(),
            },
        );
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let state = test_state();
        let handle = tokio::spawn(async move { runner.run(&state, token_clone).await });

        tokio::time::sleep(Duration::from_millis(50)).await;
        token.cancel();

        handle
            .await
            .expect("loop runner join should succeed")
            .expect("loop runner should stop cleanly");

        assert!(count.load(Ordering::SeqCst) >= 1);
    }
}
