use crate::errors::WorkerError;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::time::Duration;

pub async fn connect_and_check(database_url: &str) -> Result<PgPool, WorkerError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await?;

    check_readiness(&pool).await?;

    Ok(pool)
}

pub async fn check_readiness(pool: &PgPool) -> Result<(), WorkerError> {
    let row = sqlx::query("SELECT 1 AS readiness").fetch_one(pool).await?;
    let readiness: i32 = row.try_get("readiness")?;

    if readiness != 1 {
        return Err(WorkerError::HealthServer(
            "database readiness query returned an unexpected value".to_string(),
        ));
    }

    Ok(())
}
