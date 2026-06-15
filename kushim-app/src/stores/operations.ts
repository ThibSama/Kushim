import { create } from "zustand";
import {
  type PortfolioOperation,
  type CreateOperationPayload,
  type CreateOperationResult,
  type ReferenceItem,
  listOperations,
  createOperation as apiCreateOperation,
  listOperationTypes,
  listOperationStatuses,
} from "../lib/api/businessApi";
import { hydrateAssetDisplayCache } from "../lib/operations";
import { useAuthStore } from "./auth";

export type OperationsStatus = "idle" | "loading" | "success" | "error";

type OperationsState = {
  operations: PortfolioOperation[];
  status: OperationsStatus;
  error: string | null;
  operationTypes: ReferenceItem[];
  operationStatuses: ReferenceItem[];
  referenceLoaded: boolean;
  loadOperations: (portfolioId: string) => Promise<void>;
  createOperation: (
    portfolioId: string,
    payload: CreateOperationPayload,
  ) => Promise<CreateOperationResult>;
  reloadOperations: (portfolioId: string) => Promise<void>;
  loadReferenceData: () => Promise<void>;
  reset: () => void;
};

export const useOperationsStore = create<OperationsState>((set, get) => ({
  operations: [],
  status: "idle",
  error: null,
  operationTypes: [],
  operationStatuses: [],
  referenceLoaded: false,

  loadOperations: async (portfolioId) => {
    const token = useAuthStore.getState().token;
    if (!token) {
      set({ status: "error", error: "no_session" });
      return;
    }

    set({ status: "loading", error: null });

    try {
      const operations = await listOperations(token, portfolioId);
      set({ operations, status: "success" });
      hydrateAssetDisplayCache(operations, token, () => {
        set({ operations: [...get().operations] });
      });
    } catch (e) {
      const message = e instanceof Error ? e.message : "unknown error";
      set({ status: "error", error: message });
    }
  },

  createOperation: async (portfolioId, payload) => {
    const token = useAuthStore.getState().token;
    if (!token) throw new Error("no_session");

    const result = await apiCreateOperation(token, portfolioId, payload);
    // The created operation appears immediately in the list with its real
    // status (posted for the normal flow). The asynchronous portfolio refresh
    // is tracked separately via the refresh store; we do NOT fake a refresh.
    set({ operations: [result.operation, ...get().operations] });
    return result;
  },

  // Quiet reload (no loading flicker) used after an automatic refresh completes.
  reloadOperations: async (portfolioId) => {
    const token = useAuthStore.getState().token;
    if (!token) return;
    try {
      const operations = await listOperations(token, portfolioId);
      set({ operations, status: "success" });
      hydrateAssetDisplayCache(operations, token, () => {
        set({ operations: [...get().operations] });
      });
    } catch {
      // Non-blocking: keep the previously displayed operations on failure.
    }
  },

  loadReferenceData: async () => {
    if (get().referenceLoaded) return;
    const token = useAuthStore.getState().token;
    if (!token) return;

    try {
      const [types, statuses] = await Promise.all([
        listOperationTypes(token),
        listOperationStatuses(token),
      ]);
      set({ operationTypes: types, operationStatuses: statuses, referenceLoaded: true });
    } catch {
      // Reference data failure is non-blocking
    }
  },

  reset: () => {
    set({
      operations: [],
      status: "idle",
      error: null,
      operationTypes: [],
      operationStatuses: [],
      referenceLoaded: false,
    });
  },
}));
