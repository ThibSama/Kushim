import { create } from "zustand";
import {
  type RefreshRequestPublicStatus,
  getRefreshRequest,
} from "../lib/api/businessApi";
import { ApiRequestError } from "../lib/api/httpClient";
import {
  clearPersistedRefreshTracking,
  persistRefreshTracking,
  readPersistedRefreshTracking,
} from "../lib/api/refreshTrackingStorage";
import { useOperationsStore } from "./operations";
import { usePortfolioReadModelsStore } from "./portfolioReadModels";

// Public, frontend-facing refresh state. `timed_out` is a frontend-only
// distinction from `failed` (terminal worker failure); the timed-out entry
// keeps its sessionStorage record for at most `REFRESH_TRACKING_RECOVERY_TTL_MS`
// so a single F5 within the window resumes polling without losing the
// reference.
export type RefreshTrackingStatus =
  | "idle"
  | "pending"
  | "processing"
  | "completed"
  | "failed"
  | "timed_out";

const POLL_INTERVAL_MS = 1500;
const MAX_POLLS = 40; // ~60 s bounded polling
const COMPLETED_NOTICE_MS = 4000;

type RefreshTrackingState = {
  portfolioId: string | null;
  refreshRequestId: string | null;
  status: RefreshTrackingStatus;
  errorCode: string | null;
  startedAt: number | null;
  track: (portfolioId: string, refreshRequestId: string) => void;
  resumeFromStorage: () => void;
  clear: () => void;
  reset: () => void;
};

// Module-scoped timer + generation guard. Bumped on every track/clear/reset
// so in-flight polls from a previous request or portfolio are ignored.
let pollTimer: ReturnType<typeof setTimeout> | null = null;
let generation = 0;

function stopTimer() {
  if (pollTimer) {
    clearTimeout(pollTimer);
    pollTimer = null;
  }
}

export const useRefreshTrackingStore = create<RefreshTrackingState>((set, get) => {
  const scheduleClear = () => {
    const myGeneration = generation;
    pollTimer = setTimeout(() => {
      if (generation !== myGeneration) return;
      set({
        status: "idle",
        refreshRequestId: null,
        portfolioId: null,
        errorCode: null,
        startedAt: null,
      });
    }, COMPLETED_NOTICE_MS);
  };

  const poll = (
    portfolioId: string,
    refreshRequestId: string,
    myGeneration: number,
    attempt: number,
  ) => {
    if (myGeneration !== generation) return;
    if (attempt > MAX_POLLS) {
      // Frontend timeout — keep sessionStorage entry so a reload within the
      // recovery TTL can resume. The worker may still complete; we just
      // stopped waiting actively.
      set({ status: "timed_out" });
      return;
    }

    // The token is read by authenticatedRequest at call time; on 401 it will
    // single-flight refresh and retry exactly once. We only need to handle
    // catastrophic failures (network, 5xx) here.
    getRefreshRequest(portfolioId, refreshRequestId)
      .then((view) => {
        if (myGeneration !== generation) return;
        const status = view.status as RefreshRequestPublicStatus;

        if (status === "completed") {
          set({ status: "completed", errorCode: null });
          clearPersistedRefreshTracking();
          void usePortfolioReadModelsStore.getState().reloadAll(portfolioId);
          void useOperationsStore.getState().reloadOperations(portfolioId);
          scheduleClear();
          return;
        }

        if (status === "failed") {
          set({ status: "failed", errorCode: view.error_code });
          clearPersistedRefreshTracking();
          return;
        }

        set({ status });
        pollTimer = setTimeout(
          () => poll(portfolioId, refreshRequestId, myGeneration, attempt + 1),
          POLL_INTERVAL_MS,
        );
      })
      .catch((error) => {
        if (myGeneration !== generation) return;
        // 404 from `/refresh-requests/:id` means the request doesn't belong
        // to this user / portfolio anymore — drop the stale persistence.
        if (error instanceof ApiRequestError && error.status === 404) {
          clearPersistedRefreshTracking();
          set({
            status: "idle",
            refreshRequestId: null,
            portfolioId: null,
            errorCode: null,
            startedAt: null,
          });
          return;
        }
        // Transient (network/5xx): retry within the polling budget.
        pollTimer = setTimeout(
          () => poll(portfolioId, refreshRequestId, myGeneration, attempt + 1),
          POLL_INTERVAL_MS,
        );
      });
  };

  const startTracking = (
    portfolioId: string,
    refreshRequestId: string,
    startedAt: number,
    initialStatus: RefreshTrackingStatus = "pending",
  ) => {
    stopTimer();
    generation += 1;
    const myGeneration = generation;
    set({
      portfolioId,
      refreshRequestId,
      status: initialStatus,
      errorCode: null,
      startedAt,
    });
    poll(portfolioId, refreshRequestId, myGeneration, 1);
  };

  return {
    portfolioId: null,
    refreshRequestId: null,
    status: "idle",
    errorCode: null,
    startedAt: null,

    track: (portfolioId, refreshRequestId) => {
      const startedAt = Date.now();
      persistRefreshTracking({ portfolioId, refreshRequestId, startedAt });
      startTracking(portfolioId, refreshRequestId, startedAt);
    },

    // Idempotent under React Strict Mode: if the store already tracks the
    // persisted entry, do nothing. Otherwise resume bounded polling without
    // creating a new refresh request.
    resumeFromStorage: () => {
      const persisted = readPersistedRefreshTracking();
      if (!persisted) return;
      const current = get();
      if (
        current.refreshRequestId === persisted.refreshRequestId &&
        current.portfolioId === persisted.portfolioId &&
        current.status !== "idle"
      ) {
        return;
      }
      startTracking(
        persisted.portfolioId,
        persisted.refreshRequestId,
        persisted.startedAt,
      );
    },

    clear: () => {
      stopTimer();
      generation += 1;
      clearPersistedRefreshTracking();
      set({
        portfolioId: null,
        refreshRequestId: null,
        status: "idle",
        errorCode: null,
        startedAt: null,
      });
    },

    reset: () => {
      stopTimer();
      generation += 1;
      // Auth store also calls clearPersistedRefreshTracking() on logout, but
      // resetting the store directly should clean the slot too.
      clearPersistedRefreshTracking();
      set({
        portfolioId: null,
        refreshRequestId: null,
        status: "idle",
        errorCode: null,
        startedAt: null,
      });
    },
  };
});
