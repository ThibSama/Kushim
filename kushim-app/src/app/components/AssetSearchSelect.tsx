import React, { useState, useEffect, useRef, useCallback } from "react";
import { Search, X, Loader2 } from "lucide-react";
import { type Asset, listAssets } from "../../lib/api/businessApi";
import { useAuthStore } from "../../stores/auth";

type Props = {
  selectedAsset: Asset | null;
  onSelect: (asset: Asset | null) => void;
  label?: string;
  error?: string;
};

const ASSET_CLASS_LABELS: Record<string, string> = {
  equity: "Action",
  etf: "ETF",
  fund: "Fonds",
  bond: "Obligation",
  crypto: "Crypto",
  commodity: "Matière première",
  cash: "Cash",
  forex: "Forex",
  index: "Indice",
  real_estate: "Immobilier",
  private_equity: "Private Equity",
  derivative: "Dérivé",
  other: "Autre",
};

function assetClassLabel(cls: string): string {
  return ASSET_CLASS_LABELS[cls] ?? cls;
}

export function AssetSearchSelect({ selectedAsset, onSelect, label, error }: Props) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Asset[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const doSearch = useCallback(async (searchQuery: string) => {
    const token = useAuthStore.getState().token;
    if (!token) return;

    setLoading(true);
    setSearchError(null);
    try {
      const { assets } = await listAssets(token, {
        search: searchQuery || undefined,
        status: "active",
        limit: 20,
      });
      setResults(assets);
    } catch (e) {
      setSearchError(e instanceof Error ? e.message : "Erreur de recherche");
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      doSearch(query);
    }, 300);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query, open, doSearch]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    if (open) document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [open]);

  const handleSelect = (asset: Asset) => {
    onSelect(asset);
    setOpen(false);
    setQuery("");
  };

  const handleClear = () => {
    onSelect(null);
    setQuery("");
  };

  if (selectedAsset) {
    return (
      <div className="w-full">
        {label && (
          <label className="block mb-1.5" style={{ fontSize: "12px", fontWeight: 500, color: "var(--text-secondary)" }}>
            {label}
          </label>
        )}
        <div
          className="glass-field w-full px-4 py-2.5 rounded-[9999px] flex items-center justify-between gap-2"
          style={{ border: error ? "1px solid var(--color-loss)" : "1px solid var(--surface-2-border)" }}>
          <div className="flex items-center gap-2 min-w-0">
            <span style={{ fontSize: "14px", fontWeight: 600, color: "var(--text-primary)" }}>
              {selectedAsset.ticker ?? selectedAsset.name}
            </span>
            <span style={{ fontSize: "12px", color: "var(--text-tertiary)" }}>
              {selectedAsset.ticker ? selectedAsset.name : ""}
            </span>
            <span
              className="rounded-full px-2 py-0.5"
              style={{ fontSize: "10px", fontWeight: 600, background: "var(--surface-2-bg)", color: "var(--text-tertiary)" }}>
              {assetClassLabel(selectedAsset.asset_class)}
            </span>
          </div>
          <button
            type="button"
            onClick={handleClear}
            style={{ flexShrink: 0, color: "var(--text-tertiary)", cursor: "pointer", background: "none", border: "none", padding: "4px" }}>
            <X size={16} />
          </button>
        </div>
        {error && <p className="mt-1" style={{ fontSize: "12px", color: "var(--color-loss)" }}>{error}</p>}
      </div>
    );
  }

  return (
    <div className="w-full relative" ref={containerRef}>
      {label && (
        <label className="block mb-1.5" style={{ fontSize: "12px", fontWeight: 500, color: "var(--text-secondary)" }}>
          {label}
        </label>
      )}
      <div className="relative">
        <Search size={16} className="absolute left-4 top-1/2 -translate-y-1/2" style={{ color: "var(--text-tertiary)" }} />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onFocus={() => setOpen(true)}
          placeholder="Rechercher un actif (ticker, nom, ISIN)…"
          className="glass-field w-full py-3 rounded-[9999px]"
          style={{
            paddingLeft: "40px",
            paddingRight: "16px",
            border: error ? "1px solid var(--color-loss)" : "1px solid var(--surface-2-border)",
            fontSize: "15px",
            color: "var(--text-primary)",
          }}
        />
        {loading && (
          <Loader2 size={16} className="absolute right-4 top-1/2 -translate-y-1/2 animate-spin" style={{ color: "var(--text-tertiary)" }} />
        )}
      </div>
      {error && <p className="mt-1" style={{ fontSize: "12px", color: "var(--color-loss)" }}>{error}</p>}

      {open && (
        <div
          className="absolute z-50 w-full mt-1 rounded-xl overflow-hidden"
          style={{
            background: "var(--surface-3-bg)",
            border: "1px solid var(--surface-3-border)",
            backdropFilter: "blur(16px)",
            maxHeight: "280px",
            overflowY: "auto",
          }}>
          {loading && results.length === 0 && (
            <div className="flex items-center justify-center gap-2 py-6" style={{ color: "var(--text-tertiary)", fontSize: "13px" }}>
              <Loader2 size={14} className="animate-spin" />
              Recherche…
            </div>
          )}
          {searchError && (
            <div className="py-4 px-4" style={{ color: "var(--color-loss)", fontSize: "13px" }}>
              {searchError}
            </div>
          )}
          {!loading && !searchError && results.length === 0 && (
            <div className="py-6 px-4 text-center" style={{ color: "var(--text-tertiary)", fontSize: "13px" }}>
              {query
                ? "Aucun actif trouvé pour cette recherche."
                : "Aucun actif disponible. Les actifs doivent être seedés côté backend."}
            </div>
          )}
          {results.map((asset) => (
            <button
              key={asset.id_asset}
              type="button"
              onClick={() => handleSelect(asset)}
              className="w-full text-left px-4 py-2.5 transition-colors"
              style={{ cursor: "pointer", background: "transparent", border: "none" }}
              onMouseEnter={(e) => { e.currentTarget.style.background = "var(--surface-2-bg)"; }}
              onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}>
              <div className="flex items-center gap-2">
                <span style={{ fontSize: "14px", fontWeight: 600, color: "var(--text-primary)", minWidth: "48px" }}>
                  {asset.ticker ?? "—"}
                </span>
                <span style={{ fontSize: "13px", color: "var(--text-secondary)", flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {asset.name}
                </span>
                <span
                  className="rounded-full px-2 py-0.5 flex-shrink-0"
                  style={{ fontSize: "10px", fontWeight: 600, background: "var(--surface-1-bg)", color: "var(--text-tertiary)" }}>
                  {assetClassLabel(asset.asset_class)}
                </span>
                {asset.exchange && (
                  <span style={{ fontSize: "10px", color: "var(--text-tertiary)", flexShrink: 0 }}>
                    {asset.exchange}
                  </span>
                )}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
