// Same-origin health probe for the public status page. Mirrors the kushim-app
// probe: relative URL, no token, no-store, AbortController timeout, sanitized
// result only (the raw upstream body is never returned to the UI).

import type { BackendServiceKey } from "./types";

export const HEALTH_PATHS: Record<BackendServiceKey, string> = {
  api: "/_health/api",
  auth: "/_health/auth",
  worker: "/_health/worker",
  "market-data": "/_health/market-data",
};

// api/auth/worker return "ok"; market-data returns "ready". Nothing else counts.
const ACCEPTED_STATUSES = new Set(["ok", "ready"]);

export const DEFAULT_HEALTH_TIMEOUT_MS = 5000;

export type ProbeResult = "operational" | "unavailable";

export type ProbeOptions = {
  timeoutMs?: number;
  signal?: AbortSignal;
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
      return "unavailable";
    }

    const status = (body as { status?: unknown } | null)?.status;
    return typeof status === "string" && ACCEPTED_STATUSES.has(status)
      ? "operational"
      : "unavailable";
  } catch {
    return "unavailable";
  } finally {
    clearTimeout(timer);
  }
}

export async function probeService(
  key: BackendServiceKey,
  options: ProbeOptions = {},
): Promise<ProbeResult> {
  return checkHealthEndpoint(HEALTH_PATHS[key], options);
}
