import { apiRequest } from "./httpClient";

const API_URL = import.meta.env.VITE_API_URL || "http://localhost:8080";

export type BusinessUser = {
  id_user: string;
  public_handle: string;
  role: string;
};

export type Portfolio = {
  id_portfolio: string;
  name: string;
  base_currency: string;
  visibility: "private" | "public" | "unlisted";
  created_at: string;
  updated_at: string;
};

export type CreatePortfolioPayload = {
  name: string;
  base_currency: string;
  visibility?: "private" | "public" | "unlisted";
};

export async function getBusinessMe(
  accessToken: string,
): Promise<BusinessUser> {
  return apiRequest<BusinessUser>(API_URL, "/v1/me", {
    token: accessToken,
  });
}

export async function listPortfolios(
  accessToken: string,
): Promise<Portfolio[]> {
  const res = await apiRequest<{ portfolios: Portfolio[] }>(
    API_URL,
    "/v1/portfolios",
    { token: accessToken },
  );
  return res.portfolios;
}

export async function createPortfolio(
  accessToken: string,
  payload: CreatePortfolioPayload,
): Promise<Portfolio> {
  const res = await apiRequest<{ portfolio: Portfolio }>(
    API_URL,
    "/v1/portfolios",
    { method: "POST", token: accessToken, body: payload },
  );
  return res.portfolio;
}

export async function getPortfolio(
  accessToken: string,
  idPortfolio: string,
): Promise<Portfolio> {
  const res = await apiRequest<{ portfolio: Portfolio }>(
    API_URL,
    `/v1/portfolios/${idPortfolio}`,
    { token: accessToken },
  );
  return res.portfolio;
}

// --- Portfolio read models ---

export type Pagination = {
  limit: number;
  offset: number;
  returned: number;
  has_more: boolean;
};

export type PortfolioSummary = {
  id_portfolio: string;
  base_currency: string;
  total_value_minor: number;
  cash_balance_minor: number;
  total_invested_minor: number;
  total_pnl_minor: number;
  total_pnl_pct: string | null;
  portfolio_status: string;
  is_estimated: boolean;
  as_of: string;
  updated_at: string;
};

export type PortfolioSummaryEnvelope = {
  data_available: boolean;
  summary: PortfolioSummary | null;
  reason: "read_model_missing" | null;
};

export type HoldingAsset = {
  id_asset: string;
  name: string;
  ticker: string | null;
  isin: string | null;
  exchange: string | null;
  asset_class: string;
  status: string;
  native_currency: string | null;
};

export type PortfolioHolding = {
  id_asset: string;
  asset: HoldingAsset;
  base_currency: string;
  quantity: string;
  avg_cost_minor: number | null;
  invested_base_minor: number;
  market_value_minor: number;
  pnl_base_minor: number;
  pnl_pct: string | null;
  weight_pct: string | null;
  position_status: string;
  is_estimated: boolean;
  as_of: string;
  updated_at: string;
};

export type PortfolioHoldingsEnvelope = {
  data_available: boolean;
  holdings: PortfolioHolding[];
  pagination: Pagination;
  reason: "read_model_missing" | null;
};

export type PortfolioHoldingsQuery = {
  limit?: number;
  offset?: number;
  sort?: "weight_desc" | "value_desc" | "name_asc";
  asset_class?: string;
  search?: string;
};

export type PortfolioDailySnapshot = {
  id_portfolio_snapshot_daily: string;
  id_portfolio: string;
  snapshot_date: string;
  base_currency: string;
  cash_balance_minor: number;
  total_value_minor: number;
  total_invested_minor: number;
  total_pnl_minor: number;
  total_pnl_pct: string | null;
  is_estimated: boolean;
  source_type: string;
  created_at: string;
};

export type PortfolioDailySnapshotsEnvelope = {
  data_available: boolean;
  snapshots: PortfolioDailySnapshot[];
  pagination: Pagination;
};

export type PortfolioDailySnapshotsQuery = {
  date_from?: string;
  date_to?: string;
  limit?: number;
  offset?: number;
  sort?: "asc" | "desc";
};

export type PortfolioDailySnapshotHolding = {
  id_portfolio_holding_snapshot_daily: string;
  id_portfolio_snapshot_daily: string;
  id_asset: string;
  asset: HoldingAsset;
  base_currency: string;
  quantity: string;
  avg_cost_minor: number | null;
  invested_minor: number;
  market_value_minor: number;
  pnl_minor: number;
  pnl_pct: string | null;
  weight_pct: string | null;
  is_estimated: boolean;
  created_at: string;
};

export type PortfolioDailySnapshotHoldingsEnvelope = {
  data_available: boolean;
  snapshot: PortfolioDailySnapshot | null;
  holdings: PortfolioDailySnapshotHolding[];
  reason: "snapshot_missing" | null;
  pagination: Pagination;
};

function appendQuery(path: string, query?: Record<string, string | number | undefined>) {
  const params = new URLSearchParams();
  Object.entries(query ?? {}).forEach(([key, value]) => {
    if (value !== undefined) params.set(key, String(value));
  });
  const qs = params.toString();
  return qs ? `${path}?${qs}` : path;
}

export async function getPortfolioSummary(
  accessToken: string,
  portfolioId: string,
): Promise<PortfolioSummaryEnvelope> {
  return apiRequest<PortfolioSummaryEnvelope>(
    API_URL,
    `/v1/portfolios/${portfolioId}/summary`,
    { token: accessToken },
  );
}

export async function getPortfolioHoldings(
  accessToken: string,
  portfolioId: string,
  query?: PortfolioHoldingsQuery,
): Promise<PortfolioHoldingsEnvelope> {
  return apiRequest<PortfolioHoldingsEnvelope>(
    API_URL,
    appendQuery(`/v1/portfolios/${portfolioId}/holdings`, query),
    { token: accessToken },
  );
}

export async function getDailySnapshots(
  accessToken: string,
  portfolioId: string,
  query?: PortfolioDailySnapshotsQuery,
): Promise<PortfolioDailySnapshotsEnvelope> {
  return apiRequest<PortfolioDailySnapshotsEnvelope>(
    API_URL,
    appendQuery(`/v1/portfolios/${portfolioId}/snapshots/daily`, query),
    { token: accessToken },
  );
}

export async function getDailySnapshotHoldings(
  accessToken: string,
  portfolioId: string,
  snapshotDate: string,
  query?: PortfolioHoldingsQuery,
): Promise<PortfolioDailySnapshotHoldingsEnvelope> {
  return apiRequest<PortfolioDailySnapshotHoldingsEnvelope>(
    API_URL,
    appendQuery(
      `/v1/portfolios/${portfolioId}/snapshots/daily/${snapshotDate}/holdings`,
      query,
    ),
    { token: accessToken },
  );
}

// --- Assets ---

export type AssetMetadata = {
  country: string | null;
  website_url: string | null;
  logo_url: string | null;
  description: string | null;
  provider: string | null;
  provider_asset_id: string | null;
  sector: string | null;
  industry: string | null;
  last_synced_at: string | null;
};

export type AssetMarketData = {
  price_minor: number;
  currency: string;
  market_cap_minor: number | null;
  volume_24h_minor: number | null;
  change_24h_pct: string | null;
  change_7d_pct: string | null;
  change_30d_pct: string | null;
  data_source: string | null;
  source_asset_id: string | null;
  as_of: string;
};

export type Asset = {
  id_asset: string;
  name: string;
  ticker: string | null;
  isin: string | null;
  exchange: string | null;
  symbol: string | null;
  network: string | null;
  asset_class: string;
  status: string;
  native_currency: string | null;
  created_at: string;
  updated_at: string;
  metadata: AssetMetadata | null;
  market_data: AssetMarketData | null;
  aliases: { alias_type: string; alias_value: string }[] | null;
};

export type AssetFilters = {
  search?: string;
  asset_class?: string;
  ticker?: string;
  isin?: string;
  exchange?: string;
  status?: string;
  limit?: number;
  offset?: number;
};

export type AssetPagination = {
  limit: number;
  offset: number;
  returned: number;
  has_more: boolean;
};

export async function listAssets(
  accessToken: string,
  filters?: AssetFilters,
): Promise<{ assets: Asset[]; pagination: AssetPagination }> {
  const params = new URLSearchParams();
  if (filters?.search) params.set("search", filters.search);
  if (filters?.asset_class) params.set("asset_class", filters.asset_class);
  if (filters?.ticker) params.set("ticker", filters.ticker);
  if (filters?.isin) params.set("isin", filters.isin);
  if (filters?.exchange) params.set("exchange", filters.exchange);
  if (filters?.status) params.set("status", filters.status);
  if (filters?.limit != null) params.set("limit", String(filters.limit));
  if (filters?.offset != null) params.set("offset", String(filters.offset));
  const qs = params.toString();
  const path = `/v1/assets${qs ? `?${qs}` : ""}`;
  return apiRequest<{ assets: Asset[]; pagination: AssetPagination }>(
    API_URL,
    path,
    { token: accessToken },
  );
}

export async function getAsset(
  accessToken: string,
  assetId: string,
): Promise<Asset> {
  const res = await apiRequest<{ asset: Asset }>(
    API_URL,
    `/v1/assets/${assetId}`,
    { token: accessToken },
  );
  return res.asset;
}

// --- Operations ---

export type PortfolioOperation = {
  id_portfolio_operation: string;
  id_portfolio: string;
  id_asset: string | null;
  id_related_asset: string | null;
  operation_type: string;
  operation_status: string;
  executed_at: string;
  effective_at: string | null;
  quantity: string | null;
  related_quantity: string | null;
  price_minor: number | null;
  gross_amount_minor: number | null;
  fees_minor: number | null;
  taxes_minor: number | null;
  cash_amount_minor: number;
  currency: string;
  fx_rate_to_portfolio: string | null;
  external_provider: string | null;
  external_reference: string | null;
  id_corrected_operation: string | null;
  notes: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type CreateOperationPayload = {
  operation_type: string;
  executed_at: string;
  currency: string;
  id_asset?: string;
  id_related_asset?: string;
  operation_status?: string;
  effective_at?: string;
  quantity?: string;
  related_quantity?: string;
  price_minor?: number;
  gross_amount_minor?: number;
  fees_minor?: number;
  taxes_minor?: number;
  cash_amount_minor?: number;
  fx_rate_to_portfolio?: string;
  external_provider?: string;
  external_reference?: string;
  notes?: string;
  metadata?: Record<string, unknown>;
};

export type OperationFilters = {
  operation_type?: string;
  operation_status?: string;
  id_asset?: string;
};

export type ReferenceItem = {
  value: string;
  label: string;
};

export async function listOperations(
  accessToken: string,
  portfolioId: string,
  filters?: OperationFilters,
): Promise<PortfolioOperation[]> {
  const params = new URLSearchParams();
  if (filters?.operation_type) params.set("operation_type", filters.operation_type);
  if (filters?.operation_status) params.set("operation_status", filters.operation_status);
  if (filters?.id_asset) params.set("id_asset", filters.id_asset);
  const qs = params.toString();
  const path = `/v1/portfolios/${portfolioId}/operations${qs ? `?${qs}` : ""}`;
  const res = await apiRequest<{ operations: PortfolioOperation[] }>(
    API_URL,
    path,
    { token: accessToken },
  );
  return res.operations;
}

export async function createOperation(
  accessToken: string,
  portfolioId: string,
  payload: CreateOperationPayload,
): Promise<PortfolioOperation> {
  const res = await apiRequest<{ operation: PortfolioOperation }>(
    API_URL,
    `/v1/portfolios/${portfolioId}/operations`,
    { method: "POST", token: accessToken, body: payload },
  );
  return res.operation;
}

export async function getOperation(
  accessToken: string,
  portfolioId: string,
  operationId: string,
): Promise<PortfolioOperation> {
  const res = await apiRequest<{ operation: PortfolioOperation }>(
    API_URL,
    `/v1/portfolios/${portfolioId}/operations/${operationId}`,
    { token: accessToken },
  );
  return res.operation;
}

export async function listOperationTypes(
  accessToken: string,
): Promise<ReferenceItem[]> {
  const res = await apiRequest<{ data: ReferenceItem[] }>(
    API_URL,
    "/v1/reference/operation-types",
    { token: accessToken },
  );
  return res.data;
}

export async function listOperationStatuses(
  accessToken: string,
): Promise<ReferenceItem[]> {
  const res = await apiRequest<{ data: ReferenceItem[] }>(
    API_URL,
    "/v1/reference/operation-statuses",
    { token: accessToken },
  );
  return res.data;
}
