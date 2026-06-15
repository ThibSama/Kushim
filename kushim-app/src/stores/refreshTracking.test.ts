import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/api/businessApi", () => ({
  getRefreshRequest: vi.fn(),
}));

vi.mock("./operations", () => ({
  useOperationsStore: {
    getState: vi.fn(() => ({ reloadOperations: vi.fn(), reset: vi.fn() })),
  },
}));

vi.mock("./portfolioReadModels", () => ({
  usePortfolioReadModelsStore: {
    getState: vi.fn(() => ({ reloadAll: vi.fn(), reset: vi.fn() })),
  },
}));

import * as businessApi from "../lib/api/businessApi";
import { ApiRequestError } from "../lib/api/httpClient";
import {
  REFRESH_TRACKING_STORAGE_KEY,
  persistRefreshTracking,
  readPersistedRefreshTracking,
} from "../lib/api/refreshTrackingStorage";
import { useOperationsStore } from "./operations";
import { usePortfolioReadModelsStore } from "./portfolioReadModels";
import { useRefreshTrackingStore } from "./refreshTracking";

const PF = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const RR = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

function viewWith(status: string, error_code: string | null = null) {
  return {
    id_portfolio_refresh_request: RR,
    id_portfolio: PF,
    status: status as "pending" | "processing" | "completed" | "failed",
    attempts: 1,
    requested_at: "2026-06-14T00:00:00Z",
    processing_started_at: null,
    completed_at: null,
    updated_at: "2026-06-14T00:00:00Z",
    error_code,
  };
}

describe("refreshTracking polling store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.sessionStorage.clear();
    useRefreshTrackingStore.getState().reset();
  });

  afterEach(() => {
    useRefreshTrackingStore.getState().reset();
    vi.useRealTimers();
    vi.resetAllMocks();
    window.sessionStorage.clear();
  });

  it("track persists portfolio + request IDs before polling begins", () => {
    vi.mocked(businessApi.getRefreshRequest).mockResolvedValue(
      viewWith("pending"),
    );
    useRefreshTrackingStore.getState().track(PF, RR);

    const persisted = readPersistedRefreshTracking();
    expect(persisted).not.toBeNull();
    expect(persisted!.portfolioId).toBe(PF);
    expect(persisted!.refreshRequestId).toBe(RR);
    const state = useRefreshTrackingStore.getState();
    expect(state.portfolioId).toBe(PF);
    expect(state.refreshRequestId).toBe(RR);
    expect(state.status).toBe("pending");
  });

  it("completed clears persistence and triggers exactly one reload of read models and operations", async () => {
    const reloadAll = vi.fn().mockResolvedValue(undefined);
    const reloadOperations = vi.fn().mockResolvedValue(undefined);
    (usePortfolioReadModelsStore.getState as unknown as ReturnType<typeof vi.fn>).mockReturnValue({
      reloadAll,
      reset: vi.fn(),
    });
    (useOperationsStore.getState as unknown as ReturnType<typeof vi.fn>).mockReturnValue({
      reloadOperations,
      reset: vi.fn(),
    });

    vi.mocked(businessApi.getRefreshRequest).mockResolvedValueOnce(
      viewWith("completed"),
    );
    useRefreshTrackingStore.getState().track(PF, RR);

    // Flush the fetch microtask without advancing the 4s post-completion
    // notice clear timer.
    await Promise.resolve();
    await Promise.resolve();

    expect(useRefreshTrackingStore.getState().status).toBe("completed");
    expect(reloadAll).toHaveBeenCalledTimes(1);
    expect(reloadAll).toHaveBeenCalledWith(PF);
    expect(reloadOperations).toHaveBeenCalledTimes(1);
    expect(reloadOperations).toHaveBeenCalledWith(PF);
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
  });

  it("failed clears persistence and exposes the safe failed state with error_code", async () => {
    vi.mocked(businessApi.getRefreshRequest).mockResolvedValueOnce(
      viewWith("failed", "refresh_failed"),
    );
    useRefreshTrackingStore.getState().track(PF, RR);
    await Promise.resolve();
    await Promise.resolve();

    const state = useRefreshTrackingStore.getState();
    expect(state.status).toBe("failed");
    expect(state.errorCode).toBe("refresh_failed");
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
  });

  it("ownership 404 clears stale persistence and resets to idle", async () => {
    vi.mocked(businessApi.getRefreshRequest).mockRejectedValueOnce(
      new ApiRequestError({
        code: "not_found",
        message: "not found",
        status: 404,
      }),
    );
    useRefreshTrackingStore.getState().track(PF, RR);
    await Promise.resolve();
    await Promise.resolve();

    expect(useRefreshTrackingStore.getState().status).toBe("idle");
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
  });

  it("reset clears persistence and cancels timers (no further polls)", () => {
    vi.mocked(businessApi.getRefreshRequest).mockResolvedValue(
      viewWith("pending"),
    );
    useRefreshTrackingStore.getState().track(PF, RR);
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).not.toBeNull();

    useRefreshTrackingStore.getState().reset();

    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
    expect(useRefreshTrackingStore.getState().status).toBe("idle");
  });

  it("resumeFromStorage restarts a pending request without creating a new one", async () => {
    persistRefreshTracking({
      portfolioId: PF,
      refreshRequestId: RR,
      startedAt: Date.now(),
    });
    vi.mocked(businessApi.getRefreshRequest).mockResolvedValue(
      viewWith("pending"),
    );

    useRefreshTrackingStore.getState().resumeFromStorage();
    await Promise.resolve();
    await Promise.resolve();

    const state = useRefreshTrackingStore.getState();
    expect(state.refreshRequestId).toBe(RR);
    expect(state.status).toBe("pending");
    // Crucially: getRefreshRequest was called for the persisted id, never
    // createOperation or any "new request" path.
    expect(businessApi.getRefreshRequest).toHaveBeenCalledTimes(1);
    expect(businessApi.getRefreshRequest).toHaveBeenCalledWith(PF, RR);
  });

  it("resumeFromStorage is idempotent under duplicate invocation (Strict Mode)", async () => {
    persistRefreshTracking({
      portfolioId: PF,
      refreshRequestId: RR,
      startedAt: Date.now(),
    });
    vi.mocked(businessApi.getRefreshRequest).mockResolvedValue(
      viewWith("pending"),
    );

    useRefreshTrackingStore.getState().resumeFromStorage();
    // Flush the initial fetch microtask so state is "pending" before the
    // second resume runs its guard.
    await Promise.resolve();
    await Promise.resolve();
    const firstCallCount = vi.mocked(businessApi.getRefreshRequest).mock.calls.length;

    // Second resume on the same (portfolio, request) — store already tracks
    // it; resume must not synchronously start a parallel polling cycle.
    useRefreshTrackingStore.getState().resumeFromStorage();
    await Promise.resolve();
    const secondCallCount = vi.mocked(businessApi.getRefreshRequest).mock.calls.length;

    // The second resume must not have issued a new immediate poll — the
    // in-flight cycle owns the timer. Allowed delta: 0 (the next scheduled
    // poll only fires when fake timers advance).
    expect(secondCallCount).toBe(firstCallCount);
  });

  it("a completed status discovered during resume reloads data and clears storage", async () => {
    persistRefreshTracking({
      portfolioId: PF,
      refreshRequestId: RR,
      startedAt: Date.now(),
    });
    const reloadAll = vi.fn().mockResolvedValue(undefined);
    const reloadOperations = vi.fn().mockResolvedValue(undefined);
    (usePortfolioReadModelsStore.getState as unknown as ReturnType<typeof vi.fn>).mockReturnValue({
      reloadAll,
      reset: vi.fn(),
    });
    (useOperationsStore.getState as unknown as ReturnType<typeof vi.fn>).mockReturnValue({
      reloadOperations,
      reset: vi.fn(),
    });
    vi.mocked(businessApi.getRefreshRequest).mockResolvedValueOnce(
      viewWith("completed"),
    );

    useRefreshTrackingStore.getState().resumeFromStorage();
    // Resolve the getRefreshRequest microtask only — do NOT advance the
    // 4-second post-completion "clear" timer.
    await Promise.resolve();
    await Promise.resolve();

    expect(useRefreshTrackingStore.getState().status).toBe("completed");
    expect(reloadAll).toHaveBeenCalledTimes(1);
    expect(reloadOperations).toHaveBeenCalledTimes(1);
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
  });

  it("ignores stale responses from a previous generation when a new track() preempts", async () => {
    let resolveFirst: (
      v: ReturnType<typeof viewWith>,
    ) => void = () => {};
    vi.mocked(businessApi.getRefreshRequest)
      .mockImplementationOnce(
        () =>
          new Promise<ReturnType<typeof viewWith>>((res) => {
            resolveFirst = res;
          }),
      )
      .mockResolvedValueOnce(viewWith("pending"));

    useRefreshTrackingStore.getState().track(PF, RR);

    // A new tracking session begins (different request id) — bumps the
    // generation; the older promise's resolution must be discarded.
    const RR_NEW = "cccccccc-cccc-cccc-cccc-cccccccccccc";
    useRefreshTrackingStore.getState().track(PF, RR_NEW);

    // Resolve the old call AFTER the generation flip with a "completed" view.
    resolveFirst(viewWith("completed"));
    await Promise.resolve();
    await Promise.resolve();

    // The store must still reflect the NEW tracking session, not the stale
    // completion of the previous one.
    const state = useRefreshTrackingStore.getState();
    expect(state.refreshRequestId).toBe(RR_NEW);
    expect(state.status).not.toBe("completed");
  });

  it("expired persisted entry is discarded — resumeFromStorage is a no-op", async () => {
    persistRefreshTracking({
      portfolioId: PF,
      refreshRequestId: RR,
      // Older than the 15-min TTL — the read helper auto-clears the slot.
      startedAt: Date.now() - 16 * 60 * 1000,
    });
    useRefreshTrackingStore.getState().resumeFromStorage();
    await vi.runOnlyPendingTimersAsync();

    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).toBeNull();
    expect(useRefreshTrackingStore.getState().status).toBe("idle");
    expect(businessApi.getRefreshRequest).not.toHaveBeenCalled();
  });

  it("timed_out retains persistence inside the recovery TTL", async () => {
    // 41 polls returning "pending" → exceeds MAX_POLLS (40) → timed_out.
    vi.mocked(businessApi.getRefreshRequest).mockResolvedValue(
      viewWith("pending"),
    );
    useRefreshTrackingStore.getState().track(PF, RR);

    // Drive the polling loop past MAX_POLLS.
    for (let i = 0; i < 42; i += 1) {
      await vi.advanceTimersByTimeAsync(1500);
    }

    expect(useRefreshTrackingStore.getState().status).toBe("timed_out");
    // Persistence is intentionally kept for the recovery window.
    expect(window.sessionStorage.getItem(REFRESH_TRACKING_STORAGE_KEY)).not.toBeNull();
  });
});
