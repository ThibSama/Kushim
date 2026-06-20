use crate::{domain::fx_rate::Currency, errors::MarketDataError, symbol_filter::SymbolAllowlist};
use std::{collections::HashMap, env, net::IpAddr, str::FromStr, time::Duration};
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
    FillMissingFxHistoryCache,
}

impl MarketDataJob {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::RefreshCurrentMarketData => "refresh_current_market_data",
            Self::FillMissingPriceHistoryCache => "fill_missing_price_history_cache",
            Self::FillMissingFxHistoryCache => "fill_missing_fx_history_cache",
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
            "fill_missing_fx_history_cache" => Ok(Self::FillMissingFxHistoryCache),
            _ => Err(MarketDataError::Config(format!(
                "MARKET_DATA_JOB must be one of noop, refresh_current_market_data, fill_missing_price_history_cache, fill_missing_fx_history_cache; got `{value}`"
            ))),
        }
    }
}

/// FX-history provider selector. Only the mock provider is registered in
/// PR004; real provider selection is deferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxHistoryProviderKind {
    Mock,
}

impl FxHistoryProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
        }
    }
}

impl FromStr for FxHistoryProviderKind {
    type Err = MarketDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "mock" => Ok(Self::Mock),
            _ => Err(MarketDataError::Config(format!(
                "FX_HISTORY_PROVIDER must be `mock` (only mock is registered in PR004); got `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketDataProviderKind {
    Mock,
    Finnhub,
}

impl MarketDataProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Finnhub => "finnhub",
        }
    }
}

impl FromStr for MarketDataProviderKind {
    type Err = MarketDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "mock" => Ok(Self::Mock),
            "finnhub" => Ok(Self::Finnhub),
            _ => Err(MarketDataError::Config(format!(
                "MARKET_DATA_PROVIDER must be one of mock, finnhub; got `{value}`"
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
    pub finnhub_api_key: Option<String>,
    pub finnhub_base_url: String,
    pub symbol_allowlist: Option<SymbolAllowlist>,
    pub provider_symbol_map: HashMap<String, String>,
    pub http_timeout: Duration,
    pub provider_delay: Duration,
    pub fx_history_provider: FxHistoryProviderKind,
    pub fx_history_currencies: Option<Vec<Currency>>,
    pub fx_history_date_from: Option<Date>,
    pub fx_history_date_to: Option<Date>,
    pub fx_history_max_carry_days: i64,
    pub fx_history_chunk_days: usize,
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
        let finnhub_api_key = optional_nonblank_env("FINNHUB_API_KEY");
        let finnhub_base_url = env::var("FINNHUB_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "https://finnhub.io/api/v1".to_string());
        let symbol_allowlist = env::var("MARKET_DATA_SYMBOL_ALLOWLIST")
            .ok()
            .and_then(|value| SymbolAllowlist::parse(&value));
        let provider_symbol_map =
            optional_provider_symbol_map_env("MARKET_DATA_PROVIDER_SYMBOL_MAP")?;
        let http_timeout =
            Duration::from_secs(parse_positive_u64("MARKET_DATA_HTTP_TIMEOUT_SECONDS", 10)?);
        let provider_delay =
            Duration::from_millis(parse_u64("MARKET_DATA_PROVIDER_DELAY_MS", 1_100)?);

        let fx_history_provider: FxHistoryProviderKind = env::var("FX_HISTORY_PROVIDER")
            .unwrap_or_else(|_| "mock".to_string())
            .parse()?;
        let fx_history_currencies = optional_currency_list_env("FX_HISTORY_CURRENCIES")?;
        let fx_history_date_from = optional_date_env("FX_HISTORY_DATE_FROM")?;
        let fx_history_date_to = optional_date_env("FX_HISTORY_DATE_TO")?;
        let fx_history_max_carry_days = parse_positive_u64("FX_HISTORY_MAX_CARRY_DAYS", 7)? as i64;
        let fx_history_chunk_days = parse_positive_u64("FX_HISTORY_CHUNK_DAYS", 366)? as usize;

        if job == MarketDataJob::FillMissingFxHistoryCache {
            let from = fx_history_date_from.ok_or_else(|| {
                MarketDataError::Config(
                    "FX_HISTORY_DATE_FROM is required for fill_missing_fx_history_cache"
                        .to_string(),
                )
            })?;
            let to = fx_history_date_to.ok_or_else(|| {
                MarketDataError::Config(
                    "FX_HISTORY_DATE_TO is required for fill_missing_fx_history_cache".to_string(),
                )
            })?;
            if from > to {
                return Err(MarketDataError::Config(format!(
                    "FX_HISTORY_DATE_FROM ({from}) must be <= FX_HISTORY_DATE_TO ({to})"
                )));
            }
            let range_days = (to - from).whole_days() + 1;
            if range_days > 366 {
                return Err(MarketDataError::Config(format!(
                    "FX history date range must be at most 366 days; got {range_days}"
                )));
            }
        }

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

        if provider == MarketDataProviderKind::Finnhub {
            let api_key = finnhub_api_key.as_deref().ok_or_else(|| {
                MarketDataError::Config(
                    "FINNHUB_API_KEY is required when MARKET_DATA_PROVIDER=finnhub".to_string(),
                )
            })?;
            if api_key.eq_ignore_ascii_case("change_me") {
                return Err(MarketDataError::Config(
                    "FINNHUB_API_KEY must be set to a real API key when MARKET_DATA_PROVIDER=finnhub"
                        .to_string(),
                ));
            }
            if symbol_allowlist.is_none() {
                return Err(MarketDataError::Config(
                    "MARKET_DATA_SYMBOL_ALLOWLIST is required when MARKET_DATA_PROVIDER=finnhub"
                        .to_string(),
                ));
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
            finnhub_api_key,
            finnhub_base_url,
            symbol_allowlist,
            provider_symbol_map,
            http_timeout,
            provider_delay,
            fx_history_provider,
            fx_history_currencies,
            fx_history_date_from,
            fx_history_date_to,
            fx_history_max_carry_days,
            fx_history_chunk_days,
        })
    }
}

fn optional_currency_list_env(key: &str) -> Result<Option<Vec<Currency>>, MarketDataError> {
    let Some(value) = optional_nonblank_env(key) else {
        return Ok(None);
    };

    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for token in value.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let currency = Currency::parse(token).map_err(|e| {
            MarketDataError::Config(format!("{key} contains invalid currency `{token}`: {e}"))
        })?;
        if seen.insert(currency) {
            out.push(currency);
        }
    }
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some(out))
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

fn optional_nonblank_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn optional_provider_symbol_map_env(key: &str) -> Result<HashMap<String, String>, MarketDataError> {
    let Some(value) = optional_nonblank_env(key) else {
        return Ok(HashMap::new());
    };

    let mut map = HashMap::new();
    for entry in value.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        let Some((canonical, provider_symbol)) = entry.split_once('=') else {
            return Err(MarketDataError::Config(format!(
                "{key} entries must use CANONICAL=PROVIDER_SYMBOL format; got `{entry}`"
            )));
        };

        let canonical = canonical.trim().to_ascii_uppercase();
        let provider_symbol = provider_symbol.trim();
        if canonical.is_empty() || provider_symbol.is_empty() {
            return Err(MarketDataError::Config(format!(
                "{key} entries must not contain blank symbols; got `{entry}`"
            )));
        }

        map.insert(canonical, provider_symbol.to_string());
    }

    Ok(map)
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

fn parse_u64(key: &str, default: u64) -> Result<u64, MarketDataError> {
    let value = env::var(key).unwrap_or_else(|_| default.to_string());
    value.parse::<u64>().map_err(|_| {
        MarketDataError::Config(format!(
            "{key} must be a non-negative integer; got `{value}`"
        ))
    })
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
        "FINNHUB_API_KEY",
        "FINNHUB_BASE_URL",
        "MARKET_DATA_SYMBOL_ALLOWLIST",
        "MARKET_DATA_PROVIDER_SYMBOL_MAP",
        "MARKET_DATA_HTTP_TIMEOUT_SECONDS",
        "MARKET_DATA_PROVIDER_DELAY_MS",
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
            std::env::remove_var("FINNHUB_API_KEY");
            std::env::remove_var("FINNHUB_BASE_URL");
            std::env::remove_var("MARKET_DATA_SYMBOL_ALLOWLIST");
            std::env::remove_var("MARKET_DATA_PROVIDER_SYMBOL_MAP");
            std::env::remove_var("MARKET_DATA_HTTP_TIMEOUT_SECONDS");
            std::env::remove_var("MARKET_DATA_PROVIDER_DELAY_MS");
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
        assert_eq!(config.finnhub_base_url, "https://finnhub.io/api/v1");
        assert_eq!(config.http_timeout.as_secs(), 10);
        assert_eq!(config.provider_delay.as_millis(), 1_100);
        assert!(config.finnhub_api_key.is_none());
        assert!(config.symbol_allowlist.is_none());
        assert!(config.provider_symbol_map.is_empty());
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
        assert_eq!(config.finnhub_base_url, "https://finnhub.io/api/v1");
        assert_eq!(config.http_timeout.as_secs(), 10);
        assert_eq!(config.provider_delay.as_millis(), 1_100);
        assert!(config.finnhub_api_key.is_none());
        assert!(config.symbol_allowlist.is_none());
        assert!(config.provider_symbol_map.is_empty());
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
    fn finnhub_requires_api_key() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_PROVIDER", "finnhub");
            std::env::set_var("MARKET_DATA_SYMBOL_ALLOWLIST", "AAPL");
        }

        let error = Config::from_env().expect_err("missing Finnhub key should fail");
        assert!(error.to_string().contains("FINNHUB_API_KEY"));
    }

    #[test]
    fn finnhub_rejects_placeholder_api_key() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_PROVIDER", "finnhub");
            std::env::set_var("FINNHUB_API_KEY", "change_me");
            std::env::set_var("MARKET_DATA_SYMBOL_ALLOWLIST", "AAPL");
        }

        let error = Config::from_env().expect_err("placeholder Finnhub key should fail");
        assert!(error.to_string().contains("FINNHUB_API_KEY"));
    }

    #[test]
    fn finnhub_requires_symbol_allowlist() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_PROVIDER", "finnhub");
            std::env::set_var("FINNHUB_API_KEY", "test_key");
        }

        let error = Config::from_env().expect_err("missing allowlist should fail");
        assert!(error.to_string().contains("MARKET_DATA_SYMBOL_ALLOWLIST"));
    }

    #[test]
    fn finnhub_accepts_required_provider_env() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_PROVIDER", "finnhub");
            std::env::set_var("FINNHUB_API_KEY", "test_key");
            std::env::set_var("FINNHUB_BASE_URL", "http://127.0.0.1:9999");
            std::env::set_var("MARKET_DATA_SYMBOL_ALLOWLIST", "aapl,MSFT");
            std::env::set_var("MARKET_DATA_PROVIDER_SYMBOL_MAP", "btc=BINANCE:BTCUSDT");
            std::env::set_var("MARKET_DATA_HTTP_TIMEOUT_SECONDS", "3");
            std::env::set_var("MARKET_DATA_PROVIDER_DELAY_MS", "0");
        }

        let config = Config::from_env().expect("Finnhub config should parse");
        assert_eq!(config.provider, MarketDataProviderKind::Finnhub);
        assert_eq!(config.finnhub_api_key.as_deref(), Some("test_key"));
        assert_eq!(config.finnhub_base_url, "http://127.0.0.1:9999");
        assert_eq!(config.http_timeout.as_secs(), 3);
        assert_eq!(config.provider_delay.as_millis(), 0);
        assert_eq!(
            config.symbol_allowlist.unwrap().symbols(),
            &["AAPL".to_string(), "MSFT".to_string()]
        );
        assert_eq!(
            config.provider_symbol_map.get("BTC").map(String::as_str),
            Some("BINANCE:BTCUSDT")
        );
    }

    #[test]
    fn mock_works_without_finnhub_vars() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();

        let config = Config::from_env().expect("mock config should parse");

        assert_eq!(config.provider, MarketDataProviderKind::Mock);
        assert!(config.finnhub_api_key.is_none());
        assert!(config.symbol_allowlist.is_none());
        assert!(config.provider_symbol_map.is_empty());
    }

    #[test]
    fn provider_symbol_map_rejects_invalid_entries() {
        let _guard = lock_env();
        let _restore = EnvRestore::capture(ENV_KEYS);
        set_minimal_env();
        unsafe {
            std::env::set_var("MARKET_DATA_PROVIDER_SYMBOL_MAP", "BTC");
        }

        let error = Config::from_env().expect_err("invalid symbol map should fail");
        assert!(
            error
                .to_string()
                .contains("MARKET_DATA_PROVIDER_SYMBOL_MAP")
        );
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
