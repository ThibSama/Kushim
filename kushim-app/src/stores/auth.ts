import { create } from "zustand";

const ACCESS_TOKEN_KEY = "kushim_access_token";
const REFRESH_TOKEN_KEY = "kushim_refresh_token";
const LEGACY_TOKEN_KEY = "kushim_token";

type AuthState = {
  token: string | null;
  setTokens: (accessToken: string, refreshToken: string) => void;
  setToken: (token: string) => void;
  logout: () => void;
};

function readStoredToken(): string | null {
  if (typeof window === "undefined") return null;
  return (
    localStorage.getItem(ACCESS_TOKEN_KEY) ??
    localStorage.getItem(LEGACY_TOKEN_KEY)
  );
}

export const useAuthStore = create<AuthState>((set) => ({
  token: readStoredToken(),
  setTokens: (accessToken, refreshToken) => {
    localStorage.setItem(ACCESS_TOKEN_KEY, accessToken);
    localStorage.setItem(REFRESH_TOKEN_KEY, refreshToken);
    localStorage.removeItem(LEGACY_TOKEN_KEY);
    set({ token: accessToken });
  },
  setToken: (token) => {
    localStorage.setItem(ACCESS_TOKEN_KEY, token);
    localStorage.removeItem(LEGACY_TOKEN_KEY);
    set({ token });
  },
  logout: () => {
    localStorage.removeItem(ACCESS_TOKEN_KEY);
    localStorage.removeItem(REFRESH_TOKEN_KEY);
    localStorage.removeItem(LEGACY_TOKEN_KEY);
    set({ token: null });
  },
}));

export function getWebsiteLoginUrl() {
  const authUrl = import.meta.env.VITE_AUTH_URL || "http://localhost:3001";
  return `${authUrl.replace(/\/$/, "")}/connexion`;
}
