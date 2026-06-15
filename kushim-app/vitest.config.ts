import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Vitest configuration covering both the auth/session unit tests and the P1
// React component tests. jsdom + the React plugin let us mount components via
// @testing-library/react; `setupFiles` registers jest-dom matchers and
// guarantees automatic cleanup between tests.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    setupFiles: ["src/test-setup.ts"],
    globals: false,
    clearMocks: true,
    restoreMocks: true,
  },
});
