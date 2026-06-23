// Resolve the absolute URL of the public service-status page, which lives on the
// website host (different origin from the app). In Docker this is
// http://kushim.localhost/health; direct-dev defaults to the local Next port.
export function getPublicHealthUrl(): string {
  const siteUrl = import.meta.env.VITE_SITE_URL || "http://localhost:3000";
  return `${siteUrl.replace(/\/$/, "")}/health`;
}
