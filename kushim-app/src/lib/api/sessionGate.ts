// Centralized session/refresh coordinator. Holds:
//   - a session generation counter bumped on logout/clear, so a refresh
//     reply that arrives after the user logged out cannot silently recreate
//     a session;
//   - a single-flight refresh promise shared by all concurrent authenticated
//     callers, so N parallel 401 responses cause exactly one /auth/refresh
//     call;
//   - injectable refresh and clear callbacks so that this module does NOT
//     import the auth store (no circular dependency with stores/auth.ts or
//     authApi.ts).
//
// All token persistence goes through tokenStorage.ts.

import {
  clearStoredTokens,
  readRefreshToken,
  writeTokens,
} from "./tokenStorage";

export type RefreshTokenResponse = {
  access_token: string;
  refresh_token: string;
};

export type RefreshCallback = (
  refreshToken: string,
) => Promise<RefreshTokenResponse>;

export type SessionClearedReason =
  | "refresh_failed"
  | "retry_unauthorized"
  | "logout";

export type SessionClearedHandler = (reason: SessionClearedReason) => void;

export type TokensRotatedHandler = (accessToken: string) => void;

type SessionGateConfig = {
  refresh: RefreshCallback;
  onSessionCleared: SessionClearedHandler;
  // Called after a successful single-flight refresh once tokens have been
  // persisted, only if the session generation hasn't moved (no late logout).
  // The auth store subscribes here so `useAuthStore.getState().token` mirrors
  // the rotated access token immediately and never serves a stale value.
  onTokensRotated?: TokensRotatedHandler;
};

let config: SessionGateConfig | null = null;
let generation = 0;
let inflightRefresh: Promise<RefreshTokenResponse | null> | null = null;

export function configureSessionGate(next: SessionGateConfig): void {
  config = next;
}

export function currentSessionGeneration(): number {
  return generation;
}

// Called by the auth store right after handoff or login to mark a fresh
// authenticated session boundary. Generations are monotonic.
export function bumpSessionGeneration(): number {
  generation += 1;
  inflightRefresh = null;
  return generation;
}

// Clear local tokens, drop any in-flight refresh, bump the generation so a
// late refresh response is ignored, and notify the auth store.
export function clearSession(reason: SessionClearedReason): void {
  clearStoredTokens();
  inflightRefresh = null;
  generation += 1;
  config?.onSessionCleared(reason);
}

// Single-flight refresh. Concurrent callers share the same in-flight promise
// and receive the same rotated access token. On success, the rotated tokens
// are persisted before the promise resolves so retried requests read the new
// access token. On failure, the local session is cleared and the promise
// resolves to null.
export async function refreshAccessToken(): Promise<string | null> {
  if (!config) {
    throw new Error("sessionGate is not configured");
  }

  const startedAtGeneration = generation;

  if (!inflightRefresh) {
    const refreshToken = readRefreshToken();
    if (!refreshToken) {
      clearSession("refresh_failed");
      return null;
    }

    const refresh = config.refresh;
    inflightRefresh = (async () => {
      try {
        const rotated = await refresh(refreshToken);
        // Discard a late success that belongs to a previous session
        // (the user logged out or refreshed manually mid-flight).
        if (generation !== startedAtGeneration) {
          return null;
        }
        writeTokens(rotated.access_token, rotated.refresh_token);
        // Notify subscribers (the auth store) so their in-memory token mirrors
        // the rotated value immediately. Guarded by the same generation check
        // so a late refresh post-logout cannot resurrect the session.
        config?.onTokensRotated?.(rotated.access_token);
        return rotated;
      } catch {
        if (generation === startedAtGeneration) {
          clearSession("refresh_failed");
        }
        return null;
      } finally {
        inflightRefresh = null;
      }
    })();
  }

  const result = await inflightRefresh;
  if (!result) return null;
  // Late-resolving caller from a previous generation: do not surface the new
  // token to a stale request that should no longer execute.
  if (generation !== startedAtGeneration) return null;
  return result.access_token;
}

// Test-only reset. Not exported from the package's public surface; used by
// vitest to give each test a clean coordinator state.
export function __resetSessionGateForTests(): void {
  config = null;
  generation = 0;
  inflightRefresh = null;
}
