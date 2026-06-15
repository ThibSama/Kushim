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
import {
  CurrencySelect,
  __resetCurrencyCacheForTests,
} from "./CurrencySelect";

vi.mock("../../lib/api/businessApi", async () => {
  const actual = await vi.importActual<
    typeof import("../../lib/api/businessApi")
  >("../../lib/api/businessApi");
  return {
    ...actual,
    listCurrencies: vi.fn(),
  };
});

import { listCurrencies } from "../../lib/api/businessApi";

const REFERENCE = [
  { value: "AUD", label: "Australian Dollar" },
  { value: "EUR", label: "Euro" },
  { value: "JPY", label: "Yen" },
  { value: "USD", label: "US Dollar" },
];

beforeEach(() => {
  __resetCurrencyCacheForTests();
  (listCurrencies as unknown as Mock).mockReset();
});

afterEach(() => {
  __resetCurrencyCacheForTests();
});

async function renderAndOpen(
  initial?: Partial<React.ComponentProps<typeof CurrencySelect>>,
) {
  (listCurrencies as unknown as Mock).mockResolvedValue(REFERENCE);
  const onChange = vi.fn();
  const user = userEvent.setup();
  const result = render(
    <CurrencySelect
      value={initial?.value ?? ""}
      onChange={initial?.onChange ?? onChange}
      label="Devise"
      id="t-select"
    />,
  );
  // The toggle button carries `aria-haspopup="listbox"`, which uniquely
  // identifies it even when the catalogue has not yet loaded into the panel.
  const trigger = document.getElementById("t-select") as HTMLButtonElement;
  await user.click(trigger);
  await waitFor(() =>
    expect(screen.getByRole("listbox")).toBeInTheDocument(),
  );
  return { user, onChange, ...result };
}

describe("CurrencySelect", () => {
  it("renders currencies returned by the reference API", async () => {
    await renderAndOpen();
    for (const code of ["AUD", "EUR", "JPY", "USD"]) {
      expect(
        screen.getByRole("option", { name: new RegExp(code) }),
      ).toBeInTheDocument();
    }
  });

  it("displays the code and the localized (or fallback) label", async () => {
    await renderAndOpen();
    const eur = screen.getByRole("option", { name: /EUR/ });
    // localized FR label via Intl.DisplayNames, OR the backend label fallback.
    expect(eur).toHaveTextContent("EUR");
    expect(eur.textContent ?? "").toMatch(/[A-Za-zé]/);
  });

  it("filters by exact code", async () => {
    const { user } = await renderAndOpen();
    await user.type(
      screen.getByLabelText(/Rechercher une devise/i),
      "jpy",
    );
    await waitFor(() => {
      expect(screen.queryByRole("option", { name: /EUR/ })).toBeNull();
    });
    expect(screen.getByRole("option", { name: /JPY/ })).toBeInTheDocument();
  });

  it("filters by localized/backend label substring", async () => {
    const { user } = await renderAndOpen();
    await user.type(
      screen.getByLabelText(/Rechercher une devise/i),
      "Yen",
    );
    await waitFor(() => {
      expect(screen.queryByRole("option", { name: /EUR/ })).toBeNull();
    });
    expect(screen.getByRole("option", { name: /JPY/ })).toBeInTheDocument();
  });

  it("ArrowDown/ArrowUp navigate the list", async () => {
    const onChange = vi.fn();
    const { user } = await renderAndOpen({ onChange });
    const input = screen.getByLabelText(/Rechercher une devise/i);
    // Starts at index 0 (AUD). ArrowDown → EUR. Enter selects EUR.
    await user.click(input);
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledWith("EUR");
  });

  it("Enter selects the active option", async () => {
    const onChange = vi.fn();
    const { user } = await renderAndOpen({ onChange });
    await user.click(screen.getByLabelText(/Rechercher une devise/i));
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledWith("AUD");
  });

  it("Escape closes the list", async () => {
    const { user } = await renderAndOpen();
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByRole("listbox")).toBeNull();
    });
  });

  it("click outside closes the list", async () => {
    const { user } = await renderAndOpen();
    await user.click(document.body);
    await waitFor(() => {
      expect(screen.queryByRole("listbox")).toBeNull();
    });
  });

  it("arbitrary search text cannot become a selected currency", async () => {
    const onChange = vi.fn();
    const { user } = await renderAndOpen({ onChange });
    const input = screen.getByLabelText(/Rechercher une devise/i);
    await user.type(input, "ZZZ");
    await waitFor(() => {
      expect(screen.getByText(/Aucune devise/i)).toBeInTheDocument();
    });
    await user.keyboard("{Enter}");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("shows the loading state while the reference API is in flight", async () => {
    let resolveFn: (items: typeof REFERENCE) => void = () => {};
    (listCurrencies as unknown as Mock).mockReturnValue(
      new Promise<typeof REFERENCE>((resolve) => {
        resolveFn = resolve;
      }),
    );
    const user = userEvent.setup();
    render(
      <CurrencySelect value="" onChange={() => {}} label="Devise" id="t-load" />,
    );
    await user.click(screen.getByRole("button"));
    expect(screen.getByText(/Chargement/)).toBeInTheDocument();
    resolveFn(REFERENCE);
    await waitFor(() => {
      expect(screen.queryByText(/Chargement/)).toBeNull();
    });
  });

  it("shows a safe French error state when the reference API fails", async () => {
    (listCurrencies as unknown as Mock).mockRejectedValue(
      new Error("network down"),
    );
    const user = userEvent.setup();
    render(
      <CurrencySelect value="" onChange={() => {}} label="Devise" id="t-err" />,
    );
    await user.click(screen.getByRole("button"));
    await waitFor(() => {
      expect(
        screen.getByText(/Impossible de charger la liste des devises/),
      ).toBeInTheDocument();
    });
  });

  it("shows the no-results state when the filter matches nothing", async () => {
    const { user } = await renderAndOpen();
    await user.type(
      screen.getByLabelText(/Rechercher une devise/i),
      "qqqxxx",
    );
    await waitFor(() => {
      expect(screen.getByText(/Aucune devise/i)).toBeInTheDocument();
    });
  });
});
