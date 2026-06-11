import { create } from "zustand";
import {
  type Portfolio,
  type CreatePortfolioPayload,
  listPortfolios,
  createPortfolio as apiCreatePortfolio,
} from "../lib/api/businessApi";
import { useAuthStore } from "./auth";

const ACTIVE_PORTFOLIO_KEY = "kushim_active_portfolio_id";

export type PortfolioStatus = "idle" | "loading" | "success" | "error";

type PortfolioState = {
  portfolios: Portfolio[];
  activePortfolioId: string | null;
  status: PortfolioStatus;
  error: string | null;
  loadPortfolios: () => Promise<void>;
  createPortfolio: (payload: CreatePortfolioPayload) => Promise<Portfolio>;
  setActivePortfolio: (id: string) => void;
  reset: () => void;
};

function readPersistedActiveId(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(ACTIVE_PORTFOLIO_KEY);
}

function persistActiveId(id: string | null) {
  if (id) {
    localStorage.setItem(ACTIVE_PORTFOLIO_KEY, id);
  } else {
    localStorage.removeItem(ACTIVE_PORTFOLIO_KEY);
  }
}

function resolveActiveId(
  portfolios: Portfolio[],
  preferredId: string | null,
): string | null {
  if (portfolios.length === 0) return null;
  if (preferredId && portfolios.some((p) => p.id_portfolio === preferredId)) {
    return preferredId;
  }
  return portfolios[0].id_portfolio;
}

export const usePortfolioStore = create<PortfolioState>((set, get) => ({
  portfolios: [],
  activePortfolioId: readPersistedActiveId(),
  status: "idle",
  error: null,

  loadPortfolios: async () => {
    const token = useAuthStore.getState().token;
    if (!token) {
      set({ status: "error", error: "no_session" });
      return;
    }

    set({ status: "loading", error: null });

    try {
      const portfolios = await listPortfolios(token);
      const activeId = resolveActiveId(portfolios, get().activePortfolioId);
      persistActiveId(activeId);
      set({ portfolios, activePortfolioId: activeId, status: "success" });
    } catch (e) {
      const message = e instanceof Error ? e.message : "unknown error";
      set({ status: "error", error: message });
    }
  },

  createPortfolio: async (payload) => {
    const token = useAuthStore.getState().token;
    if (!token) throw new Error("no_session");

    const portfolio = await apiCreatePortfolio(token, payload);
    const updated = [portfolio, ...get().portfolios];
    persistActiveId(portfolio.id_portfolio);
    set({
      portfolios: updated,
      activePortfolioId: portfolio.id_portfolio,
      status: "success",
    });
    return portfolio;
  },

  setActivePortfolio: (id) => {
    persistActiveId(id);
    set({ activePortfolioId: id });
  },

  reset: () => {
    set({
      portfolios: [],
      activePortfolioId: null,
      status: "idle",
      error: null,
    });
    localStorage.removeItem(ACTIVE_PORTFOLIO_KEY);
  },
}));
