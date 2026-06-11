use crate::domain::ActiveAsset;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolAllowlist {
    symbols: Vec<String>,
}

impl SymbolAllowlist {
    pub fn parse(value: &str) -> Option<Self> {
        let mut seen = HashSet::new();
        let mut symbols = Vec::new();

        for raw in value.split(',') {
            let symbol = raw.trim().to_ascii_uppercase();
            if symbol.is_empty() || !seen.insert(symbol.clone()) {
                continue;
            }
            symbols.push(symbol);
        }

        if symbols.is_empty() {
            None
        } else {
            Some(Self { symbols })
        }
    }

    pub fn contains_asset(&self, asset: &ActiveAsset) -> bool {
        asset_lookup_symbol(asset).is_some_and(|symbol| self.symbols.contains(&symbol))
    }

    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }
}

#[derive(Debug, Clone)]
pub struct AssetSelection {
    pub selected: Vec<ActiveAsset>,
    pub missing_symbols: Vec<String>,
}

pub fn select_assets_by_allowlist(
    assets: &[ActiveAsset],
    allowlist: Option<&SymbolAllowlist>,
) -> AssetSelection {
    let Some(allowlist) = allowlist else {
        return AssetSelection {
            selected: assets.to_vec(),
            missing_symbols: Vec::new(),
        };
    };

    let selected: Vec<ActiveAsset> = assets
        .iter()
        .filter(|asset| allowlist.contains_asset(asset))
        .cloned()
        .collect();

    let available: HashSet<String> = assets.iter().filter_map(asset_lookup_symbol).collect();
    let missing_symbols = allowlist
        .symbols()
        .iter()
        .filter(|symbol| !available.contains(*symbol))
        .cloned()
        .collect();

    AssetSelection {
        selected,
        missing_symbols,
    }
}

pub fn asset_lookup_symbol(asset: &ActiveAsset) -> Option<String> {
    asset
        .symbol
        .as_deref()
        .filter(|symbol| !symbol.trim().is_empty())
        .or_else(|| {
            asset
                .ticker
                .as_deref()
                .filter(|ticker| !ticker.trim().is_empty())
        })
        .map(|symbol| symbol.trim().to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::{SymbolAllowlist, asset_lookup_symbol, select_assets_by_allowlist};
    use crate::domain::ActiveAsset;
    use uuid::Uuid;

    fn asset(symbol: Option<&str>, ticker: Option<&str>) -> ActiveAsset {
        ActiveAsset {
            id_asset: Uuid::new_v4(),
            symbol: symbol.map(ToString::to_string),
            ticker: ticker.map(ToString::to_string),
            native_currency: Some("USD".to_string()),
        }
    }

    #[test]
    fn parses_allowlist_case_insensitively_and_deduplicates() {
        let allowlist =
            SymbolAllowlist::parse(" aapl,MSFT,aapl ,, nvda ").expect("allowlist should parse");

        assert_eq!(allowlist.symbols(), &["AAPL", "MSFT", "NVDA"]);
    }

    #[test]
    fn blank_allowlist_is_rejected() {
        assert!(SymbolAllowlist::parse(" , ").is_none());
    }

    #[test]
    fn allowlist_selects_only_intended_assets() {
        let allowlist = SymbolAllowlist::parse("AAPL,NVDA").unwrap();
        let assets = vec![
            asset(Some("AAPL"), None),
            asset(Some("MSFT"), None),
            asset(Some("nvda"), None),
        ];

        let selection = select_assets_by_allowlist(&assets, Some(&allowlist));

        assert_eq!(selection.selected.len(), 2);
        assert!(selection.missing_symbols.is_empty());
    }

    #[test]
    fn symbol_is_preferred_over_ticker() {
        let selected_symbol = asset_lookup_symbol(&asset(Some("MSFT"), Some("AAPL")));

        assert_eq!(selected_symbol.as_deref(), Some("MSFT"));
    }

    #[test]
    fn ticker_is_used_when_symbol_is_absent() {
        let selected_symbol = asset_lookup_symbol(&asset(None, Some("VTI")));

        assert_eq!(selected_symbol.as_deref(), Some("VTI"));
    }

    #[test]
    fn missing_allowlisted_symbols_are_reported() {
        let allowlist = SymbolAllowlist::parse("AAPL,TSLA").unwrap();
        let assets = vec![asset(Some("AAPL"), None)];

        let selection = select_assets_by_allowlist(&assets, Some(&allowlist));

        assert_eq!(selection.selected.len(), 1);
        assert_eq!(selection.missing_symbols, vec!["TSLA"]);
    }
}
