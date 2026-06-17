// Business API client.
//
// All requests go through `authenticatedRequest`, which reads the live access
// token from `tokenStorage`, handles 401 → single-flight refresh → at-most-one
// retry, and clears the local session on terminal auth failures. Callers
// historically passed an `accessToken` as the first argument; that parameter
// is now optional and ignored — the wrapper is the single source of truth for
// the bearer token at request time. Keeping the parameter avoids touching ~40
// existing call sites in this corrective pass.

import { authenticatedRequest } from "./authenticatedRequest";

const API_URL = import.meta.env.VITE_API_URL || "http://localhost:8080";

// Discarded by authenticatedRequest — see file header.
type IgnoredToken = string | null | undefined;

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

export async function getBusinessMe(_token?: IgnoredToken): Promise<BusinessUser> {
  return authenticatedRequest<BusinessUser>(API_URL, "/v1/me");
}

export async function listPortfolios(_token?: IgnoredToken): Promise<Portfolio[]> {
  const res = await authenticatedRequest<{ portfolios: Portfolio[] }>(
    API_URL,
    "/v1/portfolios",
  );
  return res.portfolios;
}

export async function createPortfolio(
  payloadOrToken: CreatePortfolioPayload | IgnoredToken,
  maybePayload?: CreatePortfolioPayload,
): Promise<Portfolio> {
  const payload = (maybePayload ?? (payloadOrToken as CreatePortfolioPayload));
  const res = await authenticatedRequest<{ portfolio: Portfolio }>(
    API_URL,
    "/v1/portfolios",
    { method: "POST", body: payload },
  );
  return res.portfolio;
}

export async function getPortfolio(
  idOrToken: string | IgnoredToken,
  maybeId?: string,
): Promise<Portfolio> {
  const id = (maybeId ?? (idOrToken as string));
  const res = await authenticatedRequest<{ portfolio: Portfolio }>(
    API_URL,
    `/v1/portfolios/${id}`,
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
  idOrToken: string | IgnoredToken,
  maybeId?: string,
): Promise<PortfolioSummaryEnvelope> {
  const portfolioId = (maybeId ?? (idOrToken as string));
  return authenticatedRequest<PortfolioSummaryEnvelope>(
    API_URL,
    `/v1/portfolios/${portfolioId}/summary`,
  );
}

export async function getPortfolioHoldings(
  idOrToken: string | IgnoredToken,
  maybeIdOrQuery?: string | PortfolioHoldingsQuery,
  maybeQuery?: PortfolioHoldingsQuery,
): Promise<PortfolioHoldingsEnvelope> {
  // Signatures supported:
  //   getPortfolioHoldings(portfolioId, query?)
  //   getPortfolioHoldings(accessToken, portfolioId, query?) — legacy
  const portfolioId =
    typeof maybeIdOrQuery === "string" ? maybeIdOrQuery : (idOrToken as string);
  const query =
    typeof maybeIdOrQuery === "string"
      ? maybeQuery
      : (maybeIdOrQuery as PortfolioHoldingsQuery | undefined);
  return authenticatedRequest<PortfolioHoldingsEnvelope>(
    API_URL,
    appendQuery(`/v1/portfolios/${portfolioId}/holdings`, query),
  );
}

export async function getDailySnapshots(
  idOrToken: string | IgnoredToken,
  maybeIdOrQuery?: string | PortfolioDailySnapshotsQuery,
  maybeQuery?: PortfolioDailySnapshotsQuery,
): Promise<PortfolioDailySnapshotsEnvelope> {
  const portfolioId =
    typeof maybeIdOrQuery === "string" ? maybeIdOrQuery : (idOrToken as string);
  const query =
    typeof maybeIdOrQuery === "string"
      ? maybeQuery
      : (maybeIdOrQuery as PortfolioDailySnapshotsQuery | undefined);
  return authenticatedRequest<PortfolioDailySnapshotsEnvelope>(
    API_URL,
    appendQuery(`/v1/portfolios/${portfolioId}/snapshots/daily`, query),
  );
}

export async function getDailySnapshotHoldings(
  idOrToken: string | IgnoredToken,
  maybeIdOrDate?: string,
  maybeDateOrQuery?: string | PortfolioHoldingsQuery,
  maybeQuery?: PortfolioHoldingsQuery,
): Promise<PortfolioDailySnapshotHoldingsEnvelope> {
  // Supported:
  //   getDailySnapshotHoldings(portfolioId, snapshotDate, query?)
  //   getDailySnapshotHoldings(accessToken, portfolioId, snapshotDate, query?)
  let portfolioId: string;
  let snapshotDate: string;
  let query: PortfolioHoldingsQuery | undefined;
  if (
    typeof maybeIdOrDate === "string" &&
    typeof maybeDateOrQuery === "string"
  ) {
    portfolioId = maybeIdOrDate;
    snapshotDate = maybeDateOrQuery;
    query = maybeQuery;
  } else {
    portfolioId = idOrToken as string;
    snapshotDate = maybeIdOrDate as string;
    query = maybeDateOrQuery as PortfolioHoldingsQuery | undefined;
  }
  return authenticatedRequest<PortfolioDailySnapshotHoldingsEnvelope>(
    API_URL,
    appendQuery(
      `/v1/portfolios/${portfolioId}/snapshots/daily/${snapshotDate}/holdings`,
      query,
    ),
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
  filtersOrToken?: AssetFilters | IgnoredToken,
  maybeFilters?: AssetFilters,
): Promise<{ assets: Asset[]; pagination: AssetPagination }> {
  const filters: AssetFilters | undefined =
    maybeFilters ??
    (typeof filtersOrToken === "object" && filtersOrToken !== null
      ? (filtersOrToken as AssetFilters)
      : undefined);
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
  return authenticatedRequest<{ assets: Asset[]; pagination: AssetPagination }>(
    API_URL,
    path,
  );
}

export async function getAsset(
  idOrToken: string | IgnoredToken,
  maybeId?: string,
): Promise<Asset> {
  const assetId = (maybeId ?? (idOrToken as string));
  const res = await authenticatedRequest<{ asset: Asset }>(
    API_URL,
    `/v1/assets/${assetId}`,
  );
  return res.asset;
}

// --- Operations ---

/// Compact asset identity returned alongside an operation so the Transactions
/// UI can render the asset column without a separate `GET /v1/assets/{id}`
/// round trip. `null` for cash-only operations and for operations whose asset
/// id could not be resolved (legacy/corrupt data) — the UI falls back to a
/// safe placeholder rather than crashing.
export type OperationAssetRef = {
  id_asset: string;
  name: string;
  ticker: string | null;
  status: string;
};

export type PortfolioOperation = {
  id_portfolio_operation: string;
  id_portfolio: string;
  id_asset: string | null;
  id_related_asset: string | null;
  asset: OperationAssetRef | null;
  related_asset: OperationAssetRef | null;
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

export type RefreshRequestRef = {
  id_portfolio_refresh_request: string;
  status: string;
  requested_at: string;
};

export type CreateOperationResult = {
  operation: PortfolioOperation;
  refresh_request: RefreshRequestRef | null;
};

export type RefreshRequestPublicStatus =
  | "pending"
  | "processing"
  | "completed"
  | "failed";

export type RefreshRequestStatusView = {
  id_portfolio_refresh_request: string;
  id_portfolio: string;
  status: RefreshRequestPublicStatus;
  attempts: number;
  requested_at: string;
  processing_started_at: string | null;
  completed_at: string | null;
  updated_at: string;
  error_code: string | null;
};

export type ReferenceItem = {
  value: string;
  label: string;
};

export async function listOperations(
  idOrToken: string | IgnoredToken,
  maybeIdOrFilters?: string | OperationFilters,
  maybeFilters?: OperationFilters,
): Promise<PortfolioOperation[]> {
  const portfolioId =
    typeof maybeIdOrFilters === "string"
      ? maybeIdOrFilters
      : (idOrToken as string);
  const filters: OperationFilters | undefined =
    typeof maybeIdOrFilters === "string"
      ? maybeFilters
      : (maybeIdOrFilters as OperationFilters | undefined);
  const params = new URLSearchParams();
  if (filters?.operation_type) params.set("operation_type", filters.operation_type);
  if (filters?.operation_status) params.set("operation_status", filters.operation_status);
  if (filters?.id_asset) params.set("id_asset", filters.id_asset);
  const qs = params.toString();
  const path = `/v1/portfolios/${portfolioId}/operations${qs ? `?${qs}` : ""}`;
  const res = await authenticatedRequest<{ operations: PortfolioOperation[] }>(
    API_URL,
    path,
  );
  return res.operations;
}

export async function createOperation(
  idOrToken: string | IgnoredToken,
  maybeIdOrPayload: string | CreateOperationPayload,
  maybePayloadOrKey?: CreateOperationPayload | string,
  maybeIdempotencyKey?: string,
): Promise<CreateOperationResult> {
  // Supported call shapes:
  //   createOperation(portfolioId, payload, idempotencyKey)
  //   createOperation(accessToken, portfolioId, payload, idempotencyKey) — legacy
  // The idempotency key is REQUIRED by the backend P3 contract; we never
  // generate it here so the UI layer keeps the single source of truth for
  // the key lifecycle (one logical submission attempt = one UUID).
  const portfolioId =
    typeof maybeIdOrPayload === "string"
      ? maybeIdOrPayload
      : (idOrToken as string);
  const payload =
    typeof maybeIdOrPayload === "string"
      ? (maybePayloadOrKey as CreateOperationPayload)
      : (maybeIdOrPayload as CreateOperationPayload);
  const idempotencyKey =
    typeof maybeIdOrPayload === "string"
      ? maybeIdempotencyKey
      : (maybePayloadOrKey as string | undefined);
  if (!idempotencyKey) {
    throw new Error(
      "createOperation requires an idempotency key (P3 contract)",
    );
  }
  const res = await authenticatedRequest<{
    operation: PortfolioOperation;
    refresh_request: RefreshRequestRef | null;
  }>(API_URL, `/v1/portfolios/${portfolioId}/operations`, {
    method: "POST",
    body: payload,
    headers: { "Idempotency-Key": idempotencyKey },
  });
  return {
    operation: res.operation,
    refresh_request: res.refresh_request ?? null,
  };
}

export async function getRefreshRequest(
  idOrToken: string | IgnoredToken,
  maybeIdOrRequest: string,
  maybeRequest?: string,
): Promise<RefreshRequestStatusView> {
  const portfolioId = maybeRequest !== undefined
    ? maybeIdOrRequest
    : (idOrToken as string);
  const refreshRequestId = maybeRequest ?? maybeIdOrRequest;
  const res = await authenticatedRequest<{
    refresh_request: RefreshRequestStatusView;
  }>(
    API_URL,
    `/v1/portfolios/${portfolioId}/refresh-requests/${refreshRequestId}`,
  );
  return res.refresh_request;
}

export async function getOperation(
  idOrToken: string | IgnoredToken,
  maybeIdOrOp: string,
  maybeOp?: string,
): Promise<PortfolioOperation> {
  const portfolioId =
    maybeOp !== undefined ? maybeIdOrOp : (idOrToken as string);
  const operationId = maybeOp ?? maybeIdOrOp;
  const res = await authenticatedRequest<{ operation: PortfolioOperation }>(
    API_URL,
    `/v1/portfolios/${portfolioId}/operations/${operationId}`,
  );
  return res.operation;
}

export async function listOperationTypes(_token?: IgnoredToken): Promise<ReferenceItem[]> {
  const res = await authenticatedRequest<{ data: ReferenceItem[] }>(
    API_URL,
    "/v1/reference/operation-types",
  );
  return res.data;
}

export async function listOperationStatuses(_token?: IgnoredToken): Promise<ReferenceItem[]> {
  const res = await authenticatedRequest<{ data: ReferenceItem[] }>(
    API_URL,
    "/v1/reference/operation-statuses",
  );
  return res.data;
}

/// Canonical currency catalogue used to populate the CurrencySelect component.
/// Backed by the backend `GET /v1/reference/currencies` endpoint, which is the
/// single source of truth shared with backend validation.
export async function listCurrencies(_token?: IgnoredToken): Promise<ReferenceItem[]> {
  const res = await authenticatedRequest<{ data: ReferenceItem[] }>(
    API_URL,
    "/v1/reference/currencies",
  );
  return res.data;
}
