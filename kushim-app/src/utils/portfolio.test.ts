import { describe, expect, it } from "vitest";
import { formatCurrency, formatSignedCurrency } from "./portfolio";

// Regression: `formatCurrency` used to default to EUR when no currency was
// passed, so a USD portfolio's Transactions tiles and recent-transactions row
// rendered "1 234,56 €" while the portfolio was unmistakably in USD. The
// formatter is now strict: callers must supply the real ISO code, and a
// missing/empty currency renders a plain localized number — never a wrong
// symbol.
describe("formatCurrency", () => {
  it("renders the requested ISO currency symbol — USD", () => {
    const out = formatCurrency(1952.3, "USD");
    expect(out).toContain("$");
    expect(out).not.toContain("€");
    // The French locale uses a narrow no-break space (U+202F) as the thousands
    // separator on modern Node/ICU, so match the digits with a tolerant regex.
    expect(out).toMatch(/1.952,30/);
  });

  it("renders the requested ISO currency symbol — EUR", () => {
    const out = formatCurrency(1234.56, "EUR");
    expect(out).toContain("€");
    expect(out).not.toContain("$");
  });

  it("renders the requested ISO currency symbol — GBP (forward compatible)", () => {
    const out = formatCurrency(99, "GBP");
    expect(out).toContain("£");
  });

  it("renders a plain localized number without a symbol when currency is null", () => {
    const out = formatCurrency(1952.3, null);
    expect(out).not.toContain("$");
    expect(out).not.toContain("€");
    expect(out).not.toContain("£");
    // The French locale uses a narrow no-break space (U+202F) as the thousands
    // separator on modern Node/ICU, so match the digits with a tolerant regex.
    expect(out).toMatch(/1.952,30/);
  });

  it("renders a plain localized number without a symbol when currency is empty", () => {
    const out = formatCurrency(0, "");
    expect(out).not.toContain("$");
    expect(out).not.toContain("€");
  });
});

describe("formatSignedCurrency", () => {
  it("prepends '+' for positive amounts and respects the currency", () => {
    const out = formatSignedCurrency(100, "USD");
    expect(out.startsWith("+")).toBe(true);
    expect(out).toContain("$");
    expect(out).not.toContain("€");
  });

  it("prepends '-' for negative amounts and respects the currency", () => {
    const out = formatSignedCurrency(-100, "USD");
    expect(out.startsWith("-")).toBe(true);
    expect(out).toContain("$");
  });

  it("returns a bare formatted value for zero", () => {
    const out = formatSignedCurrency(0, "USD");
    expect(out.startsWith("+")).toBe(false);
    expect(out.startsWith("-")).toBe(false);
  });
});
