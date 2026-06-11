import { apiRequest } from "./httpClient";

const AUTH_API_URL =
  import.meta.env.VITE_AUTH_API_URL || "http://localhost:3002";

export type AuthUser = {
  id_user: string;
  username: string;
  public_handle: string;
  role: "user" | "admin" | "support";
  recovery_setup_completed: boolean;
  created_at: string;
};

type MeResponse = {
  user: AuthUser;
};

type RefreshResponse = {
  access_token: string;
  refresh_token: string;
  access_token_expires_at: string;
  refresh_token_expires_at: string;
};

type HandoffExchangeResponse = {
  access_token: string;
  refresh_token: string;
};

type LogoutResponse = {
  success: boolean;
};

export async function getCurrentUser(accessToken: string): Promise<AuthUser> {
  const data = await apiRequest<MeResponse>(AUTH_API_URL, "/auth/me", {
    token: accessToken,
  });
  return data.user;
}

export async function refreshSession(
  refreshToken: string,
): Promise<RefreshResponse> {
  return apiRequest<RefreshResponse>(AUTH_API_URL, "/auth/refresh", {
    method: "POST",
    body: { refresh_token: refreshToken },
  });
}

export async function logoutSession(refreshToken: string): Promise<void> {
  try {
    await apiRequest<LogoutResponse>(AUTH_API_URL, "/auth/logout", {
      method: "POST",
      body: { refresh_token: refreshToken },
    });
  } catch {
    // Logout is best-effort: always clear local state regardless
  }
}

export async function exchangeHandoffCode(
  code: string,
): Promise<HandoffExchangeResponse | null> {
  try {
    return await apiRequest<HandoffExchangeResponse>(
      AUTH_API_URL,
      "/auth/handoff/exchange",
      {
        method: "POST",
        body: { handoff_code: code },
      },
    );
  } catch {
    return null;
  }
}
