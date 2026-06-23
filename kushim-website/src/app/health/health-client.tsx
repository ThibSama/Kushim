"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Activity, RefreshCw } from "lucide-react";
import { Card } from "@/mockup/components/Card";
import { Badge } from "@/mockup/components/Badge";
import { Button } from "@/mockup/components/Button";
import { probeService } from "@/lib/service-health/probe";
import {
  AGGREGATE_LABELS,
  SERVICE_STATE_LABELS,
  computeAggregateStatus,
  type BackendStates,
} from "@/lib/service-health/aggregate";
import type {
  AggregateStatus,
  BackendServiceKey,
  ServiceState,
} from "@/lib/service-health/types";

const POLL_INTERVAL_MS = 30_000;

const INITIAL_STATES: BackendStates = {
  api: "checking",
  auth: "checking",
  worker: "checking",
  "market-data": "checking",
};

// Single polling loop: one immediate probe, re-probe every ~30s, no overlapping
// requests, and full teardown (timer + in-flight abort) on unmount.
function useHealthStatuses() {
  const [states, setStates] = useState<BackendStates>(INITIAL_STATES);
  const [lastChecked, setLastChecked] = useState<Date | null>(null);

  const inFlightRef = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  const mountedRef = useRef(true);

  const refresh = useCallback(async () => {
    if (inFlightRef.current) return;
    inFlightRef.current = true;

    const controller = new AbortController();
    abortRef.current = controller;
    const keys: BackendServiceKey[] = ["api", "auth", "worker", "market-data"];

    try {
      const results = await Promise.all(
        keys.map(async (key) => {
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
    void refresh();
    const intervalId = setInterval(() => {
      void refresh();
    }, POLL_INTERVAL_MS);

    return () => {
      mountedRef.current = false;
      clearInterval(intervalId);
      abortRef.current?.abort();
    };
  }, [refresh]);

  return { states, lastChecked, refresh };
}

type Row = { key: string; label: string; state: ServiceState };

function stateBadgeVariant(state: ServiceState): "gain" | "loss" | "neutral" {
  if (state === "operational") return "gain";
  if (state === "unavailable") return "loss";
  return "neutral";
}

function aggregateBadgeVariant(
  status: AggregateStatus,
): "gain" | "loss" | "warning" | "neutral" {
  switch (status) {
    case "operational":
      return "gain";
    case "incident":
      return "loss";
    case "degraded":
      return "warning";
    default:
      return "neutral";
  }
}

function formatLastChecked(date: Date | null): string {
  if (!date) return "—";
  return new Intl.DateTimeFormat("fr-FR", {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(date);
}

export function HealthClient() {
  const { states, lastChecked, refresh } = useHealthStatuses();
  const aggregate = computeAggregateStatus(states);

  // The website row is operational because this very page rendered and ran.
  const rows: Row[] = [
    { key: "site", label: "Site Kushim", state: "operational" },
    { key: "auth", label: "Authentification", state: states.auth },
    { key: "api", label: "API métier", state: states.api },
    { key: "worker", label: "Traitement des portefeuilles", state: states.worker },
    { key: "market-data", label: "Données de marché", state: states["market-data"] },
  ];

  return (
    <section className="px-4 sm:px-6 py-20 sm:py-28">
      <div className="mx-auto max-w-[760px]">
        <div className="flex items-center gap-3 mb-2">
          <Activity size={22} aria-hidden="true" style={{ color: "var(--text-secondary)" }} />
          <h1
            style={{
              color: "var(--text-primary)",
              fontSize: "clamp(26px, 5vw, 38px)",
              fontWeight: 800,
              lineHeight: 1.15,
            }}
          >
            État des services Kushim
          </h1>
        </div>
        <p
          className="mb-8"
          style={{ color: "var(--text-secondary)", fontSize: "clamp(14px, 2.5vw, 16px)", lineHeight: 1.6 }}
        >
          Disponibilité en temps réel des services Kushim. Cette page interroge
          uniquement l’état de disponibilité (readiness) de chaque service.
        </p>

        {/* Aggregate status — announced politely to assistive tech. */}
        <Card level={1} className="mb-6">
          <div
            aria-live="polite"
            className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4"
          >
            <div className="flex items-center gap-3">
              <span
                aria-hidden="true"
                className="rounded-full"
                style={{
                  width: "12px",
                  height: "12px",
                  flexShrink: 0,
                  background:
                    aggregate === "operational"
                      ? "var(--color-gain)"
                      : aggregate === "incident"
                        ? "var(--color-loss)"
                        : aggregate === "degraded"
                          ? "var(--color-warning)"
                          : "var(--color-neutral)",
                }}
              />
              <span
                style={{
                  color: "var(--text-primary)",
                  fontSize: "clamp(16px, 3vw, 19px)",
                  fontWeight: 700,
                }}
              >
                {AGGREGATE_LABELS[aggregate]}
              </span>
            </div>
            <Badge variant={aggregateBadgeVariant(aggregate)}>
              {AGGREGATE_LABELS[aggregate]}
            </Badge>
          </div>
        </Card>

        {/* Per-service rows. */}
        <Card level={1} noPadding>
          <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
            {rows.map((row, index) => (
              <li
                key={row.key}
                className="flex items-center justify-between gap-3"
                style={{
                  padding: "clamp(14px, 2.5vw, 18px) clamp(16px, 3vw, 22px)",
                  borderTop: index === 0 ? "none" : "1px solid var(--surface-1-border)",
                }}
              >
                <span style={{ color: "var(--text-primary)", fontSize: "clamp(14px, 2.5vw, 16px)", fontWeight: 500 }}>
                  {row.label}
                </span>
                <Badge variant={stateBadgeVariant(row.state)}>
                  {SERVICE_STATE_LABELS[row.state]}
                </Badge>
              </li>
            ))}
          </ul>
        </Card>

        <div className="mt-6 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
          <p style={{ color: "var(--text-secondary)", fontSize: "clamp(12px, 2vw, 13px)" }}>
            Dernière vérification : <span suppressHydrationWarning>{formatLastChecked(lastChecked)}</span>
          </p>
          <Button variant="secondary" icon={RefreshCw} onClick={() => void refresh()}>
            Actualiser
          </Button>
        </div>
      </div>
    </section>
  );
}
