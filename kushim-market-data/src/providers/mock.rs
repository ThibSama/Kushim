use crate::domain::{ActiveAsset, CurrentQuote, HistoricalQuote};
use crate::providers::MarketDataProvider;
use time::{Date, OffsetDateTime};

pub struct MockProvider;

struct MockQuote {
    key: &'static str,
    price_minor: i64,
    currency: &'static str,
}

const MOCK_QUOTES: &[MockQuote] = &[
    MockQuote {
        key: "AAPL",
        price_minor: 19_523,
        currency: "USD",
    },
    MockQuote {
        key: "MSFT",
        price_minor: 42_150,
        currency: "USD",
    },
    MockQuote {
        key: "NVDA",
        price_minor: 87_640,
        currency: "USD",
    },
    MockQuote {
        key: "BTC",
        price_minor: 670_000_000,
        currency: "USD",
    },
    MockQuote {
        key: "ETH",
        price_minor: 350_000,
        currency: "USD",
    },
    MockQuote {
        key: "SPY",
        price_minor: 52_830,
        currency: "USD",
    },
    MockQuote {
        key: "VTI",
        price_minor: 26_410,
        currency: "USD",
    },
];

fn deterministic_historical_price(base_price: i64, date: Date) -> i64 {
    let ordinal = date.ordinal() as i64;
    let year_factor = (date.year() % 100).unsigned_abs() as i64;
    let variation = (ordinal * 7 + year_factor * 13) % 100;
    base_price + (base_price * (variation - 50)) / 2000
}

impl MarketDataProvider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn get_quote(
        &self,
        asset: &ActiveAsset,
    ) -> Result<Option<CurrentQuote>, crate::errors::MarketDataError> {
        let Some(lookup_key) = asset.symbol.as_deref().or(asset.ticker.as_deref()) else {
            return Ok(None);
        };

        let Some(mock) = MOCK_QUOTES
            .iter()
            .find(|q| q.key.eq_ignore_ascii_case(lookup_key))
        else {
            return Ok(None);
        };

        Ok(Some(CurrentQuote {
            price_minor: mock.price_minor,
            currency: mock.currency.to_string(),
            data_source: "mock".to_string(),
            as_of: OffsetDateTime::now_utc(),
        }))
    }

    async fn get_historical_quote(
        &self,
        asset: &ActiveAsset,
        date: Date,
    ) -> Result<Option<HistoricalQuote>, crate::errors::MarketDataError> {
        let Some(lookup_key) = asset.symbol.as_deref().or(asset.ticker.as_deref()) else {
            return Ok(None);
        };

        let Some(mock) = MOCK_QUOTES
            .iter()
            .find(|q| q.key.eq_ignore_ascii_case(lookup_key))
        else {
            return Ok(None);
        };

        Ok(Some(HistoricalQuote {
            close_minor: deterministic_historical_price(mock.price_minor, date),
            currency: mock.currency.to_string(),
            data_source: "mock".to_string(),
            price_date: date,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::MockProvider;
    use crate::domain::ActiveAsset;
    use crate::providers::MarketDataProvider;
    use uuid::Uuid;

    fn asset_with_symbol(symbol: &str) -> ActiveAsset {
        ActiveAsset {
            id_asset: Uuid::new_v4(),
            symbol: Some(symbol.to_string()),
            ticker: None,
            native_currency: Some("USD".to_string()),
        }
    }

    fn asset_with_ticker(ticker: &str) -> ActiveAsset {
        ActiveAsset {
            id_asset: Uuid::new_v4(),
            symbol: None,
            ticker: Some(ticker.to_string()),
            native_currency: Some("USD".to_string()),
        }
    }

    #[tokio::test]
    async fn returns_quote_for_supported_symbol() {
        let provider = MockProvider;
        let quote = provider
            .get_quote(&asset_with_symbol("AAPL"))
            .await
            .expect("mock provider should not error")
            .expect("AAPL should be supported");

        assert_eq!(quote.price_minor, 19_523);
        assert_eq!(quote.currency, "USD");
        assert_eq!(quote.data_source, "mock");
    }

    #[tokio::test]
    async fn returns_quote_via_ticker_fallback() {
        let provider = MockProvider;
        let quote = provider
            .get_quote(&asset_with_ticker("MSFT"))
            .await
            .expect("mock provider should not error")
            .expect("MSFT ticker should be supported");

        assert_eq!(quote.price_minor, 42_150);
    }

    #[tokio::test]
    async fn returns_none_for_unsupported_symbol() {
        let provider = MockProvider;
        assert!(
            provider
                .get_quote(&asset_with_symbol("UNKNOWN"))
                .await
                .expect("mock provider should not error")
                .is_none()
        );
    }

    #[tokio::test]
    async fn returns_none_for_asset_without_identifiers() {
        let provider = MockProvider;
        let asset = ActiveAsset {
            id_asset: Uuid::new_v4(),
            symbol: None,
            ticker: None,
            native_currency: None,
        };
        assert!(
            provider
                .get_quote(&asset)
                .await
                .expect("mock provider should not error")
                .is_none()
        );
    }

    #[tokio::test]
    async fn case_insensitive_matching() {
        let provider = MockProvider;
        let quote = provider
            .get_quote(&asset_with_symbol("aapl"))
            .await
            .expect("mock provider should not error")
            .expect("lowercase aapl should match");
        assert_eq!(quote.price_minor, 19_523);
    }

    #[tokio::test]
    async fn crypto_quote_is_deterministic() {
        let provider = MockProvider;
        let quote = provider
            .get_quote(&asset_with_symbol("BTC"))
            .await
            .expect("mock provider should not error")
            .expect("BTC should be supported");
        assert_eq!(quote.price_minor, 670_000_000);
        assert_eq!(quote.currency, "USD");
    }

    #[tokio::test]
    async fn historical_quote_for_supported_asset() {
        let provider = MockProvider;
        let date = time::Date::from_calendar_date(2026, time::Month::January, 15).unwrap();
        let quote = provider
            .get_historical_quote(&asset_with_symbol("AAPL"), date)
            .await
            .expect("mock provider should not error")
            .expect("AAPL should be supported");

        assert!(quote.close_minor > 0);
        assert_eq!(quote.currency, "USD");
        assert_eq!(quote.data_source, "mock");
        assert_eq!(quote.price_date, date);
    }

    #[tokio::test]
    async fn historical_quote_is_deterministic() {
        let provider = MockProvider;
        let date = time::Date::from_calendar_date(2026, time::Month::March, 10).unwrap();
        let q1 = provider
            .get_historical_quote(&asset_with_symbol("MSFT"), date)
            .await
            .expect("mock provider should not error")
            .unwrap();
        let q2 = provider
            .get_historical_quote(&asset_with_symbol("MSFT"), date)
            .await
            .expect("mock provider should not error")
            .unwrap();

        assert_eq!(q1.close_minor, q2.close_minor);
    }

    #[tokio::test]
    async fn historical_quote_varies_by_date() {
        let provider = MockProvider;
        let d1 = time::Date::from_calendar_date(2026, time::Month::January, 1).unwrap();
        let d2 = time::Date::from_calendar_date(2026, time::Month::June, 15).unwrap();
        let q1 = provider
            .get_historical_quote(&asset_with_symbol("AAPL"), d1)
            .await
            .expect("mock provider should not error")
            .unwrap();
        let q2 = provider
            .get_historical_quote(&asset_with_symbol("AAPL"), d2)
            .await
            .expect("mock provider should not error")
            .unwrap();

        assert_ne!(q1.close_minor, q2.close_minor);
    }

    #[tokio::test]
    async fn historical_quote_none_for_unsupported() {
        let provider = MockProvider;
        let date = time::Date::from_calendar_date(2026, time::Month::January, 1).unwrap();
        assert!(
            provider
                .get_historical_quote(&asset_with_symbol("UNKNOWN"), date)
                .await
                .expect("mock provider should not error")
                .is_none()
        );
    }
}
