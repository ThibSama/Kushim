// Polling service-health hook.
//
// Owns a single polling loop for a set of services:
//   - one immediate check on mount;
//   - automatic re-check roughly every 30s;
//   - no overlapping requests (a fresh tick is skipped while one is in flight);
//   - manual `refresh()`;
//   - all timers and in-flight requests are torn down on unmount.
//
// The hook never throws: each probe resolves to "operational" | "unavailable".

import { useCallback, useEffect, useRef, useState } from "react";
import { probeService } from "../lib/service-health/probe";
import type { ServiceKey, ServiceState } from "../lib/service-health/types";

export const POLL_INTERVAL_MS = 30_000;

export type ServiceHealthStates = Record<ServiceKey, ServiceState>;

const INITIAL_STATES: ServiceHealthStates = {
  api: "checking",
  auth: "checking",
  worker: "checking",
  "market-data": "checking",
};

export type UseServiceHealthResult = {
  states: ServiceHealthStates;
  lastChecked: Date | null;
  refresh: () => Promise<void>;
};

export function useServiceHealth(
  services: ServiceKey[],
): UseServiceHealthResult {
  const [states, setStates] = useState<ServiceHealthStates>(INITIAL_STATES);
  const [lastChecked, setLastChecked] = useState<Date | null>(null);

  const inFlightRef = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  const mountedRef = useRef(true);

  // Keep the target list in a ref so the polling callback is stable and a
  // changing array identity never restarts the interval. Synced in an effect
  // (not during render) so the latest list is read at the next probe tick.
  const servicesRef = useRef(services);
  useEffect(() => {
    servicesRef.current = services;
  }, [services]);

  const runCheck = useCallback(async () => {
    if (inFlightRef.current) return;
    inFlightRef.current = true;

    const controller = new AbortController();
    abortRef.current = controller;
    const targets = servicesRef.current;

    try {
      const results = await Promise.all(
        targets.map(async (key) => {
          const result = await probeService(key, { signal: controller.signal });
          return [key, result] as const;
        }),
      );

      if (!mountedRef.current || controller.signal.aborted) return;

      setStates((prev) => {
        const next = { ...prev };
        for (const [key, result] of results) {
          next[key] = result;
        }
        return next;
      });
      setLastChecked(new Date());
    } finally {
      inFlightRef.current = false;
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    void runCheck();
    const intervalId = setInterval(() => {
      void runCheck();
    }, POLL_INTERVAL_MS);

    return () => {
      mountedRef.current = false;
      clearInterval(intervalId);
      abortRef.current?.abort();
    };
  }, [runCheck]);

  return { states, lastChecked, refresh: runCheck };
}
