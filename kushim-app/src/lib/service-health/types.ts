// Service-health domain types shared by the probe, the polling hook and the
// private-app service gate.

// The four backend services the app probes through the same-origin
// `/_health/<key>` nginx routes. The key doubles as the URL path segment.
export type ServiceKey = "api" | "auth" | "worker" | "market-data";

// A health endpoint is `operational` ONLY when the request completed before the
// timeout, returned 2xx, and the body matched the documented readiness contract
// (`status` ∈ {"ok","ready"}). Anything else — timeout, non-2xx, HTML, unknown
// JSON — is `unavailable`. `checking` is the pre-first-result state only.
export type ServiceState = "checking" | "operational" | "unavailable";

export const ALL_SERVICE_KEYS: ServiceKey[] = [
  "api",
  "auth",
  "worker",
  "market-data",
];
