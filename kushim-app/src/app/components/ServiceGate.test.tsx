import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Mutable, hoist-safe health map driving the mocked probe. Each test mutates it
// before/while rendering to model a given outage scenario.
const h = vi.hoisted(() => ({
  map: {
    api: "operational",
    auth: "operational",
    worker: "operational",
    "market-data": "operational",
  } as Record<string, "operational" | "unavailable">,
}));

vi.mock("../../lib/service-health/probe", () => ({
  probeService: vi.fn(async (key: string) => h.map[key]),
}));

import { ServiceGate } from "./ServiceGate";
import {
  readAccessToken,
  readRefreshToken,
  writeTokens,
} from "../../lib/api/tokenStorage";

const PROTECTED = "PROTECTED CONTENT";

function renderGate() {
  return render(
    <ServiceGate>
      <div>{PROTECTED}</div>
    </ServiceGate>,
  );
}

describe("ServiceGate", () => {
  beforeEach(() => {
    h.map.api = "operational";
    h.map.auth = "operational";
    h.map.worker = "operational";
    h.map["market-data"] = "operational";
    window.localStorage.clear();
  });

  afterEach(() => {
    cleanup();
    window.localStorage.clear();
  });

  it("renders protected content with no banners when all services are operational", async () => {
    renderGate();

    expect(await screen.findByText(PROTECTED)).toBeInTheDocument();
    expect(screen.queryByTestId("degraded-worker")).toBeNull();
    expect(screen.queryByTestId("degraded-market-data")).toBeNull();
    expect(
      screen.queryByText(/temporairement indisponible/i),
    ).toBeNull();
  });

  it("blocks with the fallback when the API is unavailable and never clears the session", async () => {
    h.map.api = "unavailable";
    writeTokens("access-x", "refresh-x");

    renderGate();

    expect(
      await screen.findByText(/Kushim est temporairement indisponible/i),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Réessayer/i }),
    ).toBeInTheDocument();
    // Protected business content must not mount behind the fallback.
    expect(screen.queryByText(PROTECTED)).toBeNull();
    // Session is untouched.
    expect(readAccessToken()).toBe("access-x");
    expect(readRefreshToken()).toBe("refresh-x");
  });

  it("recovers without a reload: retry removes the fallback and mounts the page", async () => {
    h.map.api = "unavailable";
    renderGate();

    await screen.findByText(/temporairement indisponible/i);

    h.map.api = "operational";
    await userEvent.click(screen.getByRole("button", { name: /Réessayer/i }));

    expect(await screen.findByText(PROTECTED)).toBeInTheDocument();
    expect(screen.queryByText(/temporairement indisponible/i)).toBeNull();
  });

  it("keeps the app usable and shows the worker banner when only the worker is down", async () => {
    h.map.worker = "unavailable";
    renderGate();

    expect(await screen.findByText(PROTECTED)).toBeInTheDocument();
    expect(screen.getByTestId("degraded-worker")).toBeInTheDocument();
    expect(screen.queryByTestId("degraded-market-data")).toBeNull();
    expect(screen.queryByText(/temporairement indisponible/i)).toBeNull();
  });

  it("keeps the app usable and shows the market-data banner when only market-data is down", async () => {
    h.map["market-data"] = "unavailable";
    renderGate();

    expect(await screen.findByText(PROTECTED)).toBeInTheDocument();
    expect(screen.getByTestId("degraded-market-data")).toBeInTheDocument();
    expect(screen.queryByTestId("degraded-worker")).toBeNull();
  });

  it("shows both compact banners when worker and market-data are down", async () => {
    h.map.worker = "unavailable";
    h.map["market-data"] = "unavailable";
    renderGate();

    expect(await screen.findByText(PROTECTED)).toBeInTheDocument();
    expect(screen.getByTestId("degraded-worker")).toBeInTheDocument();
    expect(screen.getByTestId("degraded-market-data")).toBeInTheDocument();
  });

  it("leaves the checking state (no infinite loader) when the API probe times out", async () => {
    // A timed-out probe resolves to "unavailable"; the gate must settle on the
    // fallback rather than stay on the checking screen.
    h.map.api = "unavailable";
    renderGate();

    await screen.findByText(/temporairement indisponible/i);
    expect(screen.queryByText(/Vérification des services/i)).toBeNull();
  });
});
