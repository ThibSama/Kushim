import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Mock the entire business API surface used by the operations store. Each
// fn is a spy so the regression tests can prove `getAsset` is never invoked,
// regardless of how many operations or how many distinct asset ids the list
// contains.
vi.mock("../lib/api/businessApi", () => ({
  listOperations: vi.fn(),
  createOperation: vi.fn(),
  listOperationTypes: vi.fn().mockResolvedValue([]),
  listOperationStatuses: vi.fn().mockResolvedValue([]),
  getAsset: vi.fn(),
}));

import * as businessApi from "../lib/api/businessApi";
import type { PortfolioOperation } from "../lib/api/businessApi";
import { useAuthStore } from "./auth";
import { useOperationsStore } from "./operations";

const buildOp = (
  id: string,
  asset: PortfolioOperation["asset"],
): PortfolioOperation => ({
  id_portfolio_operation: id,
  id_portfolio: "portfolio-1",
  id_asset: asset?.id_asset ?? null,
  id_related_asset: null,
  asset,
  related_asset: null,
  operation_type: asset ? "buy" : "deposit",
  operation_status: "posted",
  executed_at: "2026-06-05T10:00:00Z",
  effective_at: null,
  quantity: asset ? "1" : null,
  related_quantity: null,
  price_minor: asset ? 1234 : null,
  gross_amount_minor: 1234,
  fees_minor: null,
  taxes_minor: null,
  cash_amount_minor: 1234,
  currency: "EUR",
  fx_rate_to_portfolio: null,
  external_provider: null,
  external_reference: null,
  id_corrected_operation: null,
  notes: null,
  metadata: {},
  created_at: "2026-06-05T10:00:00Z",
  updated_at: "2026-06-05T10:00:00Z",
});

const aaplAsset: PortfolioOperation["asset"] = {
  id_asset: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
  name: "Apple Inc.",
  ticker: "AAPL",
  status: "active",
};

const msftAsset: PortfolioOperation["asset"] = {
  id_asset: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
  name: "Microsoft",
  ticker: "MSFT",
  status: "active",
};

describe("useOperationsStore (P2)", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "fake-token", user: null });
    useOperationsStore.getState().reset();
    vi.clearAllMocks();
  });

  afterEach(() => {
    useOperationsStore.getState().reset();
  });

  it("loadOperations never invokes getAsset, even with several asset-linked rows", async () => {
    vi.mocked(businessApi.listOperations).mockResolvedValue([
      buildOp("op-cash", null),
      buildOp("op-aapl-1", aaplAsset),
      buildOp("op-aapl-2", aaplAsset),
      buildOp("op-msft", msftAsset),
    ]);

    await useOperationsStore.getState().loadOperations("portfolio-1");

    expect(businessApi.listOperations).toHaveBeenCalledTimes(1);
    expect(businessApi.getAsset).not.toHaveBeenCalled();

    const ops = useOperationsStore.getState().operations;
    expect(ops).toHaveLength(4);
    // Embedded identity survives the store insertion.
    expect(ops[1].asset?.ticker).toBe("AAPL");
    expect(ops[3].asset?.ticker).toBe("MSFT");
  });

  it("reloadOperations never invokes getAsset (background reload preserves identity)", async () => {
    vi.mocked(businessApi.listOperations).mockResolvedValue([
      buildOp("op-aapl-1", aaplAsset),
      buildOp("op-aapl-2", aaplAsset),
    ]);

    await useOperationsStore.getState().reloadOperations("portfolio-1");

    expect(businessApi.getAsset).not.toHaveBeenCalled();
    expect(useOperationsStore.getState().operations[0].asset?.ticker).toBe("AAPL");
  });

  it("createOperation inserts the returned operation with its embedded asset identity", async () => {
    const created = buildOp("op-new", aaplAsset);
    vi.mocked(businessApi.createOperation).mockResolvedValue({
      operation: created,
      refresh_request: {
        id_portfolio_refresh_request: "rr-1",
        status: "pending",
        requested_at: "2026-06-05T10:00:00Z",
      },
    });

    const result = await useOperationsStore.getState().createOperation(
      "portfolio-1",
      {
        operation_type: "buy",
        executed_at: "2026-06-05T10:00:00Z",
        currency: "EUR",
        id_asset: aaplAsset.id_asset,
      },
      "00000000-0000-4000-8000-000000000001",
    );

    expect(result.operation.asset?.ticker).toBe("AAPL");
    expect(useOperationsStore.getState().operations[0].asset?.ticker).toBe("AAPL");
    expect(businessApi.getAsset).not.toHaveBeenCalled();
  });

  it("a full reset followed by reload does not trigger any asset-detail fetch", async () => {
    vi.mocked(businessApi.listOperations).mockResolvedValue([
      buildOp("op-aapl", aaplAsset),
      buildOp("op-msft", msftAsset),
    ]);

    await useOperationsStore.getState().loadOperations("portfolio-1");
    useOperationsStore.getState().reset();
    await useOperationsStore.getState().loadOperations("portfolio-1");

    // Two list calls (initial + post-reset), zero asset-detail calls.
    expect(businessApi.listOperations).toHaveBeenCalledTimes(2);
    expect(businessApi.getAsset).not.toHaveBeenCalled();
  });
});
