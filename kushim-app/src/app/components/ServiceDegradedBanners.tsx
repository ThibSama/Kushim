import { AlertTriangle } from "lucide-react";
import type { ServiceState } from "../../lib/service-health/types";

// Non-blocking degraded-service warnings, shown above the page content while the
// API is operational but a background service is not. Yellow (warning) only —
// never red — because the application stays fully usable. When both services are
// degraded, two compact banners are shown (each reason stays understandable; no
// generic duplicate). Existing valuation/unavailable states are untouched: these
// banners only explain, they never alter data.

const WORKER_MESSAGE =
  "Le traitement des portefeuilles est temporairement retardé. Les opérations récentes peuvent mettre plus de temps à apparaître.";
const MARKET_DATA_MESSAGE =
  "Les données de marché ne sont pas actualisées actuellement. Certains prix et calculs peuvent être obsolètes.";

function WarningBanner({ testId, text }: { testId: string; text: string }) {
  return (
    <div
      role="status"
      aria-live="polite"
      data-testid={testId}
      className="rounded-lg flex items-start gap-3"
      style={{
        padding: "12px 16px",
        border: "1px solid rgba(245, 158, 11, 0.30)",
        background: "rgba(245, 158, 11, 0.10)",
        backdropFilter: "blur(8px)",
        WebkitBackdropFilter: "blur(8px)",
      }}
    >
      <AlertTriangle
        size={18}
        aria-hidden="true"
        style={{ color: "var(--color-warning)", marginTop: "1px", flexShrink: 0 }}
      />
      <div
        style={{
          fontSize: "14px",
          fontWeight: 500,
          color: "var(--text-primary)",
          lineHeight: 1.5,
        }}
      >
        {text}
      </div>
    </div>
  );
}

export function ServiceDegradedBanners({
  worker,
  marketData,
}: {
  worker: ServiceState;
  marketData: ServiceState;
}) {
  const showWorker = worker === "unavailable";
  const showMarketData = marketData === "unavailable";

  if (!showWorker && !showMarketData) {
    return null;
  }

  return (
    <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 pt-6 flex flex-col gap-3">
      {showWorker && (
        <WarningBanner testId="degraded-worker" text={WORKER_MESSAGE} />
      )}
      {showMarketData && (
        <WarningBanner
          testId="degraded-market-data"
          text={MARKET_DATA_MESSAGE}
        />
      )}
    </div>
  );
}
