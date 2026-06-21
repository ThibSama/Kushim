import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, cleanup, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import type { Asset } from "../../lib/api/businessApi";

// The shared refresh-tracking store starts real polling on track(); in this page
// test we only need to assert the modal wires it up, so stub the store.
const trackMock = vi.fn();
vi.mock("../../stores/refreshTracking", () => ({
  useRefreshTrackingStore: (
    selector: (s: { track: typeof trackMock }) => unknown,
  ) => selector({ track: trackMock }),
}));

vi.mock("../../lib/api/businessApi", () => ({
  getAsset: vi.fn(),
  listAssets: vi.fn(),
  createOperation: vi.fn(),
  listOperationTypes: vi.fn(),
  listOperationStatuses: vi.fn(),
  listCurrencies: vi.fn(),
  listPortfolios: vi.fn(),
  createPortfolio: vi.fn(),
  getPortfolioSummary: vi.fn(),
  getPortfolioHoldings: vi.fn(),
  getDailySnapshots: vi.fn(),
}));

import * as businessApi from "../../lib/api/businessApi";
import { useAuthStore } from "../../stores/auth";
import { useAssetsStore } from "../../stores/assets";
import { usePortfolioStore } from "../../stores/portfolio";
import { useOperationsStore } from "../../stores/operations";
import { AssetDetail } from "./AssetDetail";

const sampleAsset: Asset = {
  id_asset: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
  name: "Apple Inc.",
  ticker: "AAPL",
  isin: null,
  exchange: "NASDAQ",
  symbol: null,
  network: null,
  asset_class: "equity",
  status: "active",
  native_currency: "USD",
  created_at: "",
  updated_at: "",
  metadata: null,
  market_data: {
    price_minor: 1500000,
    currency: "USD",
    market_cap_minor: null,
    volume_24h_minor: null,
    change_24h_pct: "+1.20",
    change_7d_pct: null,
    change_30d_pct: null,
    data_source: "test-static",
    source_asset_id: null,
    as_of: "2026-06-15T00:00:00Z",
  },
  aliases: null,
};

const usdPortfolio = {
  id_portfolio: "pf-usd",
  name: "USD portfolio",
  base_currency: "USD",
  visibility: "private" as const,
  created_at: "",
  updated_at: "",
};

function defaultRefreshOutcome() {
  return {
    refresh_request: {
      id_portfolio_refresh_request: "rr-1",
      status: "pending",
      requested_at: "2026-06-15T00:00:00Z",
    },
  };
}

function renderAssetDetail(path = `/assets/${sampleAsset.id_asset}`) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/assets/:id" element={<AssetDetail />} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  useAuthStore.setState({ token: "fake-token", user: null });
  usePortfolioStore.setState({
    portfolios: [],
    activePortfolioId: null,
    status: "idle",
    error: null,
  } as never);
  useAssetsStore.getState().reset();
  useOperationsStore.getState().reset();
  trackMock.mockReset();
  vi.clearAllMocks();

  vi.mocked(businessApi.getAsset).mockResolvedValue(sampleAsset);
  vi.mocked(businessApi.listAssets).mockResolvedValue({
    assets: [],
    pagination: { limit: 20, offset: 0, returned: 0, has_more: false },
  });
  vi.mocked(businessApi.listOperationTypes).mockResolvedValue([]);
  vi.mocked(businessApi.listOperationStatuses).mockResolvedValue([]);
  vi.mocked(businessApi.listCurrencies).mockResolvedValue([
    { value: "EUR", label: "Euro" },
    { value: "USD", label: "US Dollar" },
  ]);
  vi.mocked(businessApi.createOperation).mockResolvedValue(
    // The real client returns { operation, refresh_request }; for these page
    // tests only the refresh_request is exercised, so cast the minimal stub.
    defaultRefreshOutcome() as never,
  );
});

afterEach(() => {
  cleanup();
  useAssetsStore.getState().reset();
  useOperationsStore.getState().reset();
});

describe("AssetDetail — Ajouter au portefeuille", () => {
  it("opens the modal with buy preselected and the displayed asset, posting a buy against the active portfolio", async () => {
    usePortfolioStore.setState({
      portfolios: [usdPortfolio],
      activePortfolioId: "pf-usd",
      status: "success",
      error: null,
    } as never);

    const user = userEvent.setup();
    renderAssetDetail();

    const addBtn = await screen.findByRole("button", {
      name: /Ajouter au portefeuille/,
    });
    await user.click(addBtn);

    expect(await screen.findByText("Nouvelle opération")).toBeInTheDocument();
    expect(
      (screen.getByRole("combobox") as HTMLSelectElement).value,
    ).toBe("buy");
    // The displayed asset is preselected → the search input is hidden.
    expect(
      screen.queryByPlaceholderText("Rechercher un actif (ticker, nom, ISIN)…"),
    ).not.toBeInTheDocument();

    const byLabel = (text: string) =>
      screen
        .getAllByRole("spinbutton")
        .find((el) =>
          el.previousElementSibling?.textContent?.includes(text),
        ) as HTMLInputElement;
    await user.type(byLabel("Quantité"), "5");
    await user.type(byLabel("Prix unitaire"), "200.00");

    await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
    await waitFor(() =>
      expect(businessApi.createOperation).toHaveBeenCalled(),
    );

    // The operations store forwards (token, portfolioId, payload, key) to the
    // business-api client. portfolioId must be the ACTIVE portfolio, not EUR.
    // (Typed as any because the client's overloaded call shapes produce a union.)
    const call = vi.mocked(businessApi.createOperation).mock.calls[0] as any;
    const portfolioIdArg = call[1];
    const payload = call[2];
    const idempotencyKey = call[3];
    expect(portfolioIdArg).toBe("pf-usd");
    expect(payload.operation_type).toBe("buy");
    expect(payload.operation_status).toBe("posted");
    expect(payload.id_asset).toBe(sampleAsset.id_asset);
    expect(payload.quantity).toBe("5");
    expect(payload.price_minor).toBe(20000); // 200.00 → minor
    expect(payload.currency).toBe("USD"); // active portfolio base, not EUR
    expect(payload.gross_amount_minor).toBe(100000); // 5 × 200.00 → minor
    expect(payload.fx_rate_to_portfolio).toBeUndefined(); // no FX invented
    expect(idempotencyKey).toBeTruthy();

    expect(trackMock).toHaveBeenCalledWith("pf-usd", "rr-1");
  });

  it("shows an explicit non-broken state and no modal when there is no portfolio", async () => {
    usePortfolioStore.setState({
      portfolios: [],
      activePortfolioId: null,
      status: "success",
      error: null,
    } as never);

    renderAssetDetail();

    expect(
      await screen.findByText(/Créez d'abord un portefeuille/),
    ).toBeInTheDocument();
    expect(screen.queryByText("Nouvelle opération")).not.toBeInTheDocument();
  });

  it("keeps the action in a non-breaking loading state with no modal while the portfolio loads", async () => {
    usePortfolioStore.setState({
      portfolios: [],
      activePortfolioId: null,
      status: "loading",
      error: null,
    } as never);

    renderAssetDetail();

    // Wait for the asset itself to finish loading so we are past the
    // asset-loading screen and inside the action area. The asset name also
    // appears in the "Nom" info row, so target the page heading specifically.
    await screen.findByRole("heading", { name: sampleAsset.name });
    // The portfolio is still "loading" -> the action shows a disabled loading
    // label and never opens the operation modal.
    expect(screen.getByText(/Chargement/)).toBeInTheDocument();
    expect(screen.queryByText("Nouvelle opération")).not.toBeInTheDocument();
  });

  it("loads portfolios when reached through a direct route with the portfolio store still idle", async () => {
    // Leave the portfolio store in its initialState (idle) from beforeEach.
    // Mock the API so the real loadPortfolios() can resolve the portfolio.
    vi.mocked(businessApi.listPortfolios).mockResolvedValue([usdPortfolio]);

    renderAssetDetail();

    // The asset loads first; wait for it to appear. Meanwhile the idle-store
    // effect must have fired loadPortfolios() — proving the deep-link path
    // does NOT rely on Dashboard having loaded portfolios first.
    await screen.findByRole("heading", { name: sampleAsset.name });
    expect(businessApi.listPortfolios).toHaveBeenCalledTimes(1);

    // After the portfolios resolve, the action transitions from disabled
    // loading state to the enabled "Ajouter au portefeuille" button.
    expect(
      await screen.findByRole("button", { name: /Ajouter au portefeuille/ }),
    ).not.toBeDisabled();
  });
});
