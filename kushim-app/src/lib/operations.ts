import { type Asset, type PortfolioOperation, getAsset } from "./api/businessApi";

// Local display cache: maps id_asset -> display label (ticker or name).
// Populated when an asset is selected in the create modal or hydrated on load.
const assetDisplayCache = new Map<string, string>();

export function cacheAssetDisplay(asset: Asset): void {
  assetDisplayCache.set(asset.id_asset, asset.ticker ?? asset.name);
}

export function getAssetDisplay(idAsset: string | null): string {
  if (!idAsset) return "—";
  return assetDisplayCache.get(idAsset) ?? idAsset.slice(0, 8) + "…";
}

export function hydrateAssetDisplayCache(
  operations: PortfolioOperation[],
  accessToken: string,
  onUpdate: () => void,
): void {
  const missing = new Set<string>();
  for (const op of operations) {
    if (op.id_asset && !assetDisplayCache.has(op.id_asset)) {
      missing.add(op.id_asset);
    }
  }
  if (missing.size === 0) return;

  for (const id of missing) {
    getAsset(accessToken, id)
      .then((asset) => {
        assetDisplayCache.set(asset.id_asset, asset.ticker ?? asset.name);
        onUpdate();
      })
      .catch(() => {
        // best-effort: keep truncated UUID fallback
      });
  }
}

const OPERATION_TYPE_LABELS: Record<string, string> = {
  buy: "Achat",
  sell: "Vente",
  deposit: "Dépôt",
  withdrawal: "Retrait",
  dividend: "Dividende",
  interest: "Intérêt",
  fee: "Frais",
  tax: "Taxe",
  split: "Split",
  spin_off: "Spin Off",
  symbol_change: "Changement",
  transfer_in: "Transfert entrant",
  transfer_out: "Transfert sortant",
  adjustment: "Ajustement",
};

const OPERATION_STATUS_LABELS: Record<string, string> = {
  pending: "En attente",
  posted: "Validée",
  cancelled: "Annulée",
};

export function operationTypeLabel(type: string): string {
  return OPERATION_TYPE_LABELS[type] ?? type;
}

export function operationStatusLabel(status: string): string {
  return OPERATION_STATUS_LABELS[status] ?? status;
}

export function minorToMajor(minor: number | null | undefined): number {
  if (minor == null) return 0;
  return minor / 100;
}

function formatQuantity(value: string): string {
  const num = Number(value);
  if (!Number.isFinite(num)) return value;
  return new Intl.NumberFormat("fr-FR", {
    minimumFractionDigits: 0,
    maximumFractionDigits: 10,
  }).format(num);
}

export type TransactionRow = {
  id: string;
  date: string;
  dateRaw: string;
  asset: string;
  type: string;
  typeKey: string;
  statusKey: string;
  status: string;
  qty: string;
  price: number | null;
  fees: number;
  currency: string;
  total: number;
  notes: string;
};

export function operationToRow(op: PortfolioOperation): TransactionRow {
  const date = new Date(op.executed_at);
  const dateStr = Number.isNaN(date.getTime())
    ? op.executed_at
    : date.toLocaleDateString("fr-FR", { year: "numeric", month: "2-digit", day: "2-digit" });

  const asset = getAssetDisplay(op.id_asset);
  const qty = op.quantity != null ? formatQuantity(op.quantity) : "—";
  const price = op.price_minor != null ? minorToMajor(op.price_minor) : null;
  const fees = minorToMajor(op.fees_minor) + minorToMajor(op.taxes_minor);
  const total = minorToMajor(op.cash_amount_minor);

  return {
    id: op.id_portfolio_operation,
    date: dateStr,
    dateRaw: op.executed_at,
    asset,
    type: operationTypeLabel(op.operation_type),
    typeKey: op.operation_type,
    statusKey: op.operation_status,
    status: operationStatusLabel(op.operation_status),
    qty,
    price,
    fees,
    currency: op.currency,
    total,
    notes: op.notes ?? "",
  };
}

const TYPE_BADGE_STYLES: Record<string, { color: string; bg: string }> = {
  buy: { color: "var(--color-gain)", bg: "rgba(16, 185, 129, 0.10)" },
  sell: { color: "var(--color-loss)", bg: "rgba(239, 68, 68, 0.10)" },
  dividend: { color: "#6366F1", bg: "rgba(99, 102, 241, 0.10)" },
  interest: { color: "#6366F1", bg: "rgba(99, 102, 241, 0.10)" },
  deposit: { color: "var(--color-accent)", bg: "rgba(59, 130, 246, 0.10)" },
  withdrawal: { color: "var(--color-warning)", bg: "rgba(245, 158, 11, 0.10)" },
  fee: { color: "var(--text-secondary)", bg: "rgba(161, 161, 170, 0.08)" },
  tax: { color: "var(--text-secondary)", bg: "rgba(161, 161, 170, 0.08)" },
};

const DEFAULT_BADGE = { color: "var(--text-secondary)", bg: "rgba(161, 161, 170, 0.08)" };

export function typeBadgeStyle(typeKey: string): { color: string; bg: string } {
  return TYPE_BADGE_STYLES[typeKey] ?? DEFAULT_BADGE;
}

export const CASH_OPERATION_TYPES = [
  "deposit",
  "withdrawal",
  "fee",
  "tax",
  "interest",
  "transfer_in",
  "transfer_out",
];
