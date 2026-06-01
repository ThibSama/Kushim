export type PortfolioCurrency = "EUR" | "USD";

export interface PortfolioAsset {
  id: string;
  name: string;
  ticker: string;
  quantity: number;
  purchasePrice: number;
  currentPrice: number;
  currency: PortfolioCurrency;
}

export interface PortfolioSector {
  name: string;
  defaultCollapsed?: boolean;
  assets: PortfolioAsset[];
}

export interface PortfolioMetrics {
  investedValue: number;
  currentValue: number;
  performance: number;
  performancePct: number;
}

export type PerformanceTone = "positive" | "negative" | "neutral";

export function calculateMetrics(
  quantity: number,
  purchasePrice: number,
  currentPrice: number,
): PortfolioMetrics {
  const investedValue = quantity * purchasePrice;
  const currentValue = quantity * currentPrice;
  const performance = currentValue - investedValue;
  const performancePct =
    investedValue === 0 ? 0 : (performance / investedValue) * 100;

  return {
    investedValue,
    currentValue,
    performance,
    performancePct,
  };
}

export function calculateAssetMetrics(asset: PortfolioAsset): PortfolioMetrics {
  return calculateMetrics(
    asset.quantity,
    asset.purchasePrice,
    asset.currentPrice,
  );
}

export function calculateSectorMetrics(
  assets: PortfolioAsset[],
): PortfolioMetrics {
  const totals = assets.reduce(
    (acc, asset) => {
      const metrics = calculateAssetMetrics(asset);
      acc.investedValue += metrics.investedValue;
      acc.currentValue += metrics.currentValue;
      return acc;
    },
    { investedValue: 0, currentValue: 0 },
  );

  const performance = totals.currentValue - totals.investedValue;
  const performancePct =
    totals.investedValue === 0 ? 0 : (performance / totals.investedValue) * 100;

  return {
    investedValue: totals.investedValue,
    currentValue: totals.currentValue,
    performance,
    performancePct,
  };
}

export function getPerformanceTone(value: number): PerformanceTone {
  if (value > 0) return "positive";
  if (value < 0) return "negative";
  return "neutral";
}

export function formatCurrency(
  value: number,
  currency: PortfolioCurrency = "EUR",
): string {
  return new Intl.NumberFormat("fr-FR", {
    style: "currency",
    currency,
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value);
}

export function formatSignedCurrency(
  value: number,
  currency: PortfolioCurrency = "EUR",
): string {
  const formatted = formatCurrency(Math.abs(value), currency);
  if (value > 0) return `+${formatted}`;
  if (value < 0) return `-${formatted}`;
  return formatted;
}

export function formatSignedPercent(value: number): string {
  const abs = Math.abs(value).toFixed(2);
  if (value > 0) return `+${abs}%`;
  if (value < 0) return `-${abs}%`;
  return `${abs}%`;
}

export function formatQuantity(value: number): string {
  if (Number.isInteger(value)) return value.toString();
  return value.toFixed(4).replace(/\.?0+$/, "");
}
