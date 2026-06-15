import { create } from "zustand";
import {
  type AuthUser,
  getCurrentUser,
  logoutSession,
  refreshSession,
} from "../lib/api/authApi";
import { ApiRequestError, isUnauthorized } from "../lib/api/httpClient";
import {
  bumpSessionGeneration,
  clearSession,
  configureSessionGate,
  refreshAccessToken,
} from "../lib/api/sessionGate";
import {
  clearStoredTokens,
  readAccessToken,
  readRefreshToken,
  writeTokens,
} from "../lib/api/tokenStorage";
import { usePortfolioStore } from "./portfolio";
import { useOperationsStore } from "./operations";
import { usePortfolioReadModelsStore } from "./portfolioReadModels";
import { clearPersistedRefreshTracking } from "../lib/api/refreshTrackingStorage";
import { useRefreshTrackingStore } from "./refreshTracking";

export type SessionStatus =
  | "idle"
  | "validating"
  | "authenticated"
  | "unauthenticated";

type AuthState = {
  token: string | null;
  user: AuthUser | null;
  sessionStatus: SessionStatus;
  setTokens: (accessToken: string, refreshToken: string) => void;
  setUser: (user: AuthUser) => void;
  logout: () => Promise<void>;
  validateSession: () => Promise<boolean>;
};

function resetDomainStores() {
  usePortfolioStore.getState().reset();
  useOperationsStore.getState().reset();
  usePortfolioReadModelsStore.getState().reset();
  useRefreshTrackingStore.getState().reset();
  clearPersistedRefreshTracking();
}

export const useAuthStore = create<AuthState>((set) => {
  // Wire the session gate to this store. The refresh callback uses the raw
  // authApi (public endpoint, not the interceptor). The cleared-session
  // handler resets local state synchronously.
  configureSessionGate({
    refresh: (refreshToken) => refreshSession(refreshToken),
    onSessionCleared: () => {
      resetDomainStores();
      set({ token: null, user: null, sessionStatus: "unauthenticated" });
    },
    // After a successful rotation: keep the Zustand store in sync with the
    // new access token immediately. The gate already guarantees this fires
    // only when the session generation hasn't moved (no late logout race).
    onTokensRotated: (accessToken) => {
      set({ token: accessToken });
    },
  });

  return {
    token: readAccessToken(),
    user: null,
    sessionStatus: readAccessToken() ? "idle" : "unauthenticated",

    setTokens: (accessToken, refreshToken) => {
      writeTokens(accessToken, refreshToken);
      // Bump the session generation so a late /auth/refresh response from a
      // previous identity cannot retroactively overwrite this fresh login.
      bumpSessionGeneration();
      // sessionStatus -> "idle" triggers RequireAuth's validation effect.
      set({ token: accessToken, sessionStatus: "idle" });
    },

    setUser: (user) => {
      set({ user, sessionStatus: "authenticated" });
    },

    logout: async () => {
      const refreshToken = readRefreshToken();
      try {
        if (refreshToken) {
          await logoutSession(refreshToken);
        }
      } finally {
        // clearSession bumps the generation, clears tokens and calls
        // onSessionCleared, which resets all domain stores.
        clearSession("logout");
        // Defensive: ensure tokens are gone even if the gate is mid-configure.
        clearStoredTokens();
      }
    },

    validateSession: async () => {
      const accessToken = readAccessToken();
      if (!accessToken) {
        set({ sessionStatus: "unauthenticated" });
        return false;
      }

      set({ sessionStatus: "validating" });

      try {
        const user = await getCurrentUser(accessToken);
        set({ token: accessToken, user, sessionStatus: "authenticated" });
        return true;
      } catch (error) {
        if (!isUnauthorized(error)) {
          set({ sessionStatus: "unauthenticated" });
          return false;
        }

        // Expired access token — refresh once via the gate (single-flight
        // shared with concurrent authenticated requests).
        const rotated = await refreshAccessToken();
        if (!rotated) {
          // refreshAccessToken already cleared the session.
          return false;
        }

        try {
          const user = await getCurrentUser(rotated);
          set({ token: rotated, user, sessionStatus: "authenticated" });
          return true;
        } catch (retryError) {
          if (retryError instanceof ApiRequestError && retryError.status === 401) {
            clearSession("retry_unauthorized");
          } else {
            set({ sessionStatus: "unauthenticated" });
          }
          return false;
        }
      }
    },
  };
});

export function getWebsiteLoginUrl() {
  const authUrl = import.meta.env.VITE_AUTH_URL || "http://localhost:3001";
  return `${authUrl.replace(/\/$/, "")}/connexion`;
}
