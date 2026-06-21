import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Routes, Route } from "react-router-dom";

// recharts renders SVG measuring containers that need ResizeObserver; jsdom has
// none, so stub the chart primitives. The dashboard action bar and the shared
// operation modal are what we assert, not the charts.
vi.mock("recharts", () => {
  const Box = ({ children }: { children?: React.ReactNode }) =>
    children ?? null;
  return {
    ResponsiveContainer: Box,
    ComposedChart: Box,
    PieChart: Box,
    Area: () => null,
    XAxis: () => null,
    YAxis: () => null,
    Tooltip: () => null,
    Pie: () => null,
    Cell: () => null,
  };
});

vi.mock("../../lib/api/businessApi", () => ({
  listPortfolios: vi.fn(),
  createPortfolio: vi.fn(),
  listOperations: vi.fn(),
  createOperation: vi.fn(),
  listOperationTypes: vi.fn(),
  listOperationStatuses: vi.fn(),
  listCurrencies: vi.fn(),
  getPortfolioSummary: vi.fn(),
  getPortfolioHoldings: vi.fn(),
  getDailySnapshots: vi.fn(),
  listAssets: vi.fn(),
  getAsset: vi.fn(),
}));

import * as businessApi from "../../lib/api/businessApi";
import { useAuthStore } from "../../stores/auth";
import { usePortfolioStore } from "../../stores/portfolio";
import { useOperationsStore } from "../../stores/operations";
import { usePortfolioReadModelsStore } from "../../stores/portfolioReadModels";
import { Dashboard } from "./Dashboard";

const REFERENCE = [
  { value: "EUR", label: "Euro" },
  { value: "USD", label: "US Dollar" },
  { value: "JPY", label: "Yen" },
];

function defaultRefreshOutcome() {
  return {
    refresh_request: {
      id_portfolio_refresh_request: "rr-1",
      status: "pending",
      requested_at: "2026-06-15T00:00:00Z",
    },
  };
}

beforeEach(() => {
  useAuthStore.setState({ token: "fake-token", user: null });
  usePortfolioStore.setState({
    portfolios: [
      {
        id_portfolio: "pf-eur",
        name: "EUR portfolio",
        base_currency: "EUR",
        visibility: "private",
        created_at: "",
        updated_at: "",
      },
    ],
    activePortfolioId: "pf-eur",
    status: "success",
    error: null,
  } as never);
  useOperationsStore.getState().reset();
  usePortfolioReadModelsStore.getState().reset();
  vi.clearAllMocks();

  vi.mocked(businessApi.listOperations).mockResolvedValue([]);
  vi.mocked(businessApi.listOperationTypes).mockResolvedValue([]);
  vi.mocked(businessApi.listOperationStatuses).mockResolvedValue([]);
  vi.mocked(businessApi.listCurrencies).mockResolvedValue(REFERENCE);
  vi.mocked(businessApi.listAssets).mockResolvedValue({
    assets: [],
    pagination: { limit: 20, offset: 0, returned: 0, has_more: false },
  });
  vi.mocked(businessApi.getPortfolioSummary).mockResolvedValue({
    data_available: false,
    summary: null,
    reason: "read_model_missing",
  });
  vi.mocked(businessApi.getPortfolioHoldings).mockResolvedValue({
    data_available: false,
    holdings: [],
    pagination: { limit: 5, offset: 0, returned: 0, has_more: false },
    reason: "read_model_missing",
  });
  vi.mocked(businessApi.getDailySnapshots).mockResolvedValue({
    data_available: false,
    snapshots: [],
    pagination: { limit: 366, offset: 0, returned: 0, has_more: false },
  });
  vi.mocked(businessApi.createOperation).mockResolvedValue(
    // The real client returns { operation, refresh_request }; for these page
    // tests only the refresh_request leg is exercised, so cast the minimal stub.
    defaultRefreshOutcome() as any,
  );
});

afterEach(() => {
  cleanup();
  useOperationsStore.getState().reset();
  usePortfolioReadModelsStore.getState().reset();
});

function renderDashboard(initialPath = "/") {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/assets" element={<div data-testid="assets-page" />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("Dashboard quick actions", () => {
  it("'Ajouter un actif' opens the operation modal with buy selected and no preselected asset", async () => {
    const user = userEvent.setup();
    renderDashboard();

    await user.click(screen.getByRole("button", { name: /Ajouter un actif/ }));

    expect(screen.getByText("Nouvelle opération")).toBeInTheDocument();
    expect(
      (screen.getByRole("combobox") as HTMLSelectElement).value,
    ).toBe("buy");
    // Dashboard flow leaves the asset empty: the search input is shown.
    expect(
      screen.getByPlaceholderText("Rechercher un actif (ticker, nom, ISIN)…"),
    ).toBeInTheDocument();
  });

  it("'Ajouter une transaction' still opens the modal with the generic default (deposit)", async () => {
    const user = userEvent.setup();
    renderDashboard();

    await user.click(
      screen.getByRole("button", { name: /Ajouter une transaction/ }),
    );

    expect(screen.getByText("Nouvelle opération")).toBeInTheDocument();
    expect(
      (screen.getByRole("combobox") as HTMLSelectElement).value,
    ).toBe("deposit");
  });

  it("'Catalogue d'actifs' still navigates to /assets", async () => {
    const user = userEvent.setup();
    renderDashboard();

    await user.click(screen.getByRole("button", { name: /Catalogue d'actifs/ }));

    expect(screen.getByTestId("assets-page")).toBeInTheDocument();
  });
});
