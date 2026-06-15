import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";

// Same business API surface mock as the store regression test. The crucial
// assertion is that rendering Transactions consumes the embedded asset
// identity and never hits `getAsset`, even after a simulated reload.
vi.mock("../../lib/api/businessApi", () => ({
  listOperations: vi.fn(),
  createOperation: vi.fn(),
  listOperationTypes: vi.fn().mockResolvedValue([]),
  listOperationStatuses: vi.fn().mockResolvedValue([]),
  getAsset: vi.fn(),
}));

import * as businessApi from "../../lib/api/businessApi";
import type { PortfolioOperation } from "../../lib/api/businessApi";
import { useAuthStore } from "../../stores/auth";
import { useOperationsStore } from "../../stores/operations";
import { usePortfolioStore } from "../../stores/portfolio";
import { Transactions } from "./Transactions";

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

const buildOp = (
  id: string,
  asset: PortfolioOperation["asset"],
  overrides: Partial<PortfolioOperation> = {},
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
  ...overrides,
});

describe("Transactions page (P2)", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "fake-token", user: null });
    usePortfolioStore.setState({ activePortfolioId: "portfolio-1" } as never);
    useOperationsStore.getState().reset();
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
    useOperationsStore.getState().reset();
  });

  it("renders the embedded ticker for every operation after loading", async () => {
    vi.mocked(businessApi.listOperations).mockResolvedValue([
      buildOp("op-aapl", aaplAsset),
      buildOp("op-aapl-2", aaplAsset),
      buildOp("op-msft", msftAsset),
      buildOp("op-cash", null),
    ]);

    await useOperationsStore.getState().loadOperations("portfolio-1");
    render(<Transactions />);

    // Two AAPL rows, one MSFT row.
    expect(await screen.findAllByText("AAPL")).toHaveLength(2);
    expect(screen.getByText("MSFT")).toBeDefined();
    // Cash row's asset cell renders "—".
    expect(screen.getAllByText("—").length).toBeGreaterThan(0);
    // No asset-detail HTTP call.
    expect(businessApi.getAsset).not.toHaveBeenCalled();
  });

  it("a simulated full store reload preserves the same labels with no extra fetch", async () => {
    vi.mocked(businessApi.listOperations).mockResolvedValue([
      buildOp("op-aapl", aaplAsset),
    ]);

    await useOperationsStore.getState().loadOperations("portfolio-1");
    render(<Transactions />);
    expect(await screen.findByText("AAPL")).toBeDefined();

    // Simulate a hard reload: reset the store entirely (module-level state
    // gone, just like an F5), then re-fetch. The label must come back without
    // any asset-detail request.
    cleanup();
    useOperationsStore.getState().reset();
    await useOperationsStore.getState().loadOperations("portfolio-1");
    render(<Transactions />);
    expect(await screen.findByText("AAPL")).toBeDefined();

    expect(businessApi.getAsset).not.toHaveBeenCalled();
  });

  it("falls back to the defensive UUID placeholder when the backend returned id_asset but null asset", async () => {
    vi.mocked(businessApi.listOperations).mockResolvedValue([
      buildOp("op-malformed", null, {
        operation_type: "buy",
        id_asset: "cccccccc-cccc-cccc-cccc-cccccccccccc",
        asset: null,
        quantity: "1",
      }),
    ]);

    await useOperationsStore.getState().loadOperations("portfolio-1");
    render(<Transactions />);

    expect(await screen.findByText("cccccccc…")).toBeDefined();
    expect(businessApi.getAsset).not.toHaveBeenCalled();
  });
});
