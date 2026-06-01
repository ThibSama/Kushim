import React, { useEffect, useRef, useState } from "react";
import {
  Search,
  Calendar,
  ChevronDown,
  Plus,
  ShoppingCart,
  TrendingUp,
  ArrowDownCircle,
  ArrowUpCircle,
  Landmark,
  Receipt,
} from "lucide-react";
import { Card } from "../components/Card";
import { Button } from "../components/Button";
import { formatCurrency } from "../../utils/portfolio";
import { transactionRows } from "../../mocks/demoPortfolio";

const txData = transactionRows;

const typeOptions = [
  "Tous les types",
  "Achat",
  "Vente",
  "Dividende",
  "Dépôt",
  "Retrait",
];
const accountOptions = [
  "Tous les comptes",
  ...Array.from(
    new Set(txData.map((tx) => tx.details.replace("Compte : ", ""))),
  ),
];
const assetOptions = [
  "Tous les actifs",
  ...Array.from(new Set(txData.map((tx) => tx.asset.split(" - ")[0]))),
];
const periodOptions = [
  "Toutes les périodes",
  "30 derniers jours",
  "90 derniers jours",
  "12 derniers mois",
];
const availableTags = Array.from(
  new Set(txData.flatMap((tx) => tx.tags)),
).sort((a, b) => a.localeCompare(b, "fr"));

const badgeStyles: Record<string, { color: string; bg: string }> = {
  ACHAT: { color: "var(--color-gain)", bg: "rgba(16, 185, 129, 0.10)" },
  VENTE: { color: "var(--color-loss)", bg: "rgba(239, 68, 68, 0.10)" },
  DIVIDENDE: { color: "#6366F1", bg: "rgba(99, 102, 241, 0.10)" },
  "DÉPÔT": { color: "var(--text-secondary)", bg: "rgba(161, 161, 170, 0.08)" },
  RETRAIT: { color: "var(--text-secondary)", bg: "rgba(161, 161, 170, 0.08)" },
};

const tagStyles: Record<string, { color: string; bg: string; border: string }> =
  {
    "Long terme": {
      color: "var(--color-accent)",
      bg: "rgba(59, 130, 246, 0.10)",
      border: "rgba(59, 130, 246, 0.25)",
    },
    "Prise de profit": {
      color: "var(--color-gain)",
      bg: "rgba(16, 185, 129, 0.12)",
      border: "rgba(16, 185, 129, 0.25)",
    },
    "Rééquilibrage": {
      color: "#8B5CF6",
      bg: "rgba(139, 92, 246, 0.12)",
      border: "rgba(139, 92, 246, 0.22)",
    },
    Dividende: {
      color: "#F59E0B",
      bg: "rgba(245, 158, 11, 0.12)",
      border: "rgba(245, 158, 11, 0.22)",
    },
    Momentum: {
      color: "#EC4899",
      bg: "rgba(236, 72, 153, 0.12)",
      border: "rgba(236, 72, 153, 0.22)",
    },
    DCA: {
      color: "var(--color-accent)",
      bg: "rgba(59, 130, 246, 0.10)",
      border: "rgba(59, 130, 246, 0.25)",
    },
    "Défensif": {
      color: "var(--color-warning)",
      bg: "rgba(245, 158, 11, 0.12)",
      border: "rgba(245, 158, 11, 0.25)",
    },
    Cash: {
      color: "var(--text-secondary)",
      bg: "rgba(161, 161, 170, 0.12)",
      border: "rgba(161, 161, 170, 0.24)",
    },
  };

const thStyle: React.CSSProperties = {
  fontSize: "11px",
  fontWeight: 500,
  textTransform: "uppercase",
  letterSpacing: "0.05em",
  color: "var(--text-tertiary)",
  padding: "12px 16px",
  whiteSpace: "nowrap",
  borderBottom: "1px solid var(--surface-1-border)",
};

const monoCell: React.CSSProperties = {
  fontFamily: "'JetBrains Mono', monospace",
  fontSize: "14px",
  fontVariantNumeric: "tabular-nums",
  color: "var(--text-primary)",
  padding: "14px 16px",
  whiteSpace: "nowrap",
};

const ghostBtnStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "6px",
  minHeight: "44px",
  height: "44px",
  padding: "0 14px",
  borderRadius: "var(--radius-md)",
  border: "1px solid var(--surface-1-border)",
  background: "transparent",
  fontSize: "14px",
  fontWeight: 500,
  color: "var(--text-primary)",
  cursor: "pointer",
  whiteSpace: "nowrap",
};

export function Transactions() {
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [periodFilter, setPeriodFilter] = useState("Toutes les périodes");
  const [typeFilter, setTypeFilter] = useState("Tous les types");
  const [accountFilter, setAccountFilter] = useState("Tous les comptes");
  const [assetFilter, setAssetFilter] = useState("Tous les actifs");
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [tagMenuOpen, setTagMenuOpen] = useState(false);
  const [page, setPage] = useState(1);
  const tagMenuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!isModalOpen) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsModalOpen(false);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isModalOpen]);

  useEffect(() => {
    if (!tagMenuOpen) return;

    const handlePointerDown = (event: MouseEvent) => {
      if (
        tagMenuRef.current &&
        !tagMenuRef.current.contains(event.target as Node)
      ) {
        setTagMenuOpen(false);
      }
    };

    document.addEventListener("mousedown", handlePointerDown);
    return () => document.removeEventListener("mousedown", handlePointerDown);
  }, [tagMenuOpen]);

  const parseTransactionDate = (value: string) => {
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? null : date;
  };

  const latestTransactionTime = txData.reduce((latest, tx) => {
    const parsed = parseTransactionDate(tx.date);
    if (!parsed) return latest;
    return Math.max(latest, parsed.getTime());
  }, 0);

  const filtered = txData.filter((tx) => {
    if (
      search &&
      !tx.asset.toLowerCase().includes(search.toLowerCase()) &&
      !tx.details.toLowerCase().includes(search.toLowerCase())
    )
      return false;
    if (typeFilter !== "Tous les types" && tx.type !== typeFilter.toUpperCase())
      return false;
    if (
      accountFilter !== "Tous les comptes" &&
      !tx.details.includes(accountFilter)
    )
      return false;
    if (assetFilter !== "Tous les actifs" && !tx.asset.startsWith(assetFilter))
      return false;
    if (periodFilter !== "Toutes les périodes") {
      const parsed = parseTransactionDate(tx.date);
      if (!parsed) return false;

      const dayMs = 24 * 60 * 60 * 1000;
      const ageInDays = (latestTransactionTime - parsed.getTime()) / dayMs;

      if (periodFilter === "30 derniers jours" && ageInDays > 30) return false;
      if (periodFilter === "90 derniers jours" && ageInDays > 90) return false;
      if (periodFilter === "12 derniers mois" && ageInDays > 365)
        return false;
    }
    if (
      selectedTags.length > 0 &&
      !tx.tags.some((tag) => selectedTags.includes(tag))
    )
      return false;
    return true;
  });

  const toggleTag = (tag: string) => {
    setSelectedTags((prev) =>
      prev.includes(tag) ? prev.filter((item) => item !== tag) : [...prev, tag],
    );
  };

  const tagFilterLabel =
    selectedTags.length === 0
      ? "Toutes les étiquettes"
      : selectedTags.length === 1
        ? selectedTags[0]
        : `${selectedTags.length} étiquettes`;


  const transactionMetrics = filtered.reduce(
    (acc, tx) => {
      const total = tx.total;
      const fees = tx.fees;

      if (tx.type === "ACHAT") acc.purchases += total;
      if (tx.type === "VENTE") acc.sales += total;
      if (tx.type === "DIVIDENDE") acc.dividends += total;
      if (tx.type === "DÉPÔT") acc.deposits += total;
      if (tx.type === "RETRAIT") acc.withdrawals += total;
      acc.fees += fees;

      return acc;
    },
    {
      purchases: 0,
      sales: 0,
      deposits: 0,
      withdrawals: 0,
      dividends: 0,
      fees: 0,
    },
  );

  const metricCards = [
    {
      key: "purchases",
      label: "Achats",
      value: formatCurrency(transactionMetrics.purchases),
      icon: ShoppingCart,
      iconColor: "var(--color-gain)",
      accent: "rgba(16, 185, 129, 0.14)",
      mdSpan: "md:col-span-1",
      xlSpan: "xl:col-span-2",
      minHeight: "104px",
    },
    {
      key: "sales",
      label: "Ventes",
      value: formatCurrency(transactionMetrics.sales),
      icon: TrendingUp,
      iconColor: "var(--color-loss)",
      accent: "rgba(239, 68, 68, 0.12)",
      mdSpan: "md:col-span-1",
      xlSpan: "xl:col-span-1",
      minHeight: "104px",
    },
    {
      key: "deposits",
      label: "Dépôts",
      value: formatCurrency(transactionMetrics.deposits),
      icon: ArrowDownCircle,
      iconColor: "var(--color-accent)",
      accent: "rgba(59, 130, 246, 0.12)",
      mdSpan: "md:col-span-1",
      xlSpan: "xl:col-span-1",
      minHeight: "104px",
    },
    {
      key: "withdrawals",
      label: "Retraits",
      value: formatCurrency(transactionMetrics.withdrawals),
      icon: ArrowUpCircle,
      iconColor: "var(--color-warning)",
      accent: "rgba(245, 158, 11, 0.12)",
      mdSpan: "md:col-span-1",
      xlSpan: "xl:col-span-2",
      minHeight: "88px",
    },
    {
      key: "dividends",
      label: "Dividendes",
      value: formatCurrency(transactionMetrics.dividends),
      icon: Landmark,
      iconColor: "#6366F1",
      accent: "rgba(99, 102, 241, 0.12)",
      mdSpan: "md:col-span-1",
      xlSpan: "xl:col-span-1",
      minHeight: "88px",
    },
    {
      key: "fees",
      label: "Frais",
      value: formatCurrency(transactionMetrics.fees),
      icon: Receipt,
      iconColor: "var(--text-secondary)",
      accent: "rgba(161, 161, 170, 0.10)",
      mdSpan: "md:col-span-1",
      xlSpan: "xl:col-span-1",
      minHeight: "88px",
    },
  ];

  const totalPages = 25;
  const pageNumbers = [1, 2, 3, null, totalPages];
  const visibleTransactions = filtered.slice(0, 10);

  return (
    <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
      {/* Header */}
      <div className="mb-6 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
        <h1
          style={{
            fontSize: "clamp(24px, 5vw, 30px)",
            fontWeight: 700,
            color: "var(--text-primary)",
          }}>
          Transactions
        </h1>
        <p
          style={{
            fontSize: "clamp(13px, 2.5vw, 14px)",
            color: "var(--text-secondary)",
            marginTop: "4px",
          }}>
          Historique complet des transactions et journal d'audit
        </p>
        </div>
        <button
          onClick={() => setIsModalOpen(true)}
          className="w-full sm:w-auto"
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: "6px",
            height: "44px",
            padding: "0 16px",
            borderRadius: "var(--radius-md)",
            border: "none",
            background: "var(--color-cta-bg)",
            fontSize: "14px",
            fontWeight: 600,
            color: "var(--color-cta-text)",
            cursor: "pointer",
            flexShrink: 0,
          }}>
          <Plus size={16} />
          Ajouter une transaction
        </button>
      </div>

      {/* Controls */}
      <div
        className="flex flex-col lg:flex-row lg:flex-wrap items-stretch lg:items-center gap-3"
        style={{ marginTop: "24px" }}>
        <div className="relative w-full sm:max-w-[320px]">
          <Search
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2"
            style={{ color: "var(--text-tertiary)" }}
          />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Rechercher par actif, compte..."
            className="w-full"
            style={{
              background: "var(--surface-2-bg)",
              border: "1px solid var(--surface-2-border)",
              borderRadius: "var(--radius-md)",
              minHeight: "44px",
              height: "44px",
              paddingLeft: "36px",
              paddingRight: "16px",
              fontSize: "14px",
              color: "var(--text-primary)",
            }}
          />
        </div>

        <div className="relative min-w-[180px] flex-shrink-0">
          <select
            value={periodFilter}
            onChange={(e) => setPeriodFilter(e.target.value)}
            className="w-full appearance-none cursor-pointer"
            style={{
              ...ghostBtnStyle,
              width: "100%",
              paddingLeft: "36px",
              paddingRight: "32px",
            }}>
            {periodOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
          <Calendar
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none"
            style={{ color: "var(--text-tertiary)" }}
          />
          <ChevronDown
            size={14}
            className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none"
            style={{ color: "var(--text-tertiary)" }}
          />
        </div>

        <div className="relative min-w-[170px] flex-shrink-0">
          <select
            value={typeFilter}
            onChange={(e) => setTypeFilter(e.target.value)}
            className="w-full appearance-none cursor-pointer"
            style={{ ...ghostBtnStyle, width: "100%", paddingRight: "32px" }}>
            {typeOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
          <ChevronDown
            size={14}
            className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none"
            style={{ color: "var(--text-tertiary)" }}
          />
        </div>

        <div className="relative min-w-[190px] flex-shrink-0">
          <select
            value={accountFilter}
            onChange={(e) => setAccountFilter(e.target.value)}
            className="w-full appearance-none cursor-pointer"
            style={{ ...ghostBtnStyle, width: "100%", paddingRight: "32px" }}>
            {accountOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
          <ChevronDown
            size={14}
            className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none"
            style={{ color: "var(--text-tertiary)" }}
          />
        </div>

        <div className="relative min-w-[150px] flex-shrink-0">
          <select
            value={assetFilter}
            onChange={(e) => setAssetFilter(e.target.value)}
            className="w-full appearance-none cursor-pointer"
            style={{ ...ghostBtnStyle, width: "100%", paddingRight: "32px" }}>
            {assetOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
          <ChevronDown
            size={14}
            className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none"
            style={{ color: "var(--text-tertiary)" }}
          />
        </div>

        <div ref={tagMenuRef} className="relative min-w-[210px] flex-shrink-0">
          <button
            type="button"
            onClick={() => setTagMenuOpen((open) => !open)}
            className="w-full"
            style={{
              ...ghostBtnStyle,
              width: "100%",
              justifyContent: "space-between",
            }}>
            <span
              style={{
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}>
              {tagFilterLabel}
            </span>
            <ChevronDown
              size={14}
              style={{
                color: "var(--text-tertiary)",
                transform: tagMenuOpen ? "rotate(180deg)" : "rotate(0deg)",
                transition: "transform 180ms ease",
              }}
            />
          </button>

          {tagMenuOpen ? (
            <div
              style={{
                position: "absolute",
                top: "calc(100% + 8px)",
                left: 0,
                width: "100%",
                minWidth: "240px",
                padding: "10px",
                borderRadius: "var(--radius-lg)",
                border: "1px solid var(--surface-1-border)",
                background: "var(--surface-1-bg)",
                backdropFilter: "blur(24px)",
                WebkitBackdropFilter: "blur(24px)",
                boxShadow: "0 16px 40px rgba(0, 0, 0, 0.18)",
                zIndex: 20,
              }}>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: "12px",
                  marginBottom: "8px",
                }}>
                <span
                  style={{
                    fontSize: "12px",
                    fontWeight: 600,
                    color: "var(--text-secondary)",
                  }}>
                  Filtrer par étiquette
                </span>
                {selectedTags.length > 0 ? (
                  <button
                    type="button"
                    onClick={() => setSelectedTags([])}
                    style={{
                      border: "none",
                      background: "transparent",
                      padding: 0,
                      fontSize: "12px",
                      fontWeight: 600,
                      color: "var(--color-accent)",
                      cursor: "pointer",
                    }}>
                    Réinitialiser
                  </button>
                ) : null}
              </div>

              <div
                className="flex flex-col gap-1"
                style={{ maxHeight: "220px", overflowY: "auto" }}>
                {availableTags.map((tag) => {
                  const checked = selectedTags.includes(tag);

                  return (
                    <label
                      key={tag}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "10px",
                        minHeight: "36px",
                        padding: "0 8px",
                        borderRadius: "var(--radius-md)",
                        color: "var(--text-primary)",
                        cursor: "pointer",
                        background: checked ? "var(--surface-2-bg)" : "transparent",
                      }}>
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => toggleTag(tag)}
                        style={{ accentColor: "var(--color-accent)" }}
                      />
                      <span style={{ fontSize: "13px", lineHeight: 1.3 }}>{tag}</span>
                    </label>
                  );
                })}
              </div>
            </div>
          ) : null}
        </div>
      </div>

      {/* Metrics Bento */}
      <div
        className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3 sm:gap-4"
        style={{ marginTop: "24px", marginBottom: "32px" }}>
        {metricCards.map((metric) => {
          const Icon = metric.icon;

          return (
            <div
              key={metric.key}
              className={`glass glass-hover ${metric.mdSpan} ${metric.xlSpan}`}
              style={{
                borderRadius: "var(--radius-xl)",
                padding: "clamp(14px, 2vw, 16px)",
                minHeight: metric.minHeight,
                display: "flex",
                flexDirection: "column",
                justifyContent: "flex-start",
                gap: "8px",
                position: "relative",
                overflow: "hidden",
              }}>
              <div
                aria-hidden="true"
                style={{
                  position: "absolute",
                  inset: "-18% auto auto -4%",
                  width: "88px",
                  height: "88px",
                  borderRadius: "9999px",
                  background: metric.accent,
                  filter: "blur(16px)",
                  opacity: 0.75,
                }}
              />
              <div
                className="relative flex items-center gap-3"
                style={{ zIndex: 1 }}>
                <div
                  className="rounded-[14px] flex items-center justify-center"
                  style={{
                    width: "34px",
                    height: "34px",
                    background: "var(--surface-2-bg)",
                    border: "1px solid var(--surface-2-border)",
                    color: metric.iconColor,
                    flexShrink: 0,
                  }}>
                  <Icon size={16} />
                </div>
                <div
                  style={{
                    fontSize: "11px",
                    letterSpacing: "0.05em",
                    textTransform: "uppercase",
                    color: "var(--text-tertiary)",
                    fontWeight: 600,
                    lineHeight: 1.2,
                  }}>
                  {metric.label}
                </div>
              </div>
              <div className="relative" style={{ zIndex: 1 }}>
                <div
                  style={{
                    fontFamily: "'JetBrains Mono', monospace",
                    fontSize:
                      metric.key === "purchases"
                        ? "clamp(20px, 2vw, 22px)"
                        : metric.key === "sales"
                          ? "clamp(19px, 1.9vw, 21px)"
                          : "clamp(18px, 1.8vw, 20px)",
                    lineHeight: 1.1,
                    fontWeight: 700,
                    fontVariantNumeric: "tabular-nums",
                    color: "var(--text-primary)",
                  }}>
                  {metric.value}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Table */}
      <Card level={1} className="overflow-hidden mt-8" noPadding>
        <div style={{ overflowX: "auto", WebkitOverflowScrolling: "touch" }}>
          <table
            style={{
              width: "100%",
              borderCollapse: "collapse",
              minWidth: "1000px",
            }}>
            <thead>
              <tr>
                <th
                  style={{ ...thStyle, textAlign: "left", minWidth: "100px" }}>
                  Date
                </th>
                <th
                  style={{ ...thStyle, textAlign: "left", minWidth: "140px" }}>
                  Actif
                </th>
                <th style={{ ...thStyle, textAlign: "left", minWidth: "90px" }}>
                  Type
                </th>
                <th
                  style={{ ...thStyle, textAlign: "left", minWidth: "150px" }}>
Étiquettes
                </th>
                <th
                  style={{ ...thStyle, textAlign: "right", minWidth: "90px" }}>
                  Quantité
                </th>
                <th
                  style={{ ...thStyle, textAlign: "right", minWidth: "100px" }}>
                  Prix
                </th>
                <th
                  style={{ ...thStyle, textAlign: "right", minWidth: "80px" }}
                  className="hidden sm:table-cell">
                  Frais
                </th>
                <th
                  style={{ ...thStyle, textAlign: "center", minWidth: "70px" }}
                  className="hidden sm:table-cell">
                  Devise
                </th>
                <th
                  style={{ ...thStyle, textAlign: "right", minWidth: "110px" }}>
                  Montant total
                </th>
                <th
                  style={{ ...thStyle, textAlign: "left", minWidth: "180px" }}
                  className="hidden lg:table-cell">
                  Note
                </th>
                <th
                  style={{ ...thStyle, textAlign: "left", minWidth: "180px" }}
                  className="hidden lg:table-cell">
                  Détails
                </th>
              </tr>
            </thead>
            <tbody>
              {visibleTransactions.map((tx, i) => {
                const badge = badgeStyles[tx.type] || badgeStyles.ACHAT;
                return (
                  <tr
                    key={tx.id}
                    className="transition-colors"
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background = "var(--surface-2-bg)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background = "transparent";
                    }}
                    style={{
                      borderBottom:
                        i < visibleTransactions.length - 1
                          ? "1px solid var(--surface-1-border)"
                          : "none",
                    }}>
                    <td
                      style={{
                        padding: "14px 16px",
                        fontSize: "14px",
                        color: "var(--text-secondary)",
                        whiteSpace: "nowrap",
                        minWidth: "100px",
                      }}>
                      {tx.date}
                    </td>
                    <td
                      style={{
                        padding: "14px 16px",
                        fontSize: "14px",
                        fontWeight: 500,
                        color: "var(--text-primary)",
                        whiteSpace: "nowrap",
                        minWidth: "140px",
                      }}>
                      {tx.asset}
                    </td>
                    <td style={{ padding: "14px 16px", minWidth: "90px" }}>
                      <span
                        style={{
                          display: "inline-block",
                          padding: "4px 12px",
                          borderRadius: "var(--radius-md)",
                          fontSize: "11px",
                          fontWeight: 600,
                          textTransform: "uppercase",
                          letterSpacing: "0.03em",
                          color: badge.color,
                          background: badge.bg,
                        }}>
                        {tx.type}
                      </span>
                    </td>
                    <td
                      style={{
                        padding: "14px 16px",
                        minWidth: "150px",
                      }}>
                      <div className="flex flex-wrap gap-1.5">
                        {tx.tags.map((tag) => {
                          const style =
                            tagStyles[tag] ||
                            ({
                              color: "var(--text-secondary)",
                              bg: "rgba(161, 161, 170, 0.10)",
                              border: "rgba(161, 161, 170, 0.2)",
                            } as const);
                          return (
                            <span
                              key={tag}
                              style={{
                                display: "inline-flex",
                                alignItems: "center",
                                padding: "4px 10px",
                                borderRadius: "999px",
                                fontSize: "10px",
                                fontWeight: 700,
                                textTransform: "uppercase",
                                letterSpacing: "0.08em",
                                color: style.color,
                                background: style.bg,
                                border: `1px solid ${style.border}`,
                              }}>
                              {tag}
                            </span>
                          );
                        })}
                      </div>
                    </td>
                    <td
                      style={{
                        ...monoCell,
                        textAlign: "right",
                        minWidth: "90px",
                      }}>
                      {tx.qty}
                    </td>
                    <td
                      style={{
                        ...monoCell,
                        textAlign: "right",
                        minWidth: "100px",
                      }}>
                      {tx.price === null ? "-" : formatCurrency(tx.price)}
                    </td>
                    <td
                      style={{
                        ...monoCell,
                        textAlign: "right",
                        color: "var(--text-secondary)",
                        minWidth: "80px",
                      }}
                      className="hidden sm:table-cell">
                      {formatCurrency(tx.fees)}
                    </td>
                    <td
                      style={{
                        padding: "14px 16px",
                        textAlign: "center",
                        fontSize: "12px",
                        color: "var(--text-tertiary)",
                        whiteSpace: "nowrap",
                        minWidth: "70px",
                      }}
                      className="hidden sm:table-cell">
                      {tx.currency}
                    </td>
                    <td
                      style={{
                        ...monoCell,
                        textAlign: "right",
                        fontWeight: 600,
                        minWidth: "110px",
                      }}>
                      {formatCurrency(tx.total)}
                    </td>
                    <td
                      style={{
                        padding: "14px 16px",
                        fontSize: "12px",
                        color: "var(--text-tertiary)",
                        minWidth: "180px",
                      }}
                      className="hidden lg:table-cell">
                      {tx.note || "—"}
                    </td>
                    <td
                      style={{
                        padding: "14px 16px",
                        fontSize: "12px",
                        color: "var(--text-tertiary)",
                        whiteSpace: "nowrap",
                        minWidth: "180px",
                      }}
                      className="hidden lg:table-cell">
                      {tx.details}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        {/* Pagination */}
        <div
          className="flex flex-wrap items-center justify-between gap-4"
          style={{
            padding: "16px",
            borderTop: "1px solid var(--surface-1-border)",
          }}>
          <span style={{ fontSize: "14px", color: "var(--text-tertiary)" }}>
            Affichage 1-10 sur 243 transactions
          </span>
          <div className="flex items-center gap-2">
            <button
              disabled={page === 1}
              onClick={() => setPage(Math.max(1, page - 1))}
              style={{
                ...ghostBtnStyle,
                height: "36px",
                padding: "0 12px",
                opacity: page === 1 ? 0.4 : 1,
                cursor: page === 1 ? "not-allowed" : "pointer",
              }}>
              ← Précédent
            </button>
            {pageNumbers.map((p, i) =>
              p === null ? (
                <span
                  key={`ellipsis-${i}`}
                  style={{
                    fontSize: "14px",
                    color: "var(--text-tertiary)",
                    padding: "0 4px",
                  }}>
                  …
                </span>
              ) : (
                <button
                  key={p}
                  onClick={() => setPage(p)}
                  style={{
                    width: "36px",
                    height: "36px",
                    borderRadius: "var(--radius-md)",
                    border: "none",
                    fontSize: "14px",
                    fontWeight: 500,
                    cursor: "pointer",
                    background: page === p ? "#6366F1" : "transparent",
                    color: page === p ? "#fff" : "var(--text-secondary)",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                  }}
                  onMouseEnter={(e) => {
                    if (page !== p)
                      e.currentTarget.style.background = "var(--surface-2-bg)";
                  }}
                  onMouseLeave={(e) => {
                    if (page !== p)
                      e.currentTarget.style.background = "transparent";
                  }}>
                  {p}
                </button>
              ),
            )}
            <button
              onClick={() => setPage(Math.min(totalPages, page + 1))}
              style={{ ...ghostBtnStyle, height: "36px", padding: "0 12px" }}>
              Suivant →
            </button>
          </div>
        </div>
      </Card>

      {/* Add Transaction Modal */}
      {isModalOpen && (
        <>
          <div
            className="fixed inset-0 z-40"
            style={{
              background: "rgba(0, 0, 0, 0.40)",
              backdropFilter: "blur(4px)",
              WebkitBackdropFilter: "blur(4px)",
            }}
            onClick={() => setIsModalOpen(false)}
          />
          <div className="fixed inset-0 z-50 flex items-center justify-center p-6">
            <Card level={3} className="w-full max-w-[480px]">
              <h2
                className="mb-6"
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
                  marginBottom: "24px",
                }}>
                Fonctionnalité de démonstration. Dans une version complète, ce
                formulaire permettrait d?ajouter une nouvelle transaction à
                votre portefeuille.
              </p>
              <div className="flex gap-3 justify-end">
                <Button variant="ghost" onClick={() => setIsModalOpen(false)}>
                  Annuler
                </Button>
                <Button variant="primary" onClick={() => setIsModalOpen(false)}>
                  Enregistrer
                </Button>
              </div>
            </Card>
          </div>
        </>
      )}

    </div>
  );
}
