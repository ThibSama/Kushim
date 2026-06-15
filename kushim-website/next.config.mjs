/** @type {import("next").NextConfig} */
const nextConfig = {
  // Next.js 15+ blocks cross-origin requests to dev resources (HMR, RSC,
  // /_next/...) by default. Behind our local nginx reverse proxy the Host is
  // `kushim.localhost`, not `localhost`, so without this allowlist the dev
  // server blocks the HMR/runtime channel and React never hydrates.
  // See: https://nextjs.org/docs/app/api-reference/config/next-config-js/allowedDevOrigins
  allowedDevOrigins: ["kushim.localhost"],
};

export default nextConfig;
