import { type ReactNode } from "react";
import { useServiceHealth } from "../../hooks/useServiceHealth";
import { ServiceUnavailable } from "./ServiceUnavailable";
import { ServiceDegradedBanners } from "./ServiceDegradedBanners";

// Global private-app service gate. Runs AFTER session validation (mounted inside
// RequireAuth), so business-API readiness is never part of authentication and a
// down API never logs the user out.
//
// - api === "checking": brief, bounded probe state (resolves within the probe
//   timeout — never an infinite loader).
// - api === "unavailable": blocking fallback in place of the page; tokens and
//   session are left intact; `refresh` re-probes without a full reload.
// - api === "operational": the protected page mounts and loads normally, with
//   non-blocking worker / market-data degraded banners layered on top.
//
// Auth is intentionally NOT probed here — auth readiness is the website/login
// concern, and the public /health page covers it.
export function ServiceGate({ children }: { children: ReactNode }) {
  const { states, refresh } = useServiceHealth([
    "api",
    "worker",
    "market-data",
  ]);

  if (states.api === "checking") {
    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          minHeight: "60vh",
          color: "var(--text-secondary)",
          fontSize: "15px",
        }}
      >
        Vérification des services…
      </div>
    );
  }

  if (states.api === "unavailable") {
    return <ServiceUnavailable onRetry={() => void refresh()} />;
  }

  return (
    <>
      <ServiceDegradedBanners
        worker={states.worker}
        marketData={states["market-data"]}
      />
      {children}
    </>
  );
}
