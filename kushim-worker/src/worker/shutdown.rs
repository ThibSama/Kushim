use crate::errors::WorkerError;

pub async fn wait_for_shutdown_signal() -> Result<&'static str, WorkerError> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => Ok("ctrl_c"),
            _ = terminate.recv() => Ok("sigterm"),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
        Ok("ctrl_c")
    }
}
