import React, { useState } from "react";
import { Link } from "react-router-dom";
import { Card } from "../components/Card";
import { KPICard } from "../components/KPICard";
import { Button } from "../components/Button";
import { Input } from "../components/Input";
import { SwapModal } from "../components/SwapModal";
import {
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  Area,
  Line,
  ComposedChart,
} from "recharts";
import { Plus, ArrowLeftRight, PlusCircle, ChevronUp } from "lucide-react";
import { formatCurrency, formatSignedCurrency } from "../../utils/portfolio";
import {
  allocationData,
  benchmarkData,
  dashboardRecentTransactions,
  dashboardTopAssets,
  portfolioEvolutionData,
  portfolioSummary,
} from "../../mocks/demoPortfolio";

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

const portfolioData = portfolioEvolutionData;
const topHoldings = dashboardTopAssets;
const recentTransactions = dashboardRecentTransactions;

const typeBadgeColor: Record<string, string> = {
  Achat: "var(--color-gain)",
  Vente: "var(--color-loss)",
  Dividende: "#6366F1",
};

export function Dashboard() {
  const [period, setPeriod] = useState("1Y");
  const [benchPeriod, setBenchPeriod] = useState("1Y");
  const [showSwap, setShowSwap] = useState(false);
  const [showAddTransaction, setShowAddTransaction] = useState(false);
  const [showAddAsset, setShowAddAsset] = useState(false);
  const periods = ["1M", "3M", "6M", "1Y", "MAX"];

  return (
    <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
      {/* Page Header */}
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

      {/* KPI Row */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 sm:gap-6 mb-8">
        <KPICard
          label="Valeur nette"
          value={formatCurrency(portfolioSummary.netValue)}
          change={{ value: "+36.66%", isPositive: true }}
        />
        <KPICard label="Total investi" value={formatCurrency(portfolioSummary.investedCapital)} />
        <KPICard
          label="Gain / Perte"
          value={formatSignedCurrency(portfolioSummary.gainLoss)}
          change={{ value: "+36.66%", isPositive: true }}
        />
        <KPICard
          label="Meilleur actif"
          value={portfolioSummary.bestPerformer.name}
          change={{ value: "+141.67%", isPositive: true }}
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
              Alimentez le portefeuille en quelques secondes.
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
              variant="primary"
              icon={ArrowLeftRight}
              className="w-full sm:w-auto"
              onClick={() => setShowSwap(true)}>
              Échanger des actifs
            </Button>
            <Button
              variant="secondary"
              icon={PlusCircle}
              className="w-full sm:w-auto"
              onClick={() => setShowAddAsset(true)}>
              Ajouter un actif
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
              tickFormatter={(value) => `${Math.round(Number(value) / 1000)} k?`}
            />
            <Tooltip
              contentStyle={{
                background: "var(--surface-3-bg)",
                backdropFilter: "blur(16px)",
                border: "1px solid var(--surface-3-border)",
                borderRadius: "var(--radius-md)",
                fontSize: "12px",
              }}
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
          <div className="flex flex-col lg:flex-row items-center gap-6">
            <ResponsiveContainer width="100%" height={200}>
              <PieChart id="dashboard-pie-chart">
                <Pie
                  data={allocationData}
                  cx="50%"
                  cy="50%"
                  innerRadius={60}
                  outerRadius={80}
                  paddingAngle={2}
                  dataKey="value"
                  id="dashboard-pie">
                  {allocationData.map((entry, index) => (
                    <Cell key={`cell-${index}`} fill={entry.color} />
                  ))}
                </Pie>
              </PieChart>
            </ResponsiveContainer>
            <div className="flex flex-col gap-2 w-full">
              {allocationData.map((item) => (
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

          {/* Allocation Metrics */}
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
                  {portfolioSummary.openPositions}
                </div>
              </div>
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
                    BTC
                  </span>
                  <span
                    style={{
                      fontSize: "14px",
                      fontWeight: 600,
                      color: "var(--color-gain)",
                    }}>
                    +141.67%
                  </span>
                </div>
              </div>
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
                    PFE
                  </span>
                  <span
                    style={{
                      fontSize: "14px",
                      fontWeight: 600,
                      color: "var(--color-loss)",
                    }}>
                    -24.29%
                  </span>
                </div>
              </div>
            </div>
          </div>
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
          <div className="space-y-3">
            {topHoldings.map((holding) => (
              <div
                key={holding.ticker}
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
                    {holding.ticker.slice(0, 2)}
                  </div>
                  <div>
                    <div
                      style={{
                        fontSize: "14px",
                        fontWeight: 500,
                        color: "var(--text-primary)",
                      }}>
                      {holding.name}
                    </div>
                    <div
                      style={{
                        fontSize: "12px",
                        color: "var(--text-tertiary)",
                      }}>
                      {holding.ticker}
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
                    {formatCurrency(holding.value)}
                  </div>
                  <div
                    style={{
                      fontSize: "12px",
                      fontWeight: 600,
                      color: holding.isPositive
                        ? "var(--color-gain)"
                        : "var(--color-loss)",
                    }}>
                    {holding.pnl}
                  </div>
                </div>
              </div>
            ))}
          </div>
          <Link
            to="/actifs"
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
        <div style={{ overflowX: "auto" }}>
          <table
            style={{
              width: "100%",
              borderCollapse: "collapse",
            }}>
            <thead>
              <tr>
                {["Date", "Actif", "Type", "Montant"].map((h) => (
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
              {recentTransactions.map((tx, i) => (
                <tr
                  key={i}
                  className="transition-colors"
                  style={{ cursor: "default" }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = "var(--surface-2-bg)";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = "transparent";
                  }}>
                  <td
                    style={{
                      padding: "14px 8px 14px 0",
                      fontSize: "13px",
                      fontFamily: "'JetBrains Mono', monospace",
                      color: "var(--text-secondary)",
                      borderBottom:
                        i < recentTransactions.length - 1
                          ? "1px solid var(--surface-1-border)"
                          : "none",
                      whiteSpace: "nowrap",
                    }}>
                    {tx.date}
                  </td>
                  <td
                    style={{
                      padding: "14px 8px",
                      borderBottom:
                        i < recentTransactions.length - 1
                          ? "1px solid var(--surface-1-border)"
                          : "none",
                    }}>
                    <span
                      style={{
                        fontSize: "14px",
                        fontWeight: 500,
                        color: "var(--text-primary)",
                      }}>
                      {tx.name}
                    </span>
                    <span
                      style={{
                        fontSize: "12px",
                        color: "var(--text-tertiary)",
                        marginLeft: "6px",
                      }}>
                      ({tx.ticker})
                    </span>
                  </td>
                  <td
                    style={{
                      padding: "14px 8px",
                      borderBottom:
                        i < recentTransactions.length - 1
                          ? "1px solid var(--surface-1-border)"
                          : "none",
                    }}>
                    <span
                      style={{
                        display: "inline-block",
                        padding: "3px 10px",
                        borderRadius: "9999px",
                        fontSize: "12px",
                        fontWeight: 600,
                        color: typeBadgeColor[tx.type],
                        background: `color-mix(in srgb, ${typeBadgeColor[tx.type]} 12%, transparent)`,
                      }}>
                      {tx.type}
                    </span>
                  </td>
                  <td
                    style={{
                      padding: "14px 0 14px 8px",
                      textAlign: "right",
                      fontFamily: "'JetBrains Mono', monospace",
                      fontSize: "14px",
                      fontWeight: 600,
                      color: "var(--text-primary)",
                      borderBottom:
                        i < recentTransactions.length - 1
                          ? "1px solid var(--surface-1-border)"
                          : "none",
                      whiteSpace: "nowrap",
                    }}>
                    {formatCurrency(tx.amount)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
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

      {/* Performance vs Benchmark */}
      <Card level={1} className="mb-8">
        <div className="flex items-center justify-between mb-6">
          <h2
            style={{
              fontSize: "18px",
              fontWeight: 600,
              color: "var(--text-primary)",
            }}>
            Performance vs indice de r?f?rence
          </h2>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-[2fr_1fr] gap-6">
          {/* Left — Chart */}
          <div>
            <div className="flex justify-end mb-4">
              <div className="flex gap-1 flex-wrap">
                {periods.map((p) => (
                  <button
                    key={p}
                    onClick={() => setBenchPeriod(p)}
                    className="rounded-[9999px] transition-all hover:-translate-y-[1px]"
                    style={{
                      fontSize: "12px",
                      fontWeight: 600,
                      minHeight: "36px",
                      padding: "0 clamp(12px, 2vw, 14px)",
                      background:
                        benchPeriod === p
                          ? "var(--color-cta-bg)"
                          : "transparent",
                      color:
                        benchPeriod === p
                          ? "var(--color-cta-text)"
                          : "var(--text-secondary)",
                    }}>
                    {p}
                  </button>
                ))}
              </div>
            </div>
            <ResponsiveContainer width="100%" height={280}>
              <ComposedChart data={benchmarkData} id="benchmark-composed-chart">
                <defs>
                  <linearGradient
                    id="portfolioFill"
                    x1="0"
                    y1="0"
                    x2="0"
                    y2="1">
                    <stop offset="0%" stopColor="#6366F1" stopOpacity={0.15} />
                    <stop offset="100%" stopColor="#6366F1" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <XAxis
                  dataKey="month"
                  stroke="var(--text-tertiary)"
                  style={{ fontSize: "12px" }}
                  tick={{ fill: "var(--text-tertiary)" }}
                />
                <YAxis
                  stroke="var(--text-tertiary)"
                  style={{ fontSize: "12px" }}
                  tick={{ fill: "var(--text-tertiary)" }}
                  tickFormatter={(v) => `${v}%`}
                  domain={[0, 40]}
                />
                <Tooltip
                  contentStyle={{
                    background: "var(--surface-3-bg)",
                    backdropFilter: "blur(16px)",
                    border: "1px solid var(--surface-3-border)",
                    borderRadius: "var(--radius-md)",
                    fontSize: "12px",
                  }}
                  formatter={(value, name) => {
                    const amount = typeof value === "number" ? value : Number(value ?? 0);
                    const key = String(name);
                    const labels: Record<string, string> = {
                      portfolio: "Mon portefeuille",
                      sp500: "S&P 500",
                      msci: "MSCI World",
                    };
                    return [`+${amount.toFixed(1)}%`, labels[key] || key];
                  }}
                />
                <Area
                  type="monotone"
                  dataKey="portfolio"
                  fill="url(#portfolioFill)"
                  stroke="#6366F1"
                  strokeWidth={2.5}
                  dot={false}
                  name="portfolio"
                />
                <Line
                  type="monotone"
                  dataKey="sp500"
                  stroke="#F59E0B"
                  strokeWidth={1.5}
                  strokeDasharray="6 3"
                  dot={false}
                  name="sp500"
                />
                <Line
                  type="monotone"
                  dataKey="msci"
                  stroke="#8B5CF6"
                  strokeWidth={1.5}
                  strokeDasharray="6 3"
                  dot={false}
                  name="msci"
                />
              </ComposedChart>
            </ResponsiveContainer>
          </div>

          {/* Right — Summary */}
          <div className="flex flex-col md:justify-center">
            {/* On mobile: horizontal scroll row */}
            <div className="flex md:flex-col gap-4 md:gap-0 overflow-x-auto md:overflow-visible pb-2 md:pb-0">
              {/* Bloc 1 — Mon portefeuille */}
              <div
                className="min-w-[180px] md:min-w-0 flex-shrink-0 md:flex-shrink"
                style={{
                  paddingBottom: "16px",
                  borderBottom: "1px solid var(--surface-1-border)",
                }}>
                <div className="flex items-center gap-2 mb-2">
                  <div
                    className="w-2.5 h-2.5 rounded-full"
                    style={{ background: "#6366F1" }}
                  />
                  <span
                    style={{
                      fontSize: "13px",
                      color: "var(--text-secondary)",
                    }}>
                    Mon portefeuille
                  </span>
                </div>
                <div
                  style={{
                    fontFamily: "'JetBrains Mono', monospace",
                    fontSize: "24px",
                    fontWeight: 700,
                    color: "var(--color-gain)",
                  }}>
                  +36.66%
                </div>
                <span
                  style={{
                    fontSize: "12px",
                    color: "var(--text-tertiary)",
                  }}>
                  sur 1 an
                </span>
              </div>

              {/* Bloc 2 — S&P 500 */}
              <div
                className="min-w-[180px] md:min-w-0 flex-shrink-0 md:flex-shrink"
                style={{
                  padding: "16px 0",
                  borderBottom: "1px solid var(--surface-1-border)",
                }}>
                <div className="flex items-center gap-2 mb-2">
                  <div
                    className="w-2.5 h-2.5 rounded-full"
                    style={{ background: "#F59E0B" }}
                  />
                  <span
                    style={{
                      fontSize: "13px",
                      color: "var(--text-secondary)",
                    }}>
                    S&P 500
                  </span>
                </div>
                <div
                  style={{
                    fontFamily: "'JetBrains Mono', monospace",
                    fontSize: "20px",
                    fontWeight: 600,
                    color: "var(--text-primary)",
                  }}>
                  +10.6%
                </div>
                <div className="flex items-center gap-1 mt-1">
                  <ChevronUp size={14} style={{ color: "var(--color-gain)" }} />
                  <span
                    style={{
                      fontSize: "12px",
                      fontWeight: 500,
                      color: "var(--color-gain)",
                    }}>
                    +10.6%
                  </span>
                </div>
              </div>

              {/* Bloc 3 — MSCI World */}
              <div
                className="min-w-[180px] md:min-w-0 flex-shrink-0 md:flex-shrink"
                style={{ paddingTop: "16px" }}>
                <div className="flex items-center gap-2 mb-2">
                  <div
                    className="w-2.5 h-2.5 rounded-full"
                    style={{ background: "#8B5CF6" }}
                  />
                  <span
                    style={{
                      fontSize: "13px",
                      color: "var(--text-secondary)",
                    }}>
                    MSCI World
                  </span>
                </div>
                <div
                  style={{
                    fontFamily: "'JetBrains Mono', monospace",
                    fontSize: "20px",
                    fontWeight: 600,
                    color: "var(--text-primary)",
                  }}>
                  +9.2%
                </div>
                <div className="flex items-center gap-1 mt-1">
                  <ChevronUp size={14} style={{ color: "var(--color-gain)" }} />
                  <span
                    style={{
                      fontSize: "12px",
                      fontWeight: 500,
                      color: "var(--color-gain)",
                    }}>
                    +9.2%
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Card>

      <SwapModal isOpen={showSwap} onClose={() => setShowSwap(false)} />

      {/* Add Transaction Modal */}
      {showAddTransaction && (
        <>
          <div
            className="fixed inset-0 z-40"
            style={{
              background: "rgba(0, 0, 0, 0.40)",
              backdropFilter: "blur(4px)",
              WebkitBackdropFilter: "blur(4px)",
            }}
            onClick={() => setShowAddTransaction(false)}
          />
          <div className="fixed inset-0 z-50 flex items-center justify-center p-6">
            <Card level={3} className="w-full max-w-[520px]">
              <h2
                className="mb-4"
                style={{
                  fontSize: "18px",
                  fontWeight: 600,
                  color: "var(--text-primary)",
                }}>
                Ajouter une transaction
              </h2>
              <p
                style={{
                  fontSize: "14px",
                  color: "var(--text-secondary)",
                  marginBottom: "20px",
                }}>
                Enregistrez un achat, une vente ou un mouvement pour mettre à
                jour vos performances.
              </p>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <Input label="Actif" placeholder="BTC, AAPL, IWDA" />
                <Input label="Type" placeholder="Achat, Vente, Dividende" />
                <Input label="Montant" placeholder="1 250,00 €" />
                <Input label="Étiquette" placeholder="Long terme, Rééquilibrage" />
              </div>
              <div className="mt-5">
                <Input
                  label="Note"
                  placeholder="Pourquoi cette transaction ?"
                />
              </div>
              <div className="flex gap-3 justify-end mt-6">
                <Button
                  variant="ghost"
                  onClick={() => setShowAddTransaction(false)}>
                  Annuler
                </Button>
                <Button
                  variant="primary"
                  onClick={() => setShowAddTransaction(false)}>
                  Enregistrer
                </Button>
              </div>
            </Card>
          </div>
        </>
      )}

      {/* Add Asset Modal */}
      {showAddAsset && (
        <>
          <div
            className="fixed inset-0 z-40"
            style={{
              background: "rgba(0, 0, 0, 0.40)",
              backdropFilter: "blur(4px)",
              WebkitBackdropFilter: "blur(4px)",
            }}
            onClick={() => setShowAddAsset(false)}
          />
          <div className="fixed inset-0 z-50 flex items-center justify-center p-6">
            <Card level={3} className="w-full max-w-[520px]">
              <h2
                className="mb-4"
                style={{
                  fontSize: "18px",
                  fontWeight: 600,
                  color: "var(--text-primary)",
                }}>
                Ajouter un actif
              </h2>
              <p
                style={{
                  fontSize: "14px",
                  color: "var(--text-secondary)",
                  marginBottom: "20px",
                }}>
                Ajoutez un nouvel actif a votre portefeuille et suivez sa
                valeur.
              </p>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <Input label="Nom" placeholder="Apple Inc." />
                <Input label="Ticker" placeholder="AAPL" />
                <Input label="Quantite" placeholder="20" />
                <Input label="Prix d'achat" placeholder="€145.00" />
              </div>
              <div className="flex gap-3 justify-end mt-6">
                <Button variant="ghost" onClick={() => setShowAddAsset(false)}>
                  Annuler
                </Button>
                <Button
                  variant="primary"
                  onClick={() => setShowAddAsset(false)}>
                  Ajouter
                </Button>
              </div>
            </Card>
          </div>
        </>
      )}
    </div>
  );
}
