use crate::errors::WorkerError;
use std::{env, net::IpAddr, str::FromStr, time::Duration};
use time::{Date, macros::format_description};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerMode {
    Idle,
    Once,
    Loop,
}

impl WorkerMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Once => "once",
            Self::Loop => "loop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerJob {
    Noop,
    RebuildCurrentReadModels,
    GenerateDailySnapshots,
    RefreshCurrentPortfolioState,
    BackfillDailySnapshots,
}

impl WorkerJob {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::RebuildCurrentReadModels => "rebuild_current_read_models",
            Self::GenerateDailySnapshots => "generate_daily_snapshots",
            Self::RefreshCurrentPortfolioState => "refresh_current_portfolio_state",
            Self::BackfillDailySnapshots => "backfill_daily_snapshots",
        }
    }
}

impl FromStr for WorkerJob {
    type Err = WorkerError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "noop" => Ok(Self::Noop),
            "rebuild_current_read_models" => Ok(Self::RebuildCurrentReadModels),
            "generate_daily_snapshots" => Ok(Self::GenerateDailySnapshots),
            "refresh_current_portfolio_state" => Ok(Self::RefreshCurrentPortfolioState),
            "backfill_daily_snapshots" => Ok(Self::BackfillDailySnapshots),
            _ => Err(WorkerError::Config(format!(
                "WORKER_JOB must be one of noop, rebuild_current_read_models, generate_daily_snapshots, refresh_current_portfolio_state, backfill_daily_snapshots; got `{value}`"
            ))),
        }
    }
}

impl FromStr for WorkerMode {
    type Err = WorkerError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "idle" => Ok(Self::Idle),
            "once" => Ok(Self::Once),
            "loop" => Ok(Self::Loop),
            _ => Err(WorkerError::Config(format!(
                "WORKER_MODE must be one of idle, once, loop; got `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthConfig {
    pub host: IpAddr,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub app_env: String,
    pub rust_log: String,
    pub worker_name: String,
    pub worker_mode: WorkerMode,
    pub worker_job: WorkerJob,
    pub worker_poll_interval: Duration,
    pub target_portfolio_id: Option<Uuid>,
    pub snapshot_date: Option<Date>,
    pub backfill_date_from: Option<Date>,
    pub backfill_date_to: Option<Date>,
    pub redis_url: Option<String>,
    pub health: Option<HealthConfig>,
}

impl Config {
    pub fn from_env() -> Result<Self, WorkerError> {
        let database_url = required_env("DATABASE_URL")?;
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let worker_name = env::var("WORKER_NAME").unwrap_or_else(|_| "kushim-worker".to_string());
        let worker_mode = env::var("WORKER_MODE")
            .unwrap_or_else(|_| "idle".to_string())
            .parse()?;
        let worker_job = env::var("WORKER_JOB")
            .unwrap_or_else(|_| "noop".to_string())
            .parse()?;
        let worker_poll_interval =
            Duration::from_secs(parse_positive_u64("WORKER_POLL_INTERVAL_SECONDS", 30)?);
        let target_portfolio_id = optional_uuid_env("WORKER_TARGET_PORTFOLIO_ID")?;
        let snapshot_date = optional_date_env("WORKER_SNAPSHOT_DATE")?;
        let backfill_date_from = optional_date_env("WORKER_BACKFILL_DATE_FROM")?;
        let backfill_date_to = optional_date_env("WORKER_BACKFILL_DATE_TO")?;
        let redis_url = env::var("REDIS_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());

        let health = match (
            env::var("WORKER_HEALTH_HOST").ok(),
            env::var("WORKER_HEALTH_PORT").ok(),
        ) {
            (Some(host), Some(port)) => {
                let host = host.parse::<IpAddr>().map_err(|_| {
                    WorkerError::Config(format!(
                        "WORKER_HEALTH_HOST must be a valid IP address; got `{host}`"
                    ))
                })?;
                let port = port.parse::<u16>().map_err(|_| {
                    WorkerError::Config(format!(
                        "WORKER_HEALTH_PORT must be a valid TCP port; got `{port}`"
                    ))
                })?;

                Some(HealthConfig { host, port })
            }
            (None, None) => None,
            _ => {
                return Err(WorkerError::Config(
                    "WORKER_HEALTH_HOST and WORKER_HEALTH_PORT must be set together".to_string(),
                ));
            }
        };

        let config = Self {
            database_url,
            app_env,
            rust_log,
            worker_name,
            worker_mode,
            worker_job,
            worker_poll_interval,
            target_portfolio_id,
            snapshot_date,
            backfill_date_from,
            backfill_date_to,
            redis_url,
            health,
        };

        validate_job_specific_config(&config)?;
        Ok(config)
    }
}

fn validate_job_specific_config(config: &Config) -> Result<(), WorkerError> {
    if config.worker_job != WorkerJob::BackfillDailySnapshots {
        return Ok(());
    }

    if config.worker_mode == WorkerMode::Loop {
        return Err(WorkerError::Config(
            "WORKER_JOB=backfill_daily_snapshots supports only WORKER_MODE=once or idle in V1"
                .to_string(),
        ));
    }

    if config.target_portfolio_id.is_none() {
        return Err(WorkerError::Config(
            "WORKER_TARGET_PORTFOLIO_ID must be set for WORKER_JOB=backfill_daily_snapshots"
                .to_string(),
        ));
    }

    let Some(date_from) = config.backfill_date_from else {
        return Err(WorkerError::Config(
            "WORKER_BACKFILL_DATE_FROM must be set for WORKER_JOB=backfill_daily_snapshots"
                .to_string(),
        ));
    };
    let Some(date_to) = config.backfill_date_to else {
        return Err(WorkerError::Config(
            "WORKER_BACKFILL_DATE_TO must be set for WORKER_JOB=backfill_daily_snapshots"
                .to_string(),
        ));
    };

    if date_from > date_to {
        return Err(WorkerError::Config(
            "WORKER_BACKFILL_DATE_FROM must be less than or equal to WORKER_BACKFILL_DATE_TO"
                .to_string(),
        ));
    }

    let mut day_count = 0_u32;
    let mut current = date_from;
    loop {
        day_count += 1;
        if day_count > 366 {
            return Err(WorkerError::Config(
                "WORKER_JOB=backfill_daily_snapshots supports a maximum range of 366 days in V1"
                    .to_string(),
            ));
        }

        if current == date_to {
            break;
        }

        current = current.next_day().ok_or_else(|| {
            WorkerError::Config("WORKER_BACKFILL_DATE_TO exceeds supported date bounds".into())
        })?;
    }

    Ok(())
}

fn required_env(key: &str) -> Result<String, WorkerError> {
    env::var(key).map_err(|_| WorkerError::Config(format!("{key} must be set for kushim-worker")))
}

fn parse_positive_u64(key: &str, default: u64) -> Result<u64, WorkerError> {
    let value = env::var(key).unwrap_or_else(|_| default.to_string());
    let parsed = value.parse::<u64>().map_err(|_| {
        WorkerError::Config(format!("{key} must be a positive integer; got `{value}`"))
    })?;

    if parsed == 0 {
        return Err(WorkerError::Config(format!(
            "{key} must be greater than zero; got `{value}`"
        )));
    }

    Ok(parsed)
}

fn optional_uuid_env(key: &str) -> Result<Option<Uuid>, WorkerError> {
    match env::var(key) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => value
            .parse::<Uuid>()
            .map(Some)
            .map_err(|_| WorkerError::Config(format!("{key} must be a valid UUID; got `{value}`"))),
        Err(_) => Ok(None),
    }
}

fn optional_date_env(key: &str) -> Result<Option<Date>, WorkerError> {
    match env::var(key) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Date::parse(&value, format_description!("[year]-[month]-[day]"))
            .map(Some)
            .map_err(|_| {
                WorkerError::Config(format!(
                    "{key} must be a valid ISO date YYYY-MM-DD; got `{value}`"
                ))
            }),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, WorkerJob, WorkerMode};
    use crate::test_utils::lock_env;

    struct EnvRestore {
        values: Vec<(&'static str, Option<String>)>,
    }

    impl EnvRestore {
        fn capture(keys: &[&'static str]) -> Self {
            Self {
                values: keys
                    .iter()
                    .map(|key| (*key, std::env::var(key).ok()))
                    .collect(),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }

    #[test]
    fn config_accepts_valid_env() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "APP_ENV",
            "RUST_LOG",
            "WORKER_NAME",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_POLL_INTERVAL_SECONDS",
            "WORKER_TARGET_PORTFOLIO_ID",
            "WORKER_SNAPSHOT_DATE",
            "WORKER_BACKFILL_DATE_FROM",
            "WORKER_BACKFILL_DATE_TO",
            "REDIS_URL",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("APP_ENV", "test");
            std::env::set_var("RUST_LOG", "debug");
            std::env::set_var("WORKER_NAME", "worker-test");
            std::env::set_var("WORKER_MODE", "loop");
            std::env::set_var("WORKER_JOB", "rebuild_current_read_models");
            std::env::set_var("WORKER_POLL_INTERVAL_SECONDS", "5");
            std::env::set_var(
                "WORKER_TARGET_PORTFOLIO_ID",
                "123e4567-e89b-12d3-a456-426614174000",
            );
            std::env::set_var("WORKER_SNAPSHOT_DATE", "2026-06-06");
            std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379/0");
            std::env::set_var("WORKER_HEALTH_HOST", "127.0.0.1");
            std::env::set_var("WORKER_HEALTH_PORT", "8081");
        }

        let config = Config::from_env().expect("config should parse");

        assert_eq!(config.app_env, "test");
        assert_eq!(config.rust_log, "debug");
        assert_eq!(config.worker_name, "worker-test");
        assert_eq!(config.worker_mode, WorkerMode::Loop);
        assert_eq!(config.worker_job, WorkerJob::RebuildCurrentReadModels);
        assert_eq!(config.worker_poll_interval.as_secs(), 5);
        assert_eq!(
            config.target_portfolio_id.unwrap().to_string(),
            "123e4567-e89b-12d3-a456-426614174000"
        );
        assert_eq!(config.snapshot_date.unwrap().to_string(), "2026-06-06");
        assert!(config.backfill_date_from.is_none());
        assert!(config.backfill_date_to.is_none());
        assert_eq!(
            config.redis_url.as_deref(),
            Some("redis://127.0.0.1:6379/0")
        );
        assert_eq!(config.health.as_ref().unwrap().port, 8081);
    }

    #[test]
    fn invalid_worker_mode_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_JOB", "noop");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
            std::env::set_var("WORKER_MODE", "broken");
        }

        let error = Config::from_env().expect_err("invalid mode should fail");
        assert!(error.to_string().contains("WORKER_MODE"));
    }

    #[test]
    fn invalid_interval_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_POLL_INTERVAL_SECONDS",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "idle");
            std::env::set_var("WORKER_JOB", "noop");
            std::env::set_var("WORKER_POLL_INTERVAL_SECONDS", "0");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let error = Config::from_env().expect_err("zero interval should fail");
        assert!(error.to_string().contains("WORKER_POLL_INTERVAL_SECONDS"));
    }

    #[test]
    fn invalid_worker_job_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "idle");
            std::env::set_var("WORKER_JOB", "broken");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let error = Config::from_env().expect_err("invalid job should fail");
        assert!(error.to_string().contains("WORKER_JOB"));
    }

    #[test]
    fn valid_snapshot_date_is_accepted() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_SNAPSHOT_DATE",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "once");
            std::env::set_var("WORKER_JOB", "generate_daily_snapshots");
            std::env::set_var("WORKER_SNAPSHOT_DATE", "2026-06-06");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let config = Config::from_env().expect("snapshot date should parse");
        assert_eq!(config.worker_job, WorkerJob::GenerateDailySnapshots);
        assert_eq!(config.snapshot_date.unwrap().to_string(), "2026-06-06");
    }

    #[test]
    fn refresh_current_portfolio_state_job_is_accepted() {
        let parsed = "refresh_current_portfolio_state"
            .parse::<WorkerJob>()
            .expect("refresh_current_portfolio_state should parse");

        assert_eq!(parsed, WorkerJob::RefreshCurrentPortfolioState);
    }

    #[test]
    fn backfill_daily_snapshots_job_is_accepted() {
        let parsed = "backfill_daily_snapshots"
            .parse::<WorkerJob>()
            .expect("backfill_daily_snapshots should parse");

        assert_eq!(parsed, WorkerJob::BackfillDailySnapshots);
    }

    #[test]
    fn invalid_snapshot_date_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_SNAPSHOT_DATE",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "once");
            std::env::set_var("WORKER_JOB", "generate_daily_snapshots");
            std::env::set_var("WORKER_SNAPSHOT_DATE", "2026-13-99");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let error = Config::from_env().expect_err("invalid snapshot date should fail");
        assert!(error.to_string().contains("WORKER_SNAPSHOT_DATE"));
    }

    #[test]
    fn invalid_target_portfolio_id_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_TARGET_PORTFOLIO_ID",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "once");
            std::env::set_var("WORKER_JOB", "refresh_current_portfolio_state");
            std::env::set_var("WORKER_TARGET_PORTFOLIO_ID", "not-a-uuid");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let error = Config::from_env().expect_err("invalid target portfolio id should fail");
        assert!(error.to_string().contains("WORKER_TARGET_PORTFOLIO_ID"));
    }

    #[test]
    fn backfill_requires_target_portfolio_id() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_TARGET_PORTFOLIO_ID",
            "WORKER_BACKFILL_DATE_FROM",
            "WORKER_BACKFILL_DATE_TO",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "once");
            std::env::set_var("WORKER_JOB", "backfill_daily_snapshots");
            std::env::remove_var("WORKER_TARGET_PORTFOLIO_ID");
            std::env::set_var("WORKER_BACKFILL_DATE_FROM", "2026-06-01");
            std::env::set_var("WORKER_BACKFILL_DATE_TO", "2026-06-03");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let error = Config::from_env().expect_err("backfill should require target portfolio");
        assert!(error.to_string().contains("WORKER_TARGET_PORTFOLIO_ID"));
    }

    #[test]
    fn backfill_requires_date_range() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_TARGET_PORTFOLIO_ID",
            "WORKER_BACKFILL_DATE_FROM",
            "WORKER_BACKFILL_DATE_TO",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "once");
            std::env::set_var("WORKER_JOB", "backfill_daily_snapshots");
            std::env::set_var(
                "WORKER_TARGET_PORTFOLIO_ID",
                "123e4567-e89b-12d3-a456-426614174000",
            );
            std::env::remove_var("WORKER_BACKFILL_DATE_FROM");
            std::env::remove_var("WORKER_BACKFILL_DATE_TO");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let error = Config::from_env().expect_err("backfill should require date range");
        assert!(error.to_string().contains("WORKER_BACKFILL_DATE_FROM"));
    }

    #[test]
    fn backfill_rejects_invalid_date_range_order() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_TARGET_PORTFOLIO_ID",
            "WORKER_BACKFILL_DATE_FROM",
            "WORKER_BACKFILL_DATE_TO",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "once");
            std::env::set_var("WORKER_JOB", "backfill_daily_snapshots");
            std::env::set_var(
                "WORKER_TARGET_PORTFOLIO_ID",
                "123e4567-e89b-12d3-a456-426614174000",
            );
            std::env::set_var("WORKER_BACKFILL_DATE_FROM", "2026-06-04");
            std::env::set_var("WORKER_BACKFILL_DATE_TO", "2026-06-03");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let error = Config::from_env().expect_err("invalid date order should fail");
        assert!(error.to_string().contains("WORKER_BACKFILL_DATE_FROM"));
    }

    #[test]
    fn backfill_rejects_range_larger_than_366_days() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_TARGET_PORTFOLIO_ID",
            "WORKER_BACKFILL_DATE_FROM",
            "WORKER_BACKFILL_DATE_TO",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "once");
            std::env::set_var("WORKER_JOB", "backfill_daily_snapshots");
            std::env::set_var(
                "WORKER_TARGET_PORTFOLIO_ID",
                "123e4567-e89b-12d3-a456-426614174000",
            );
            std::env::set_var("WORKER_BACKFILL_DATE_FROM", "2026-01-01");
            std::env::set_var("WORKER_BACKFILL_DATE_TO", "2027-01-02");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let error = Config::from_env().expect_err("range > 366 days should fail");
        assert!(error.to_string().contains("366"));
    }

    #[test]
    fn backfill_rejects_loop_mode_in_v1() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(&[
            "DATABASE_URL",
            "WORKER_MODE",
            "WORKER_JOB",
            "WORKER_TARGET_PORTFOLIO_ID",
            "WORKER_BACKFILL_DATE_FROM",
            "WORKER_BACKFILL_DATE_TO",
            "WORKER_HEALTH_HOST",
            "WORKER_HEALTH_PORT",
        ]);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://postgres:postgres@localhost:5432/test",
            );
            std::env::set_var("WORKER_MODE", "loop");
            std::env::set_var("WORKER_JOB", "backfill_daily_snapshots");
            std::env::set_var(
                "WORKER_TARGET_PORTFOLIO_ID",
                "123e4567-e89b-12d3-a456-426614174000",
            );
            std::env::set_var("WORKER_BACKFILL_DATE_FROM", "2026-06-01");
            std::env::set_var("WORKER_BACKFILL_DATE_TO", "2026-06-03");
            std::env::remove_var("WORKER_HEALTH_HOST");
            std::env::remove_var("WORKER_HEALTH_PORT");
        }

        let error = Config::from_env().expect_err("backfill loop mode should fail in V1");
        assert!(error.to_string().contains("WORKER_MODE"));
    }
}
