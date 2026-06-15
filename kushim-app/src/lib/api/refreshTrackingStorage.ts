// Non-sensitive persistence for the active portfolio refresh request.
// SessionStorage carries only:
//   - portfolioId      (UUID)
//   - refreshRequestId (UUID)
//   - startedAt        (Unix ms — used to enforce a recovery TTL)
//
// Never persisted: access/refresh tokens, raw worker errors (`last_error`),
// or any portfolio financial data. The key is namespaced under
// `kushim_active_portfolio_refresh` and survives a full-page reload of the
// same tab; logout / completion / failure / ownership 404 all clear it.

const STORAGE_KEY = "kushim_active_portfolio_refresh";

// Recovery window — a persisted entry older than this is discarded on
// reload. Matches the bounded polling budget (~60 s) plus reasonable slack
// to cover a slow worker queue. See documentation/operations/validation-commands.md.
export const REFRESH_TRACKING_RECOVERY_TTL_MS = 15 * 60 * 1000;

export type PersistedRefreshTracking = {
  portfolioId: string;
  refreshRequestId: string;
  startedAt: number;
};

function getStorage(): Storage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.sessionStorage;
  } catch {
    return null;
  }
}

function isUuid(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value)
  );
}

export function persistRefreshTracking(
  entry: PersistedRefreshTracking,
): void {
  const storage = getStorage();
  if (!storage) return;
  storage.setItem(STORAGE_KEY, JSON.stringify(entry));
}

export function clearPersistedRefreshTracking(): void {
  const storage = getStorage();
  if (!storage) return;
  storage.removeItem(STORAGE_KEY);
}

// Read and validate the persisted entry. Returns `null` and clears the slot
// if the payload is malformed or older than the recovery TTL.
export function readPersistedRefreshTracking(
  now: number = Date.now(),
): PersistedRefreshTracking | null {
  const storage = getStorage();
  if (!storage) return null;
  const raw = storage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<PersistedRefreshTracking>;
    if (
      !isUuid(parsed.portfolioId) ||
      !isUuid(parsed.refreshRequestId) ||
      typeof parsed.startedAt !== "number"
    ) {
      storage.removeItem(STORAGE_KEY);
      return null;
    }
    if (now - parsed.startedAt > REFRESH_TRACKING_RECOVERY_TTL_MS) {
      storage.removeItem(STORAGE_KEY);
      return null;
    }
    return {
      portfolioId: parsed.portfolioId,
      refreshRequestId: parsed.refreshRequestId,
      startedAt: parsed.startedAt,
    };
  } catch {
    storage.removeItem(STORAGE_KEY);
    return null;
  }
}

export const REFRESH_TRACKING_STORAGE_KEY = STORAGE_KEY;
