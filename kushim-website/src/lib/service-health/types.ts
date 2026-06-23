// Service-health types for the public status page.

// Backend services probed via the same-origin `/_health/<key>` nginx routes.
export type BackendServiceKey = "api" | "auth" | "worker" | "market-data";

// `operational` requires a 2xx response before timeout whose body matches the
// readiness contract (`status` ∈ {"ok","ready"}). Everything else is
// `unavailable`. `checking` is the pre-first-result state only.
export type ServiceState = "checking" | "operational" | "unavailable";

// Page-level rollup. `incident` = API or auth down (major outage);
// `degraded` = only worker/market-data down; `checking` = first probe pending.
export type AggregateStatus = "checking" | "operational" | "degraded" | "incident";

export const BACKEND_SERVICE_KEYS: BackendServiceKey[] = [
  "api",
  "auth",
  "worker",
  "market-data",
];
