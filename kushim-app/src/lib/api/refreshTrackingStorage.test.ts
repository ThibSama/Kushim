import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  REFRESH_TRACKING_RECOVERY_TTL_MS,
  REFRESH_TRACKING_STORAGE_KEY,
  clearPersistedRefreshTracking,
  persistRefreshTracking,
  readPersistedRefreshTracking,
} from "./refreshTrackingStorage";

const PF = "11111111-1111-1111-1111-111111111111";
const RR = "22222222-2222-2222-2222-222222222222";

describe("refreshTrackingStorage", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
  });

  afterEach(() => {
    window.sessionStorage.clear();
  });

  it("round-trips a fresh entry", () => {
    persistRefreshTracking({
      portfolioId: PF,
      refreshRequestId: RR,
      startedAt: 1000,
    });
    expect(readPersistedRefreshTracking(1500)).toEqual({
      portfolioId: PF,
      refreshRequestId: RR,
      startedAt: 1000,
    });
  });

  it("clear removes the slot", () => {
    persistRefreshTracking({
      portfolioId: PF,
      refreshRequestId: RR,
      startedAt: 1000,
    });
    clearPersistedRefreshTracking();
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
    expect(readPersistedRefreshTracking()).toBeNull();
  });

  it("discards a malformed payload and clears the slot", () => {
    window.sessionStorage.setItem(
      REFRESH_TRACKING_STORAGE_KEY,
      JSON.stringify({ portfolioId: "not-a-uuid", refreshRequestId: RR, startedAt: 1 }),
    );
    expect(readPersistedRefreshTracking()).toBeNull();
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
  });

  it("discards garbage JSON", () => {
    window.sessionStorage.setItem(REFRESH_TRACKING_STORAGE_KEY, "{not json");
    expect(readPersistedRefreshTracking()).toBeNull();
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
  });

  it("rejects entries older than the recovery TTL", () => {
    persistRefreshTracking({
      portfolioId: PF,
      refreshRequestId: RR,
      startedAt: 1000,
    });
    expect(
      readPersistedRefreshTracking(1000 + REFRESH_TRACKING_RECOVERY_TTL_MS + 1),
    ).toBeNull();
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
  });

  it("accepts entries exactly at the TTL boundary", () => {
    persistRefreshTracking({
      portfolioId: PF,
      refreshRequestId: RR,
      startedAt: 1000,
    });
    expect(
      readPersistedRefreshTracking(1000 + REFRESH_TRACKING_RECOVERY_TTL_MS),
    ).not.toBeNull();
  });
});
