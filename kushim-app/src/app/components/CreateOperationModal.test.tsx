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
import { ApiRequestError } from "../../lib/api/httpClient";

// Narrow API mocks.
vi.mock("../../lib/api/businessApi", async () => {
  const actual = await vi.importActual<
    typeof import("../../lib/api/businessApi")
  >("../../lib/api/businessApi");
  return {
    ...actual,
    listCurrencies: vi.fn(),
  };
});

// Stores.
const createOperationMock = vi.fn();
const loadReferenceDataMock = vi.fn();
const trackMock = vi.fn();

vi.mock("../../stores/operations", () => ({
  useOperationsStore: () => ({
    createOperation: createOperationMock,
    operationTypes: [],
    loadReferenceData: loadReferenceDataMock,
  }),
}));

vi.mock("../../stores/portfolio", () => ({
  usePortfolioStore: (selector: (state: PortfolioStoreLike) => unknown) =>
    selector({
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
    }),
}));

type PortfolioStoreLike = {
  portfolios: Array<{
    id_portfolio: string;
    name: string;
    base_currency: string;
    visibility: string;
    created_at: string;
    updated_at: string;
  }>;
};

vi.mock("../../stores/refreshTracking", () => ({
  useRefreshTrackingStore: (selector: (state: { track: typeof trackMock }) => unknown) =>
    selector({ track: trackMock }),
}));

import { CreateOperationModal } from "./CreateOperationModal";
import { listCurrencies } from "../../lib/api/businessApi";

const REFERENCE = [
  { value: "EUR", label: "Euro" },
  { value: "JPY", label: "Yen" },
  { value: "USD", label: "US Dollar" },
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
  __resetCurrencyCacheForTests();
  (listCurrencies as unknown as Mock).mockReset();
  (listCurrencies as unknown as Mock).mockResolvedValue(REFERENCE);
  createOperationMock.mockReset();
  createOperationMock.mockResolvedValue(defaultRefreshOutcome());
  loadReferenceDataMock.mockReset();
  trackMock.mockReset();
});

afterEach(() => {
  __resetCurrencyCacheForTests();
});

async function pickCurrency(
  user: ReturnType<typeof userEvent.setup>,
  code: string,
) {
  const trigger = document.getElementById(
    "operation-currency",
  ) as HTMLButtonElement;
  await user.click(trigger);
  await waitFor(() =>
    expect(screen.getByRole("listbox")).toBeInTheDocument(),
  );
  await user.click(screen.getByRole("option", { name: new RegExp(code) }));
}

async function setMontantBrut(
  user: ReturnType<typeof userEvent.setup>,
  value: string,
) {
  // The gross-amount input has placeholder "100.00" by default.
  const input = screen
    .getAllByRole("spinbutton")
    .find((el) =>
      el.previousElementSibling?.textContent?.includes("Montant brut"),
    );
  if (!input) throw new Error("Montant brut input not found");
  await user.clear(input);
  if (value) await user.type(input, value);
}

async function selectType(
  user: ReturnType<typeof userEvent.setup>,
  value: string,
) {
  const select = screen.getByRole("combobox") as HTMLSelectElement;
  await user.selectOptions(select, value);
}

describe("CreateOperationModal", () => {
  it("defaults the currency to the active portfolio base currency", async () => {
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    const trigger = document.getElementById(
      "operation-currency",
    ) as HTMLButtonElement;
    expect(trigger).toHaveTextContent(/EUR/);
  });

  it("hides the FX field when operation currency matches the portfolio base currency", async () => {
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await setMontantBrut(user, "100");
    expect(
      screen.queryByText(/Taux de change/),
    ).not.toBeInTheDocument();
  });

  it("shows the FX field for a cross-currency buy/deposit with positive cash", async () => {
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await setMontantBrut(user, "100");
    await pickCurrency(user, "USD");
    expect(screen.getByText(/1 USD = X EUR/)).toBeInTheDocument();
  });

  it("shows the FX field for a cross-currency transfer_in with positive cash", async () => {
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await selectType(user, "transfer_in");
    await setMontantBrut(user, "100");
    await pickCurrency(user, "USD");
    expect(screen.getByText(/1 USD = X EUR/)).toBeInTheDocument();
  });

  it("shows the FX field for a cross-currency transfer_out with positive cash", async () => {
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await selectType(user, "transfer_out");
    await setMontantBrut(user, "100");
    await pickCurrency(user, "USD");
    expect(screen.getByText(/1 USD = X EUR/)).toBeInTheDocument();
  });

  it("does not require FX for a zero-cash transfer", async () => {
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await selectType(user, "transfer_in");
    await pickCurrency(user, "USD");
    // Leave montant brut at 0 → no monetary leg → no FX required.
    expect(screen.queryByText(/Taux de change/)).not.toBeInTheDocument();
  });

  it("blocks submission when FX is required and missing", async () => {
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await setMontantBrut(user, "100");
    await pickCurrency(user, "USD");
    await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
    expect(
      screen.getByText(/Le taux de change est requis \(1 USD = X EUR\)/),
    ).toBeInTheDocument();
    expect(createOperationMock).not.toHaveBeenCalled();
  });

  it("submits a positive-decimal FX as the original string", async () => {
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await setMontantBrut(user, "100");
    await pickCurrency(user, "USD");
    const fxInputs = screen.getAllByRole("spinbutton");
    const fxInput = fxInputs.find((el) =>
      el.previousElementSibling?.textContent?.includes("Taux de change"),
    );
    expect(fxInput).toBeTruthy();
    await user.type(fxInput as HTMLElement, "0.92");
    await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
    await waitFor(() => expect(createOperationMock).toHaveBeenCalled());
    const payload = createOperationMock.mock.calls[0][1];
    expect(payload.fx_rate_to_portfolio).toBe("0.92");
    expect(payload.currency).toBe("USD");
  });

  it("switching back to the base currency clears the FX field and prevents stale submission", async () => {
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await setMontantBrut(user, "100");
    await pickCurrency(user, "USD");
    const fxInputs = screen.getAllByRole("spinbutton");
    const fxInput = fxInputs.find((el) =>
      el.previousElementSibling?.textContent?.includes("Taux de change"),
    ) as HTMLInputElement;
    await user.type(fxInput, "0.92");
    // Switch back to EUR.
    await pickCurrency(user, "EUR");
    expect(screen.queryByText(/Taux de change/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
    await waitFor(() => expect(createOperationMock).toHaveBeenCalled());
    const payload = createOperationMock.mock.calls[0][1];
    expect(payload.fx_rate_to_portfolio).toBeUndefined();
    expect(payload.currency).toBe("EUR");
  });

  it("zero monetary leg does not submit a stale FX rate even in cross-currency", async () => {
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    // Pick a structural cross-currency transfer with positive cash, then
    // shrink the cash to 0 — the monetary leg drops, FX must not be sent.
    await selectType(user, "transfer_in");
    await pickCurrency(user, "USD");
    await setMontantBrut(user, "100");
    // FX appears with positive cash; type a value.
    const fxInputs = screen.getAllByRole("spinbutton");
    const fxInput = fxInputs.find((el) =>
      el.previousElementSibling?.textContent?.includes("Taux de change"),
    ) as HTMLInputElement;
    await user.type(fxInput, "0.92");
    // Now zero out the monetary leg.
    await setMontantBrut(user, "");
    await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
    await waitFor(() => expect(createOperationMock).toHaveBeenCalled());
    const payload = createOperationMock.mock.calls[0][1];
    expect(payload.fx_rate_to_portfolio).toBeUndefined();
  });

  it("maps unsupported_cross_currency to a clear French message", async () => {
    createOperationMock.mockRejectedValueOnce(
      new ApiRequestError({
        code: "unsupported_cross_currency",
        message: "raw backend message",
        status: 422,
      }),
    );
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await setMontantBrut(user, "100");
    await pickCurrency(user, "USD");
    const fxInputs = screen.getAllByRole("spinbutton");
    const fxInput = fxInputs.find((el) =>
      el.previousElementSibling?.textContent?.includes("Taux de change"),
    ) as HTMLInputElement;
    await user.type(fxInput, "0.92");
    await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
    await waitFor(() =>
      expect(
        screen.getByText(
          /Le taux de change est requis lorsque la devise de l'opération diffère de la devise de base/,
        ),
      ).toBeInTheDocument(),
    );
  });

  it("maps unsupported_currency to a clear French message", async () => {
    createOperationMock.mockRejectedValueOnce(
      new ApiRequestError({
        code: "unsupported_currency",
        message: "raw backend message",
        status: 422,
      }),
    );
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await setMontantBrut(user, "100");
    await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
    await waitFor(() =>
      expect(
        screen.getByText(/Cette devise n'est pas prise en charge/),
      ).toBeInTheDocument(),
    );
  });
});
