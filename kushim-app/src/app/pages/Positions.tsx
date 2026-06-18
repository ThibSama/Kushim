import { useEffect, useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import {
  Search,
  Loader2,
  ChevronDown,
  AlertCircle,
  AlertTriangle,
  TrendingUp,
  TrendingDown,
  Info,
} from "lucide-react";
import { Card } from "../components/Card";
import { usePortfolioStore } from "../../stores/portfolio";
import { usePortfolioReadModelsStore } from "../../stores/portfolioReadModels";
import type {
  PortfolioHolding,
  PortfolioHoldingsQuery,
} from "../../lib/api/businessApi";

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

const SORT_OPTIONS = [
  { value: "weight_desc", label: "Poids ↓" },
  { value: "value_desc", label: "Valeur ↓" },
  { value: "name_asc", label: "Nom A→Z" },
];

function formatMinorCurrency(valueMinor: number, currency: string) {
  return new Intl.NumberFormat("fr-FR", {
    style: "currency",
    currency,
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(valueMinor / 100);
}

function formatSignedMinorCurrency(valueMinor: number, currency: string) {
  const formatted = formatMinorCurrency(Math.abs(valueMinor), currency);
  if (valueMinor > 0) return `+${formatted}`;
  if (valueMinor < 0) return `-${formatted}`;
  return formatted;
}

function parsePercent(value: string | null): number | null {
  if (!value) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function formatPercent(value: number): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)} %`;
}

function formatQuantity(value: string): string {
  const num = Number(value);
  if (!Number.isFinite(num)) return value;
  return new Intl.NumberFormat("fr-FR", {
    minimumFractionDigits: 0,
    maximumFractionDigits: 10,
  }).format(num);
}

function assetClassLabel(cls: string): string {
  return ASSET_CLASS_LABELS[cls] ?? cls;
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

export function Positions() {
  const { activePortfolioId, portfolios, status: portfolioStatus } =
    usePortfolioStore();
  const { holdings, holdingsPagination, loadHoldings, loadMoreHoldings } =
    usePortfolioReadModelsStore();

  const [search, setSearch] = useState("");
  const [searchDebounced, setSearchDebounced] = useState("");
  const [assetClass, setAssetClass] = useState("");
  const [sort, setSort] = useState<PortfolioHoldingsQuery["sort"]>("weight_desc");
  const navigate = useNavigate();

  useEffect(() => {
    const timer = setTimeout(() => setSearchDebounced(search), 300);
    return () => clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    if (!activePortfolioId) return;
    const query: PortfolioHoldingsQuery = { limit: 25, sort };
    if (assetClass) query.asset_class = assetClass;
    if (searchDebounced) query.search = searchDebounced;
    loadHoldings(activePortfolioId, query);
  }, [activePortfolioId, searchDebounced, assetClass, sort, loadHoldings]);

  const activePortfolio = portfolios.find(
    (p) => p.id_portfolio === activePortfolioId,
  );

  const loading = holdings.status === "loading" || holdings.status === "idle";
  const holdingsList = holdings.data;
  const hasEstimated = holdingsList.some((h) => h.is_estimated);
  // Distinguish positions with no market-data row from those merely estimated:
  // an unavailable market_data block is an objective fact from the API, while
  // is_estimated stays overloaded (FX, splits, missing price, …).
  const missingMarketData = holdingsList.filter(
    (h) => !h.market_data.available,
  );
  const valuedCount = holdingsList.length - missingMarketData.length;
  const partialValuation =
    holdingsList.length > 0 &&
    missingMarketData.length > 0 &&
    valuedCount > 0;
  const fullyUnvalued =
    holdingsList.length > 0 && valuedCount === 0;

  const totals = useMemo(() => {
    if (!holdings.dataAvailable || holdingsList.length === 0) return null;
    // Sum ONLY positions that actually have market data so the displayed
    // total never silently absorbs an invested-cost fallback as if it were a
    // real market value. The label adapts to convey the partial state.
    let totalValue = 0;
    let totalPnl = 0;
    for (const h of holdingsList) {
      if (!h.market_data.available) continue;
      totalValue += h.market_value_minor;
      totalPnl += h.pnl_base_minor;
    }
    return {
      totalValue,
      totalPnl,
      count: holdingsList.length,
      valuedCount,
    };
  }, [holdingsList, holdings.dataAvailable, valuedCount]);

  const currency =
    activePortfolio?.base_currency ??
    holdingsList[0]?.base_currency ??
    "EUR";

  // No active portfolio
  if (portfolioStatus === "success" && !activePortfolioId) {
    return (
      <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
        <h1
          style={{
            fontSize: "clamp(24px, 5vw, 30px)",
            fontWeight: 700,
            color: "var(--text-primary)",
            marginBottom: "24px",
          }}>
          Positions
        </h1>
        <Card level={1}>
          <div
            className="flex items-center gap-3"
            style={{ padding: "24px 16px", color: "var(--text-secondary)" }}>
            <Info size={20} style={{ color: "var(--text-tertiary)", flexShrink: 0 }} />
            <div>
              <div style={{ fontWeight: 600, fontSize: "14px" }}>
                Aucun portefeuille sélectionné
              </div>
              <div
                style={{
                  fontSize: "13px",
                  color: "var(--text-tertiary)",
                  marginTop: "2px",
                }}>
                Sélectionnez ou créez un portefeuille pour voir vos positions.
              </div>
            </div>
          </div>
        </Card>
      </div>
    );
  }

  return (
    <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
      {/* Header */}
      <div className="mb-6">
        <h1
          style={{
            fontSize: "clamp(24px, 5vw, 30px)",
            fontWeight: 700,
            color: "var(--text-primary)",
          }}>
          Positions
        </h1>
        <p
          style={{
            fontSize: "clamp(13px, 2.5vw, 14px)",
            color: "var(--text-secondary)",
            marginTop: "4px",
          }}>
          Actifs détenus dans le portefeuille actif
          {activePortfolio && (
            <span style={{ fontWeight: 600 }}>
              {" "}
              — {activePortfolio.name} ({activePortfolio.base_currency})
            </span>
          )}
        </p>
      </div>

      {/* Summary cards — only when data is available */}
      {holdings.dataAvailable && totals && (
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 mb-8">
          <Card level={1}>
            <div
              style={{
                padding: "4px 0",
              }}>
              <div
                className="uppercase mb-1"
                style={{
                  fontSize: "11px",
                  color: "var(--text-tertiary)",
                  letterSpacing: "0.05em",
                }}>
                Positions
              </div>
              <div
                style={{
                  fontFamily: "'JetBrains Mono', monospace",
                  fontSize: "20px",
                  fontWeight: 700,
                  color: "var(--text-primary)",
                }}>
                {totals.count}
              </div>
            </div>
          </Card>
          <Card level={1}>
            <div style={{ padding: "4px 0" }}>
              <div
                className="uppercase mb-1"
                style={{
                  fontSize: "11px",
                  color: "var(--text-tertiary)",
                  letterSpacing: "0.05em",
                }}>
                Valeur de marché
              </div>
              <div
                style={{
                  fontFamily: "'JetBrains Mono', monospace",
                  fontSize: "20px",
                  fontWeight: 700,
                  color: "var(--text-primary)",
                }}>
                {formatMinorCurrency(totals.totalValue, currency)}
              </div>
            </div>
          </Card>
          <Card level={1}>
            <div style={{ padding: "4px 0" }}>
              <div
                className="uppercase mb-1"
                style={{
                  fontSize: "11px",
                  color: "var(--text-tertiary)",
                  letterSpacing: "0.05em",
                }}>
                P&L total
              </div>
              <div
                style={{
                  fontFamily: "'JetBrains Mono', monospace",
                  fontSize: "20px",
                  fontWeight: 700,
                  color:
                    totals.totalPnl >= 0
                      ? "var(--color-gain)"
                      : "var(--color-loss)",
                }}>
                {formatSignedMinorCurrency(totals.totalPnl, currency)}
              </div>
            </div>
          </Card>
          {(hasEstimated || missingMarketData.length > 0) && (
            <Card level={1}>
              <div
                className="flex items-center gap-2"
                style={{ padding: "4px 0" }}>
                <AlertTriangle
                  size={16}
                  style={{ color: "var(--color-warning, #f59e0b)", flexShrink: 0 }}
                />
                <div>
                  <div
                    className="uppercase mb-1"
                    style={{
                      fontSize: "11px",
                      color: "var(--text-tertiary)",
                      letterSpacing: "0.05em",
                    }}>
                    Valorisation
                  </div>
                  <div
                    style={{
                      fontSize: "14px",
                      fontWeight: 600,
                      color: "var(--color-warning, #f59e0b)",
                    }}>
                    {fullyUnvalued
                      ? "Indisponible"
                      : missingMarketData.length > 0
                        ? `Partielle (${valuedCount}/${holdingsList.length})`
                        : "Estimée"}
                  </div>
                </div>
              </div>
            </Card>
          )}
        </div>
      )}

      {/* Partial-valuation banner — surfaces when at least one open position
          has no market-data row. Distinct from is_estimated overload. */}
      {holdings.dataAvailable && (partialValuation || fullyUnvalued) && (
        <Card level={1} className="mb-6">
          <div
            className="flex items-start gap-3"
            style={{ padding: "12px 4px", color: "var(--text-secondary)" }}>
            <AlertTriangle
              size={18}
              style={{
                color: "var(--color-warning, #f59e0b)",
                flexShrink: 0,
                marginTop: "2px",
              }}
            />
            <div>
              <div style={{ fontWeight: 600, fontSize: "14px", color: "var(--text-primary)" }}>
                {fullyUnvalued
                  ? "Aucune position n'a de prix de marché"
                  : `${missingMarketData.length} position${missingMarketData.length > 1 ? "s" : ""} sans prix de marché`}
              </div>
              <div style={{ fontSize: "13px", marginTop: "2px" }}>
                Le total affiché ne comprend que les positions valorisées. Les
                positions sans donnée de marché sont identifiées dans le tableau.
              </div>
            </div>
          </div>
        </Card>
      )}

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
            placeholder="Rechercher (nom, ticker)…"
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
            value={sort}
            onChange={(e) =>
              setSort(e.target.value as PortfolioHoldingsQuery["sort"])
            }
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
            {SORT_OPTIONS.map((o) => (
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
      </div>

      {/* Error state */}
      {holdings.status === "error" && (
        <Card level={1} className="mb-6">
          <div
            className="flex items-center gap-3"
            style={{ color: "var(--color-loss)" }}>
            <AlertCircle size={20} />
            <div>
              <div style={{ fontWeight: 600, fontSize: "14px" }}>
                Impossible de charger les positions
              </div>
              {holdings.error && (
                <div
                  style={{
                    fontSize: "13px",
                    color: "var(--text-secondary)",
                    marginTop: "2px",
                  }}>
                  {holdings.error}
                </div>
              )}
            </div>
          </div>
        </Card>
      )}

      {/* Loading state (initial) */}
      {loading && holdingsList.length === 0 && holdings.status !== "error" && (
        <div
          className="flex items-center justify-center gap-3"
          style={{
            minHeight: "200px",
            color: "var(--text-tertiary)",
            fontSize: "14px",
          }}>
          <Loader2 size={20} className="animate-spin" />
          Chargement des positions…
        </div>
      )}

      {/* Data unavailable — read_model_missing */}
      {holdings.status === "success" &&
        holdings.dataAvailable === false && (
          <Card level={1}>
            <div
              className="flex items-center gap-3"
              style={{ padding: "24px 16px", color: "var(--text-secondary)" }}>
              <Info
                size={20}
                style={{ color: "var(--text-tertiary)", flexShrink: 0 }}
              />
              <div>
                <div style={{ fontWeight: 600, fontSize: "14px" }}>
                  {holdings.reason === "read_model_missing"
                    ? "Positions en préparation"
                    : "Positions indisponibles"}
                </div>
                <div
                  style={{
                    fontSize: "13px",
                    color: "var(--text-tertiary)",
                    marginTop: "2px",
                  }}>
                  {holdings.reason === "read_model_missing"
                    ? "Lancez la génération du portefeuille pour calculer les positions."
                    : "Les données de positions ne sont pas encore disponibles pour ce portefeuille."}
                </div>
              </div>
            </div>
          </Card>
        )}

      {/* Empty holdings */}
      {holdings.status === "success" &&
        holdings.dataAvailable !== false &&
        holdingsList.length === 0 && (
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
                Aucune position pour le moment
              </div>
              <div style={{ fontSize: "14px" }}>
                {searchDebounced || assetClass
                  ? "Essayez de modifier vos critères de recherche ou filtres."
                  : "Les positions apparaissent après des opérations d'achat et la génération des read models par le worker."}
              </div>
            </div>
          </Card>
        )}

      {/* Positions table */}
      {holdingsList.length > 0 && holdings.dataAvailable !== false && (
        <Card level={1} noPadding>
          <div style={{ overflowX: "auto", WebkitOverflowScrolling: "touch" }}>
            <table
              style={{
                width: "100%",
                borderCollapse: "collapse",
                minWidth: "800px",
              }}>
              <thead>
                <tr>
                  <th
                    style={{ ...thStyle, textAlign: "left", minWidth: "180px" }}>
                    Actif
                  </th>
                  <th
                    style={{ ...thStyle, textAlign: "center", minWidth: "80px" }}>
                    Classe
                  </th>
                  <th
                    style={{ ...thStyle, textAlign: "right", minWidth: "90px" }}>
                    Quantité
                  </th>
                  <th
                    style={{ ...thStyle, textAlign: "right", minWidth: "110px" }}
                    className="hidden sm:table-cell">
                    Coût moyen
                  </th>
                  <th
                    style={{ ...thStyle, textAlign: "right", minWidth: "120px" }}>
                    Valeur
                  </th>
                  <th
                    style={{ ...thStyle, textAlign: "right", minWidth: "120px" }}>
                    P&L
                  </th>
                  <th
                    style={{ ...thStyle, textAlign: "right", minWidth: "70px" }}>
                    Poids
                  </th>
                  <th
                    style={{ ...thStyle, textAlign: "center", minWidth: "30px" }}>
                  </th>
                </tr>
              </thead>
              <tbody>
                {holdingsList.map((holding, i) => (
                  <PositionRow
                    key={holding.id_asset}
                    holding={holding}
                    isLast={i === holdingsList.length - 1}
                    onNavigate={() =>
                      navigate(`/assets/${holding.id_asset}`)
                    }
                  />
                ))}
              </tbody>
            </table>
          </div>

          {/* Load more */}
          {holdingsPagination?.has_more && (
            <div
              style={{
                borderTop: "1px solid var(--surface-1-border)",
                padding: "16px",
                textAlign: "center",
              }}>
              <button
                onClick={() => {
                  if (!activePortfolioId) return;
                  const query: PortfolioHoldingsQuery = { limit: 25, sort };
                  if (assetClass) query.asset_class = assetClass;
                  if (searchDebounced) query.search = searchDebounced;
                  loadMoreHoldings(activePortfolioId, query);
                }}
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
                Charger plus de positions
              </button>
            </div>
          )}
        </Card>
      )}
    </div>
  );
}

function formatProvenanceTooltip(holding: PortfolioHolding): string {
  const md = holding.market_data;
  // Honest copy: the labels must never claim a "last synchronisation" — only
  // the stored record-update timestamp is known. Likewise, when invested-cost
  // fallback is the source we say so explicitly.
  if (!md.available) {
    if (md.unavailable_reason === "valuation_provenance_missing") {
      return "Provenance de valorisation indisponible — recalcul requis.";
    }
    if (md.unavailable_reason === "unsupported_market_data_currency") {
      const provider = md.provider ?? "fournisseur inconnu";
      const priceAsOf = md.market_data_as_of
        ? new Date(md.market_data_as_of).toLocaleString("fr-FR")
        : "—";
      const recordUpdatedAt = md.record_updated_at
        ? new Date(md.record_updated_at).toLocaleString("fr-FR")
        : "—";
      return [
        "Devise du cours non prise en charge.",
        `Source : ${provider}`,
        `Cours daté du : ${priceAsOf} (${md.currency ?? "?"})`,
        `Enregistrement de marché mis à jour le : ${recordUpdatedAt}`,
        "Valorisation au coût investi.",
      ].join("\n");
    }
    return "Donnée de marché indisponible — la position est valorisée au coût investi.";
  }
  const provider = md.provider ?? "fournisseur inconnu";
  const priceAsOf = md.market_data_as_of
    ? new Date(md.market_data_as_of).toLocaleString("fr-FR")
    : "—";
  const recordUpdatedAt = md.record_updated_at
    ? new Date(md.record_updated_at).toLocaleString("fr-FR")
    : "—";
  return [
    `Source : ${provider}`,
    `Cours daté du : ${priceAsOf}`,
    `Enregistrement de marché mis à jour le : ${recordUpdatedAt}`,
  ].join("\n");
}

function PositionRow({
  holding,
  isLast,
  onNavigate,
}: {
  holding: PortfolioHolding;
  isLast: boolean;
  onNavigate: () => void;
}) {
  const pnlPct = parsePercent(holding.pnl_pct);
  const weightPct = parsePercent(holding.weight_pct);
  const isPositive = holding.pnl_base_minor >= 0;
  const marketDataAvailable = holding.market_data.available;
  const provenanceTooltip = formatProvenanceTooltip(holding);

  return (
    <tr
      onClick={onNavigate}
      className="transition-colors cursor-pointer"
      onMouseEnter={(e) => {
        e.currentTarget.style.background = "var(--surface-2-bg)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "transparent";
      }}
      style={{
        borderBottom: isLast ? "none" : "1px solid var(--surface-1-border)",
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
            maxWidth: "260px",
          }}>
          {holding.asset.name}
        </div>
        <div className="flex items-center gap-2 mt-0.5">
          {holding.asset.ticker && (
            <span
              style={{
                fontSize: "12px",
                fontWeight: 600,
                color: "var(--text-secondary)",
                fontFamily: "'JetBrains Mono', monospace",
              }}>
              {holding.asset.ticker}
            </span>
          )}
          {holding.asset.exchange && (
            <span style={{ fontSize: "11px", color: "var(--text-tertiary)" }}>
              {holding.asset.exchange}
            </span>
          )}
        </div>
      </td>
      <td
        style={{
          padding: "14px 12px",
          textAlign: "center",
          minWidth: "80px",
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
          {assetClassLabel(holding.asset.asset_class)}
        </span>
      </td>
      <td
        style={{
          padding: "14px 12px",
          textAlign: "right",
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: "13px",
          color: "var(--text-primary)",
          minWidth: "90px",
        }}>
        {formatQuantity(holding.quantity)}
      </td>
      <td
        style={{
          padding: "14px 12px",
          textAlign: "right",
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: "13px",
          color: "var(--text-secondary)",
          minWidth: "110px",
        }}
        className="hidden sm:table-cell">
        {holding.avg_cost_minor != null
          ? formatMinorCurrency(holding.avg_cost_minor, holding.base_currency)
          : "—"}
      </td>
      <td
        title={provenanceTooltip}
        style={{
          padding: "14px 12px",
          textAlign: "right",
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: "13px",
          fontWeight: 600,
          color: marketDataAvailable
            ? "var(--text-primary)"
            : "var(--text-tertiary)",
          minWidth: "120px",
        }}>
        {marketDataAvailable
          ? formatMinorCurrency(holding.market_value_minor, holding.base_currency)
          : "—"}
      </td>
      <td
        title={provenanceTooltip}
        style={{
          padding: "14px 12px",
          textAlign: "right",
          minWidth: "120px",
        }}>
        {marketDataAvailable ? (
          <>
            <div
              className="flex items-center justify-end gap-1"
              style={{
                fontFamily: "'JetBrains Mono', monospace",
                fontSize: "13px",
                fontWeight: 600,
                color: isPositive ? "var(--color-gain)" : "var(--color-loss)",
              }}>
              {isPositive ? (
                <TrendingUp size={14} />
              ) : (
                <TrendingDown size={14} />
              )}
              <span>
                {pnlPct != null
                  ? formatPercent(pnlPct)
                  : formatSignedMinorCurrency(
                      holding.pnl_base_minor,
                      holding.base_currency,
                    )}
              </span>
            </div>
            <div
              style={{
                fontSize: "11px",
                color: "var(--text-tertiary)",
                textAlign: "right",
                marginTop: "1px",
              }}>
              {formatSignedMinorCurrency(
                holding.pnl_base_minor,
                holding.base_currency,
              )}
            </div>
          </>
        ) : (
          <div
            style={{
              fontSize: "12px",
              color: "var(--text-tertiary)",
              textAlign: "right",
              fontStyle: "italic",
            }}>
            —
          </div>
        )}
      </td>
      <td
        style={{
          padding: "14px 12px",
          textAlign: "right",
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: "13px",
          color: "var(--text-secondary)",
          minWidth: "70px",
        }}>
        {weightPct != null ? `${weightPct.toFixed(1)}%` : "—"}
      </td>
      <td
        style={{
          padding: "14px 8px",
          textAlign: "center",
          minWidth: "30px",
        }}>
        {!marketDataAvailable ? (
          <span
            title={provenanceTooltip}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "2px",
              fontSize: "10px",
              fontWeight: 600,
              color: "var(--color-loss)",
              background:
                "color-mix(in srgb, var(--color-loss) 12%, transparent)",
              borderRadius: "var(--radius-md)",
              padding: "2px 6px",
              whiteSpace: "nowrap",
            }}>
            <AlertCircle size={10} />
            Indispo.
          </span>
        ) : holding.is_estimated ? (
          <span
            title={`Valeur estimée — prix de marché ou taux de change manquant.\n${provenanceTooltip}`}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "2px",
              fontSize: "10px",
              fontWeight: 600,
              color: "var(--color-warning, #f59e0b)",
              background:
                "color-mix(in srgb, var(--color-warning, #f59e0b) 12%, transparent)",
              borderRadius: "var(--radius-md)",
              padding: "2px 6px",
              whiteSpace: "nowrap",
            }}>
            <AlertTriangle size={10} />
            Est.
          </span>
        ) : null}
      </td>
    </tr>
  );
}
