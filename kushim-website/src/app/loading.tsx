export default function Loading() {
  return (
    <section className="px-4 sm:px-6 py-20 sm:py-28" aria-live="polite" aria-busy="true">
      <div className="mx-auto flex min-h-[280px] max-w-[680px] items-center justify-center">
        <div className="text-center" role="status">
          <span className="loading-indicator mx-auto block" aria-hidden="true" />
          <p className="mt-4" style={{ color: "var(--text-secondary)" }}>Chargement…</p>
        </div>
      </div>
    </section>
  );
}
