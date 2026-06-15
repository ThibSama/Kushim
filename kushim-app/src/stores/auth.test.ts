import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The auth store wires the session gate at module load. We need stable mocks
// for the public auth API endpoints (refresh/logout/getCurrentUser) and the
// raw fetch used by the centralized authenticated wrapper.
vi.mock("../lib/api/authApi", () => ({
  getCurrentUser: vi.fn(),
  logoutSession: vi.fn(),
  refreshSession: vi.fn(),
}));

import { authenticatedRequest } from "../lib/api/authenticatedRequest";
import { ApiRequestError } from "../lib/api/httpClient";
import * as authApi from "../lib/api/authApi";
import { writeTokens, readAccessToken } from "../lib/api/tokenStorage";
import { useAuthStore } from "./auth";

function makeResponse(status: number, body?: unknown): Response {
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

describe("auth store ↔ sessionGate token synchronization", () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    window.localStorage.clear();
    fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    // Seed a session and reset the store to a known state.
    writeTokens("access-old", "refresh-old");
    useAuthStore.setState({
      token: "access-old",
      user: null,
      sessionStatus: "authenticated",
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetAllMocks();
    window.localStorage.clear();
  });

  it("rotates Zustand token after a successful single-flight refresh", async () => {
    vi.mocked(authApi.refreshSession).mockResolvedValueOnce({
      access_token: "access-new",
      refresh_token: "refresh-new",
      access_token_expires_at: "",
      refresh_token_expires_at: "",
    });

    fetchMock
      .mockResolvedValueOnce(makeResponse(401))
      .mockResolvedValueOnce(makeResponse(200, { ok: true }));

    await authenticatedRequest("http://api", "/v1/me");

    // localStorage, tokenStorage helper and the Zustand store must all show
    // the rotated access token.
    expect(readAccessToken()).toBe("access-new");
    expect(window.localStorage.getItem("kushim_access_token")).toBe("access-new");
    expect(useAuthStore.getState().token).toBe("access-new");
  });

  it("does not update Zustand token when the refresh fails", async () => {
    vi.mocked(authApi.refreshSession).mockRejectedValueOnce(
      new ApiRequestError({
        code: "invalid_refresh_token",
        message: "x",
        status: 401,
      }),
    );

    fetchMock.mockResolvedValueOnce(makeResponse(401));

    await expect(
      authenticatedRequest("http://api", "/v1/me"),
    ).rejects.toBeInstanceOf(ApiRequestError);

    // Session cleared: stored tokens gone, store reset to unauthenticated,
    // and certainly no rotated token resurrected.
    expect(readAccessToken()).toBeNull();
    expect(useAuthStore.getState().token).toBeNull();
    expect(useAuthStore.getState().sessionStatus).toBe("unauthenticated");
  });
});
