import { create } from "zustand";
import {
  type AuthUser,
  getCurrentUser,
  logoutSession,
  refreshSession,
} from "../lib/api/authApi";
import { isUnauthorized } from "../lib/api/httpClient";
import { usePortfolioStore } from "./portfolio";
import { useOperationsStore } from "./operations";
import { usePortfolioReadModelsStore } from "./portfolioReadModels";

const ACCESS_TOKEN_KEY = "kushim_access_token";
const REFRESH_TOKEN_KEY = "kushim_refresh_token";
const LEGACY_TOKEN_KEY = "kushim_token";

export type SessionStatus = "idle" | "validating" | "authenticated" | "unauthenticated";

type AuthState = {
  token: string | null;
  user: AuthUser | null;
  sessionStatus: SessionStatus;
  setTokens: (accessToken: string, refreshToken: string) => void;
  setUser: (user: AuthUser) => void;
  logout: () => Promise<void>;
  validateSession: () => Promise<boolean>;
};

function readStoredToken(): string | null {
  if (typeof window === "undefined") return null;
  return (
    localStorage.getItem(ACCESS_TOKEN_KEY) ??
    localStorage.getItem(LEGACY_TOKEN_KEY)
  );
}

function readStoredRefreshToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(REFRESH_TOKEN_KEY);
}

function clearAllTokens() {
  localStorage.removeItem(ACCESS_TOKEN_KEY);
  localStorage.removeItem(REFRESH_TOKEN_KEY);
  localStorage.removeItem(LEGACY_TOKEN_KEY);
}

export const useAuthStore = create<AuthState>((set, get) => ({
  token: readStoredToken(),
  user: null,
  sessionStatus: readStoredToken() ? "idle" : "unauthenticated",

  setTokens: (accessToken, refreshToken) => {
    localStorage.setItem(ACCESS_TOKEN_KEY, accessToken);
    localStorage.setItem(REFRESH_TOKEN_KEY, refreshToken);
    localStorage.removeItem(LEGACY_TOKEN_KEY);
    set({ token: accessToken });
  },

  setUser: (user) => {
    set({ user, sessionStatus: "authenticated" });
  },

  logout: async () => {
    const refreshToken = readStoredRefreshToken();
    if (refreshToken) {
      await logoutSession(refreshToken);
    }
    clearAllTokens();
    usePortfolioStore.getState().reset();
    useOperationsStore.getState().reset();
    usePortfolioReadModelsStore.getState().reset();
    set({ token: null, user: null, sessionStatus: "unauthenticated" });
  },

  validateSession: async () => {
    const accessToken = get().token;
    if (!accessToken) {
      set({ sessionStatus: "unauthenticated" });
      return false;
    }

    set({ sessionStatus: "validating" });

    try {
      const user = await getCurrentUser(accessToken);
      set({ user, sessionStatus: "authenticated" });
      return true;
    } catch (error) {
      if (!isUnauthorized(error)) {
        set({ sessionStatus: "unauthenticated" });
        return false;
      }

      // Access token expired — attempt refresh
      const refreshToken = readStoredRefreshToken();
      if (!refreshToken) {
        clearAllTokens();
        set({ token: null, user: null, sessionStatus: "unauthenticated" });
        return false;
      }

      try {
        const result = await refreshSession(refreshToken);
        localStorage.setItem(ACCESS_TOKEN_KEY, result.access_token);
        localStorage.setItem(REFRESH_TOKEN_KEY, result.refresh_token);
        set({ token: result.access_token });

        const user = await getCurrentUser(result.access_token);
        set({ user, sessionStatus: "authenticated" });
        return true;
      } catch {
        clearAllTokens();
        set({ token: null, user: null, sessionStatus: "unauthenticated" });
        return false;
      }
    }
  },
}));

export function getWebsiteLoginUrl() {
  const authUrl = import.meta.env.VITE_AUTH_URL || "http://localhost:3001";
  return `${authUrl.replace(/\/$/, "")}/connexion`;
}
