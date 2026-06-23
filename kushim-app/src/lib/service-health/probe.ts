// Same-origin health probe.
//
// Each probe hits the corresponding nginx `/_health/<key>` route, which proxies
// to the service `/ready`. The probe is intentionally minimal and side-effect
// free apart from the network call:
//   - same-origin relative URL (no host, no token);
//   - `cache: "no-store"` so a probe always reflects current state;
//   - an AbortController timeout so a hung request resolves to `unavailable`
//     instead of pinning the caller in `checking` forever;
//   - sanitized result only — the raw upstream body is never returned.

import type { ServiceKey } from "./types";

export const HEALTH_PATHS: Record<ServiceKey, string> = {
  api: "/_health/api",
  auth: "/_health/auth",
  worker: "/_health/worker",
  "market-data": "/_health/market-data",
};

// The only successful readiness values in the current backend contract:
// api/worker/auth return "ok"; market-data returns "ready". Arbitrary JSON or
// 200 HTML must NOT count as operational.
const ACCEPTED_STATUSES = new Set(["ok", "ready"]);

export const DEFAULT_HEALTH_TIMEOUT_MS = 5000;

export type ProbeResult = "operational" | "unavailable";

export type ProbeOptions = {
  timeoutMs?: number;
  signal?: AbortSignal;
  // Injectable for tests; defaults to the global fetch.
  fetchImpl?: typeof fetch;
};

export async function checkHealthEndpoint(
  url: string,
  options: ProbeOptions = {},
): Promise<ProbeResult> {
  const {
    timeoutMs = DEFAULT_HEALTH_TIMEOUT_MS,
    signal,
    fetchImpl = fetch,
  } = options;

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  if (signal) {
    if (signal.aborted) {
      controller.abort();
    } else {
      signal.addEventListener("abort", () => controller.abort(), {
        once: true,
      });
    }
  }

  try {
    const response = await fetchImpl(url, {
      method: "GET",
      cache: "no-store",
      signal: controller.signal,
      headers: { Accept: "application/json" },
    });

    if (!response.ok) {
      return "unavailable";
    }

    let body: unknown;
    try {
      body = await response.json();
    } catch {
      // 200 with a non-JSON body (e.g. an SPA/HTML fallback) is not a valid
      // readiness response.
      return "unavailable";
    }

    const status = (body as { status?: unknown } | null)?.status;
    return typeof status === "string" && ACCEPTED_STATUSES.has(status)
      ? "operational"
      : "unavailable";
  } catch {
    // Timeout, abort or network failure. Never surface the raw error.
    return "unavailable";
  } finally {
    clearTimeout(timer);
  }
}

export async function probeService(
  key: ServiceKey,
  options: ProbeOptions = {},
): Promise<ProbeResult> {
  return checkHealthEndpoint(HEALTH_PATHS[key], options);
}
