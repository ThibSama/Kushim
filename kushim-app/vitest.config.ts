import { defineConfig } from "vitest/config";

// Minimal Vitest configuration scoped to the auth/session critical surface.
// jsdom gives us window.localStorage / sessionStorage / fetch shims; no React
// rendering is needed by the current test set.
export default defineConfig({
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    globals: false,
    clearMocks: true,
    restoreMocks: true,
  },
});
