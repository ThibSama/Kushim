use std::{env, time::Duration};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let interval_seconds = env::var("JOB_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30);

    tracing::info!(interval_seconds, "starting internal kushim-worker scaffold");

    loop {
        tracing::info!("worker job stub");
        tokio::time::sleep(Duration::from_secs(interval_seconds)).await;
    }
}
