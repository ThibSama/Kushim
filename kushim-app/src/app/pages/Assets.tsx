import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Search, Loader2, ChevronDown, AlertCircle } from "lucide-react";
import { Card } from "../components/Card";
import { type AssetFilters } from "../../lib/api/businessApi";
import { useAssetsStore } from "../../stores/assets";

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

const ASSET_CLASS_OPTIONS = [
  { value: "", label: "Toutes les classes" },
  { value: "equity", label: "Actions" },
  { value: "etf", label: "ETF" },
  { value: "fund", label: "Fonds" },
  { value: "bond", label: "Obligations" },
  { value: "crypto", label: "Crypto" },
  { value: "commodity", label: "Matières premières" },
  { value: "real_estate", label: "Immobilier" },
  { value: "other", label: "Autre" },
];

const STATUS_OPTIONS = [
  { value: "", label: "Tous les statuts" },
  { value: "active", label: "Actif" },
  { value: "inactive", label: "Inactif" },
  { value: "delisted", label: "Délisté" },
];

const PAGE_SIZE = 25;

function assetClassLabel(cls: string): string {
  return ASSET_CLASS_LABELS[cls] ?? cls;
}

function formatPrice(minor: number, currency: string): string {
  const value = minor / 100;
  try {
    return new Intl.NumberFormat("fr-FR", {
      style: "currency",
      currency,
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(value);
  } catch {
    return `${value.toFixed(2)} ${currency}`;
  }
}

function buildFilters(
  search: string,
  assetClass: string,
  statusVal: string,
): AssetFilters {
  const f: AssetFilters = { limit: PAGE_SIZE };
  if (search) f.search = search;
  if (assetClass) f.asset_class = assetClass;
  if (statusVal) f.status = statusVal;
  return f;
}

const thStyle: React.CSSProperties = {
  fontSize: "11px",
  fontWeight: 500,
  textTransform: "uppercase",
  letterSpacing: "0.05em",
  color: "var(--text-tertiary)",
  padding: "10px 12px",
  whiteSpace: "nowrap",
};

export function Assets() {
  const { assets, pagination, status, error, loadAssets, loadMoreAssets } =
    useAssetsStore();
  const [search, setSearch] = useState("");
  const [assetClass, setAssetClass] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const [searchDebounced, setSearchDebounced] = useState("");
  const navigate = useNavigate();

  useEffect(() => {
    const timer = setTimeout(() => setSearchDebounced(search), 300);
    return () => clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    loadAssets(buildFilters(searchDebounced, assetClass, statusFilter));
  }, [searchDebounced, assetClass, statusFilter, loadAssets]);

  const loading = status === "loading";

  return (
    <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
      <div className="mb-6">
        <h1
          style={{
            fontSize: "clamp(24px, 5vw, 30px)",
            fontWeight: 700,
            color: "var(--text-primary)",
          }}>
          Catalogue d'actifs
        </h1>
        <p
          style={{
            fontSize: "clamp(13px, 2.5vw, 14px)",
            color: "var(--text-secondary)",
            marginTop: "4px",
          }}>
          Instruments disponibles dans Kushim
        </p>
      </div>

      {/* Toolbar */}
      <div className="flex flex-col sm:flex-row sm:flex-wrap sm:items-center gap-3 mt-6 mb-8">
        <div className="relative w-full sm:w-auto sm:flex-1 sm:max-w-[320px]">
          <Search
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2"
            style={{ color: "var(--text-tertiary)" }}
          />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Rechercher (nom, ticker, ISIN)…"
            className="w-full"
            style={{
              background: "var(--surface-2-bg)",
              border: "1px solid var(--surface-2-border)",
              borderRadius: "var(--radius-md)",
              height: "44px",
              paddingLeft: "36px",
              paddingRight: "16px",
              fontSize: "14px",
              color: "var(--text-primary)",
            }}
          />
        </div>

        <div className="relative w-full sm:w-auto">
          <select
            value={assetClass}
            onChange={(e) => setAssetClass(e.target.value)}
            className="appearance-none pr-8 cursor-pointer w-full sm:w-auto"
            style={{
              background: "var(--surface-2-bg)",
              border: "1px solid var(--surface-2-border)",
              borderRadius: "var(--radius-md)",
              height: "44px",
              paddingLeft: "16px",
              paddingRight: "36px",
              fontSize: "14px",
              color: "var(--text-primary)",
              fontWeight: 500,
            }}>
            {ASSET_CLASS_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
          <ChevronDown
            size={16}
            className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none"
            style={{ color: "var(--text-tertiary)" }}
          />
        </div>

        <div className="relative w-full sm:w-auto">
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            className="appearance-none pr-8 cursor-pointer w-full sm:w-auto"
            style={{
              background: "var(--surface-2-bg)",
              border: "1px solid var(--surface-2-border)",
              borderRadius: "var(--radius-md)",
              height: "44px",
              paddingLeft: "16px",
              paddingRight: "36px",
              fontSize: "14px",
              color: "var(--text-primary)",
              fontWeight: 500,
            }}>
            {STATUS_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
          <ChevronDown
            size={16}
            className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none"
            style={{ color: "var(--text-tertiary)" }}
          />
        </div>

        {status === "success" && pagination && (
          <span
            style={{
              fontSize: "13px",
              color: "var(--text-tertiary)",
              whiteSpace: "nowrap",
            }}>
            {assets.length} actif{assets.length !== 1 ? "s" : ""} affiché
            {assets.length !== 1 ? "s" : ""}
            {pagination.has_more ? " (d'autres disponibles)" : ""}
          </span>
        )}
      </div>

      {/* Error state */}
      {error && (
        <Card level={1} className="mb-6">
          <div
            className="flex items-center gap-3"
            style={{ color: "var(--color-loss)" }}>
            <AlertCircle size={20} />
            <div>
              <div style={{ fontWeight: 600, fontSize: "14px" }}>
                Impossible de charger les actifs
              </div>
              <div
                style={{
                  fontSize: "13px",
                  color: "var(--text-secondary)",
                  marginTop: "2px",
                }}>
                {error}
              </div>
            </div>
          </div>
        </Card>
      )}

      {/* Loading state (initial) */}
      {loading && assets.length === 0 && !error && (
        <div
          className="flex items-center justify-center gap-3"
          style={{
            minHeight: "200px",
            color: "var(--text-tertiary)",
            fontSize: "14px",
          }}>
          <Loader2 size={20} className="animate-spin" />
          Chargement des actifs…
        </div>
      )}

      {/* Empty state */}
      {!loading && !error && assets.length === 0 && (
        <Card level={1}>
          <div
            className="text-center"
            style={{
              padding: "40px 20px",
              color: "var(--text-tertiary)",
            }}>
            <div
              style={{
                fontSize: "16px",
                fontWeight: 600,
                marginBottom: "8px",
                color: "var(--text-secondary)",
              }}>
              Aucun actif trouvé
            </div>
            <div style={{ fontSize: "14px" }}>
              {searchDebounced || assetClass || statusFilter
                ? "Essayez de modifier vos critères de recherche ou filtres."
                : "Aucun actif disponible dans le catalogue."}
            </div>
          </div>
        </Card>
      )}

      {/* Asset table */}
      {assets.length > 0 && (
        <Card level={1} noPadding>
          <div style={{ overflowX: "auto", WebkitOverflowScrolling: "touch" }}>
            <table
              style={{
                width: "100%",
                borderCollapse: "collapse",
                minWidth: "700px",
              }}>
              <thead>
                <tr>
                  <th style={{ ...thStyle, textAlign: "left", minWidth: "180px" }}>
                    Actif
                  </th>
                  <th style={{ ...thStyle, textAlign: "center", minWidth: "90px" }}>
                    Classe
                  </th>
                  <th
                    style={{ ...thStyle, textAlign: "center", minWidth: "90px" }}
                    className="hidden sm:table-cell">
                    Bourse
                  </th>
                  <th style={{ ...thStyle, textAlign: "center", minWidth: "70px" }}>
                    Devise
                  </th>
                  <th
                    style={{ ...thStyle, textAlign: "right", minWidth: "120px" }}
                    className="hidden sm:table-cell">
                    Prix
                  </th>
                  <th style={{ ...thStyle, textAlign: "center", minWidth: "70px" }}>
                    Statut
                  </th>
                </tr>
              </thead>
              <tbody>
                {assets.map((asset, i) => (
                  <tr
                    key={asset.id_asset}
                    onClick={() => navigate(`/assets/${asset.id_asset}`)}
                    className="transition-colors cursor-pointer"
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background = "var(--surface-2-bg)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background = "transparent";
                    }}
                    style={{
                      borderBottom:
                        i < assets.length - 1
                          ? "1px solid var(--surface-1-border)"
                          : "none",
                    }}>
                    <td style={{ padding: "14px 12px", minWidth: "180px" }}>
                      <div
                        style={{
                          fontSize: "14px",
                          fontWeight: 500,
                          color: "var(--text-primary)",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                          maxWidth: "280px",
                        }}>
                        {asset.name}
                      </div>
                      <div className="flex items-center gap-2 mt-0.5">
                        {(asset.ticker ?? asset.symbol) && (
                          <span
                            style={{
                              fontSize: "12px",
                              fontWeight: 600,
                              color: "var(--text-secondary)",
                              fontFamily: "'JetBrains Mono', monospace",
                            }}>
                            {asset.ticker ?? asset.symbol}
                          </span>
                        )}
                        {asset.isin && (
                          <span
                            style={{
                              fontSize: "11px",
                              color: "var(--text-tertiary)",
                            }}>
                            {asset.isin}
                          </span>
                        )}
                      </div>
                    </td>
                    <td
                      style={{
                        padding: "14px 12px",
                        textAlign: "center",
                        minWidth: "90px",
                      }}>
                      <span
                        className="rounded-full px-2 py-0.5"
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          background: "var(--surface-2-bg)",
                          color: "var(--text-secondary)",
                          whiteSpace: "nowrap",
                        }}>
                        {assetClassLabel(asset.asset_class)}
                      </span>
                    </td>
                    <td
                      style={{
                        padding: "14px 12px",
                        textAlign: "center",
                        fontSize: "13px",
                        color: "var(--text-secondary)",
                        minWidth: "90px",
                      }}
                      className="hidden sm:table-cell">
                      {asset.exchange ?? "—"}
                    </td>
                    <td
                      style={{
                        padding: "14px 12px",
                        textAlign: "center",
                        fontSize: "13px",
                        color: "var(--text-secondary)",
                        minWidth: "70px",
                      }}>
                      {asset.native_currency ?? "—"}
                    </td>
                    <td
                      style={{
                        padding: "14px 12px",
                        textAlign: "right",
                        fontFamily: "'JetBrains Mono', monospace",
                        fontSize: "13px",
                        fontVariantNumeric: "tabular-nums",
                        color: "var(--text-primary)",
                        minWidth: "120px",
                      }}
                      className="hidden sm:table-cell">
                      {asset.market_data
                        ? formatPrice(
                            asset.market_data.price_minor,
                            asset.market_data.currency,
                          )
                        : "—"}
                    </td>
                    <td
                      style={{
                        padding: "14px 12px",
                        textAlign: "center",
                        minWidth: "70px",
                      }}>
                      <span
                        className="rounded-full px-2 py-0.5"
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          background:
                            asset.status === "active"
                              ? "color-mix(in srgb, var(--color-gain) 12%, transparent)"
                              : "var(--surface-2-bg)",
                          color:
                            asset.status === "active"
                              ? "var(--color-gain)"
                              : "var(--text-tertiary)",
                        }}>
                        {asset.status === "active"
                          ? "Actif"
                          : asset.status === "inactive"
                            ? "Inactif"
                            : asset.status === "delisted"
                              ? "Délisté"
                              : asset.status}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Load more */}
          {pagination?.has_more && (
            <div
              style={{
                borderTop: "1px solid var(--surface-1-border)",
                padding: "16px",
                textAlign: "center",
              }}>
              <button
                onClick={() =>
                  loadMoreAssets(
                    buildFilters(searchDebounced, assetClass, statusFilter),
                  )
                }
                disabled={loading}
                style={{
                  height: "40px",
                  padding: "0 24px",
                  borderRadius: "var(--radius-md)",
                  border: "1px solid var(--surface-1-border)",
                  background: "transparent",
                  fontSize: "14px",
                  fontWeight: 500,
                  color: "var(--text-primary)",
                  cursor: loading ? "default" : "pointer",
                  opacity: loading ? 0.6 : 1,
                  display: "inline-flex",
                  alignItems: "center",
                  gap: "8px",
                }}>
                {loading && <Loader2 size={14} className="animate-spin" />}
                Charger plus d'actifs
              </button>
            </div>
          )}
        </Card>
      )}
    </div>
  );
}
