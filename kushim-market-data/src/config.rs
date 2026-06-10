use crate::errors::MarketDataError;
use std::{env, net::IpAddr, str::FromStr, time::Duration};
use time::{Date, Month};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketDataMode {
    Idle,
    Once,
    Loop,
}

impl MarketDataMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Once => "once",
            Self::Loop => "loop",
        }
    }
}

impl FromStr for MarketDataMode {
    type Err = MarketDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "idle" => Ok(Self::Idle),
            "once" => Ok(Self::Once),
            "loop" => Ok(Self::Loop),
            _ => Err(MarketDataError::Config(format!(
                "MARKET_DATA_MODE must be one of idle, once, loop; got `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketDataJob {
    Noop,
    RefreshCurrentMarketData,
    FillMissingPriceHistoryCache,
}

impl MarketDataJob {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::RefreshCurrentMarketData => "refresh_current_market_data",
            Self::FillMissingPriceHistoryCache => "fill_missing_price_history_cache",
        }
    }
}

impl FromStr for MarketDataJob {
    type Err = MarketDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "noop" => Ok(Self::Noop),
            "refresh_current_market_data" => Ok(Self::RefreshCurrentMarketData),
            "fill_missing_price_history_cache" => Ok(Self::FillMissingPriceHistoryCache),
            _ => Err(MarketDataError::Config(format!(
                "MARKET_DATA_JOB must be one of noop, refresh_current_market_data, fill_missing_price_history_cache; got `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketDataProviderKind {
    Mock,
}

impl MarketDataProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
        }
    }
}

impl FromStr for MarketDataProviderKind {
    type Err = MarketDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "mock" => Ok(Self::Mock),
            _ => Err(MarketDataError::Config(format!(
                "MARKET_DATA_PROVIDER must be one of mock; got `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub app_env: String,
    pub rust_log: String,
    pub host: IpAddr,
    pub port: u16,
    pub mode: MarketDataMode,
    pub job: MarketDataJob,
    pub provider: MarketDataProviderKind,
    pub run_interval: Duration,
    pub history_date_from: Option<Date>,
    pub history_date_to: Option<Date>,
}

impl Config {
    pub fn from_env() -> Result<Self, MarketDataError> {
        let database_url = required_env("DATABASE_URL")?;
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        let host = env::var("MARKET_DATA_HOST")
            .unwrap_or_else(|_| "0.0.0.0".to_string())
            .parse::<IpAddr>()
            .map_err(|_| {
                MarketDataError::Config("MARKET_DATA_HOST must be a valid IP address".to_string())
            })?;

        let port = env::var("MARKET_DATA_PORT")
            .unwrap_or_else(|_| "8082".to_string())
            .parse::<u16>()
            .map_err(|_| {
                MarketDataError::Config("MARKET_DATA_PORT must be a valid TCP port".to_string())
            })?;

        let mode = env::var("MARKET_DATA_MODE")
            .unwrap_or_else(|_| "idle".to_string())
            .parse()?;

        let job = env::var("MARKET_DATA_JOB")
            .unwrap_or_else(|_| "noop".to_string())
            .parse()?;

        let provider = env::var("MARKET_DATA_PROVIDER")
            .unwrap_or_else(|_| "mock".to_string())
            .parse()?;

        let run_interval =
            Duration::from_secs(parse_positive_u64("MARKET_DATA_RUN_INTERVAL_SECONDS", 300)?);

        let history_date_from = optional_date_env("MARKET_DATA_HISTORY_DATE_FROM")?;
        let history_date_to = optional_date_env("MARKET_DATA_HISTORY_DATE_TO")?;

        if job == MarketDataJob::FillMissingPriceHistoryCache {
            let from = history_date_from.ok_or_else(|| {
                MarketDataError::Config(
                    "MARKET_DATA_HISTORY_DATE_FROM is required for fill_missing_price_history_cache"
                        .to_string(),
                )
            })?;
            let to = history_date_to.ok_or_else(|| {
                MarketDataError::Config(
                    "MARKET_DATA_HISTORY_DATE_TO is required for fill_missing_price_history_cache"
                        .to_string(),
                )
            })?;
            if from > to {
                return Err(MarketDataError::Config(format!(
                    "MARKET_DATA_HISTORY_DATE_FROM ({from}) must be <= MARKET_DATA_HISTORY_DATE_TO ({to})"
                )));
            }
            let range_days = (to - from).whole_days() + 1;
            if range_days > 366 {
                return Err(MarketDataError::Config(format!(
                    "date range must be at most 366 days; got {range_days}"
                )));
            }
        }

        Ok(Self {
            database_url,
            app_env,
            rust_log,
            host,
            port,
            mode,
            job,
            provider,
            run_interval,
            history_date_from,
            history_date_to,
        })
    }
}

fn required_env(key: &str) -> Result<String, MarketDataError> {
    let value = env::var(key).map_err(|_| {
        MarketDataError::Config(format!("{key} must be set for kushim-market-data"))
    })?;

    if value.trim().is_empty() {
        return Err(MarketDataError::Config(format!(
            "{key} must not be blank for kushim-market-data"
        )));
    }

    Ok(value)
}

fn optional_date_env(key: &str) -> Result<Option<Date>, MarketDataError> {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => parse_date(key, &value).map(Some),
        _ => Ok(None),
    }
}

fn parse_date(key: &str, value: &str) -> Result<Date, MarketDataError> {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3 {
        return Err(MarketDataError::Config(format!(
            "{key} must be in YYYY-MM-DD format; got `{value}`"
        )));
    }
    let year = parts[0]
        .parse::<i32>()
        .map_err(|_| MarketDataError::Config(format!("{key} has invalid year; got `{value}`")))?;
    let month_num = parts[1]
        .parse::<u8>()
        .map_err(|_| MarketDataError::Config(format!("{key} has invalid month; got `{value}`")))?;
    let day = parts[2]
        .parse::<u8>()
        .map_err(|_| MarketDataError::Config(format!("{key} has invalid day; got `{value}`")))?;
    let month = Month::try_from(month_num)
        .map_err(|_| MarketDataError::Config(format!("{key} has invalid month; got `{value}`")))?;
    Date::from_calendar_date(year, month, day)
        .map_err(|_| MarketDataError::Config(format!("{key} is not a valid date; got `{value}`")))
}

fn parse_positive_u64(key: &str, default: u64) -> Result<u64, MarketDataError> {
    let value = env::var(key).unwrap_or_else(|_| default.to_string());
    let parsed = value.parse::<u64>().map_err(|_| {
        MarketDataError::Config(format!("{key} must be a positive integer; got `{value}`"))
    })?;

    if parsed == 0 {
        return Err(MarketDataError::Config(format!(
            "{key} must be greater than zero; got `{value}`"
        )));
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::{Config, MarketDataJob, MarketDataMode, MarketDataProviderKind};
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

    const ENV_KEYS: &[&str] = &[
        "DATABASE_URL",
        "APP_ENV",
        "RUST_LOG",
        "MARKET_DATA_HOST",
        "MARKET_DATA_PORT",
        "MARKET_DATA_MODE",
        "MARKET_DATA_JOB",
        "MARKET_DATA_RUN_INTERVAL_SECONDS",
        "MARKET_DATA_PROVIDER",
        "MARKET_DATA_HISTORY_DATE_FROM",
        "MARKET_DATA_HISTORY_DATE_TO",
    ];

    fn set_minimal_env() {
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://kushim:kushim_secret_dev@localhost:5432/kushim",
            );
            std::env::remove_var("APP_ENV");
            std::env::remove_var("RUST_LOG");
            std::env::remove_var("MARKET_DATA_HOST");
            std::env::remove_var("MARKET_DATA_PORT");
            std::env::remove_var("MARKET_DATA_MODE");
            std::env::remove_var("MARKET_DATA_JOB");
            std::env::remove_var("MARKET_DATA_RUN_INTERVAL_SECONDS");
            std::env::remove_var("MARKET_DATA_PROVIDER");
            std::env::remove_var("MARKET_DATA_HISTORY_DATE_FROM");
            std::env::remove_var("MARKET_DATA_HISTORY_DATE_TO");
        }
    }

    #[test]
    fn config_accepts_valid_env() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgresql://kushim:kushim_secret_dev@localhost:5432/kushim",
            );
            std::env::set_var("APP_ENV", "test");
            std::env::set_var("RUST_LOG", "debug");
            std::env::set_var("MARKET_DATA_HOST", "127.0.0.1");
            std::env::set_var("MARKET_DATA_PORT", "9090");
            std::env::set_var("MARKET_DATA_MODE", "loop");
            std::env::set_var("MARKET_DATA_JOB", "noop");
            std::env::set_var("MARKET_DATA_RUN_INTERVAL_SECONDS", "60");
        }

        let config = Config::from_env().expect("config should parse");

        assert_eq!(config.app_env, "test");
        assert_eq!(config.rust_log, "debug");
        assert_eq!(config.host.to_string(), "127.0.0.1");
        assert_eq!(config.port, 9090);
        assert_eq!(config.mode, MarketDataMode::Loop);
        assert_eq!(config.job, MarketDataJob::Noop);
        assert_eq!(config.provider, MarketDataProviderKind::Mock);
        assert_eq!(config.run_interval.as_secs(), 60);
        assert!(config.history_date_from.is_none());
        assert!(config.history_date_to.is_none());
    }

    #[test]
    fn config_uses_defaults_when_optional_vars_are_absent() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();

        let config = Config::from_env().expect("config should parse with defaults");

        assert_eq!(config.app_env, "development");
        assert_eq!(config.rust_log, "info");
        assert_eq!(config.host.to_string(), "0.0.0.0");
        assert_eq!(config.port, 8082);
        assert_eq!(config.mode, MarketDataMode::Idle);
        assert_eq!(config.job, MarketDataJob::Noop);
        assert_eq!(config.provider, MarketDataProviderKind::Mock);
        assert_eq!(config.run_interval.as_secs(), 300);
        assert!(config.history_date_from.is_none());
        assert!(config.history_date_to.is_none());
    }

    #[test]
    fn missing_database_url_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::remove_var("DATABASE_URL");
        }

        let error = Config::from_env().expect_err("missing DATABASE_URL should fail");
        assert!(error.to_string().contains("DATABASE_URL"));
    }

    #[test]
    fn blank_database_url_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("DATABASE_URL", "   ");
        }

        let error = Config::from_env().expect_err("blank DATABASE_URL should fail");
        assert!(error.to_string().contains("DATABASE_URL"));
    }

    #[test]
    fn invalid_host_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_HOST", "not_an_ip");
        }

        let error = Config::from_env().expect_err("invalid host should fail");
        assert!(error.to_string().contains("MARKET_DATA_HOST"));
    }

    #[test]
    fn invalid_port_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_PORT", "abc");
        }

        let error = Config::from_env().expect_err("invalid port should fail");
        assert!(error.to_string().contains("MARKET_DATA_PORT"));
    }

    #[test]
    fn invalid_mode_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_MODE", "broken");
        }

        let error = Config::from_env().expect_err("invalid mode should fail");
        assert!(error.to_string().contains("MARKET_DATA_MODE"));
    }

    #[test]
    fn invalid_job_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_JOB", "broken");
        }

        let error = Config::from_env().expect_err("invalid job should fail");
        assert!(error.to_string().contains("MARKET_DATA_JOB"));
    }

    #[test]
    fn zero_interval_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_RUN_INTERVAL_SECONDS", "0");
        }

        let error = Config::from_env().expect_err("zero interval should fail");
        assert!(
            error
                .to_string()
                .contains("MARKET_DATA_RUN_INTERVAL_SECONDS")
        );
    }

    #[test]
    fn refresh_current_market_data_job_is_accepted() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_JOB", "refresh_current_market_data");
        }

        let config = Config::from_env().expect("config should parse");
        assert_eq!(config.job, MarketDataJob::RefreshCurrentMarketData);
    }

    #[test]
    fn invalid_provider_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_PROVIDER", "coinmarketcap");
        }

        let error = Config::from_env().expect_err("invalid provider should fail");
        assert!(error.to_string().contains("MARKET_DATA_PROVIDER"));
    }

    #[test]
    fn non_numeric_interval_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_RUN_INTERVAL_SECONDS", "abc");
        }

        let error = Config::from_env().expect_err("non-numeric interval should fail");
        assert!(
            error
                .to_string()
                .contains("MARKET_DATA_RUN_INTERVAL_SECONDS")
        );
    }

    #[test]
    fn fill_missing_job_is_accepted_with_dates() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_JOB", "fill_missing_price_history_cache");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_FROM", "2026-01-01");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_TO", "2026-01-31");
        }

        let config = Config::from_env().expect("config should parse");
        assert_eq!(config.job, MarketDataJob::FillMissingPriceHistoryCache);
        assert!(config.history_date_from.is_some());
        assert!(config.history_date_to.is_some());
    }

    #[test]
    fn fill_missing_job_rejected_without_date_from() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_JOB", "fill_missing_price_history_cache");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_TO", "2026-01-31");
        }

        let error = Config::from_env().expect_err("missing date_from should fail");
        assert!(error.to_string().contains("MARKET_DATA_HISTORY_DATE_FROM"));
    }

    #[test]
    fn fill_missing_job_rejected_without_date_to() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_JOB", "fill_missing_price_history_cache");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_FROM", "2026-01-01");
        }

        let error = Config::from_env().expect_err("missing date_to should fail");
        assert!(error.to_string().contains("MARKET_DATA_HISTORY_DATE_TO"));
    }

    #[test]
    fn fill_missing_job_rejected_when_from_after_to() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_JOB", "fill_missing_price_history_cache");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_FROM", "2026-06-01");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_TO", "2026-01-01");
        }

        let error = Config::from_env().expect_err("from > to should fail");
        assert!(error.to_string().contains("must be <="));
    }

    #[test]
    fn fill_missing_job_rejected_when_range_exceeds_366() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_JOB", "fill_missing_price_history_cache");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_FROM", "2025-01-01");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_TO", "2026-06-01");
        }

        let error = Config::from_env().expect_err("range > 366 should fail");
        assert!(error.to_string().contains("366"));
    }

    #[test]
    fn invalid_date_format_is_rejected() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_JOB", "fill_missing_price_history_cache");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_FROM", "01/01/2026");
            std::env::set_var("MARKET_DATA_HISTORY_DATE_TO", "2026-01-31");
        }

        let error = Config::from_env().expect_err("invalid date format should fail");
        assert!(error.to_string().contains("MARKET_DATA_HISTORY_DATE_FROM"));
    }
}
