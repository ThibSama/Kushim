// Single source of truth for browser-side token storage. All other modules
// (auth store, session gate, authenticated request layer) go through these
// helpers. Keys stay compatible with prior P0/P0.1 callers. Tokens are never
// logged, never copied to sessionStorage, and never embedded in URLs.

const ACCESS_TOKEN_KEY = "kushim_access_token";
const REFRESH_TOKEN_KEY = "kushim_refresh_token";
const LEGACY_TOKEN_KEY = "kushim_token";

export type StoredTokens = {
  accessToken: string | null;
  refreshToken: string | null;
};

function getStorage(): Storage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function readAccessToken(): string | null {
  const storage = getStorage();
  if (!storage) return null;
  return storage.getItem(ACCESS_TOKEN_KEY) ?? storage.getItem(LEGACY_TOKEN_KEY);
}

export function readRefreshToken(): string | null {
  const storage = getStorage();
  if (!storage) return null;
  return storage.getItem(REFRESH_TOKEN_KEY);
}

export function readStoredTokens(): StoredTokens {
  return {
    accessToken: readAccessToken(),
    refreshToken: readRefreshToken(),
  };
}

export function writeTokens(accessToken: string, refreshToken: string): void {
  const storage = getStorage();
  if (!storage) return;
  storage.setItem(ACCESS_TOKEN_KEY, accessToken);
  storage.setItem(REFRESH_TOKEN_KEY, refreshToken);
  storage.removeItem(LEGACY_TOKEN_KEY);
}

export function clearStoredTokens(): void {
  const storage = getStorage();
  if (!storage) return;
  storage.removeItem(ACCESS_TOKEN_KEY);
  storage.removeItem(REFRESH_TOKEN_KEY);
  storage.removeItem(LEGACY_TOKEN_KEY);
}

export const TOKEN_KEYS = {
  ACCESS: ACCESS_TOKEN_KEY,
  REFRESH: REFRESH_TOKEN_KEY,
  LEGACY: LEGACY_TOKEN_KEY,
} as const;
