use crate::{
    domain::{ActiveAsset, CurrentQuote, HistoricalQuote},
    errors::MarketDataError,
    providers::MarketDataProvider,
    symbol_filter::asset_lookup_symbol,
};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde_json::Value;
use std::{collections::HashMap, str::FromStr, time::Duration};
use time::{Date, OffsetDateTime};

const SOURCE: &str = "finnhub";
const DEFAULT_CURRENCY: &str = "USD";

pub struct FinnhubProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    provider_delay: Duration,
    provider_symbol_map: HashMap<String, String>,
}

impl FinnhubProvider {
    pub fn new(
        base_url: String,
        api_key: String,
        http_timeout: Duration,
        provider_delay: Duration,
        provider_symbol_map: HashMap<String, String>,
    ) -> Result<Self, MarketDataError> {
        let client = reqwest::Client::builder()
            .timeout(http_timeout)
            .build()
            .map_err(|e| MarketDataError::Provider(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            provider_delay,
            provider_symbol_map,
        })
    }

    async fn request_json(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, MarketDataError> {
        if !self.provider_delay.is_zero() {
            tokio::time::sleep(self.provider_delay).await;
        }

        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let response = self
            .client
            .get(url)
            .query(query)
            .header("X-Finnhub-Token", &self.api_key)
            .send()
            .await
            .map_err(|e| MarketDataError::Provider(format!("Finnhub request failed: {e}")))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(MarketDataError::Provider(
                "Finnhub request unauthorized: check the configured API key".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            let message = if path.trim_start_matches('/') == "stock/candle" {
                "Finnhub historical candles access forbidden: the API key or plan may not allow /stock/candle"
            } else {
                "Finnhub endpoint access forbidden: the API key or plan may not allow this endpoint"
            };
            return Err(MarketDataError::Provider(message.to_string()));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(MarketDataError::Provider(
                "Finnhub rate limit response (HTTP 429)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(MarketDataError::Provider(format!(
                "Finnhub returned HTTP status {status}"
            )));
        }

        response
            .json::<Value>()
            .await
            .map_err(|e| MarketDataError::Provider(format!("Finnhub JSON parsing failed: {e}")))
    }

    fn provider_symbol_for_asset(&self, asset: &ActiveAsset) -> Option<String> {
        let canonical = asset_lookup_symbol(asset)?;
        self.provider_symbol_map
            .get(&canonical)
            .cloned()
            .or(Some(canonical))
    }
}

impl MarketDataProvider for FinnhubProvider {
    fn name(&self) -> &'static str {
        SOURCE
    }

    async fn get_quote(
        &self,
        asset: &ActiveAsset,
    ) -> Result<Option<CurrentQuote>, MarketDataError> {
        let Some(symbol) = self.provider_symbol_for_asset(asset) else {
            return Ok(None);
        };

        let payload = self.request_json("quote", &[("symbol", symbol)]).await?;

        parse_quote_payload(&payload)
    }

    async fn get_historical_quote(
        &self,
        asset: &ActiveAsset,
        date: Date,
    ) -> Result<Option<HistoricalQuote>, MarketDataError> {
        let Some(symbol) = self.provider_symbol_for_asset(asset) else {
            return Ok(None);
        };

        let from = date_to_unix_start(date);
        let to = date_to_unix_end(date);
        let payload = self
            .request_json(
                "stock/candle",
                &[
                    ("symbol", symbol),
                    ("resolution", "D".to_string()),
                    ("from", from.to_string()),
                    ("to", to.to_string()),
                ],
            )
            .await?;

        parse_candle_payload(&payload, date)
    }
}

fn parse_quote_payload(payload: &Value) -> Result<Option<CurrentQuote>, MarketDataError> {
    reject_error_payload(payload)?;

    let Some(price_minor) = price_value_to_minor(payload.get("c"))? else {
        return Ok(None);
    };

    if price_minor <= 0 {
        return Ok(None);
    }

    let as_of = payload
        .get("t")
        .and_then(Value::as_i64)
        .filter(|timestamp| *timestamp > 0)
        .and_then(|timestamp| OffsetDateTime::from_unix_timestamp(timestamp).ok())
        .unwrap_or_else(OffsetDateTime::now_utc);

    Ok(Some(CurrentQuote {
        price_minor,
        currency: DEFAULT_CURRENCY.to_string(),
        data_source: SOURCE.to_string(),
        as_of,
    }))
}

fn parse_candle_payload(
    payload: &Value,
    date: Date,
) -> Result<Option<HistoricalQuote>, MarketDataError> {
    reject_error_payload(payload)?;

    match payload.get("s").and_then(Value::as_str) {
        Some("no_data") => return Ok(None),
        Some("ok") => {}
        Some(status) => {
            return Err(MarketDataError::Provider(format!(
                "Finnhub candle status `{status}`"
            )));
        }
        None => return Ok(None),
    }

    let Some(closes) = payload.get("c").and_then(Value::as_array) else {
        return Ok(None);
    };
    let Some(timestamps) = payload.get("t").and_then(Value::as_array) else {
        return Ok(None);
    };

    for (idx, timestamp) in timestamps.iter().enumerate() {
        let Some(timestamp) = timestamp.as_i64() else {
            continue;
        };
        let Ok(price_date) = OffsetDateTime::from_unix_timestamp(timestamp).map(|dt| dt.date())
        else {
            continue;
        };

        if price_date == date {
            let Some(close) = closes.get(idx) else {
                return Ok(None);
            };
            let Some(close_minor) = price_value_to_minor(Some(close))? else {
                return Ok(None);
            };

            return Ok(Some(HistoricalQuote {
                close_minor,
                currency: DEFAULT_CURRENCY.to_string(),
                data_source: SOURCE.to_string(),
                price_date: date,
            }));
        }
    }

    Ok(None)
}

fn reject_error_payload(payload: &Value) -> Result<(), MarketDataError> {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return Err(MarketDataError::Provider(format!("Finnhub error: {error}")));
    }
    if let Some(message) = payload.get("message").and_then(Value::as_str)
        && message.to_ascii_lowercase().contains("limit")
    {
        return Err(MarketDataError::Provider(format!(
            "Finnhub rate limit response: {message}"
        )));
    }
    Ok(())
}

fn price_value_to_minor(value: Option<&Value>) -> Result<Option<i64>, MarketDataError> {
    let Some(value) = value else {
        return Ok(None);
    };

    let raw = match value {
        Value::Number(number) => number.to_string(),
        Value::String(value) => value.clone(),
        _ => return Ok(None),
    };

    price_str_to_minor(&raw).map(Some)
}

fn price_str_to_minor(value: &str) -> Result<i64, MarketDataError> {
    let decimal = Decimal::from_str(value.trim()).map_err(|_| {
        MarketDataError::Provider("Finnhub price value is not a valid decimal".to_string())
    })?;
    let cents = (decimal * Decimal::from(100)).round_dp(0);

    cents
        .to_i64()
        .ok_or_else(|| MarketDataError::Provider("Finnhub price value is out of range".to_string()))
}

fn date_to_unix_start(date: Date) -> i64 {
    date.midnight().assume_utc().unix_timestamp()
}

fn date_to_unix_end(date: Date) -> i64 {
    let next = date.next_day().unwrap_or(date);
    next.midnight().assume_utc().unix_timestamp() - 1
}

#[cfg(test)]
mod tests {
    use super::{FinnhubProvider, parse_candle_payload, parse_quote_payload, price_str_to_minor};
    use crate::{domain::ActiveAsset, providers::MarketDataProvider};
    use serde_json::json;
    use std::{collections::HashMap, time::Duration};
    use time::{Date, Month};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use uuid::Uuid;

    fn asset(symbol: &str) -> ActiveAsset {
        ActiveAsset {
            id_asset: Uuid::new_v4(),
            symbol: Some(symbol.to_string()),
            ticker: None,
            native_currency: Some("USD".to_string()),
        }
    }

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
    }

    #[test]
    fn parses_successful_quote_fixture() {
        let payload = json!({
            "c": 195.23,
            "d": 1.1,
            "dp": 0.5,
            "h": 196.0,
            "l": 194.0,
            "o": 195.0,
            "pc": 194.13,
            "t": 1770652800
        });

        let quote = parse_quote_payload(&payload)
            .expect("payload should parse")
            .expect("quote should be present");

        assert_eq!(quote.price_minor, 19_523);
        assert_eq!(quote.currency, "USD");
        assert_eq!(quote.data_source, "finnhub");
    }

    #[test]
    fn parse_quote_handles_missing_price() {
        let payload = json!({ "t": 1770652800 });

        let quote = parse_quote_payload(&payload).expect("missing price should not error");

        assert!(quote.is_none());
    }

    #[test]
    fn parse_quote_rejects_error_payload() {
        let payload = json!({ "error": "API limit reached" });

        let error = parse_quote_payload(&payload).expect_err("error payload should fail");

        assert!(error.to_string().contains("Finnhub error"));
    }

    #[test]
    fn parse_quote_rejects_rate_limit_message_payload() {
        let payload = json!({ "message": "API limit reached" });

        let error = parse_quote_payload(&payload).expect_err("rate-limit payload should fail");

        assert!(error.to_string().contains("rate limit"));
    }

    #[test]
    fn parses_successful_daily_candle_fixture() {
        let payload = json!({
            "c": [195.23],
            "h": [196.0],
            "l": [194.0],
            "o": [195.0],
            "s": "ok",
            "t": [1770681600],
            "v": [1000]
        });

        let quote = parse_candle_payload(&payload, date(2026, 2, 10))
            .expect("payload should parse")
            .expect("historical quote should be present");

        assert_eq!(quote.close_minor, 19_523);
        assert_eq!(quote.currency, "USD");
        assert_eq!(quote.data_source, "finnhub");
        assert_eq!(quote.price_date, date(2026, 2, 10));
    }

    #[test]
    fn parse_candle_no_data_returns_none() {
        let payload = json!({ "s": "no_data" });

        let quote =
            parse_candle_payload(&payload, date(2026, 2, 10)).expect("no_data should not error");

        assert!(quote.is_none());
    }

    #[test]
    fn converts_prices_to_minor_units_deterministically() {
        assert_eq!(price_str_to_minor("195.23").unwrap(), 19_523);
        assert_eq!(price_str_to_minor("195").unwrap(), 19_500);
        assert_eq!(price_str_to_minor("0.015").unwrap(), 2);
        assert_eq!(price_str_to_minor("195.235").unwrap(), 19_524);
    }

    #[tokio::test]
    async fn provider_uses_mocked_http_quote_response() {
        let base_url = spawn_response_server(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nconnection: close\r\n\r\n{\"c\":195.23,\"t\":1}",
        )
        .await;
        let provider = FinnhubProvider::new(
            base_url,
            "test_key".to_string(),
            Duration::from_secs(5),
            Duration::from_millis(0),
            HashMap::new(),
        )
        .expect("provider should build");

        let quote = provider
            .get_quote(&asset("AAPL"))
            .await
            .expect("HTTP quote should succeed")
            .expect("quote should exist");

        assert_eq!(quote.price_minor, 19_523);
    }

    #[tokio::test]
    async fn provider_handles_http_rate_limit() {
        let base_url =
            spawn_response_server("HTTP/1.1 429 Too Many Requests\r\ncontent-length: 0\r\n\r\n")
                .await;
        let provider = FinnhubProvider::new(
            base_url,
            "test_key".to_string(),
            Duration::from_secs(5),
            Duration::from_millis(0),
            HashMap::new(),
        )
        .expect("provider should build");

        let error = provider
            .get_quote(&asset("AAPL"))
            .await
            .expect_err("rate limit should fail");

        assert!(error.to_string().contains("429"));
    }

    #[tokio::test]
    async fn provider_uses_symbol_map_for_request() {
        let base_url = spawn_asserting_response_server(
            "symbol=BINANCE%3ABTCUSDT",
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nconnection: close\r\n\r\n{\"c\":281234.56,\"t\":1}",
        )
        .await;
        let provider = FinnhubProvider::new(
            base_url,
            "test_key".to_string(),
            Duration::from_secs(5),
            Duration::from_millis(0),
            HashMap::from([("BTC".to_string(), "BINANCE:BTCUSDT".to_string())]),
        )
        .expect("provider should build");

        let quote = provider
            .get_quote(&asset("BTC"))
            .await
            .expect("mapped HTTP quote should succeed")
            .expect("quote should exist");

        assert_eq!(quote.price_minor, 28_123_456);
    }

    #[tokio::test]
    async fn provider_quote_403_reports_access_error_without_api_key() {
        let base_url =
            spawn_response_server("HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\n\r\n").await;
        let api_key = "test_secret_key";
        let provider = FinnhubProvider::new(
            base_url,
            api_key.to_string(),
            Duration::from_secs(5),
            Duration::from_millis(0),
            HashMap::new(),
        )
        .expect("provider should build");

        let error = provider
            .get_quote(&asset("AAPL"))
            .await
            .expect_err("forbidden quote should fail");
        let error = error.to_string();

        assert!(error.contains("endpoint access forbidden"));
        assert!(!error.contains(api_key));
    }

    #[tokio::test]
    async fn provider_candle_403_reports_entitlement_error_without_api_key() {
        let base_url =
            spawn_response_server("HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\n\r\n").await;
        let api_key = "test_secret_key";
        let provider = FinnhubProvider::new(
            base_url,
            api_key.to_string(),
            Duration::from_secs(5),
            Duration::from_millis(0),
            HashMap::new(),
        )
        .expect("provider should build");

        let error = provider
            .get_historical_quote(&asset("AAPL"), date(2026, 6, 1))
            .await
            .expect_err("forbidden candle should fail");
        let error = error.to_string();

        assert!(error.contains("historical candles access forbidden"));
        assert!(error.contains("/stock/candle"));
        assert!(!error.contains(api_key));
    }

    async fn spawn_response_server(response: &'static str) -> String {
        spawn_asserting_response_server("", response).await
    }

    async fn spawn_asserting_response_server(
        expected_request_fragment: &'static str,
        response: &'static str,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should exist");

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("connection should arrive");
            let mut buffer = [0_u8; 2048];
            let read = socket.read(&mut buffer).await.expect("request should read");
            let request = String::from_utf8_lossy(&buffer[..read]);
            assert!(
                request.contains(expected_request_fragment),
                "request `{request}` should contain `{expected_request_fragment}`"
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response should be written");
        });

        format!("http://{addr}")
    }
}
