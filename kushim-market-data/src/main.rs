use kushim_market_data::{
    config::{
        Config, FxHistoryProviderKind, MarketDataJob, MarketDataMode, MarketDataProviderKind,
    },
    db, health,
    jobs::{
        fill_missing_fx_history_cache::FillMissingFxHistoryCacheJob,
        fill_missing_price_history_cache::FillMissingPriceHistoryCacheJob, noop::NoopJob,
        refresh_current_market_data::RefreshCurrentMarketDataJob,
    },
    providers::{
        finnhub::FinnhubProvider, mock::MockProvider, mock_fx_history::MockFxHistoryProvider,
        mock_fx_history::supported_currencies,
    },
    runner::JobRunner,
    state::AppState,
};
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    let config = Config::from_env().expect("failed to load configuration");

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| config.rust_log.clone().into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!(
        app_env = %config.app_env,
        mode = %config.mode.as_str(),
        job = %config.job.as_str(),
        provider = %config.provider.as_str(),
        "starting kushim-market-data"
    );

    let pg_pool = db::connect_and_check(&config.database_url)
        .await
        .expect("failed to connect to PostgreSQL");

    tracing::info!("PostgreSQL connection verified");

    let state = AppState { pg_pool };
    let cancel = CancellationToken::new();

    let health_addr = SocketAddr::new(config.host, config.port);
    let health_cancel = cancel.clone();
    let health_state = state.clone();

    let health_handle = tokio::spawn(async move {
        if let Err(e) = health::spawn_health_server(health_state, health_addr, health_cancel).await
        {
            tracing::error!(%e, "health server failed");
        }
    });

    let shutdown_handle = if matches!(config.mode, MarketDataMode::Idle | MarketDataMode::Loop) {
        let shutdown_cancel = cancel.clone();
        Some(tokio::spawn(async move {
            wait_for_shutdown_signal().await;
            tracing::info!("shutdown signal received");
            shutdown_cancel.cancel();
        }))
    } else {
        None
    };

    let run_result = run_job(&config, &state, cancel.clone()).await;

    cancel.cancel();

    if let Some(handle) = shutdown_handle {
        let _ = handle.await;
    }
    let _ = health_handle.await;

    if let Err(e) = run_result {
        tracing::error!(%e, "job runner failed");
        std::process::exit(1);
    }

    tracing::info!("kushim-market-data shut down cleanly");
}

async fn run_job(
    config: &Config,
    state: &AppState,
    cancel: CancellationToken,
) -> Result<(), kushim_market_data::errors::MarketDataError> {
    match config.job {
        MarketDataJob::Noop => {
            let runner = JobRunner::new(config.mode, config.run_interval, NoopJob);
            runner.run(state, cancel).await
        }
        MarketDataJob::RefreshCurrentMarketData => match config.provider {
            MarketDataProviderKind::Mock => {
                let job = RefreshCurrentMarketDataJob::new(MockProvider);
                let runner = JobRunner::new(config.mode, config.run_interval, job);
                runner.run(state, cancel).await
            }
            MarketDataProviderKind::Finnhub => {
                let provider = FinnhubProvider::new(
                    config.finnhub_base_url.clone(),
                    config.finnhub_api_key.clone().expect("validated in config"),
                    config.http_timeout,
                    config.provider_delay,
                    config.provider_symbol_map.clone(),
                )?;
                let job = RefreshCurrentMarketDataJob::new_with_symbol_allowlist(
                    provider,
                    config
                        .symbol_allowlist
                        .clone()
                        .expect("validated in config"),
                );
                let runner = JobRunner::new(config.mode, config.run_interval, job);
                runner.run(state, cancel).await
            }
        },
        MarketDataJob::FillMissingFxHistoryCache => match config.fx_history_provider {
            FxHistoryProviderKind::Mock => {
                let date_from = config.fx_history_date_from.expect("validated in config");
                let date_to = config.fx_history_date_to.expect("validated in config");
                let currencies = config
                    .fx_history_currencies
                    .clone()
                    .unwrap_or_else(supported_currencies);
                let job = FillMissingFxHistoryCacheJob::for_currency_set(
                    MockFxHistoryProvider,
                    date_from,
                    date_to,
                    &currencies,
                    config.fx_history_chunk_days,
                )?;
                let runner = JobRunner::new(config.mode, config.run_interval, job);
                runner.run(state, cancel).await
            }
        },
        MarketDataJob::FillMissingPriceHistoryCache => match config.provider {
            MarketDataProviderKind::Mock => {
                let date_from = config.history_date_from.expect("validated in config");
                let date_to = config.history_date_to.expect("validated in config");
                let job = FillMissingPriceHistoryCacheJob::new(MockProvider, date_from, date_to);
                let runner = JobRunner::new(config.mode, config.run_interval, job);
                runner.run(state, cancel).await
            }
            MarketDataProviderKind::Finnhub => {
                let date_from = config.history_date_from.expect("validated in config");
                let date_to = config.history_date_to.expect("validated in config");
                let provider = FinnhubProvider::new(
                    config.finnhub_base_url.clone(),
                    config.finnhub_api_key.clone().expect("validated in config"),
                    config.http_timeout,
                    config.provider_delay,
                    config.provider_symbol_map.clone(),
                )?;
                let job = FillMissingPriceHistoryCacheJob::new_with_symbol_allowlist(
                    provider,
                    date_from,
                    date_to,
                    config
                        .symbol_allowlist
                        .clone()
                        .expect("validated in config"),
                );
                let runner = JobRunner::new(config.mode, config.run_interval, job);
                runner.run(state, cancel).await
            }
        },
    }
}

async fn wait_for_shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.expect("failed to listen for ctrl+c");
    }
}
