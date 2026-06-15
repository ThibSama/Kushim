//! Canonical currency catalogue for portfolio denomination and transaction
//! entry, derived from ISO 4217 active ordinary currencies.
//!
//! Source: ISO 4217 active ordinary currency codes. Snapshot taken 2026-06-15
//! against the ISO Online Browsing Platform list of active currency codes.
//!
//! Inclusion policy: active circulating fiat currencies that a portfolio could
//! plausibly be denominated in or that a transaction could be entered in.
//!
//! Exclusion policy:
//! - precious-metal accounting units (XAU, XAG, XPD, XPT);
//! - fund or settlement units (XBA, XBB, XBC, XBD, XSU, XUA, XDR);
//! - test code (XTS) and no-currency code (XXX);
//! - cryptocurrencies (not part of ISO 4217 active list);
//! - withdrawn or replaced codes (HRK replaced by EUR in 2023, CUC withdrawn in
//!   2021, VEF replaced by VES, MRO replaced by MRU, STD replaced by STN,
//!   BYR replaced by BYN, SLL replaced by SLE).
//!
//! The catalogue is the single source of truth used for:
//! - portfolio `base_currency` validation;
//! - portfolio operation `currency` validation;
//! - the `GET /v1/reference/currencies` reference endpoint exposed to the
//!   frontend.
//!
//! Update procedure: when ISO publishes an amendment, update the static slice
//! below in alphabetical-by-code order, refresh the snapshot date in this
//! header, and add or update the relevant tests.

/// One entry of the canonical currency catalogue.
#[derive(Debug, Clone, Copy)]
pub struct CurrencyEntry {
    /// Canonical uppercase ISO 4217 three-letter code (e.g. `EUR`).
    pub code: &'static str,
    /// English label exposed by the reference endpoint for fallback display.
    pub label: &'static str,
}

/// Canonical catalogue of supported currencies, in deterministic
/// alphabetical-by-code order.
pub static SUPPORTED_CURRENCIES: &[CurrencyEntry] = &[
    CurrencyEntry {
        code: "AED",
        label: "UAE Dirham",
    },
    CurrencyEntry {
        code: "AFN",
        label: "Afghani",
    },
    CurrencyEntry {
        code: "ALL",
        label: "Lek",
    },
    CurrencyEntry {
        code: "AMD",
        label: "Armenian Dram",
    },
    CurrencyEntry {
        code: "ANG",
        label: "Netherlands Antillean Guilder",
    },
    CurrencyEntry {
        code: "AOA",
        label: "Kwanza",
    },
    CurrencyEntry {
        code: "ARS",
        label: "Argentine Peso",
    },
    CurrencyEntry {
        code: "AUD",
        label: "Australian Dollar",
    },
    CurrencyEntry {
        code: "AWG",
        label: "Aruban Florin",
    },
    CurrencyEntry {
        code: "AZN",
        label: "Azerbaijan Manat",
    },
    CurrencyEntry {
        code: "BAM",
        label: "Convertible Mark",
    },
    CurrencyEntry {
        code: "BBD",
        label: "Barbados Dollar",
    },
    CurrencyEntry {
        code: "BDT",
        label: "Taka",
    },
    CurrencyEntry {
        code: "BGN",
        label: "Bulgarian Lev",
    },
    CurrencyEntry {
        code: "BHD",
        label: "Bahraini Dinar",
    },
    CurrencyEntry {
        code: "BIF",
        label: "Burundi Franc",
    },
    CurrencyEntry {
        code: "BMD",
        label: "Bermudian Dollar",
    },
    CurrencyEntry {
        code: "BND",
        label: "Brunei Dollar",
    },
    CurrencyEntry {
        code: "BOB",
        label: "Boliviano",
    },
    CurrencyEntry {
        code: "BRL",
        label: "Brazilian Real",
    },
    CurrencyEntry {
        code: "BSD",
        label: "Bahamian Dollar",
    },
    CurrencyEntry {
        code: "BTN",
        label: "Ngultrum",
    },
    CurrencyEntry {
        code: "BWP",
        label: "Pula",
    },
    CurrencyEntry {
        code: "BYN",
        label: "Belarusian Ruble",
    },
    CurrencyEntry {
        code: "BZD",
        label: "Belize Dollar",
    },
    CurrencyEntry {
        code: "CAD",
        label: "Canadian Dollar",
    },
    CurrencyEntry {
        code: "CDF",
        label: "Congolese Franc",
    },
    CurrencyEntry {
        code: "CHF",
        label: "Swiss Franc",
    },
    CurrencyEntry {
        code: "CLP",
        label: "Chilean Peso",
    },
    CurrencyEntry {
        code: "CNY",
        label: "Yuan Renminbi",
    },
    CurrencyEntry {
        code: "COP",
        label: "Colombian Peso",
    },
    CurrencyEntry {
        code: "CRC",
        label: "Costa Rican Colon",
    },
    CurrencyEntry {
        code: "CUP",
        label: "Cuban Peso",
    },
    CurrencyEntry {
        code: "CVE",
        label: "Cabo Verde Escudo",
    },
    CurrencyEntry {
        code: "CZK",
        label: "Czech Koruna",
    },
    CurrencyEntry {
        code: "DJF",
        label: "Djibouti Franc",
    },
    CurrencyEntry {
        code: "DKK",
        label: "Danish Krone",
    },
    CurrencyEntry {
        code: "DOP",
        label: "Dominican Peso",
    },
    CurrencyEntry {
        code: "DZD",
        label: "Algerian Dinar",
    },
    CurrencyEntry {
        code: "EGP",
        label: "Egyptian Pound",
    },
    CurrencyEntry {
        code: "ERN",
        label: "Nakfa",
    },
    CurrencyEntry {
        code: "ETB",
        label: "Ethiopian Birr",
    },
    CurrencyEntry {
        code: "EUR",
        label: "Euro",
    },
    CurrencyEntry {
        code: "FJD",
        label: "Fiji Dollar",
    },
    CurrencyEntry {
        code: "FKP",
        label: "Falkland Islands Pound",
    },
    CurrencyEntry {
        code: "GBP",
        label: "Pound Sterling",
    },
    CurrencyEntry {
        code: "GEL",
        label: "Lari",
    },
    CurrencyEntry {
        code: "GHS",
        label: "Ghana Cedi",
    },
    CurrencyEntry {
        code: "GIP",
        label: "Gibraltar Pound",
    },
    CurrencyEntry {
        code: "GMD",
        label: "Dalasi",
    },
    CurrencyEntry {
        code: "GNF",
        label: "Guinean Franc",
    },
    CurrencyEntry {
        code: "GTQ",
        label: "Quetzal",
    },
    CurrencyEntry {
        code: "GYD",
        label: "Guyana Dollar",
    },
    CurrencyEntry {
        code: "HKD",
        label: "Hong Kong Dollar",
    },
    CurrencyEntry {
        code: "HNL",
        label: "Lempira",
    },
    CurrencyEntry {
        code: "HTG",
        label: "Gourde",
    },
    CurrencyEntry {
        code: "HUF",
        label: "Forint",
    },
    CurrencyEntry {
        code: "IDR",
        label: "Rupiah",
    },
    CurrencyEntry {
        code: "ILS",
        label: "New Israeli Sheqel",
    },
    CurrencyEntry {
        code: "INR",
        label: "Indian Rupee",
    },
    CurrencyEntry {
        code: "IQD",
        label: "Iraqi Dinar",
    },
    CurrencyEntry {
        code: "IRR",
        label: "Iranian Rial",
    },
    CurrencyEntry {
        code: "ISK",
        label: "Iceland Krona",
    },
    CurrencyEntry {
        code: "JMD",
        label: "Jamaican Dollar",
    },
    CurrencyEntry {
        code: "JOD",
        label: "Jordanian Dinar",
    },
    CurrencyEntry {
        code: "JPY",
        label: "Yen",
    },
    CurrencyEntry {
        code: "KES",
        label: "Kenyan Shilling",
    },
    CurrencyEntry {
        code: "KGS",
        label: "Som",
    },
    CurrencyEntry {
        code: "KHR",
        label: "Riel",
    },
    CurrencyEntry {
        code: "KMF",
        label: "Comorian Franc",
    },
    CurrencyEntry {
        code: "KRW",
        label: "Won",
    },
    CurrencyEntry {
        code: "KWD",
        label: "Kuwaiti Dinar",
    },
    CurrencyEntry {
        code: "KYD",
        label: "Cayman Islands Dollar",
    },
    CurrencyEntry {
        code: "KZT",
        label: "Tenge",
    },
    CurrencyEntry {
        code: "LAK",
        label: "Lao Kip",
    },
    CurrencyEntry {
        code: "LBP",
        label: "Lebanese Pound",
    },
    CurrencyEntry {
        code: "LKR",
        label: "Sri Lanka Rupee",
    },
    CurrencyEntry {
        code: "LRD",
        label: "Liberian Dollar",
    },
    CurrencyEntry {
        code: "LSL",
        label: "Loti",
    },
    CurrencyEntry {
        code: "LYD",
        label: "Libyan Dinar",
    },
    CurrencyEntry {
        code: "MAD",
        label: "Moroccan Dirham",
    },
    CurrencyEntry {
        code: "MDL",
        label: "Moldovan Leu",
    },
    CurrencyEntry {
        code: "MGA",
        label: "Malagasy Ariary",
    },
    CurrencyEntry {
        code: "MKD",
        label: "Denar",
    },
    CurrencyEntry {
        code: "MMK",
        label: "Kyat",
    },
    CurrencyEntry {
        code: "MNT",
        label: "Tugrik",
    },
    CurrencyEntry {
        code: "MOP",
        label: "Pataca",
    },
    CurrencyEntry {
        code: "MRU",
        label: "Ouguiya",
    },
    CurrencyEntry {
        code: "MUR",
        label: "Mauritius Rupee",
    },
    CurrencyEntry {
        code: "MVR",
        label: "Rufiyaa",
    },
    CurrencyEntry {
        code: "MWK",
        label: "Malawi Kwacha",
    },
    CurrencyEntry {
        code: "MXN",
        label: "Mexican Peso",
    },
    CurrencyEntry {
        code: "MYR",
        label: "Malaysian Ringgit",
    },
    CurrencyEntry {
        code: "MZN",
        label: "Mozambique Metical",
    },
    CurrencyEntry {
        code: "NAD",
        label: "Namibia Dollar",
    },
    CurrencyEntry {
        code: "NGN",
        label: "Naira",
    },
    CurrencyEntry {
        code: "NIO",
        label: "Cordoba Oro",
    },
    CurrencyEntry {
        code: "NOK",
        label: "Norwegian Krone",
    },
    CurrencyEntry {
        code: "NPR",
        label: "Nepalese Rupee",
    },
    CurrencyEntry {
        code: "NZD",
        label: "New Zealand Dollar",
    },
    CurrencyEntry {
        code: "OMR",
        label: "Rial Omani",
    },
    CurrencyEntry {
        code: "PAB",
        label: "Balboa",
    },
    CurrencyEntry {
        code: "PEN",
        label: "Sol",
    },
    CurrencyEntry {
        code: "PGK",
        label: "Kina",
    },
    CurrencyEntry {
        code: "PHP",
        label: "Philippine Peso",
    },
    CurrencyEntry {
        code: "PKR",
        label: "Pakistan Rupee",
    },
    CurrencyEntry {
        code: "PLN",
        label: "Zloty",
    },
    CurrencyEntry {
        code: "PYG",
        label: "Guarani",
    },
    CurrencyEntry {
        code: "QAR",
        label: "Qatari Rial",
    },
    CurrencyEntry {
        code: "RON",
        label: "Romanian Leu",
    },
    CurrencyEntry {
        code: "RSD",
        label: "Serbian Dinar",
    },
    CurrencyEntry {
        code: "RUB",
        label: "Russian Ruble",
    },
    CurrencyEntry {
        code: "RWF",
        label: "Rwanda Franc",
    },
    CurrencyEntry {
        code: "SAR",
        label: "Saudi Riyal",
    },
    CurrencyEntry {
        code: "SBD",
        label: "Solomon Islands Dollar",
    },
    CurrencyEntry {
        code: "SCR",
        label: "Seychelles Rupee",
    },
    CurrencyEntry {
        code: "SDG",
        label: "Sudanese Pound",
    },
    CurrencyEntry {
        code: "SEK",
        label: "Swedish Krona",
    },
    CurrencyEntry {
        code: "SGD",
        label: "Singapore Dollar",
    },
    CurrencyEntry {
        code: "SHP",
        label: "Saint Helena Pound",
    },
    CurrencyEntry {
        code: "SLE",
        label: "Leone",
    },
    CurrencyEntry {
        code: "SOS",
        label: "Somali Shilling",
    },
    CurrencyEntry {
        code: "SRD",
        label: "Surinam Dollar",
    },
    CurrencyEntry {
        code: "SSP",
        label: "South Sudanese Pound",
    },
    CurrencyEntry {
        code: "STN",
        label: "Dobra",
    },
    CurrencyEntry {
        code: "SVC",
        label: "El Salvador Colon",
    },
    CurrencyEntry {
        code: "SYP",
        label: "Syrian Pound",
    },
    CurrencyEntry {
        code: "SZL",
        label: "Lilangeni",
    },
    CurrencyEntry {
        code: "THB",
        label: "Baht",
    },
    CurrencyEntry {
        code: "TJS",
        label: "Somoni",
    },
    CurrencyEntry {
        code: "TMT",
        label: "Turkmenistan New Manat",
    },
    CurrencyEntry {
        code: "TND",
        label: "Tunisian Dinar",
    },
    CurrencyEntry {
        code: "TOP",
        label: "Pa'anga",
    },
    CurrencyEntry {
        code: "TRY",
        label: "Turkish Lira",
    },
    CurrencyEntry {
        code: "TTD",
        label: "Trinidad and Tobago Dollar",
    },
    CurrencyEntry {
        code: "TWD",
        label: "New Taiwan Dollar",
    },
    CurrencyEntry {
        code: "TZS",
        label: "Tanzanian Shilling",
    },
    CurrencyEntry {
        code: "UAH",
        label: "Hryvnia",
    },
    CurrencyEntry {
        code: "UGX",
        label: "Uganda Shilling",
    },
    CurrencyEntry {
        code: "USD",
        label: "US Dollar",
    },
    CurrencyEntry {
        code: "UYU",
        label: "Peso Uruguayo",
    },
    CurrencyEntry {
        code: "UZS",
        label: "Uzbekistan Sum",
    },
    CurrencyEntry {
        code: "VES",
        label: "Bolivar Soberano",
    },
    CurrencyEntry {
        code: "VND",
        label: "Dong",
    },
    CurrencyEntry {
        code: "VUV",
        label: "Vatu",
    },
    CurrencyEntry {
        code: "WST",
        label: "Tala",
    },
    CurrencyEntry {
        code: "XAF",
        label: "CFA Franc BEAC",
    },
    CurrencyEntry {
        code: "XCD",
        label: "East Caribbean Dollar",
    },
    CurrencyEntry {
        code: "XOF",
        label: "CFA Franc BCEAO",
    },
    CurrencyEntry {
        code: "XPF",
        label: "CFP Franc",
    },
    CurrencyEntry {
        code: "YER",
        label: "Yemeni Rial",
    },
    CurrencyEntry {
        code: "ZAR",
        label: "Rand",
    },
    CurrencyEntry {
        code: "ZMW",
        label: "Zambian Kwacha",
    },
    CurrencyEntry {
        code: "ZWG",
        label: "Zimbabwe Gold",
    },
];

/// Error returned when the candidate currency value is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrencyValidationError {
    /// Value was empty or only whitespace.
    Empty,
    /// Value was not exactly three letters once normalized.
    InvalidFormat,
    /// Value is a syntactically valid three-letter code but is not part of the
    /// canonical catalogue.
    Unsupported,
}

/// Trim, uppercase and validate `value` against the canonical catalogue.
///
/// On success returns the canonical uppercase code stored in
/// [`SUPPORTED_CURRENCIES`] (always 3 ASCII uppercase letters).
pub fn normalize_and_validate(value: &str) -> Result<&'static str, CurrencyValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CurrencyValidationError::Empty);
    }

    let upper = trimmed.to_ascii_uppercase();
    if upper.len() != 3 || !upper.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(CurrencyValidationError::InvalidFormat);
    }

    SUPPORTED_CURRENCIES
        .iter()
        .find(|entry| entry.code == upper.as_str())
        .map(|entry| entry.code)
        .ok_or(CurrencyValidationError::Unsupported)
}

/// Returns `true` if `value` (already canonical uppercase) is part of the
/// catalogue. Use for fast contains checks when normalization already happened.
pub fn is_supported(value: &str) -> bool {
    SUPPORTED_CURRENCIES.iter().any(|entry| entry.code == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_includes_mandatory_minimum() {
        for code in ["EUR", "USD", "GBP", "JPY", "CHF", "CAD", "AUD"] {
            assert!(is_supported(code), "expected {code} in catalogue");
        }
    }

    #[test]
    fn catalogue_codes_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for entry in SUPPORTED_CURRENCIES {
            assert!(
                seen.insert(entry.code),
                "duplicate currency code in catalogue: {}",
                entry.code
            );
        }
    }

    #[test]
    fn catalogue_codes_are_uppercase_and_three_letters() {
        for entry in SUPPORTED_CURRENCIES {
            assert_eq!(entry.code.len(), 3, "code length: {}", entry.code);
            assert!(
                entry.code.chars().all(|c| c.is_ascii_uppercase()),
                "code is not uppercase ASCII: {}",
                entry.code
            );
        }
    }

    #[test]
    fn catalogue_is_alphabetically_ordered() {
        let mut previous: Option<&str> = None;
        for entry in SUPPORTED_CURRENCIES {
            if let Some(prev) = previous {
                assert!(
                    prev < entry.code,
                    "catalogue is not alphabetically ordered at {} (after {})",
                    entry.code,
                    prev
                );
            }
            previous = Some(entry.code);
        }
    }

    #[test]
    fn catalogue_excludes_documented_special_codes() {
        for code in [
            "XAU", "XAG", "XPD", "XPT", // precious metals
            "XBA", "XBB", "XBC", "XBD", // bond market units
            "XSU", "XUA", "XDR", // settlement / fund units
            "XTS", // test code
            "XXX", // no-currency
            "BTC", "XBT", // crypto, never ISO 4217 active
        ] {
            assert!(
                !is_supported(code),
                "excluded code {code} must not appear in catalogue"
            );
        }
    }

    #[test]
    fn normalize_accepts_lowercase_and_whitespace() {
        assert_eq!(normalize_and_validate("  eur "), Ok("EUR"));
        assert_eq!(normalize_and_validate("usd"), Ok("USD"));
        assert_eq!(normalize_and_validate("EUR"), Ok("EUR"));
    }

    #[test]
    fn normalize_rejects_unknown_three_letter_code() {
        assert_eq!(
            normalize_and_validate("ZZZ"),
            Err(CurrencyValidationError::Unsupported)
        );
    }

    #[test]
    fn normalize_rejects_invalid_format() {
        assert_eq!(
            normalize_and_validate("EURO"),
            Err(CurrencyValidationError::InvalidFormat)
        );
        assert_eq!(
            normalize_and_validate("EU"),
            Err(CurrencyValidationError::InvalidFormat)
        );
        assert_eq!(
            normalize_and_validate("12A"),
            Err(CurrencyValidationError::InvalidFormat)
        );
    }

    #[test]
    fn normalize_rejects_empty() {
        assert_eq!(
            normalize_and_validate(""),
            Err(CurrencyValidationError::Empty)
        );
        assert_eq!(
            normalize_and_validate("   "),
            Err(CurrencyValidationError::Empty)
        );
    }
}
