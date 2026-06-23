import { describe, expect, it, vi } from "vitest";
import { checkHealthEndpoint, probeService } from "./probe";

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response;
}

function htmlResponse(): Response {
  return {
    ok: true,
    status: 200,
    json: async () => {
      throw new SyntaxError("Unexpected token < in JSON");
    },
  } as unknown as Response;
}

describe("checkHealthEndpoint", () => {
  it('treats {status:"ok"} 2xx as operational', async () => {
    const fetchImpl = vi.fn(async () => jsonResponse(200, { status: "ok" }));
    await expect(
      checkHealthEndpoint("/_health/api", { fetchImpl }),
    ).resolves.toBe("operational");
  });

  it('treats {status:"ready"} 2xx (market-data) as operational', async () => {
    const fetchImpl = vi.fn(async () => jsonResponse(200, { status: "ready" }));
    await expect(
      checkHealthEndpoint("/_health/market-data", { fetchImpl }),
    ).resolves.toBe("operational");
  });

  it("rejects unknown JSON status as unavailable", async () => {
    const fetchImpl = vi.fn(async () =>
      jsonResponse(200, { status: "degraded" }),
    );
    await expect(
      checkHealthEndpoint("/_health/api", { fetchImpl }),
    ).resolves.toBe("unavailable");
  });

  it("rejects 200 HTML (non-JSON body) as unavailable", async () => {
    const fetchImpl = vi.fn(async () => htmlResponse());
    await expect(
      checkHealthEndpoint("/_health/api", { fetchImpl }),
    ).resolves.toBe("unavailable");
  });

  it("treats a non-2xx response as unavailable", async () => {
    const fetchImpl = vi.fn(async () => jsonResponse(503, { error: "nope" }));
    await expect(
      checkHealthEndpoint("/_health/api", { fetchImpl }),
    ).resolves.toBe("unavailable");
  });

  it("resolves to unavailable (never hangs) on timeout", async () => {
    // fetch never resolves on its own; it only rejects when aborted.
    const fetchImpl = vi.fn(
      (_url: string, init?: RequestInit) =>
        new Promise<Response>((_resolve, reject) => {
          init?.signal?.addEventListener("abort", () =>
            reject(new DOMException("Aborted", "AbortError")),
          );
        }),
    ) as unknown as typeof fetch;

    await expect(
      checkHealthEndpoint("/_health/api", { fetchImpl, timeoutMs: 10 }),
    ).resolves.toBe("unavailable");
  });

  it("sends a same-origin, no-store, token-less request", async () => {
    const fetchImpl = vi.fn((_url: string, _init?: RequestInit) =>
      Promise.resolve(jsonResponse(200, { status: "ok" })),
    );
    await probeService("worker", {
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      "/_health/worker",
      expect.objectContaining({ method: "GET", cache: "no-store" }),
    );
    const init = fetchImpl.mock.calls[0][1];
    expect(init).toBeDefined();
    expect(init?.headers).not.toHaveProperty("Authorization");
  });
});
