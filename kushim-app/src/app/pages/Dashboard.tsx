import React, { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Card } from "../components/Card";
import { KPICard } from "../components/KPICard";
import { Button } from "../components/Button";
import { RefreshNotice } from "../components/RefreshNotice";
import { CreateOperationModal } from "../components/CreateOperationModal";
import { CreatePortfolioModal } from "../components/CreatePortfolioModal";
import {
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  Area,
  ComposedChart,
} from "recharts";
import { Plus, PlusCircle, Briefcase } from "lucide-react";
import { formatCurrency, formatSignedPercent } from "../../utils/portfolio";
import { usePortfolioStore } from "../../stores/portfolio";
import { useOperationsStore } from "../../stores/operations";
import { usePortfolioReadModelsStore } from "../../stores/portfolioReadModels";
import { operationToRow, typeBadgeStyle } from "../../lib/operations";

// Suppress recharts internal null-key warning (known v2 bug with SVG defs)
const originalConsoleError = console.error;
console.error = (...args: unknown[]) => {
  if (
    typeof args[0] === "string" &&
    args[0].includes("Encountered two children with the same key")
  )
    return;
  originalConsoleError(...args);
};

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

function parsePercent(value: string | null) {
  if (!value) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function formatAssetInitials(name: string, ticker: string | null) {
  const source = ticker || name;
  return source.slice(0, 2).toUpperCase();
}

const allocationColors = ["#6366F1", "#10B981", "#F59E0B", "#8B5CF6", "#EF4444"];

function toIsoDate(date: Date) {
  return date.toISOString().slice(0, 10);
}

function getSnapshotQuery(period: string) {
  const now = new Date();
  const from = new Date(now);

  if (period === "MAX") {
    return { limit: 366, sort: "asc" as const };
  }

  const monthsByPeriod: Record<string, number> = {
    "1M": 1,
    "3M": 3,
    "6M": 6,
    "1Y": 12,
  };

  from.setMonth(from.getMonth() - (monthsByPeriod[period] ?? 12));
  return {
    date_from: toIsoDate(from),
    date_to: toIsoDate(now),
    limit: 366,
    sort: "asc" as const,
  };
}

function EmptyPortfolioState({ onCreate }: { onCreate: () => void }) {
  return (
    <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
      <div className="mb-8">
        <h1
          className="mb-2"
          style={{
            fontSize: "clamp(22px, 4vw, 24px)",
            fontWeight: 600,
            color: "var(--text-primary)",
          }}>
          Tableau de bord
        </h1>
      </div>
      <Card level={1}>
        <div
          className="flex flex-col items-center justify-center text-center"
          style={{ padding: "clamp(40px, 8vw, 64px) clamp(16px, 4vw, 32px)" }}>
          <div
            className="rounded-full flex items-center justify-center mb-6"
            style={{
              width: "72px",
              height: "72px",
              background: "linear-gradient(135deg, rgba(99, 102, 241, 0.15), rgba(16, 185, 129, 0.10))",
              border: "1px solid var(--surface-2-border)",
            }}>
            <Briefcase size={32} style={{ color: "var(--color-accent)" }} />
          </div>
          <h2
            style={{
              fontSize: "20px",
              fontWeight: 600,
              color: "var(--text-primary)",
              marginBottom: "8px",
            }}>
            Créez votre premier portefeuille
          </h2>
          <p
            style={{
              fontSize: "14px",
              color: "var(--text-secondary)",
              maxWidth: "420px",
              marginBottom: "24px",
              lineHeight: "1.6",
            }}>
            Un portefeuille organise vos transactions et actifs en un seul endroit.
            Suivez vos performances, analysez votre allocation et gardez le contrôle sur vos investissements.
          </p>
          <Button variant="primary" icon={Plus} onClick={onCreate}>
            Créer un portefeuille
          </Button>
        </div>
      </Card>
    </div>
  );
}

function PortfolioSelector() {
  const { portfolios, activePortfolioId, setActivePortfolio } = usePortfolioStore();
  if (portfolios.length <= 1) return null;

  return (
    <select
      value={activePortfolioId ?? ""}
      onChange={(e) => setActivePortfolio(e.target.value)}
      className="glass-field rounded-[9999px] px-3 py-1.5"
      style={{
        fontSize: "13px",
        color: "var(--text-primary)",
        border: "1px solid var(--surface-2-border)",
        background: "var(--surface-1-bg)",
        cursor: "pointer",
      }}>
      {portfolios.map((p) => (
        <option key={p.id_portfolio} value={p.id_portfolio}>
          {p.name}
        </option>
      ))}
    </select>
  );
}

export function Dashboard() {
  const navigate = useNavigate();
  const { portfolios, activePortfolioId, status, loadPortfolios } = usePortfolioStore();
  const { operations, loadOperations } = useOperationsStore();
  const {
    portfolioId: readModelPortfolioId,
    summary,
    holdings,
    snapshots,
    loadSummary,
    loadHoldings,
    loadSnapshots,
  } = usePortfolioReadModelsStore();
  const activePortfolio = portfolios.find((p) => p.id_portfolio === activePortfolioId) ?? null;
  const [showCreatePortfolio, setShowCreatePortfolio] = useState(false);

  const [period, setPeriod] = useState("1Y");
  const [showAddTransaction, setShowAddTransaction] = useState(false);
  const periods = ["1M", "3M", "6M", "1Y", "MAX"];

  useEffect(() => {
    if (status === "idle") {
      loadPortfolios();
    }
  }, [status, loadPortfolios]);

  useEffect(() => {
    if (activePortfolioId) {
      loadOperations(activePortfolioId);
    }
  }, [activePortfolioId, loadOperations]);

  useEffect(() => {
    if (!activePortfolioId) return;
    if (
      readModelPortfolioId === activePortfolioId &&
      (summary.status === "loading" || summary.status === "success")
    ) {
      return;
    }
    loadSummary(activePortfolioId);
  }, [activePortfolioId, loadSummary, readModelPortfolioId, summary.status]);

  useEffect(() => {
    if (!activePortfolioId) return;
    if (
      readModelPortfolioId === activePortfolioId &&
      (holdings.status === "loading" || holdings.status === "success")
    ) {
      return;
    }
    loadHoldings(activePortfolioId, { sort: "value_desc", limit: 5 });
  }, [activePortfolioId, holdings.status, loadHoldings, readModelPortfolioId]);

  useEffect(() => {
    if (!activePortfolioId) return;
    loadSnapshots(activePortfolioId, getSnapshotQuery(period));
  }, [activePortfolioId, loadSnapshots, period]);

  const recentRows = operations.slice(0, 5).map(operationToRow);
  const summaryData = summary.data;
  const summaryCurrency = summaryData?.base_currency ?? activePortfolio?.base_currency ?? "EUR";
  const summaryPnlPct = parsePercent(summaryData?.total_pnl_pct ?? null);
  const isSummaryLoading = summary.status === "loading" || summary.status === "idle";
  const isSummaryUnavailable =
    summary.status === "success" && summary.dataAvailable === false;
  const summaryError = summary.status === "error" ? summary.error : null;
  const kpiPlaceholder = "—";
  const kpiValues = {
    netValue: summaryData
      ? formatMinorCurrency(summaryData.total_value_minor, summaryCurrency)
      : kpiPlaceholder,
    investedCapital: summaryData
      ? formatMinorCurrency(summaryData.total_invested_minor, summaryCurrency)
      : kpiPlaceholder,
    gainLoss: summaryData
      ? formatSignedMinorCurrency(summaryData.total_pnl_minor, summaryCurrency)
      : kpiPlaceholder,
  };
  const gainLossChange =
    summaryPnlPct == null
      ? undefined
      : {
          value: formatSignedPercent(summaryPnlPct),
          isPositive: summaryPnlPct >= 0,
        };
  const isHoldingsLoading = holdings.status === "loading" || holdings.status === "idle";
  const isHoldingsUnavailable =
    holdings.status === "success" && holdings.dataAvailable === false;
  const holdingsError = holdings.status === "error" ? holdings.error : null;
  const hasTopHoldings = holdings.status === "success" && holdings.data.length > 0;
  const allocationTotal = holdings.data.reduce(
    (total, holding) => total + holding.market_value_minor,
    0,
  );
  const allocationByClass = Array.from(
    holdings.data.reduce((groups, holding) => {
      const key = holding.asset.asset_class;
      groups.set(key, (groups.get(key) ?? 0) + holding.market_value_minor);
      return groups;
    }, new Map<string, number>()),
  )
    .map(([name, valueMinor], index) => ({
      name,
      value: allocationTotal > 0 ? Math.round((valueMinor / allocationTotal) * 1000) / 10 : 0,
      color: allocationColors[index % allocationColors.length],
    }))
    .filter((item) => item.value > 0);
  const hasAllocation = holdings.status === "success" && allocationByClass.length > 0;
  const holdingsWithPnl = holdings.data
    .map((h) => ({ asset: h.asset, pnlValue: parsePercent(h.pnl_pct) }))
    .filter((h): h is { asset: typeof h.asset; pnlValue: number } => h.pnlValue != null);
  const bestHolding = holdingsWithPnl.length > 0
    ? holdingsWithPnl.reduce((best, h) => (h.pnlValue > best.pnlValue ? h : best))
    : null;
  const worstHolding = holdingsWithPnl.length > 0
    ? holdingsWithPnl.reduce((worst, h) => (h.pnlValue < worst.pnlValue ? h : worst))
    : null;
  const isSnapshotsLoading = snapshots.status === "loading" || snapshots.status === "idle";
  const isSnapshotsUnavailable =
    snapshots.status === "success" && snapshots.dataAvailable === false;
  const snapshotsError = snapshots.status === "error" ? snapshots.error : null;
  const portfolioData = snapshots.data.map((snapshot) => ({
    date: snapshot.snapshot_date,
    value: snapshot.total_value_minor / 100,
  }));
  const hasPortfolioData = snapshots.status === "success" && portfolioData.length > 0;

  if (status === "loading" || status === "idle") {
    return (
      <div
        className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          minHeight: "40vh",
          color: "var(--text-secondary)",
          fontSize: "15px",
        }}>
        Chargement des portefeuilles…
      </div>
    );
  }

  if (status === "success" && portfolios.length === 0) {
    return (
      <>
        <EmptyPortfolioState onCreate={() => setShowCreatePortfolio(true)} />
        {showCreatePortfolio && (
          <CreatePortfolioModal onClose={() => setShowCreatePortfolio(false)} />
        )}
      </>
    );
  }

  return (
    <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
      {/* Page Header */}
      <div className="mb-8">
        <div className="flex items-center justify-between flex-wrap gap-3">
          <div>
            <h1
              className="mb-1"
              style={{
                fontSize: "clamp(22px, 4vw, 24px)",
                fontWeight: 600,
                color: "var(--text-primary)",
              }}>
              Tableau de bord
            </h1>
            {activePortfolio && (
              <p style={{ fontSize: "14px", color: "var(--text-secondary)" }}>
                {activePortfolio.name}
                <span style={{ marginLeft: "6px", fontSize: "12px", color: "var(--text-tertiary)" }}>
                  ({activePortfolio.base_currency})
                </span>
              </p>
            )}
          </div>
          <PortfolioSelector />
        </div>
      </div>

      <RefreshNotice />

      {/* KPI Row */}
      {(isSummaryLoading || isSummaryUnavailable || summaryError) && (
        <div
          className="mb-4 rounded-lg"
          style={{
            padding: "10px 16px",
            fontSize: "13px",
            color: summaryError ? "var(--color-loss)" : "var(--text-tertiary)",
            background: "var(--surface-1-bg)",
            border: "1px solid var(--surface-1-border)",
          }}>
          {isSummaryLoading && "Chargement des indicateurs du portefeuille..."}
          {isSummaryUnavailable &&
            "Données en préparation. Les indicateurs seront disponibles après génération du portefeuille."}
          {summaryError && `Impossible de charger les indicateurs: ${summaryError}`}
        </div>
      )}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 sm:gap-6 mb-8">
        <KPICard
          label="Valeur nette"
          value={kpiValues.netValue}
        />
        <KPICard label="Total investi" value={kpiValues.investedCapital} />
        <KPICard
          label="Gain / Perte"
          value={kpiValues.gainLoss}
          change={gainLossChange}
        />
        <KPICard
          label="Meilleur actif"
          value={
            isHoldingsLoading
              ? "Chargement…"
              : bestHolding
                ? bestHolding.asset.ticker ?? bestHolding.asset.name
                : kpiPlaceholder
          }
          change={
            bestHolding
              ? {
                  value: formatSignedPercent(bestHolding.pnlValue),
                  isPositive: bestHolding.pnlValue >= 0,
                }
              : undefined
          }
        />
      </div>

      {/* Action Bar */}
      <Card level={2} className="mb-8">
        <div className="flex flex-col md:flex-row gap-3 md:items-center md:justify-between">
          <div>
            <div
              style={{
                fontSize: "12px",
                textTransform: "uppercase",
                letterSpacing: "0.08em",
                color: "var(--text-tertiary)",
                fontWeight: 600,
              }}>
              Actions rapides
            </div>
            <div
              style={{
                fontSize: "14px",
                color: "var(--text-secondary)",
                marginTop: "6px",
              }}>
              Ajoutez des opérations ou consultez le catalogue d'actifs.
            </div>
          </div>
          <div className="flex flex-col sm:flex-row gap-3 w-full md:w-auto">
            <Button
              variant="primary"
              icon={Plus}
              className="w-full sm:w-auto"
              onClick={() => setShowAddTransaction(true)}>
              Ajouter une transaction
            </Button>
            <Button
              variant="secondary"
              icon={PlusCircle}
              className="w-full sm:w-auto"
              onClick={() => navigate("/assets")}>
              Catalogue d'actifs
            </Button>
          </div>
        </div>
      </Card>

      {/* Portfolio Chart */}
      <Card level={1} className="mb-8 relative overflow-hidden">
        <div
          className="pointer-events-none"
          style={{
            position: "absolute",
            inset: "-80px auto auto -40px",
            width: "280px",
            height: "280px",
            background:
              "radial-gradient(circle at center, rgba(16, 185, 129, 0.25), transparent 60%)",
            filter: "blur(4px)",
            animation: "floatGlow 12s ease-in-out infinite",
          }}
        />
        <div
          className="pointer-events-none"
          style={{
            position: "absolute",
            top: "-120px",
            right: "-60px",
            width: "320px",
            height: "320px",
            background:
              "radial-gradient(circle at center, rgba(59, 130, 246, 0.18), transparent 60%)",
            filter: "blur(6px)",
            animation: "floatGlow 14s ease-in-out infinite",
          }}
        />
        <div className="flex items-center justify-between mb-6">
          <h2
            style={{
              fontSize: "18px",
              fontWeight: 600,
              color: "var(--text-primary)",
            }}>
            Évolution du portefeuille
          </h2>
          <div className="flex gap-1 flex-wrap">
            {periods.map((p) => (
              <button
                key={p}
                onClick={() => setPeriod(p)}
                className="rounded-[9999px] transition-all hover:-translate-y-[1px]"
                style={{
                  fontSize: "12px",
                  fontWeight: 600,
                  minHeight: "36px",
                  padding: "0 clamp(12px, 2vw, 14px)",
                  background:
                    period === p ? "var(--color-cta-bg)" : "transparent",
                  color:
                    period === p
                      ? "var(--color-cta-text)"
                      : "var(--text-secondary)",
                }}>
                {p}
              </button>
            ))}
          </div>
        </div>
        {isSnapshotsLoading || isSnapshotsUnavailable || snapshotsError ? (
          <div
            className="flex items-center justify-center text-center"
            style={{
              minHeight: "340px",
              fontSize: "14px",
              color: snapshotsError ? "var(--color-loss)" : "var(--text-tertiary)",
            }}>
            {isSnapshotsLoading && "Chargement de l'historique..."}
            {isSnapshotsUnavailable &&
              "Historique en préparation. Le graphique sera disponible après génération des snapshots."}
            {snapshotsError && `Impossible de charger l'historique: ${snapshotsError}`}
          </div>
        ) : hasPortfolioData ? (
          <ResponsiveContainer width="100%" height={340}>
            <ComposedChart data={portfolioData} id="dashboard-line-chart">
              <defs>
                <linearGradient id="portfolioArea" x1="0" y1="0" x2="0" y2="1">
                  <stop
                    offset="0%"
                    stopColor="var(--color-gain)"
                    stopOpacity={0.25}
                  />
                  <stop
                    offset="100%"
                    stopColor="var(--color-gain)"
                    stopOpacity={0}
                  />
                </linearGradient>
              </defs>
              <XAxis
                dataKey="date"
                stroke="var(--text-tertiary)"
                style={{ fontSize: "12px" }}
                tick={{ fill: "var(--text-tertiary)" }}
              />
              <YAxis
                stroke="var(--text-tertiary)"
                style={{ fontSize: "12px" }}
                tick={{ fill: "var(--text-tertiary)" }}
                tickFormatter={(value) => `${Math.round(Number(value) / 1000)} k`}
              />
              <Tooltip
                contentStyle={{
                  background: "var(--surface-3-bg)",
                  backdropFilter: "blur(16px)",
                  border: "1px solid var(--surface-3-border)",
                  borderRadius: "var(--radius-md)",
                  fontSize: "12px",
                }}
                formatter={(value) => [
                  formatMinorCurrency(Math.round(Number(value) * 100), summaryCurrency),
                  "Valeur",
                ]}
              />
              <Area
                type="monotone"
                dataKey="value"
                fill="url(#portfolioArea)"
                stroke="var(--color-gain)"
                strokeWidth={2.5}
                dot={false}
                isAnimationActive
                animationDuration={900}
              />
            </ComposedChart>
          </ResponsiveContainer>
        ) : (
          <div
            className="flex items-center justify-center text-center"
            style={{
              minHeight: "340px",
              fontSize: "14px",
              color: "var(--text-tertiary)",
            }}>
            Aucun historique de portefeuille disponible pour cette période.
          </div>
        )}
      </Card>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-8 mb-8">
        {/* Allocation */}
        <Card level={1}>
          <h2
            className="mb-6"
            style={{
              fontSize: "18px",
              fontWeight: 600,
              color: "var(--text-primary)",
            }}>
            Allocation
          </h2>
          {isHoldingsLoading || isHoldingsUnavailable || holdingsError ? (
            <p
              style={{
                fontSize: "14px",
                color: holdingsError ? "var(--color-loss)" : "var(--text-tertiary)",
                padding: "24px 0",
                textAlign: "center",
              }}>
              {isHoldingsLoading && "Chargement de l'allocation..."}
              {isHoldingsUnavailable && "Allocation en préparation."}
              {holdingsError && `Impossible de charger l'allocation: ${holdingsError}`}
            </p>
          ) : hasAllocation ? (
            <div className="flex flex-col lg:flex-row items-center gap-6">
              <ResponsiveContainer width="100%" height={200}>
                <PieChart id="dashboard-pie-chart">
                  <Pie
                    data={allocationByClass}
                    cx="50%"
                    cy="50%"
                    innerRadius={60}
                    outerRadius={80}
                    paddingAngle={2}
                    dataKey="value"
                    id="dashboard-pie">
                    {allocationByClass.map((entry, index) => (
                      <Cell key={`cell-${index}`} fill={entry.color} />
                    ))}
                  </Pie>
                </PieChart>
              </ResponsiveContainer>
              <div className="flex flex-col gap-2 w-full">
                {allocationByClass.map((item) => (
                  <div
                    key={item.name}
                    className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <div
                        className="w-3 h-3 rounded-full"
                        style={{ background: item.color }}
                      />
                      <span
                        style={{
                          fontSize: "14px",
                          color: "var(--text-primary)",
                        }}>
                        {item.name}
                      </span>
                    </div>
                    <span
                      style={{
                        fontFamily: "'JetBrains Mono', monospace",
                        fontSize: "14px",
                        fontWeight: 600,
                        color: "var(--text-primary)",
                      }}>
                      {item.value}%
                    </span>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <p
              style={{
                fontSize: "14px",
                color: "var(--text-tertiary)",
                padding: "24px 0",
                textAlign: "center",
              }}>
              Aucune position à répartir.
            </p>
          )}

          {/* Allocation Metrics — derived from real holdings */}
          {holdings.status === "success" && holdings.dataAvailable !== false && (
            <div
              style={{
                borderTop: "1px solid var(--surface-1-border)",
                marginTop: "20px",
                paddingTop: "20px",
              }}>
              <div
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: "12px",
                }}>
                <div>
                  <div
                    style={{
                      fontSize: "12px",
                      color: "var(--text-tertiary)",
                    }}>
                    Positions ouvertes
                  </div>
                  <div
                    style={{
                      fontFamily: "'JetBrains Mono', monospace",
                      fontSize: "18px",
                      fontWeight: 600,
                      color: "var(--text-primary)",
                    }}>
                    {holdings.data.length}
                  </div>
                </div>
                {bestHolding && (
                  <div>
                    <div
                      style={{
                        fontSize: "12px",
                        color: "var(--text-tertiary)",
                      }}>
                      Meilleure performance
                    </div>
                    <div
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "6px",
                      }}>
                      <span
                        style={{
                          fontSize: "14px",
                          fontWeight: 500,
                          color: "var(--text-primary)",
                        }}>
                        {bestHolding.asset.ticker ?? bestHolding.asset.name}
                      </span>
                      <span
                        style={{
                          fontSize: "14px",
                          fontWeight: 600,
                          color: bestHolding.pnlValue >= 0 ? "var(--color-gain)" : "var(--color-loss)",
                        }}>
                        {formatSignedPercent(bestHolding.pnlValue)}
                      </span>
                    </div>
                  </div>
                )}
                {worstHolding && (
                  <div>
                    <div
                      style={{
                        fontSize: "12px",
                        color: "var(--text-tertiary)",
                      }}>
                      Pire performance
                    </div>
                    <div
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "6px",
                      }}>
                      <span
                        style={{
                          fontSize: "14px",
                          fontWeight: 500,
                          color: "var(--text-primary)",
                        }}>
                        {worstHolding.asset.ticker ?? worstHolding.asset.name}
                      </span>
                      <span
                        style={{
                          fontSize: "14px",
                          fontWeight: 600,
                          color: worstHolding.pnlValue >= 0 ? "var(--color-gain)" : "var(--color-loss)",
                        }}>
                        {formatSignedPercent(worstHolding.pnlValue)}
                      </span>
                    </div>
                  </div>
                )}
                {!bestHolding && !worstHolding && holdings.data.length > 0 && (
                  <div
                    style={{
                      fontSize: "13px",
                      color: "var(--text-tertiary)",
                    }}>
                    Performance non disponible
                  </div>
                )}
              </div>
            </div>
          )}
        </Card>

        {/* Top Holdings */}
        <Card level={1}>
          <h2
            className="mb-6"
            style={{
              fontSize: "18px",
              fontWeight: 600,
              color: "var(--text-primary)",
            }}>
            Top 5 des actifs
          </h2>
          {isHoldingsLoading || isHoldingsUnavailable || holdingsError ? (
            <p
              style={{
                fontSize: "14px",
                color: holdingsError ? "var(--color-loss)" : "var(--text-tertiary)",
                padding: "24px 0",
                textAlign: "center",
              }}>
              {isHoldingsLoading && "Chargement des actifs..."}
              {isHoldingsUnavailable &&
                "Positions en préparation. Les actifs seront disponibles après génération du portefeuille."}
              {holdingsError && `Impossible de charger les actifs: ${holdingsError}`}
            </p>
          ) : hasTopHoldings ? (
            <div className="space-y-3">
              {holdings.data.map((holding) => {
                const pnlPct = parsePercent(holding.pnl_pct);
                const isPositive = holding.pnl_base_minor >= 0;
                return (
                  <div
                    key={holding.id_asset}
                    className="flex items-center justify-between p-3 rounded-lg transition-all hover:bg-[var(--surface-2-bg)]">
                    <div className="flex items-center gap-3">
                      <div
                        className="w-10 h-10 rounded-full flex items-center justify-center"
                        style={{
                          background: "var(--color-accent)",
                          color: "white",
                          fontSize: "14px",
                          fontWeight: 600,
                        }}>
                        {formatAssetInitials(holding.asset.name, holding.asset.ticker)}
                      </div>
                      <div>
                        <div
                          style={{
                            fontSize: "14px",
                            fontWeight: 500,
                            color: "var(--text-primary)",
                          }}>
                          {holding.asset.name}
                        </div>
                        <div
                          style={{
                            fontSize: "12px",
                            color: "var(--text-tertiary)",
                          }}>
                          {holding.asset.ticker ?? holding.asset.asset_class}
                        </div>
                      </div>
                    </div>
                    <div className="text-right">
                      <div
                        style={{
                          fontFamily: "'JetBrains Mono', monospace",
                          fontSize: "14px",
                          fontWeight: 600,
                          color: "var(--text-primary)",
                        }}>
                        {formatMinorCurrency(holding.market_value_minor, holding.base_currency)}
                      </div>
                      <div
                        style={{
                          fontSize: "12px",
                          fontWeight: 600,
                          color: isPositive
                            ? "var(--color-gain)"
                            : "var(--color-loss)",
                        }}>
                        {pnlPct == null
                          ? formatSignedMinorCurrency(
                              holding.pnl_base_minor,
                              holding.base_currency,
                            )
                          : formatSignedPercent(pnlPct)}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <p
              style={{
                fontSize: "14px",
                color: "var(--text-tertiary)",
                padding: "24px 0",
                textAlign: "center",
              }}>
              Aucune position ouverte dans ce portefeuille.
            </p>
          )}
          <Link
            to="/assets"
            className="block text-center mt-4"
            style={{
              fontSize: "14px",
              color: "var(--color-accent)",
              fontWeight: 500,
            }}>
            Voir tous les actifs →
          </Link>
        </Card>
      </div>

      {/* Transactions récentes */}
      <Card level={1} className="mb-8">
        <h2
          className="mb-6"
          style={{
            fontSize: "18px",
            fontWeight: 600,
            color: "var(--text-primary)",
          }}>
          Transactions récentes
        </h2>
        {recentRows.length === 0 ? (
          <p style={{ fontSize: "14px", color: "var(--text-tertiary)", textAlign: "center", padding: "24px 0" }}>
            Aucune opération enregistrée.
          </p>
        ) : (
          <div style={{ overflowX: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse" }}>
              <thead>
                <tr>
                  {["Date", "Type", "Devise", "Montant"].map((h) => (
                    <th
                      key={h}
                      style={{
                        textAlign: h === "Montant" ? "right" : "left",
                        padding: "0 0 12px 0",
                        fontSize: "12px",
                        fontWeight: 500,
                        color: "var(--text-tertiary)",
                        borderBottom: "1px solid var(--surface-1-border)",
                      }}>
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {recentRows.map((r, i) => {
                  const badge = typeBadgeStyle(r.typeKey);
                  return (
                    <tr
                      key={r.id}
                      className="transition-colors"
                      style={{ cursor: "default" }}
                      onMouseEnter={(e) => { e.currentTarget.style.background = "var(--surface-2-bg)"; }}
                      onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}>
                      <td style={{ padding: "14px 8px 14px 0", fontSize: "13px", fontFamily: "'JetBrains Mono', monospace", color: "var(--text-secondary)", borderBottom: i < recentRows.length - 1 ? "1px solid var(--surface-1-border)" : "none", whiteSpace: "nowrap" }}>
                        {r.date}
                      </td>
                      <td style={{ padding: "14px 8px", borderBottom: i < recentRows.length - 1 ? "1px solid var(--surface-1-border)" : "none" }}>
                        <span style={{ display: "inline-block", padding: "3px 10px", borderRadius: "9999px", fontSize: "12px", fontWeight: 600, color: badge.color, background: badge.bg }}>
                          {r.type}
                        </span>
                      </td>
                      <td style={{ padding: "14px 8px", fontSize: "12px", color: "var(--text-tertiary)", borderBottom: i < recentRows.length - 1 ? "1px solid var(--surface-1-border)" : "none" }}>
                        {r.currency}
                      </td>
                      <td style={{ padding: "14px 0 14px 8px", textAlign: "right", fontFamily: "'JetBrains Mono', monospace", fontSize: "14px", fontWeight: 600, color: "var(--text-primary)", borderBottom: i < recentRows.length - 1 ? "1px solid var(--surface-1-border)" : "none", whiteSpace: "nowrap" }}>
                        {formatCurrency(r.total)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
        <Link
          to="/transactions"
          className="block text-center mt-5"
          style={{
            fontSize: "14px",
            color: "var(--color-accent)",
            fontWeight: 500,
          }}>
          Voir toutes les transactions →
        </Link>
      </Card>
      {showCreatePortfolio && (
        <CreatePortfolioModal onClose={() => setShowCreatePortfolio(false)} />
      )}

      {/* Add Transaction Modal — real operation creation */}
      {showAddTransaction && activePortfolioId && (
        <CreateOperationModal
          portfolioId={activePortfolioId}
          onClose={() => setShowAddTransaction(false)}
        />
      )}

    </div>
  );
}
