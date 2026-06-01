import React, { useState } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { Card } from "../components/Card";
import {
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  Area,
  ComposedChart,
} from "recharts";

const priceData = [
  { month: "Jan", price: 145 },
  { month: "Fév", price: 152 },
  { month: "Mar", price: 168 },
  { month: "Avr", price: 190 },
  { month: "Mai", price: 201 },
  { month: "Juin", price: 208 },
];

const detailsLeft = [
  { label: "Type", value: "Action" },
  { label: "Secteur", value: "Technologie" },
  { label: "Bourse", value: "NASDAQ" },
  { label: "Devise native", value: "USD" },
  { label: "ISIN", value: "US0378331005" },
];

const detailsRight = [
  { label: "Quantité détenue", value: "40" },
  { label: "Poids dans le portefeuille", value: "17%" },
  { label: "Plus haut (1Y)", value: "€210.00" },
  { label: "Plus bas (1Y)", value: "€142.00" },
  { label: "Dividende perçu (total)", value: "€45.80" },
];

const transactions = [
  {
    date: "08/06/2026",
    type: "Dividende",
    qty: "—",
    price: "—",
    fees: "€0.00",
    total: "€45.80",
  },
  {
    date: "05/03/2026",
    type: "Achat",
    qty: "30",
    price: "€208.00",
    fees: "€10.00",
    total: "€6,240.00",
  },
  {
    date: "05/01/2026",
    type: "Achat",
    qty: "10",
    price: "€145.00",
    fees: "€5.00",
    total: "€1,450.00",
  },
];

const typeBadgeColor: Record<string, string> = {
  Achat: "var(--color-gain)",
  Vente: "var(--color-loss)",
  Dividende: "#6366F1",
};

const kpis = [
  { label: "Total investi", value: "€5,800.00" },
  { label: "Valeur actuelle", value: "€8,320.00" },
  { label: "Prix moyen d'achat", value: "€145.00" },
  {
    label: "Gains / Pertes",
    value: "+€2,520.00",
    sub: "(+43.45%)",
    isGain: true,
  },
];

const periods = ["1M", "3M", "6M", "1Y", "MAX"];

const monoCell: React.CSSProperties = {
  fontFamily: "'JetBrains Mono', monospace",
  fontSize: "14px",
  fontVariantNumeric: "tabular-nums",
  color: "var(--text-primary)",
  padding: "14px 12px",
  textAlign: "right",
  whiteSpace: "nowrap",
};

const thStyle: React.CSSProperties = {
  fontSize: "11px",
  fontWeight: 500,
  textTransform: "uppercase",
  letterSpacing: "0.05em",
  color: "var(--text-tertiary)",
  padding: "10px 12px",
  whiteSpace: "nowrap",
};

export function AssetDetail() {
  const navigate = useNavigate();
  const [period, setPeriod] = useState("1Y");

  return (
    <div
      className="max-w-[1200px] mx-auto px-4 sm:px-6 py-12"
      style={{ paddingTop: "clamp(100px, 15vw, 120px)" }}>
      {/* Header */}
      <div className="flex flex-wrap items-start justify-between gap-4 mb-8">
        <div>
          <div className="flex items-center gap-3 mb-2">
            <button
              onClick={() => navigate("/actifs")}
              className="p-1 transition-colors"
              style={{
                background: "none",
                border: "none",
                cursor: "pointer",
                color: "var(--text-secondary)",
                minWidth: "44px",
                minHeight: "44px",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.color = "var(--text-primary)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.color = "var(--text-secondary)";
              }}>
              <ArrowLeft size={20} />
            </button>
            <h1
              style={{
                fontSize: "clamp(20px, 4vw, 24px)",
                fontWeight: 700,
                color: "var(--text-primary)",
              }}>
              Apple Inc.
            </h1>
            <span
              style={{
                fontSize: "12px",
                fontWeight: 500,
                color: "var(--text-secondary)",
                background: "var(--surface-2-bg)",
                border: "1px solid var(--surface-2-border)",
                borderRadius: "var(--radius-md)",
                padding: "4px 8px",
              }}>
              AAPL
            </span>
          </div>
          <div className="flex items-center gap-3 ml-9">
            <span
              style={{
                fontSize: "18px",
                fontWeight: 600,
                color: "var(--text-primary)",
                fontFamily: "'JetBrains Mono', monospace",
              }}>
              €208.00
            </span>
            <span
              style={{
                fontSize: "14px",
                color: "var(--color-gain)",
                fontWeight: 500,
              }}>
              +€7.00 (+3.48%)
            </span>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <button
            style={{
              height: "36px",
              padding: "0 16px",
              borderRadius: "var(--radius-md)",
              border: "none",
              background: "var(--color-cta-bg)",
              color: "var(--color-cta-text)",
              fontSize: "14px",
              fontWeight: 600,
              cursor: "pointer",
            }}>
            Acheter
          </button>
          <button
            style={{
              height: "36px",
              padding: "0 16px",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--surface-1-border)",
              background: "transparent",
              color: "var(--text-primary)",
              fontSize: "14px",
              fontWeight: 500,
              cursor: "pointer",
            }}>
            Vendre
          </button>
        </div>
      </div>

      {/* KPI Cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
        {kpis.map((kpi) => (
          <div
            key={kpi.label}
            className="glass glass-hover"
            style={{
              borderRadius: "var(--radius-xl)",
              padding: "20px",
            }}>
            <div
              className="uppercase mb-2"
              style={{
                fontSize: "11px",
                color: "var(--text-tertiary)",
                letterSpacing: "0.05em",
              }}>
              {kpi.label}
            </div>
            <div
              style={{
                fontFamily: "'JetBrains Mono', monospace",
                fontSize: "20px",
                fontWeight: 700,
                fontVariantNumeric: "tabular-nums",
                color: kpi.isGain ? "var(--color-gain)" : "var(--text-primary)",
              }}>
              {kpi.value}
            </div>
            {kpi.sub && (
              <span style={{ fontSize: "12px", color: "var(--color-gain)" }}>
                {kpi.sub}
              </span>
            )}
          </div>
        ))}
      </div>

      {/* Price chart */}
      <Card level={1} className="mb-8">
        <div className="flex items-center justify-between mb-6">
          <h2
            style={{
              fontSize: "18px",
              fontWeight: 600,
              color: "var(--text-primary)",
            }}>
            Évolution du prix
          </h2>
          <div className="flex gap-1">
            {periods.map((p) => (
              <button
                key={p}
                onClick={() => setPeriod(p)}
                className="px-3 py-1 rounded-[9999px] transition-all"
                style={{
                  fontSize: "12px",
                  fontWeight: 600,
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
        <ResponsiveContainer width="100%" height={280}>
          <ComposedChart data={priceData}>
            <defs>
              <linearGradient id="aaplFill" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#6366F1" stopOpacity={0.12} />
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
              tickFormatter={(v) => `€${v}`}
              domain={["auto", "auto"]}
            />
            <Tooltip
              contentStyle={{
                background: "var(--surface-3-bg)",
                backdropFilter: "blur(16px)",
                border: "1px solid var(--surface-3-border)",
                borderRadius: "var(--radius-md)",
                fontSize: "12px",
              }}
              formatter={(value) => {
                const amount = typeof value === "number" ? value : Number(value ?? 0);
                return [`€${amount.toFixed(2)}`];
              }}
            />
            <Area
              type="monotone"
              dataKey="price"
              fill="url(#aaplFill)"
              stroke="#6366F1"
              strokeWidth={2}
              dot={false}
            />
          </ComposedChart>
        </ResponsiveContainer>
      </Card>

      {/* Asset info */}
      <Card level={1} className="mb-8">
        <h2
          className="mb-6"
          style={{
            fontSize: "18px",
            fontWeight: 600,
            color: "var(--text-primary)",
          }}>
          Informations sur l'actif
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
          {[detailsLeft, detailsRight].map((col, ci) => (
            <div key={ci}>
              {ci === 0 && (
                <div
                  className="mb-3"
                  style={{
                    fontSize: "13px",
                    fontWeight: 600,
                    color: "var(--text-secondary)",
                  }}>
                  Détails de l'actif
                </div>
              )}
              {ci === 1 && (
                <div
                  className="mb-3"
                  style={{
                    fontSize: "13px",
                    fontWeight: 600,
                    color: "var(--text-secondary)",
                  }}>
                  Métriques
                </div>
              )}
              {col.map((row, ri) => (
                <div
                  key={row.label}
                  className="flex items-center justify-between"
                  style={{
                    padding: "10px 0",
                    borderBottom:
                      ri < col.length - 1
                        ? "1px solid var(--surface-1-border)"
                        : "none",
                  }}>
                  <span
                    style={{ fontSize: "14px", color: "var(--text-tertiary)" }}>
                    {row.label}
                  </span>
                  <span
                    style={{
                      fontSize: "14px",
                      fontWeight: 500,
                      fontFamily: "'JetBrains Mono', monospace",
                      color: "var(--text-primary)",
                    }}>
                    {row.value}
                  </span>
                </div>
              ))}
            </div>
          ))}
        </div>
      </Card>

      {/* Transaction history */}
      <Card level={1} className="mb-8">
        <div className="flex items-center justify-between mb-6">
          <h2
            style={{
              fontSize: "18px",
              fontWeight: 600,
              color: "var(--text-primary)",
            }}>
            Historique des transactions
          </h2>
          <button
            style={{
              background: "none",
              border: "none",
              fontSize: "14px",
              fontWeight: 500,
              color: "var(--color-accent)",
              cursor: "pointer",
            }}>
            Voir tout →
          </button>
        </div>
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr>
                <th style={{ ...thStyle, textAlign: "left" }}>Date</th>
                <th style={{ ...thStyle, textAlign: "left" }}>Type</th>
                <th style={{ ...thStyle, textAlign: "right" }}>Quantité</th>
                <th style={{ ...thStyle, textAlign: "right" }}>
                  Prix unitaire
                </th>
                <th style={{ ...thStyle, textAlign: "right" }}>Frais</th>
                <th style={{ ...thStyle, textAlign: "right" }}>Total</th>
              </tr>
            </thead>
            <tbody>
              {transactions.map((tx, i) => (
                <tr
                  key={i}
                  className="transition-colors"
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = "var(--surface-2-bg)";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = "transparent";
                  }}
                  style={{
                    borderBottom:
                      i < transactions.length - 1
                        ? "1px solid var(--surface-1-border)"
                        : "none",
                  }}>
                  <td
                    style={{
                      padding: "14px 12px",
                      fontSize: "13px",
                      fontFamily: "'JetBrains Mono', monospace",
                      color: "var(--text-secondary)",
                      whiteSpace: "nowrap",
                    }}>
                    {tx.date}
                  </td>
                  <td style={{ padding: "14px 12px" }}>
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
                  <td style={monoCell}>{tx.qty}</td>
                  <td style={monoCell}>{tx.price}</td>
                  <td style={monoCell}>{tx.fees}</td>
                  <td style={{ ...monoCell, fontWeight: 600 }}>{tx.total}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
    </div>
  );
}
