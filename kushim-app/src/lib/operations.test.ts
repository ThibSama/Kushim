import { describe, expect, it } from "vitest";
import type { PortfolioOperation } from "./api/businessApi";
import { operationAssetLabel, operationToRow } from "./operations";

const baseOp: PortfolioOperation = {
  id_portfolio_operation: "11111111-1111-1111-1111-111111111111",
  id_portfolio: "22222222-2222-2222-2222-222222222222",
  id_asset: null,
  id_related_asset: null,
  asset: null,
  related_asset: null,
  operation_type: "deposit",
  operation_status: "posted",
  executed_at: "2026-06-05T10:00:00Z",
  effective_at: null,
  quantity: null,
  related_quantity: null,
  price_minor: null,
  gross_amount_minor: 100000,
  fees_minor: null,
  taxes_minor: null,
  cash_amount_minor: 100000,
  currency: "EUR",
  fx_rate_to_portfolio: null,
  external_provider: null,
  external_reference: null,
  id_corrected_operation: null,
  notes: null,
  metadata: {},
  created_at: "2026-06-05T10:00:00Z",
  updated_at: "2026-06-05T10:00:00Z",
};

const aaplOp: PortfolioOperation = {
  ...baseOp,
  operation_type: "buy",
  id_asset: "33333333-3333-3333-3333-333333333333",
  asset: {
    id_asset: "33333333-3333-3333-3333-333333333333",
    name: "Apple Inc.",
    ticker: "AAPL",
    status: "active",
  },
  quantity: "10.5",
  price_minor: 12345,
};

describe("operationAssetLabel", () => {
  it("returns the ticker when present on operation.asset", () => {
    expect(operationAssetLabel(aaplOp)).toBe("AAPL");
  });

  it("falls back to the asset name when the ticker is null", () => {
    const op: PortfolioOperation = {
      ...aaplOp,
      asset: { ...aaplOp.asset!, ticker: null },
    };
    expect(operationAssetLabel(op)).toBe("Apple Inc.");
  });

  it("returns the cash placeholder for a cash-only operation", () => {
    expect(operationAssetLabel(baseOp)).toBe("—");
  });

  it("returns a truncated UUID when id_asset is present but asset failed to resolve", () => {
    const malformed: PortfolioOperation = {
      ...aaplOp,
      asset: null,
    };
    expect(operationAssetLabel(malformed)).toBe("33333333…");
  });
});

describe("operationToRow", () => {
  it("derives the asset label directly from the embedded asset response", () => {
    expect(operationToRow(aaplOp).asset).toBe("AAPL");
  });

  it("renders '—' for a deposit operation", () => {
    expect(operationToRow(baseOp).asset).toBe("—");
  });
});

describe("typed contract", () => {
  it("accepts an operation that carries both asset and related_asset", () => {
    const spinOff: PortfolioOperation = {
      ...baseOp,
      operation_type: "spin_off",
      id_asset: "33333333-3333-3333-3333-333333333333",
      id_related_asset: "44444444-4444-4444-4444-444444444444",
      asset: {
        id_asset: "33333333-3333-3333-3333-333333333333",
        name: "Parent Co.",
        ticker: "PRNT",
        status: "active",
      },
      related_asset: {
        id_asset: "44444444-4444-4444-4444-444444444444",
        name: "Child Co.",
        ticker: "CHLD",
        status: "active",
      },
    };
    expect(spinOff.asset?.ticker).toBe("PRNT");
    expect(spinOff.related_asset?.ticker).toBe("CHLD");
    // Display still derives from the primary asset.
    expect(operationToRow(spinOff).asset).toBe("PRNT");
  });
});
