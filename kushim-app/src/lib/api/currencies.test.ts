import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { listCurrencies } from "./businessApi";
import { __resetSessionGateForTests, configureSessionGate } from "./sessionGate";
import { writeTokens } from "./tokenStorage";

// Verifies that the frontend uses the single source of truth — the backend
// reference endpoint — for the currency catalogue, with proper bearer-token
// authentication and request shape.

function makeJsonResponse(body: unknown): Response {
  return {
    ok: true,
    status: 200,
    statusText: "HTTP 200",
    json: async () => body,
  } as unknown as Response;
}

describe("listCurrencies", () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    __resetSessionGateForTests();
    window.localStorage.clear();
    fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    writeTokens("access-test", "refresh-test");
    configureSessionGate({
      refresh: async () => ({
        access_token: "access-2",
        refresh_token: "refresh-2",
      }),
      onSessionCleared: () => {},
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    __resetSessionGateForTests();
    window.localStorage.clear();
  });

  it("calls /v1/reference/currencies with the bearer token and returns the catalogue", async () => {
    fetchMock.mockResolvedValueOnce(
      makeJsonResponse({
        data: [
          { value: "EUR", label: "Euro" },
          { value: "USD", label: "US Dollar" },
        ],
      }),
    );

    const items = await listCurrencies();

    expect(items).toHaveLength(2);
    expect(items[0]).toEqual({ value: "EUR", label: "Euro" });
    expect(items[1]).toEqual({ value: "USD", label: "US Dollar" });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toMatch(/\/v1\/reference\/currencies$/);
    const headers = init.headers as Record<string, string>;
    expect(headers.Authorization).toBe("Bearer access-test");
  });

  it("propagates request errors instead of returning a fallback list", async () => {
    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 500,
      statusText: "HTTP 500",
      json: async () => ({
        error: { code: "internal_error", message: "boom" },
      }),
    } as unknown as Response);

    await expect(listCurrencies()).rejects.toBeDefined();
  });
});
