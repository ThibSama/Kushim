import React from "react";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
  type Mock,
} from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { __resetCurrencyCacheForTests } from "./CurrencySelect";

// Narrow mocks: businessApi (so listCurrencies resolves), and the portfolio
// store (so we can observe submission payload without exercising the real
// Zustand store + http client).
vi.mock("../../lib/api/businessApi", async () => {
  const actual = await vi.importActual<
    typeof import("../../lib/api/businessApi")
  >("../../lib/api/businessApi");
  return {
    ...actual,
    listCurrencies: vi.fn(),
  };
});

const createPortfolioMock = vi.fn();
vi.mock("../../stores/portfolio", () => ({
  usePortfolioStore: () => ({
    createPortfolio: createPortfolioMock,
  }),
}));

import { CreatePortfolioModal } from "./CreatePortfolioModal";
import { listCurrencies } from "../../lib/api/businessApi";

const REFERENCE = [
  { value: "EUR", label: "Euro" },
  { value: "JPY", label: "Yen" },
  { value: "USD", label: "US Dollar" },
];

beforeEach(() => {
  __resetCurrencyCacheForTests();
  (listCurrencies as unknown as Mock).mockReset();
  (listCurrencies as unknown as Mock).mockResolvedValue(REFERENCE);
  createPortfolioMock.mockReset();
  createPortfolioMock.mockResolvedValue({
    id_portfolio: "pf-1",
    name: "x",
    base_currency: "EUR",
    visibility: "private",
    created_at: "",
    updated_at: "",
  });
});

afterEach(() => {
  __resetCurrencyCacheForTests();
});

describe("CreatePortfolioModal", () => {
  it("defaults the base currency to EUR", async () => {
    render(<CreatePortfolioModal onClose={() => {}} />);
    const trigger = document.getElementById(
      "portfolio-base-currency",
    ) as HTMLButtonElement;
    expect(trigger).toHaveTextContent(/EUR/);
  });

  it("submits the currently selected currency", async () => {
    const user = userEvent.setup();
    render(<CreatePortfolioModal onClose={() => {}} />);

    await user.type(
      screen.getByPlaceholderText("Mon portefeuille principal"),
      "Test",
    );

    const trigger = document.getElementById(
      "portfolio-base-currency",
    ) as HTMLButtonElement;
    await user.click(trigger);
    await waitFor(() =>
      expect(screen.getByRole("listbox")).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("option", { name: /USD/ }));

    await user.click(screen.getByRole("button", { name: /Créer/ }));

    await waitFor(() => expect(createPortfolioMock).toHaveBeenCalled());
    expect(createPortfolioMock.mock.calls[0][0]).toMatchObject({
      name: "Test",
      base_currency: "USD",
      visibility: "private",
    });
  });

  it("never submits unsupported arbitrary text — the CurrencySelect only emits canonical codes", async () => {
    const user = userEvent.setup();
    render(<CreatePortfolioModal onClose={() => {}} />);

    await user.type(
      screen.getByPlaceholderText("Mon portefeuille principal"),
      "Test",
    );

    // Open the catalogue, search for an unsupported code, attempt Enter.
    const trigger = document.getElementById(
      "portfolio-base-currency",
    ) as HTMLButtonElement;
    await user.click(trigger);
    await waitFor(() =>
      expect(screen.getByRole("listbox")).toBeInTheDocument(),
    );
    await user.type(
      screen.getByLabelText(/Rechercher une devise/i),
      "ZZZ",
    );
    await waitFor(() =>
      expect(screen.getByText(/Aucune devise/i)).toBeInTheDocument(),
    );
    await user.keyboard("{Enter}");

    // Submit — the selected currency is still the default (EUR), never ZZZ.
    await user.click(screen.getByRole("button", { name: /Créer/ }));
    await waitFor(() => expect(createPortfolioMock).toHaveBeenCalled());
    expect(createPortfolioMock.mock.calls[0][0].base_currency).toBe("EUR");
  });
});
