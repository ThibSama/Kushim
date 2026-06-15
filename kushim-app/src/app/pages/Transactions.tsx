import React, { useEffect, useState } from "react";
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
  AlertCircle,
  ListX,
} from "lucide-react";
import { Card } from "../components/Card";
import { Button } from "../components/Button";
import { CreateOperationModal } from "../components/CreateOperationModal";
import { RefreshNotice } from "../components/RefreshNotice";
import { formatCurrency } from "../../utils/portfolio";
import { usePortfolioStore } from "../../stores/portfolio";
import { useOperationsStore } from "../../stores/operations";
import {
  operationToRow,
  typeBadgeStyle,
} from "../../lib/operations";
import type { TransactionRow } from "../../lib/operations";

const ROWS_PER_PAGE = 15;

const typeFilterOptions = [
  { value: "", label: "Tous les types" },
  { value: "buy", label: "Achat" },
  { value: "sell", label: "Vente" },
  { value: "dividend", label: "Dividende" },
  { value: "deposit", label: "Dépôt" },
  { value: "withdrawal", label: "Retrait" },
  { value: "fee", label: "Frais" },
  { value: "tax", label: "Taxe" },
  { value: "interest", label: "Intérêt" },
];

const statusFilterOptions = [
  { value: "", label: "Tous les statuts" },
  { value: "pending", label: "En attente" },
  { value: "posted", label: "Validée" },
  { value: "cancelled", label: "Annulée" },
];

const periodOptions = [
  "Toutes les périodes",
  "30 derniers jours",
  "90 derniers jours",
  "12 derniers mois",
];

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

function EmptyOperationsState({ onCreate }: { onCreate: () => void }) {
  return (
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
          <ListX size={32} style={{ color: "var(--color-accent)" }} />
        </div>
        <h2
          style={{
            fontSize: "20px",
            fontWeight: 600,
            color: "var(--text-primary)",
            marginBottom: "8px",
          }}>
          Aucune opération
        </h2>
        <p
          style={{
            fontSize: "14px",
            color: "var(--text-secondary)",
            maxWidth: "420px",
            marginBottom: "24px",
            lineHeight: "1.6",
          }}>
          Ce portefeuille n'a pas encore d'opérations. Commencez par enregistrer un dépôt ou une transaction.
        </p>
        <Button variant="primary" icon={Plus} onClick={onCreate}>
          Ajouter une opération
        </Button>
      </div>
    </Card>
  );
}

export function Transactions() {
  const { activePortfolioId } = usePortfolioStore();
  const { operations, status, loadOperations } = useOperationsStore();

  const [isModalOpen, setIsModalOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [periodFilter, setPeriodFilter] = useState("Toutes les périodes");
  const [typeFilter, setTypeFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const [page, setPage] = useState(1);

  useEffect(() => {
    if (activePortfolioId) {
      loadOperations(activePortfolioId);
    }
  }, [activePortfolioId, loadOperations]);

  const rows: TransactionRow[] = operations.map(operationToRow);

  const latestTime = rows.reduce((latest, r) => {
    const t = new Date(r.dateRaw).getTime();
    return Number.isNaN(t) ? latest : Math.max(latest, t);
  }, 0);

  const filtered = rows.filter((r) => {
    if (search) {
      const q = search.toLowerCase();
      if (!r.asset.toLowerCase().includes(q) && !r.notes.toLowerCase().includes(q) && !r.type.toLowerCase().includes(q))
        return false;
    }
    if (typeFilter && r.typeKey !== typeFilter) return false;
    if (statusFilter && r.statusKey !== statusFilter) return false;
    if (periodFilter !== "Toutes les périodes") {
      const t = new Date(r.dateRaw).getTime();
      if (Number.isNaN(t)) return false;
      const dayMs = 86400000;
      const age = (latestTime - t) / dayMs;
      if (periodFilter === "30 derniers jours" && age > 30) return false;
      if (periodFilter === "90 derniers jours" && age > 90) return false;
      if (periodFilter === "12 derniers mois" && age > 365) return false;
    }
    return true;
  });

  const totalPages = Math.max(1, Math.ceil(filtered.length / ROWS_PER_PAGE));
  const safePage = Math.min(page, totalPages);
  const pageStart = (safePage - 1) * ROWS_PER_PAGE;
  const visible = filtered.slice(pageStart, pageStart + ROWS_PER_PAGE);

  const metrics = filtered.reduce(
    (acc, r) => {
      if (r.typeKey === "buy") acc.purchases += r.total;
      if (r.typeKey === "sell") acc.sales += r.total;
      if (r.typeKey === "dividend") acc.dividends += r.total;
      if (r.typeKey === "deposit") acc.deposits += r.total;
      if (r.typeKey === "withdrawal") acc.withdrawals += r.total;
      acc.fees += r.fees;
      return acc;
    },
    { purchases: 0, sales: 0, deposits: 0, withdrawals: 0, dividends: 0, fees: 0 },
  );

  const metricCards = [
    { key: "purchases", label: "Achats", value: formatCurrency(metrics.purchases), icon: ShoppingCart, iconColor: "var(--color-gain)", accent: "rgba(16, 185, 129, 0.14)", mdSpan: "md:col-span-1", xlSpan: "xl:col-span-2", minHeight: "104px" },
    { key: "sales", label: "Ventes", value: formatCurrency(metrics.sales), icon: TrendingUp, iconColor: "var(--color-loss)", accent: "rgba(239, 68, 68, 0.12)", mdSpan: "md:col-span-1", xlSpan: "xl:col-span-1", minHeight: "104px" },
    { key: "deposits", label: "Dépôts", value: formatCurrency(metrics.deposits), icon: ArrowDownCircle, iconColor: "var(--color-accent)", accent: "rgba(59, 130, 246, 0.12)", mdSpan: "md:col-span-1", xlSpan: "xl:col-span-1", minHeight: "104px" },
    { key: "withdrawals", label: "Retraits", value: formatCurrency(metrics.withdrawals), icon: ArrowUpCircle, iconColor: "var(--color-warning)", accent: "rgba(245, 158, 11, 0.12)", mdSpan: "md:col-span-1", xlSpan: "xl:col-span-2", minHeight: "88px" },
    { key: "dividends", label: "Dividendes", value: formatCurrency(metrics.dividends), icon: Landmark, iconColor: "#6366F1", accent: "rgba(99, 102, 241, 0.12)", mdSpan: "md:col-span-1", xlSpan: "xl:col-span-1", minHeight: "88px" },
    { key: "fees", label: "Frais", value: formatCurrency(metrics.fees), icon: Receipt, iconColor: "var(--text-secondary)", accent: "rgba(161, 161, 170, 0.10)", mdSpan: "md:col-span-1", xlSpan: "xl:col-span-1", minHeight: "88px" },
  ];

  if (!activePortfolioId) {
    return (
      <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
        <h1 style={{ fontSize: "clamp(24px, 5vw, 30px)", fontWeight: 700, color: "var(--text-primary)" }}>
          Transactions
        </h1>
        <p style={{ fontSize: "14px", color: "var(--text-secondary)", marginTop: "12px" }}>
          Sélectionnez un portefeuille pour afficher les opérations.
        </p>
      </div>
    );
  }

  if (status === "loading" || status === "idle") {
    return (
      <div
        className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12"
        style={{ display: "flex", alignItems: "center", justifyContent: "center", minHeight: "40vh", color: "var(--text-secondary)", fontSize: "15px" }}>
        Chargement des opérations…
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
        <h1 style={{ fontSize: "clamp(24px, 5vw, 30px)", fontWeight: 700, color: "var(--text-primary)" }}>Transactions</h1>
        <Card level={1} className="mt-6">
          <div className="flex items-start gap-3" style={{ padding: "24px", color: "var(--color-loss)" }}>
            <AlertCircle size={20} />
            <div>
              <p style={{ fontWeight: 600 }}>Erreur de chargement</p>
              <p style={{ fontSize: "14px", color: "var(--text-secondary)", marginTop: "4px" }}>
                {useOperationsStore.getState().error}
              </p>
            </div>
          </div>
        </Card>
      </div>
    );
  }

  return (
    <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
      {/* Header */}
      <div className="mb-6 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 style={{ fontSize: "clamp(24px, 5vw, 30px)", fontWeight: 700, color: "var(--text-primary)" }}>
            Transactions
          </h1>
          <p style={{ fontSize: "clamp(13px, 2.5vw, 14px)", color: "var(--text-secondary)", marginTop: "4px" }}>
            Historique des opérations du portefeuille
          </p>
        </div>
        <button
          onClick={() => setIsModalOpen(true)}
          className="w-full sm:w-auto"
          style={{
            display: "flex", alignItems: "center", justifyContent: "center", gap: "6px",
            height: "44px", padding: "0 16px", borderRadius: "var(--radius-md)", border: "none",
            background: "var(--color-cta-bg)", fontSize: "14px", fontWeight: 600,
            color: "var(--color-cta-text)", cursor: "pointer", flexShrink: 0,
          }}>
          <Plus size={16} />
          Ajouter une opération
        </button>
      </div>

      <RefreshNotice />

      {operations.length === 0 ? (
        <EmptyOperationsState onCreate={() => setIsModalOpen(true)} />
      ) : (
        <>
          {/* Controls */}
          <div className="flex flex-col lg:flex-row lg:flex-wrap items-stretch lg:items-center gap-3" style={{ marginTop: "24px" }}>
            <div className="relative w-full sm:max-w-[320px]">
              <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2" style={{ color: "var(--text-tertiary)" }} />
              <input
                type="text" value={search} onChange={(e) => { setSearch(e.target.value); setPage(1); }}
                placeholder="Rechercher par type, note..."
                className="w-full"
                style={{ background: "var(--surface-2-bg)", border: "1px solid var(--surface-2-border)", borderRadius: "var(--radius-md)", minHeight: "44px", height: "44px", paddingLeft: "36px", paddingRight: "16px", fontSize: "14px", color: "var(--text-primary)" }}
              />
            </div>

            <div className="relative min-w-[180px] flex-shrink-0">
              <select value={periodFilter} onChange={(e) => { setPeriodFilter(e.target.value); setPage(1); }} className="w-full appearance-none cursor-pointer" style={{ ...ghostBtnStyle, width: "100%", paddingLeft: "36px", paddingRight: "32px" }}>
                {periodOptions.map((o) => (<option key={o} value={o}>{o}</option>))}
              </select>
              <Calendar size={16} className="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" style={{ color: "var(--text-tertiary)" }} />
              <ChevronDown size={14} className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none" style={{ color: "var(--text-tertiary)" }} />
            </div>

            <div className="relative min-w-[170px] flex-shrink-0">
              <select value={typeFilter} onChange={(e) => { setTypeFilter(e.target.value); setPage(1); }} className="w-full appearance-none cursor-pointer" style={{ ...ghostBtnStyle, width: "100%", paddingRight: "32px" }}>
                {typeFilterOptions.map((o) => (<option key={o.value} value={o.value}>{o.label}</option>))}
              </select>
              <ChevronDown size={14} className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none" style={{ color: "var(--text-tertiary)" }} />
            </div>

            <div className="relative min-w-[170px] flex-shrink-0">
              <select value={statusFilter} onChange={(e) => { setStatusFilter(e.target.value); setPage(1); }} className="w-full appearance-none cursor-pointer" style={{ ...ghostBtnStyle, width: "100%", paddingRight: "32px" }}>
                {statusFilterOptions.map((o) => (<option key={o.value} value={o.value}>{o.label}</option>))}
              </select>
              <ChevronDown size={14} className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none" style={{ color: "var(--text-tertiary)" }} />
            </div>
          </div>

          {/* Metrics */}
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3 sm:gap-4" style={{ marginTop: "24px", marginBottom: "32px" }}>
            {metricCards.map((m) => {
              const Icon = m.icon;
              return (
                <div key={m.key} className={`glass glass-hover ${m.mdSpan} ${m.xlSpan}`} style={{ borderRadius: "var(--radius-xl)", padding: "clamp(14px, 2vw, 16px)", minHeight: m.minHeight, display: "flex", flexDirection: "column", justifyContent: "flex-start", gap: "8px", position: "relative", overflow: "hidden" }}>
                  <div aria-hidden="true" style={{ position: "absolute", inset: "-18% auto auto -4%", width: "88px", height: "88px", borderRadius: "9999px", background: m.accent, filter: "blur(16px)", opacity: 0.75 }} />
                  <div className="relative flex items-center gap-3" style={{ zIndex: 1 }}>
                    <div className="rounded-[14px] flex items-center justify-center" style={{ width: "34px", height: "34px", background: "var(--surface-2-bg)", border: "1px solid var(--surface-2-border)", color: m.iconColor, flexShrink: 0 }}>
                      <Icon size={16} />
                    </div>
                    <div style={{ fontSize: "11px", letterSpacing: "0.05em", textTransform: "uppercase", color: "var(--text-tertiary)", fontWeight: 600, lineHeight: 1.2 }}>{m.label}</div>
                  </div>
                  <div className="relative" style={{ zIndex: 1 }}>
                    <div style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: "clamp(18px, 1.8vw, 22px)", lineHeight: 1.1, fontWeight: 700, fontVariantNumeric: "tabular-nums", color: "var(--text-primary)" }}>{m.value}</div>
                  </div>
                </div>
              );
            })}
          </div>

          {/* Table */}
          <Card level={1} className="overflow-hidden mt-8" noPadding>
            <div style={{ overflowX: "auto", WebkitOverflowScrolling: "touch" }}>
              <table style={{ width: "100%", borderCollapse: "collapse", minWidth: "800px" }}>
                <thead>
                  <tr>
                    <th style={{ ...thStyle, textAlign: "left", minWidth: "100px" }}>Date</th>
                    <th style={{ ...thStyle, textAlign: "left", minWidth: "80px" }}>Actif</th>
                    <th style={{ ...thStyle, textAlign: "left", minWidth: "90px" }}>Type</th>
                    <th style={{ ...thStyle, textAlign: "left", minWidth: "80px" }}>Statut</th>
                    <th style={{ ...thStyle, textAlign: "right", minWidth: "90px" }}>Quantité</th>
                    <th style={{ ...thStyle, textAlign: "right", minWidth: "100px" }}>Prix</th>
                    <th style={{ ...thStyle, textAlign: "right", minWidth: "80px" }} className="hidden sm:table-cell">Frais</th>
                    <th style={{ ...thStyle, textAlign: "center", minWidth: "70px" }} className="hidden sm:table-cell">Devise</th>
                    <th style={{ ...thStyle, textAlign: "right", minWidth: "110px" }}>Montant</th>
                    <th style={{ ...thStyle, textAlign: "left", minWidth: "180px" }} className="hidden lg:table-cell">Note</th>
                  </tr>
                </thead>
                <tbody>
                  {visible.map((r, i) => {
                    const badge = typeBadgeStyle(r.typeKey);
                    return (
                      <tr
                        key={r.id}
                        className="transition-colors"
                        onMouseEnter={(e) => { e.currentTarget.style.background = "var(--surface-2-bg)"; }}
                        onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
                        style={{ borderBottom: i < visible.length - 1 ? "1px solid var(--surface-1-border)" : "none" }}>
                        <td style={{ padding: "14px 16px", fontSize: "14px", color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{r.date}</td>
                        <td style={{ padding: "14px 16px", fontSize: "13px", fontWeight: 500, color: r.asset === "—" ? "var(--text-tertiary)" : "var(--text-primary)", whiteSpace: "nowrap" }}>{r.asset}</td>
                        <td style={{ padding: "14px 16px" }}>
                          <span style={{ display: "inline-block", padding: "4px 12px", borderRadius: "var(--radius-md)", fontSize: "11px", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.03em", color: badge.color, background: badge.bg }}>{r.type}</span>
                        </td>
                        <td style={{ padding: "14px 16px", fontSize: "12px", color: r.statusKey === "cancelled" ? "var(--color-loss)" : "var(--text-tertiary)" }}>{r.status}</td>
                        <td style={{ ...monoCell, textAlign: "right" }}>{r.qty}</td>
                        <td style={{ ...monoCell, textAlign: "right" }}>{r.price === null ? "—" : formatCurrency(r.price)}</td>
                        <td style={{ ...monoCell, textAlign: "right", color: "var(--text-secondary)" }} className="hidden sm:table-cell">{r.fees > 0 ? formatCurrency(r.fees) : "—"}</td>
                        <td style={{ padding: "14px 16px", textAlign: "center", fontSize: "12px", color: "var(--text-tertiary)", whiteSpace: "nowrap" }} className="hidden sm:table-cell">{r.currency}</td>
                        <td style={{ ...monoCell, textAlign: "right", fontWeight: 600 }}>{formatCurrency(r.total)}</td>
                        <td style={{ padding: "14px 16px", fontSize: "12px", color: "var(--text-tertiary)", minWidth: "180px" }} className="hidden lg:table-cell">{r.notes || "—"}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>

            {/* Pagination */}
            <div className="flex flex-wrap items-center justify-between gap-4" style={{ padding: "16px", borderTop: "1px solid var(--surface-1-border)" }}>
              <span style={{ fontSize: "14px", color: "var(--text-tertiary)" }}>
                {filtered.length === 0
                  ? "Aucun résultat"
                  : `Affichage ${pageStart + 1}-${Math.min(pageStart + ROWS_PER_PAGE, filtered.length)} sur ${filtered.length} opérations`}
              </span>
              {totalPages > 1 && (
                <div className="flex items-center gap-2">
                  <button disabled={safePage === 1} onClick={() => setPage(safePage - 1)} style={{ ...ghostBtnStyle, height: "36px", padding: "0 12px", opacity: safePage === 1 ? 0.4 : 1, cursor: safePage === 1 ? "not-allowed" : "pointer" }}>← Précédent</button>
                  <span style={{ fontSize: "14px", color: "var(--text-secondary)" }}>{safePage} / {totalPages}</span>
                  <button disabled={safePage === totalPages} onClick={() => setPage(safePage + 1)} style={{ ...ghostBtnStyle, height: "36px", padding: "0 12px", opacity: safePage === totalPages ? 0.4 : 1, cursor: safePage === totalPages ? "not-allowed" : "pointer" }}>Suivant →</button>
                </div>
              )}
            </div>
          </Card>
        </>
      )}

      {isModalOpen && activePortfolioId && (
        <CreateOperationModal portfolioId={activePortfolioId} onClose={() => setIsModalOpen(false)} />
      )}
    </div>
  );
}
