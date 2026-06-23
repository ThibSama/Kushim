import { RotateCcw, Activity, ServerCrash } from "lucide-react";
import { Button } from "./Button";
import { Card } from "./Card";
import { getPublicHealthUrl } from "../../lib/service-health/publicHealthUrl";

// Blocking fallback shown in place of the protected page content when the
// business API (`kushim-api`) is unavailable. It is rendered INSIDE the Root
// chrome (navbar, background, footer stay), never mounts the business pages,
// never redirects to login and never clears the session. The copy stays factual
// and does not over-promise on data safety.
export function ServiceUnavailable({ onRetry }: { onRetry: () => void }) {
  return (
    <div className="app-page-container max-w-[680px] mx-auto px-4 sm:px-6 py-16 sm:py-24">
      <Card level={1}>
        <div className="text-center py-6 sm:py-10 px-2 sm:px-6">
          <span
            className="inline-flex items-center justify-center rounded-full mb-5"
            style={{
              width: "56px",
              height: "56px",
              background: "rgba(245, 158, 11, 0.12)",
              color: "var(--color-warning)",
            }}
          >
            <ServerCrash size={28} aria-hidden="true" />
          </span>
          <h1
            style={{
              color: "var(--text-primary)",
              fontSize: "clamp(24px, 5vw, 34px)",
              fontWeight: 800,
              lineHeight: 1.15,
            }}
          >
            Kushim est temporairement indisponible
          </h1>
          <p
            className="mt-4"
            style={{
              color: "var(--text-secondary)",
              fontSize: "clamp(15px, 2.5vw, 17px)",
              lineHeight: 1.6,
            }}
          >
            Le service principal ne répond pas actuellement. Vos données ne sont
            pas perdues. Réessayez dans quelques instants.
          </p>
          <div className="mt-8 flex flex-col sm:flex-row items-stretch sm:items-center justify-center gap-3">
            <Button variant="primary" icon={RotateCcw} onClick={onRetry}>
              Réessayer
            </Button>
            <a
              href={getPublicHealthUrl()}
              className="glass-interactive rounded-[9999px] flex min-h-[44px] items-center justify-center gap-2 px-6 py-3"
              style={{
                color: "var(--text-primary)",
                border: "1px solid var(--surface-1-border)",
                textDecoration: "none",
              }}
            >
              <Activity size={16} aria-hidden="true" />
              État des services
            </a>
          </div>
        </div>
      </Card>
    </div>
  );
}
