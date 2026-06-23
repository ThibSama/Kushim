// Pure aggregate-status logic for the public status page. Kept free of React and
// the network so it is unit-testable in isolation (kushim-website currently has
// no test runner — see the task notes — but this module is deliberately pure so
// coverage can be added without restructuring).

import type {
  AggregateStatus,
  BackendServiceKey,
  ServiceState,
} from "./types";

export type BackendStates = Record<BackendServiceKey, ServiceState>;

// Aggregate rules:
//   - any backend service still in its first probe        → "checking"
//   - API or auth unavailable                             → "incident"
//   - API & auth operational, worker/market-data down     → "degraded"
//   - everything operational                              → "operational"
// The website itself is implicitly operational (the page loaded) and is not part
// of this rollup.
export function computeAggregateStatus(states: BackendStates): AggregateStatus {
  const all: ServiceState[] = [
    states.api,
    states.auth,
    states.worker,
    states["market-data"],
  ];

  if (all.some((state) => state === "checking")) {
    return "checking";
  }
  if (states.api === "unavailable" || states.auth === "unavailable") {
    return "incident";
  }
  if (
    states.worker === "unavailable" ||
    states["market-data"] === "unavailable"
  ) {
    return "degraded";
  }
  return "operational";
}

export const AGGREGATE_LABELS: Record<AggregateStatus, string> = {
  checking: "Vérification des services",
  operational: "Tous les services sont opérationnels",
  degraded: "Services dégradés",
  incident: "Incident en cours",
};

export const SERVICE_STATE_LABELS: Record<ServiceState, string> = {
  checking: "Vérification…",
  operational: "Opérationnel",
  unavailable: "Indisponible",
};
