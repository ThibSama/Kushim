import { create } from "zustand";

const TOKEN_KEY = "kushim_token";

type AuthState = {
  token: string | null;
  setToken: (token: string) => void;
  logout: () => void;
};

function readStoredToken() {
  return typeof window === "undefined" ? null : localStorage.getItem(TOKEN_KEY);
}

export const useAuthStore = create<AuthState>((set) => ({
  token: readStoredToken(),
  setToken: (token) => {
    localStorage.setItem(TOKEN_KEY, token);
    set({ token });
  },
  logout: () => {
    localStorage.removeItem(TOKEN_KEY);
    set({ token: null });
  },
}));

export function getWebsiteLoginUrl() {
  const authUrl = import.meta.env.VITE_AUTH_URL || "http://localhost:3001";
  return `${authUrl.replace(/\/$/, "")}/connexion`;
}
