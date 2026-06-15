import { useRefreshTrackingStore } from "../../stores/refreshTracking";

// Compact, non-blocking notice reflecting the asynchronous portfolio refresh
// that follows a posted operation. It never presents a failed refresh as a
// failed financial operation: the operation is recorded regardless.
export function RefreshNotice() {
  const status = useRefreshTrackingStore((s) => s.status);

  if (status === "idle") return null;

  const palette = (() => {
    switch (status) {
      case "completed":
        return {
          border: "rgba(16, 185, 129, 0.30)",
          background: "rgba(16, 185, 129, 0.10)",
          dot: "var(--color-gain)",
        };
      case "failed":
        return {
          border: "rgba(239, 68, 68, 0.28)",
          background: "rgba(239, 68, 68, 0.08)",
          dot: "var(--color-loss)",
        };
      case "timed_out":
        return {
          border: "rgba(245, 158, 11, 0.30)",
          background: "rgba(245, 158, 11, 0.10)",
          dot: "#F59E0B",
        };
      default:
        return {
          border: "var(--surface-2-border)",
          background: "var(--surface-1-bg)",
          dot: "var(--color-accent)",
        };
    }
  })();

  const { title, detail } = (() => {
    switch (status) {
      case "pending":
      case "processing":
        return {
          title: "Mise à jour du portefeuille…",
          detail:
            "L'opération est enregistrée. Les positions et indicateurs sont en cours de recalcul.",
        };
      case "completed":
        return { title: "Portefeuille à jour.", detail: null };
      case "failed":
        return {
          title:
            "L'opération est enregistrée, mais les indicateurs n'ont pas pu être recalculés.",
          detail: null,
        };
      case "timed_out":
        return {
          title:
            "L'opération est enregistrée. La mise à jour prend plus de temps que prévu.",
          detail: null,
        };
      default:
        return { title: "", detail: null };
    }
  })();

  const animated = status === "pending" || status === "processing";

  return (
    <div
      role="status"
      aria-live="polite"
      className="mb-6 rounded-lg flex items-start gap-3"
      style={{
        padding: "12px 16px",
        border: `1px solid ${palette.border}`,
        background: palette.background,
        backdropFilter: "blur(8px)",
        WebkitBackdropFilter: "blur(8px)",
      }}
    >
      <span
        className="rounded-full shrink-0"
        style={{
          width: "10px",
          height: "10px",
          marginTop: "5px",
          background: palette.dot,
          animation: animated ? "pulse 1.4s ease-in-out infinite" : "none",
        }}
      />
      <div>
        <div
          style={{
            fontSize: "14px",
            fontWeight: 600,
            color: "var(--text-primary)",
          }}
        >
          {title}
        </div>
        {detail && (
          <div
            style={{
              marginTop: "2px",
              fontSize: "13px",
              color: "var(--text-secondary)",
            }}
          >
            {detail}
          </div>
        )}
      </div>
    </div>
  );
}
