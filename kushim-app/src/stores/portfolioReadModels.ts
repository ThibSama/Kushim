import { create } from "zustand";
import {
  type Pagination,
  type PortfolioDailySnapshot,
  type PortfolioDailySnapshotHoldingsEnvelope,
  type PortfolioDailySnapshotsQuery,
  type PortfolioHolding,
  type PortfolioHoldingsQuery,
  type PortfolioSummary,
  getDailySnapshotHoldings,
  getDailySnapshots,
  getPortfolioHoldings,
  getPortfolioSummary,
} from "../lib/api/businessApi";
import { useAuthStore } from "./auth";

export type ReadModelStatus = "idle" | "loading" | "success" | "error";

type ReadModelSlice<T, Reason extends string = string> = {
  data: T;
  status: ReadModelStatus;
  error: string | null;
  dataAvailable: boolean | null;
  reason: Reason | null;
};

type PortfolioReadModelsState = {
  portfolioId: string | null;
  summary: ReadModelSlice<PortfolioSummary | null, "read_model_missing">;
  holdings: ReadModelSlice<PortfolioHolding[], "read_model_missing">;
  holdingsPagination: Pagination | null;
  snapshots: ReadModelSlice<PortfolioDailySnapshot[]>;
  snapshotHoldings: Record<
    string,
    ReadModelSlice<
      PortfolioDailySnapshotHoldingsEnvelope["holdings"],
      "snapshot_missing"
    > & { snapshot: PortfolioDailySnapshot | null }
  >;
  loadSummary: (portfolioId: string) => Promise<void>;
  loadHoldings: (
    portfolioId: string,
    query?: PortfolioHoldingsQuery,
  ) => Promise<void>;
  loadMoreHoldings: (
    portfolioId: string,
    query?: PortfolioHoldingsQuery,
  ) => Promise<void>;
  loadSnapshots: (
    portfolioId: string,
    query?: PortfolioDailySnapshotsQuery,
  ) => Promise<void>;
  loadSnapshotHoldings: (
    portfolioId: string,
    snapshotDate: string,
    query?: PortfolioHoldingsQuery,
  ) => Promise<void>;
  clearForPortfolio: (portfolioId: string | null) => void;
  reset: () => void;
};

const initialSummary: PortfolioReadModelsState["summary"] = {
  data: null,
  status: "idle",
  error: null,
  dataAvailable: null,
  reason: null,
};

const initialHoldings: PortfolioReadModelsState["holdings"] = {
  data: [],
  status: "idle",
  error: null,
  dataAvailable: null,
  reason: null,
};

const initialSnapshots: PortfolioReadModelsState["snapshots"] = {
  data: [],
  status: "idle",
  error: null,
  dataAvailable: null,
  reason: null,
};

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : "unknown error";
}

function requireToken() {
  const token = useAuthStore.getState().token;
  if (!token) throw new Error("no_session");
  return token;
}

export const usePortfolioReadModelsStore = create<PortfolioReadModelsState>(
  (set, get) => ({
    portfolioId: null,
    summary: initialSummary,
    holdings: initialHoldings,
    holdingsPagination: null,
    snapshots: initialSnapshots,
    snapshotHoldings: {},

    loadSummary: async (portfolioId) => {
      set({
        portfolioId,
        summary: { ...get().summary, status: "loading", error: null },
      });

      try {
        const res = await getPortfolioSummary(requireToken(), portfolioId);
        set({
          portfolioId,
          summary: {
            data: res.summary,
            status: "success",
            error: null,
            dataAvailable: res.data_available,
            reason: res.reason,
          },
        });
      } catch (error) {
        set({
          summary: {
            ...get().summary,
            status: "error",
            error: errorMessage(error),
          },
        });
      }
    },

    loadHoldings: async (portfolioId, query) => {
      set({
        portfolioId,
        holdings: { ...get().holdings, status: "loading", error: null },
      });

      try {
        const res = await getPortfolioHoldings(
          requireToken(),
          portfolioId,
          query,
        );
        set({
          portfolioId,
          holdings: {
            data: res.holdings,
            status: "success",
            error: null,
            dataAvailable: res.data_available,
            reason: res.reason,
          },
          holdingsPagination: res.pagination,
        });
      } catch (error) {
        set({
          holdings: {
            ...get().holdings,
            status: "error",
            error: errorMessage(error),
          },
        });
      }
    },

    loadMoreHoldings: async (portfolioId, query) => {
      const { holdingsPagination, holdings: prev } = get();
      if (!holdingsPagination) return;

      const nextOffset = holdingsPagination.offset + holdingsPagination.returned;
      set({
        holdings: { ...prev, status: "loading" },
      });

      try {
        const res = await getPortfolioHoldings(requireToken(), portfolioId, {
          ...query,
          offset: nextOffset,
        });
        set({
          holdings: {
            data: [...prev.data, ...res.holdings],
            status: "success",
            error: null,
            dataAvailable: res.data_available,
            reason: res.reason,
          },
          holdingsPagination: res.pagination,
        });
      } catch (error) {
        set({
          holdings: {
            ...prev,
            status: "error",
            error: errorMessage(error),
          },
        });
      }
    },

    loadSnapshots: async (portfolioId, query) => {
      set({
        portfolioId,
        snapshots: { ...get().snapshots, status: "loading", error: null },
      });

      try {
        const res = await getDailySnapshots(requireToken(), portfolioId, query);
        set({
          portfolioId,
          snapshots: {
            data: res.snapshots,
            status: "success",
            error: null,
            dataAvailable: res.data_available,
            reason: null,
          },
        });
      } catch (error) {
        set({
          snapshots: {
            ...get().snapshots,
            status: "error",
            error: errorMessage(error),
          },
        });
      }
    },

    loadSnapshotHoldings: async (portfolioId, snapshotDate, query) => {
      const current = get().snapshotHoldings[snapshotDate];
      set({
        portfolioId,
        snapshotHoldings: {
          ...get().snapshotHoldings,
          [snapshotDate]: {
            data: current?.data ?? [],
            snapshot: current?.snapshot ?? null,
            status: "loading",
            error: null,
            dataAvailable: current?.dataAvailable ?? null,
            reason: current?.reason ?? null,
          },
        },
      });

      try {
        const res = await getDailySnapshotHoldings(
          requireToken(),
          portfolioId,
          snapshotDate,
          query,
        );
        set({
          portfolioId,
          snapshotHoldings: {
            ...get().snapshotHoldings,
            [snapshotDate]: {
              data: res.holdings,
              snapshot: res.snapshot,
              status: "success",
              error: null,
              dataAvailable: res.data_available,
              reason: res.reason,
            },
          },
        });
      } catch (error) {
        const previous = get().snapshotHoldings[snapshotDate];
        set({
          snapshotHoldings: {
            ...get().snapshotHoldings,
            [snapshotDate]: {
              data: previous?.data ?? [],
              snapshot: previous?.snapshot ?? null,
              status: "error",
              error: errorMessage(error),
              dataAvailable: previous?.dataAvailable ?? null,
              reason: previous?.reason ?? null,
            },
          },
        });
      }
    },

    clearForPortfolio: (portfolioId) => {
      set({
        portfolioId,
        summary: initialSummary,
        holdings: initialHoldings,
        holdingsPagination: null,
        snapshots: initialSnapshots,
        snapshotHoldings: {},
      });
    },

    reset: () => {
      set({
        portfolioId: null,
        summary: initialSummary,
        holdings: initialHoldings,
        holdingsPagination: null,
        snapshots: initialSnapshots,
        snapshotHoldings: {},
      });
    },
  }),
);
