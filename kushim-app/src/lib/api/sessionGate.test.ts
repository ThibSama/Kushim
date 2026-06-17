import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiRequestError } from "./httpClient";
import {
  __resetSessionGateForTests,
  bumpSessionGeneration,
  clearSession,
  configureSessionGate,
  currentSessionGeneration,
} from "./sessionGate";
import { authenticatedRequest } from "./authenticatedRequest";
import {
  TOKEN_KEYS,
  clearStoredTokens,
  readAccessToken,
  readRefreshToken,
  writeTokens,
} from "./tokenStorage";

type FetchResponseInit = {
  status?: number;
  body?: unknown;
};

function makeResponse({ status = 200, body }: FetchResponseInit): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: `HTTP ${status}`,
    json: async () =>
      body ??
      (status >= 400
        ? { error: { code: "unauthorized", message: "Unauthorized" } }
        : {}),
  } as unknown as Response;
}

describe("sessionGate + authenticatedRequest", () => {
  let fetchMock: ReturnType<typeof vi.fn>;
  let clearedReasons: string[];
  let refreshCalls: number;

  beforeEach(() => {
    __resetSessionGateForTests();
    window.localStorage.clear();
    fetchMock = vi.fn();
    clearedReasons = [];
    refreshCalls = 0;
    vi.stubGlobal("fetch", fetchMock);
    writeTokens("access-1", "refresh-1");

    configureSessionGate({
      refresh: async (refreshToken) => {
        refreshCalls += 1;
        if (refreshToken === "refresh-fail") {
          throw new ApiRequestError({
            code: "invalid_refresh_token",
            message: "Invalid refresh token",
            status: 401,
          });
        }
        return {
          access_token: `access-${refreshCalls + 1}`,
          refresh_token: `refresh-${refreshCalls + 1}`,
        };
      },
      onSessionCleared: (reason) => {
        clearedReasons.push(reason);
      },
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    __resetSessionGateForTests();
    window.localStorage.clear();
  });

  it("token storage round-trips access and refresh tokens", () => {
    expect(readAccessToken()).toBe("access-1");
    expect(readRefreshToken()).toBe("refresh-1");
    writeTokens("a2", "r2");
    expect(readAccessToken()).toBe("a2");
    expect(readRefreshToken()).toBe("r2");
    expect(window.localStorage.getItem(TOKEN_KEYS.LEGACY)).toBeNull();
    clearStoredTokens();
    expect(readAccessToken()).toBeNull();
    expect(readRefreshToken()).toBeNull();
  });

  it("legacy token key is preserved on read but cleared on write", () => {
    window.localStorage.clear();
    window.localStorage.setItem(TOKEN_KEYS.LEGACY, "legacy-1");
    expect(readAccessToken()).toBe("legacy-1");
    writeTokens("a2", "r2");
    expect(window.localStorage.getItem(TOKEN_KEYS.LEGACY)).toBeNull();
  });

  it("successful authenticated request returns the JSON body", async () => {
    fetchMock.mockResolvedValueOnce(makeResponse({ body: { ok: true } }));
    const result = await authenticatedRequest<{ ok: boolean }>(
      "http://api",
      "/v1/me",
    );
    expect(result).toEqual({ ok: true });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const req = fetchMock.mock.calls[0];
    expect(req[1].headers["Authorization"]).toBe("Bearer access-1");
  });

  it("preserves caller-provided Idempotency-Key across a 401 refresh retry (P3)", async () => {
    fetchMock
      .mockResolvedValueOnce(makeResponse({ status: 401 }))
      .mockResolvedValueOnce(makeResponse({ body: { ok: true } }));

    const key = "00000000-0000-4000-8000-000000000099";
    await authenticatedRequest<{ ok: boolean }>("http://api", "/v1/x", {
      method: "POST",
      body: { foo: 1 },
      headers: { "Idempotency-Key": key },
    });

    expect(fetchMock).toHaveBeenCalledTimes(2);
    const firstHeaders = fetchMock.mock.calls[0][1].headers;
    const secondHeaders = fetchMock.mock.calls[1][1].headers;
    // Same key on the original AND on the post-refresh retry — backend
    // sees the same logical attempt and replays the original write.
    expect(firstHeaders["Idempotency-Key"]).toBe(key);
    expect(secondHeaders["Idempotency-Key"]).toBe(key);
    // Authorization rotated.
    expect(firstHeaders["Authorization"]).toBe("Bearer access-1");
    expect(secondHeaders["Authorization"]).toBe("Bearer access-2");
  });

  it("401 triggers a refresh then retries the original request with the new token", async () => {
    fetchMock
      .mockResolvedValueOnce(makeResponse({ status: 401 }))
      .mockResolvedValueOnce(makeResponse({ body: { ok: true } }));

    const result = await authenticatedRequest<{ ok: boolean }>(
      "http://api",
      "/v1/me",
    );

    expect(result).toEqual({ ok: true });
    expect(refreshCalls).toBe(1);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[1][1].headers["Authorization"]).toBe(
      "Bearer access-2",
    );
    expect(readAccessToken()).toBe("access-2");
    expect(readRefreshToken()).toBe("refresh-2");
  });

  it("three concurrent 401s share exactly one refresh call", async () => {
    // 3 initial 401s, then 3 retried 200s (queue is FIFO via mockResolvedValueOnce).
    fetchMock
      .mockResolvedValueOnce(makeResponse({ status: 401 }))
      .mockResolvedValueOnce(makeResponse({ status: 401 }))
      .mockResolvedValueOnce(makeResponse({ status: 401 }))
      .mockResolvedValueOnce(makeResponse({ body: { id: 1 } }))
      .mockResolvedValueOnce(makeResponse({ body: { id: 2 } }))
      .mockResolvedValueOnce(makeResponse({ body: { id: 3 } }));

    const [a, b, c] = await Promise.all([
      authenticatedRequest<{ id: number }>("http://api", "/v1/a"),
      authenticatedRequest<{ id: number }>("http://api", "/v1/b"),
      authenticatedRequest<{ id: number }>("http://api", "/v1/c"),
    ]);

    expect(refreshCalls).toBe(1);
    expect([a.id, b.id, c.id].sort()).toEqual([1, 2, 3]);
    // All three retries use the rotated token.
    const retried = fetchMock.mock.calls.slice(3);
    for (const call of retried) {
      expect(call[1].headers["Authorization"]).toBe("Bearer access-2");
    }
  });

  it("a second 401 after refresh clears the session and propagates without a second refresh", async () => {
    fetchMock
      .mockResolvedValueOnce(makeResponse({ status: 401 }))
      .mockResolvedValueOnce(makeResponse({ status: 401 }));

    await expect(
      authenticatedRequest("http://api", "/v1/me"),
    ).rejects.toBeInstanceOf(ApiRequestError);

    expect(refreshCalls).toBe(1);
    expect(clearedReasons).toEqual(["retry_unauthorized"]);
    expect(readAccessToken()).toBeNull();
  });

  it("refresh failure clears the session", async () => {
    writeTokens("access-1", "refresh-fail");
    fetchMock.mockResolvedValueOnce(makeResponse({ status: 401 }));

    await expect(
      authenticatedRequest("http://api", "/v1/me"),
    ).rejects.toBeInstanceOf(ApiRequestError);

    expect(refreshCalls).toBe(1);
    expect(clearedReasons).toEqual(["refresh_failed"]);
    expect(readAccessToken()).toBeNull();
    expect(readRefreshToken()).toBeNull();
  });

  it("logout during in-flight refresh prevents late re-authentication", async () => {
    let resolveRefresh: (v: { access_token: string; refresh_token: string }) => void = () => {};
    let refreshStartedCount = 0;
    const refreshStarted = new Promise<void>((resolve) => {
      configureSessionGate({
        refresh: () =>
          new Promise((res) => {
            refreshStartedCount += 1;
            resolveRefresh = res;
            resolve();
          }),
        onSessionCleared: (reason) => {
          clearedReasons.push(reason);
        },
      });
    });

    fetchMock.mockResolvedValueOnce(makeResponse({ status: 401 }));
    const inflight = authenticatedRequest("http://api", "/v1/me");

    // Wait until refresh has actually been invoked, then log out mid-flight.
    await refreshStarted;
    expect(refreshStartedCount).toBe(1);
    clearSession("logout");

    // A late successful refresh now resolves — but must NOT recreate session.
    resolveRefresh({ access_token: "late", refresh_token: "late" });

    await expect(inflight).rejects.toBeInstanceOf(ApiRequestError);
    expect(readAccessToken()).toBeNull();
    expect(clearedReasons).toContain("logout");
  });

  it("bumpSessionGeneration is monotonic and resets in-flight refresh", () => {
    const before = currentSessionGeneration();
    const after = bumpSessionGeneration();
    expect(after).toBeGreaterThan(before);
  });

  it("preserves method, body, and caller headers on retry", async () => {
    fetchMock
      .mockResolvedValueOnce(makeResponse({ status: 401 }))
      .mockResolvedValueOnce(makeResponse({ body: { ok: true } }));

    await authenticatedRequest("http://api", "/v1/portfolios", {
      method: "POST",
      body: { name: "wallet" },
      headers: { "X-Trace": "abc" },
    });

    expect(fetchMock).toHaveBeenCalledTimes(2);
    const original = fetchMock.mock.calls[0][1];
    const retried = fetchMock.mock.calls[1][1];
    expect(original.method).toBe("POST");
    expect(retried.method).toBe("POST");
    expect(retried.body).toBe(JSON.stringify({ name: "wallet" }));
    expect(retried.headers["X-Trace"]).toBe("abc");
    expect(retried.headers["Authorization"]).toBe("Bearer access-2");
  });

  it("onTokensRotated fires with the new access token after a successful refresh", async () => {
    const rotated: string[] = [];
    configureSessionGate({
      refresh: async () => {
        refreshCalls += 1;
        return {
          access_token: `rotated-${refreshCalls}`,
          refresh_token: `refresh-${refreshCalls + 1}`,
        };
      },
      onSessionCleared: (reason) => clearedReasons.push(reason),
      onTokensRotated: (token) => rotated.push(token),
    });

    fetchMock
      .mockResolvedValueOnce(makeResponse({ status: 401 }))
      .mockResolvedValueOnce(makeResponse({ body: { ok: true } }));

    await authenticatedRequest("http://api", "/v1/me");

    expect(rotated).toEqual(["rotated-1"]);
    // localStorage AND the rotated handler agree on the same token.
    expect(readAccessToken()).toBe("rotated-1");
  });

  it("onTokensRotated does not fire when logout races the refresh response", async () => {
    const rotated: string[] = [];
    let resolveRefresh: (v: { access_token: string; refresh_token: string }) => void = () => {};
    const refreshStarted = new Promise<void>((resolve) => {
      configureSessionGate({
        refresh: () =>
          new Promise((res) => {
            resolveRefresh = res;
            resolve();
          }),
        onSessionCleared: (reason) => clearedReasons.push(reason),
        onTokensRotated: (token) => rotated.push(token),
      });
    });

    fetchMock.mockResolvedValueOnce(makeResponse({ status: 401 }));
    const inflight = authenticatedRequest("http://api", "/v1/me");

    await refreshStarted;
    clearSession("logout");
    resolveRefresh({ access_token: "late", refresh_token: "late" });

    await expect(inflight).rejects.toBeInstanceOf(ApiRequestError);
    expect(rotated).toEqual([]);
    expect(readAccessToken()).toBeNull();
  });

  it("non-401 errors are not retried", async () => {
    fetchMock.mockResolvedValueOnce(makeResponse({ status: 500 }));
    await expect(authenticatedRequest("http://api", "/x")).rejects.toBeInstanceOf(
      ApiRequestError,
    );
    expect(refreshCalls).toBe(0);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
