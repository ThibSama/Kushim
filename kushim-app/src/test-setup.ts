// Vitest setup: registers @testing-library/jest-dom matchers (`toBeInTheDocument`,
// `toHaveTextContent`, etc.) and ensures rendered DOM trees are torn down
// between tests so component tests don't leak.
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

afterEach(() => {
  cleanup();
});
