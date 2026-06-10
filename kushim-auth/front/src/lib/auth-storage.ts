/**
 * Local MVP token storage. Not production-grade — tokens are stored in
 * localStorage for convenience during local development. A production
 * implementation should use httpOnly cookies or a more secure mechanism.
 */

const ACCESS_TOKEN_KEY = "kushim_access_token";
const REFRESH_TOKEN_KEY = "kushim_refresh_token";

export function storeTokens(accessToken: string, refreshToken: string): void {
  localStorage.setItem(ACCESS_TOKEN_KEY, accessToken);
  localStorage.setItem(REFRESH_TOKEN_KEY, refreshToken);
}

export function getAccessToken(): string | null {
  return localStorage.getItem(ACCESS_TOKEN_KEY);
}

export function getRefreshToken(): string | null {
  return localStorage.getItem(REFRESH_TOKEN_KEY);
}

export function clearTokens(): void {
  localStorage.removeItem(ACCESS_TOKEN_KEY);
  localStorage.removeItem(REFRESH_TOKEN_KEY);
}
