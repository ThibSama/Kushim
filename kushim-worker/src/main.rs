mod calculation;
mod config;
mod db;
mod domain;
mod errors;
mod health;
mod jobs;
mod repositories;
mod state;
#[cfg(test)]
mod test_utils;
mod worker;

use crate::{
    config::{Config, WorkerJob, WorkerMode},
    db::connect_and_check,
    errors::WorkerError,
    health::spawn_health_server,
    jobs::{
        Job, backfill_daily_snapshots::BackfillDailySnapshotsJob,
        generate_daily_snapshots::GenerateDailySnapshotsJob, noop::NoopJob,
        rebuild_current_read_models::RebuildCurrentReadModelsJob,
        refresh_current_portfolio_state::RefreshCurrentPortfolioStateJob,
    },
    state::AppState,
    worker::{runner::JobRunner, shutdown::wait_for_shutdown_signal},
};
use redis::AsyncCommands;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), WorkerError> {
    let config = Config::from_env()?;
    init_tracing(&config);

    tracing::info!(
        worker = %config.worker_name,
        env = %config.app_env,
        mode = %config.worker_mode.as_str(),
        job = %config.worker_job.as_str(),
        "starting kushim-worker"
    );

    let pg_pool = connect_and_check(&config.database_url).await?;
    tracing::info!(worker = %config.worker_name, "PostgreSQL connection established");

    if let Some(redis_url) = config.redis_url.as_deref() {
        check_redis(redis_url).await?;
        tracing::info!(worker = %config.worker_name, "Redis connectivity check succeeded");
    } else {
        tracing::info!(worker = %config.worker_name, "Redis not configured for this worker run");
    }

    let state = AppState {
        pg_pool,
        worker_name: config.worker_name.clone(),
    };

    let cancellation_token = CancellationToken::new();
    let health_handle =
        start_health_server_if_configured(&config, state.clone(), cancellation_token.clone())
            .await?;

    let shutdown_handle = if matches!(config.worker_mode, WorkerMode::Idle | WorkerMode::Loop) {
        Some(spawn_shutdown_listener(cancellation_token.clone()))
    } else {
        None
    };

    let runner = JobRunner::new(
        config.worker_mode,
        config.worker_poll_interval,
        build_job(&config)?,
    );

    let run_result = runner.run(&state, cancellation_token.clone()).await;
    cancellation_token.cancel();

    if let Some(handle) = shutdown_handle {
        handle
            .await
            .map_err(|error| WorkerError::Job(format!("shutdown task join failure: {error}")))??;
    }

    if let Some(handle) = health_handle {
        handle.await.map_err(|error| {
            WorkerError::HealthServer(format!("health task join failure: {error}"))
        })??;
    }

    run_result?;

    tracing::info!(worker = %config.worker_name, "kushim-worker stopped cleanly");
    Ok(())
}

fn init_tracing(config: &Config) {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(config.rust_log.clone())),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn start_health_server_if_configured(
    config: &Config,
    state: AppState,
    cancellation_token: CancellationToken,
) -> Result<Option<JoinHandle<Result<(), WorkerError>>>, WorkerError> {
    match config.health.clone() {
        Some(health_config) => {
            tracing::info!(
                worker = %config.worker_name,
                host = %health_config.host,
                port = health_config.port,
                "starting internal worker health server"
            );
            Ok(Some(
                spawn_health_server(health_config, state, cancellation_token).await?,
            ))
        }
        None => Ok(None),
    }
}

fn spawn_shutdown_listener(
    cancellation_token: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    tokio::spawn(async move {
        let signal = wait_for_shutdown_signal().await?;
        tracing::info!(signal, "worker shutdown signal received");
        cancellation_token.cancel();
        Ok(())
    })
}

async fn check_redis(redis_url: &str) -> Result<(), WorkerError> {
    let client = redis::Client::open(redis_url)?;
    let mut connection = client.get_multiplexed_async_connection().await?;
    let _: String = connection.ping().await?;
    Ok(())
}

fn build_job(config: &Config) -> Result<Arc<dyn Job>, WorkerError> {
    Ok(match config.worker_job {
        WorkerJob::Noop => Arc::new(NoopJob),
        WorkerJob::RebuildCurrentReadModels => {
            Arc::new(RebuildCurrentReadModelsJob::from_config(config))
        }
        WorkerJob::GenerateDailySnapshots => {
            Arc::new(GenerateDailySnapshotsJob::from_config(config))
        }
        WorkerJob::RefreshCurrentPortfolioState => {
            Arc::new(RefreshCurrentPortfolioStateJob::from_config(config))
        }
        WorkerJob::BackfillDailySnapshots => {
            Arc::new(BackfillDailySnapshotsJob::from_config(config)?)
        }
    })
}
