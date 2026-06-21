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
import type { Asset } from "../../lib/api/businessApi";

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

  it("surfaces a visible contract error when refresh_request is missing from the response", async () => {
    // A posted operation MUST come back with a refresh_request. If the backend
    // omits it, the modal must NOT silently close — it surfaces a contract
    // error so the user knows the portfolio update was not planned.
    createOperationMock.mockResolvedValueOnce({
      refresh_request: null,
    });
    const user = userEvent.setup();
    render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
    await setMontantBrut(user, "100");
    await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
    await waitFor(() =>
      expect(
        screen.getByText(
          /mise à jour du portefeuille n'a pas pu être planifiée/,
        ),
      ).toBeInTheDocument(),
    );
    // The modal stays open (onClose not called) and the operation was sent.
    expect(createOperationMock).toHaveBeenCalledTimes(1);
    // The refresh must NOT have been tracked — there is nothing to track.
    expect(trackMock).not.toHaveBeenCalled();
  });

  // ===================================================================
  // "Ajouter un actif" / AssetDetail initialization (initialOperationType,
  // initialAsset). These presets only seed the mount; they do not weaken
  // validation, minor-unit conversion, idempotency rotation, or FX rules.
  // ===================================================================
  describe("add-asset initialization (initialOperationType / initialAsset)", () => {
    const sampleAsset: Asset = {
      id_asset: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
      name: "Apple Inc.",
      ticker: "AAPL",
      isin: null,
      exchange: null,
      symbol: null,
      network: null,
      asset_class: "equity",
      status: "active",
      native_currency: null,
      created_at: "",
      updated_at: "",
      metadata: null,
      market_data: null,
      aliases: null,
    };

    it("without init props defaults to the generic operation type (deposit)", () => {
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      expect(
        (screen.getByRole("combobox") as HTMLSelectElement).value,
      ).toBe("deposit");
    });

    it("initialOperationType='buy' initializes the selector to buy and leaves the asset empty", () => {
      render(
        <CreateOperationModal
          portfolioId="pf-eur"
          initialOperationType="buy"
          onClose={() => {}}
        />,
      );
      expect(
        (screen.getByRole("combobox") as HTMLSelectElement).value,
      ).toBe("buy");
      // buy is an asset type → the asset search input is visible and empty
      // (no preselection when no initialAsset is supplied).
      expect(
        screen.getByPlaceholderText("Rechercher un actif (ticker, nom, ISIN)…"),
      ).toBeInTheDocument();
    });

    it("initialAsset is displayed as already selected", () => {
      render(
        <CreateOperationModal
          portfolioId="pf-eur"
          initialOperationType="buy"
          initialAsset={sampleAsset}
          onClose={() => {}}
        />,
      );
      // The selected-asset chip shows the ticker.
      expect(screen.getByText("AAPL")).toBeInTheDocument();
      // The search input is NOT shown because an asset is already selected.
      expect(
        screen.queryByPlaceholderText("Rechercher un actif (ticker, nom, ISIN)…"),
      ).not.toBeInTheDocument();
    });

    it("submits an initialized buy with the full posted payload and tracks the refresh", async () => {
      const user = userEvent.setup();
      render(
        <CreateOperationModal
          portfolioId="pf-eur"
          initialOperationType="buy"
          initialAsset={sampleAsset}
          onClose={() => {}}
        />,
      );

      const byLabel = (text: string) =>
        screen
          .getAllByRole("spinbutton")
          .find((el) =>
            el.previousElementSibling?.textContent?.includes(text),
          ) as HTMLInputElement;

      await user.type(byLabel("Quantité"), "10");
      await user.type(byLabel("Prix unitaire"), "150.00");
      await user.type(byLabel("Frais"), "2.50");

      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalled());

      const [portfolioIdArg, payload, idempotencyKey] =
        createOperationMock.mock.calls[0];
      expect(portfolioIdArg).toBe("pf-eur");
      expect(payload.operation_type).toBe("buy");
      expect(payload.operation_status).toBe("posted");
      expect(payload.id_asset).toBe(sampleAsset.id_asset);
      expect(payload.quantity).toBe("10");
      expect(payload.price_minor).toBe(15000); // 150.00 → minor
      expect(payload.currency).toBe("EUR"); // portfolio base, no FX
      expect(payload.gross_amount_minor).toBe(150000); // 10 × 150.00 → minor
      expect(payload.fees_minor).toBe(250); // 2.50 → minor
      expect(payload.executed_at).toBeTruthy();
      // Same currency as the portfolio base → no FX rate, nothing invented.
      expect(payload.fx_rate_to_portfolio).toBeUndefined();
      // P3 idempotency key is still generated and forwarded.
      expect(idempotencyKey).toBeTruthy();

      // The returned refresh request is tracked against the active portfolio.
      expect(trackMock).toHaveBeenCalledWith("pf-eur", "rr-1");
    });

    it("does not contaminate a later generic open after an initialized instance is closed", () => {
      const { unmount } = render(
        <CreateOperationModal
          portfolioId="pf-eur"
          initialOperationType="buy"
          onClose={() => {}}
        />,
      );
      expect(
        (screen.getByRole("combobox") as HTMLSelectElement).value,
      ).toBe("buy");
      unmount();

      // A fresh generic open must default to deposit — the buy preset must not
      // leak across mount lifecycles (no global draft state).
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      expect(
        (screen.getByRole("combobox") as HTMLSelectElement).value,
      ).toBe("deposit");
    });
  });

  // ===================================================================
  // P3: Idempotency-Key lifecycle
  // ===================================================================

  describe("P3 idempotency key lifecycle", () => {
    let originalRandomUUID: typeof crypto.randomUUID;
    let uuidCounter = 0;

    beforeEach(() => {
      uuidCounter = 0;
      originalRandomUUID = crypto.randomUUID;
      // Deterministic UUID generator so the tests can assert reuse vs
      // rotation without depending on real random values.
      Object.defineProperty(crypto, "randomUUID", {
        configurable: true,
        value: () => {
          uuidCounter += 1;
          // RFC4122-shaped string so the backend extractor would accept it.
          return `00000000-0000-4000-8000-00000000000${uuidCounter}` as `${string}-${string}-${string}-${string}-${string}`;
        },
      });
    });

    afterEach(() => {
      Object.defineProperty(crypto, "randomUUID", {
        configurable: true,
        value: originalRandomUUID,
      });
    });

    it("first submission generates a UUID and passes it to the store", async () => {
      const user = userEvent.setup();
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalled());
      const key = createOperationMock.mock.calls[0][2];
      expect(key).toBe("00000000-0000-4000-8000-000000000001");
    });

    it("retrying the same unchanged payload reuses the same UUID", async () => {
      createOperationMock.mockRejectedValueOnce(
        new ApiRequestError({
          code: "internal_error",
          message: "transient",
          status: 500,
        }),
      );
      createOperationMock.mockResolvedValueOnce(defaultRefreshOutcome());
      const user = userEvent.setup();
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(1));
      // Retry without editing the form.
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(2));
      const key1 = createOperationMock.mock.calls[0][2];
      const key2 = createOperationMock.mock.calls[1][2];
      expect(key2).toBe(key1);
    });

    it("editing the amount rotates the UUID for the next submission", async () => {
      createOperationMock.mockRejectedValueOnce(
        new ApiRequestError({
          code: "internal_error",
          message: "transient",
          status: 500,
        }),
      );
      createOperationMock.mockResolvedValueOnce(defaultRefreshOutcome());
      const user = userEvent.setup();
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(1));
      // Change the amount, then retry — the payload now differs.
      await setMontantBrut(user, "250");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(2));
      const key1 = createOperationMock.mock.calls[0][2];
      const key2 = createOperationMock.mock.calls[1][2];
      expect(key1).not.toBe(key2);
    });

    it("maps idempotency_key_conflict to a clear French message", async () => {
      createOperationMock.mockRejectedValueOnce(
        new ApiRequestError({
          code: "idempotency_key_conflict",
          message: "raw backend message",
          status: 409,
        }),
      );
      const user = userEvent.setup();
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() =>
        expect(
          screen.getByText(
            /Cette tentative correspond à une opération différente/,
          ),
        ).toBeInTheDocument(),
      );
    });

    it("maps missing_idempotency_key to a safe retry-prompt French message", async () => {
      createOperationMock.mockRejectedValueOnce(
        new ApiRequestError({
          code: "missing_idempotency_key",
          message: "raw backend message",
          status: 400,
        }),
      );
      const user = userEvent.setup();
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() =>
        expect(
          screen.getByText(
            /La requête ne peut pas être sécurisée. Veuillez réessayer\./,
          ),
        ).toBeInTheDocument(),
      );
    });

    it("changing the currency rotates the UUID", async () => {
      createOperationMock.mockRejectedValueOnce(
        new ApiRequestError({
          code: "internal_error",
          message: "transient",
          status: 500,
        }),
      );
      createOperationMock.mockResolvedValueOnce(defaultRefreshOutcome());
      const user = userEvent.setup();
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(1));
      // Switch to USD (cross-currency). The canonical payload changes
      // (currency field) AND now requires fx_rate_to_portfolio, so the
      // submitted payload differs materially → key must rotate.
      await pickCurrency(user, "USD");
      const fxInput = screen
        .getAllByRole("spinbutton")
        .find((el) =>
          el.previousElementSibling?.textContent?.includes("Taux de change"),
        ) as HTMLInputElement;
      await user.type(fxInput, "1.05");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(2));
      const key1 = createOperationMock.mock.calls[0][2];
      const key2 = createOperationMock.mock.calls[1][2];
      expect(key1).not.toBe(key2);
      const payload2 = createOperationMock.mock.calls[1][1];
      expect(payload2.currency).toBe("USD");
      expect(payload2.fx_rate_to_portfolio).toBe("1.05");
    });

    it("changing the FX rate alone rotates the UUID for a cross-currency submission", async () => {
      createOperationMock.mockRejectedValueOnce(
        new ApiRequestError({
          code: "internal_error",
          message: "transient",
          status: 500,
        }),
      );
      createOperationMock.mockResolvedValueOnce(defaultRefreshOutcome());
      const user = userEvent.setup();
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      await pickCurrency(user, "USD");
      const fxInputs = () =>
        screen
          .getAllByRole("spinbutton")
          .find((el) =>
            el.previousElementSibling?.textContent?.includes("Taux de change"),
          ) as HTMLInputElement;
      await user.type(fxInputs(), "0.92");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(1));
      // Now edit only the FX rate.
      await user.clear(fxInputs());
      await user.type(fxInputs(), "1.10");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(2));
      const key1 = createOperationMock.mock.calls[0][2];
      const key2 = createOperationMock.mock.calls[1][2];
      expect(key1).not.toBe(key2);
    });

    it("closing and reopening the modal generates a new key", async () => {
      const user = userEvent.setup();
      // First mount: submit successfully, then unmount.
      const { unmount } = render(
        <CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />,
      );
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(1));
      unmount();

      // Second mount with the SAME payload — a new key must be generated.
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(2));
      const key1 = createOperationMock.mock.calls[0][2];
      const key2 = createOperationMock.mock.calls[1][2];
      expect(key1).not.toBe(key2);
    });

    it("confirmed success clears the attempt so the next op uses a new key", async () => {
      const user = userEvent.setup();
      // First call: success. Unmount before reopening so we don't end up
      // with two modals on screen (each rendering its own Enregistrer
      // button) — same pattern as the close-and-reopen test above.
      const { unmount } = render(
        <CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />,
      );
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(1));
      const key1 = createOperationMock.mock.calls[0][2];
      unmount();

      // Reopen with the same payload — a new key must be generated because
      // the modal's attempt ref was cleared on success and the new mount
      // starts fresh.
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      await user.click(screen.getByRole("button", { name: /Enregistrer/ }));
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(2));
      const key2 = createOperationMock.mock.calls[1][2];
      expect(key2).not.toBe(key1);
    });

    it("rapid double submission uses a single logical key", async () => {
      // Stub the resolved outcome with a controllable promise so the
      // first call is still in-flight when the second click happens.
      let resolveFirst!: (value: ReturnType<typeof defaultRefreshOutcome>) => void;
      createOperationMock.mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveFirst = resolve;
          }),
      );
      const user = userEvent.setup();
      render(<CreateOperationModal portfolioId="pf-eur" onClose={() => {}} />);
      await setMontantBrut(user, "100");
      const button = screen.getByRole("button", { name: /Enregistrer/ });
      // Double-click before the first call resolves. The submit button is
      // disabled while submitting, so the second click is a no-op — exactly
      // one logical key was sent.
      await user.click(button);
      await user.click(button);
      resolveFirst!(defaultRefreshOutcome());
      await waitFor(() => expect(createOperationMock).toHaveBeenCalledTimes(1));
    });
  });
});
