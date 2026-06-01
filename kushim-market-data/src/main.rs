use std::{env, time::Duration};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let interval_seconds = env::var("POLL_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(60);

    tracing::info!(
        interval_seconds,
        "starting internal kushim-market-data scaffold"
    );

    loop {
        tracing::info!("market data poll stub");
        tokio::time::sleep(Duration::from_secs(interval_seconds)).await;
    }
}
