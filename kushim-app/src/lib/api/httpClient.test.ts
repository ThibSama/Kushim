import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  ApiRequestError,
  TIMEOUT_ERROR_CODE,
  apiRequest,
  isTimeoutError,
  isUnauthorized,
} from "./httpClient";
import { authenticatedRequest } from "./authenticatedRequest";
import {
  __resetSessionGateForTests,
  configureSessionGate,
} from "./sessionGate";
import { readAccessToken, readRefreshToken, writeTokens } from "./tokenStorage";

// A fetch that never resolves on its own and only rejects once its abort signal
// fires — i.e. it models a request that would hang without the timeout.
function hangingFetch() {
  return vi.fn(
    (_url: string, init?: RequestInit) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () =>
          reject(new DOMException("Aborted", "AbortError")),
        );
      }),
  ) as unknown as typeof fetch;
}

describe("apiRequest timeout", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("produces a stable, sanitized timeout error (status 0, not a 401)", async () => {
    vi.stubGlobal("fetch", hangingFetch());

    const error = await apiRequest("http://api", "/v1/me", {
      timeoutMs: 10,
    }).then(
      () => null,
      (e) => e,
    );

    expect(error).toBeInstanceOf(ApiRequestError);
    expect(error.code).toBe(TIMEOUT_ERROR_CODE);
    expect(error.status).toBe(0);
    expect(isTimeoutError(error)).toBe(true);
    // Critically: a timeout must never look like an auth failure.
    expect(isUnauthorized(error)).toBe(false);
  });
});

describe("authenticatedRequest timeout does not touch the session", () => {
  let refreshCalls: number;
  let clearedReasons: string[];

  beforeEach(() => {
    __resetSessionGateForTests();
    window.localStorage.clear();
    refreshCalls = 0;
    clearedReasons = [];
    configureSessionGate({
      refresh: async (token) => {
        refreshCalls += 1;
        return {
          access_token: "rotated",
          refresh_token: "rotated-refresh",
          // token argument intentionally unused beyond the spy
          ...(token ? {} : {}),
        };
      },
      onSessionCleared: (reason) => {
        clearedReasons.push(reason);
      },
    });
    writeTokens("access-1", "refresh-1");
    vi.stubGlobal("fetch", hangingFetch());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    window.localStorage.clear();
  });

  it("surfaces the timeout without refreshing or clearing the session", async () => {
    const error = await authenticatedRequest("http://api", "/v1/me", {
      timeoutMs: 10,
    }).then(
      () => null,
      (e) => e,
    );

    expect(isTimeoutError(error)).toBe(true);
    // No refresh attempt, no session clear, tokens intact.
    expect(refreshCalls).toBe(0);
    expect(clearedReasons).toEqual([]);
    expect(readAccessToken()).toBe("access-1");
    expect(readRefreshToken()).toBe("refresh-1");
  });
});
