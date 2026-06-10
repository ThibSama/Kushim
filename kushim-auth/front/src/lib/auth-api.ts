const BASE_URL =
  process.env.NEXT_PUBLIC_AUTH_API_URL ?? "http://localhost:3002";

export interface UserResponse {
  id_user: string;
  username: string;
  public_handle: string;
  role: string;
  recovery_setup_completed: boolean;
  created_at: string;
}

export interface AuthTokensResponse {
  user: UserResponse;
  access_token: string;
  refresh_token: string;
  access_token_expires_at: string;
  refresh_token_expires_at: string;
}

export interface RefreshResponse {
  access_token: string;
  refresh_token: string;
  access_token_expires_at: string;
  refresh_token_expires_at: string;
}

export interface MeResponse {
  user: UserResponse;
}

export interface ApiErrorBody {
  error: { code: string; message: string };
}

export class AuthApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    public serverMessage: string,
  ) {
    super(serverMessage);
    this.name = "AuthApiError";
  }
}

async function request<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE_URL}${path}`, {
      ...options,
      headers: {
        "Content-Type": "application/json",
        ...options.headers,
      },
    });
  } catch {
    throw new AuthApiError(0, "network_error", "Impossible de contacter le serveur d'authentification.");
  }

  if (!response.ok) {
    let body: ApiErrorBody | undefined;
    try {
      body = (await response.json()) as ApiErrorBody;
    } catch {
      // non-JSON error
    }
    throw new AuthApiError(
      response.status,
      body?.error?.code ?? "unknown",
      body?.error?.message ?? `Erreur serveur (${response.status})`,
    );
  }

  return (await response.json()) as T;
}

export function signup(payload: {
  username: string;
  password: string;
}): Promise<AuthTokensResponse> {
  return request<AuthTokensResponse>("/auth/signup", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function login(payload: {
  username: string;
  password: string;
}): Promise<AuthTokensResponse> {
  return request<AuthTokensResponse>("/auth/login", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function refresh(refreshToken: string): Promise<RefreshResponse> {
  return request<RefreshResponse>("/auth/refresh", {
    method: "POST",
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
}

export function logout(refreshToken: string): Promise<{ success: boolean }> {
  return request<{ success: boolean }>("/auth/logout", {
    method: "POST",
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
}

export function setupRecoveryPhrase(
  accessToken: string,
  payload: { current_password: string; recovery_phrase: string },
): Promise<{ success: boolean }> {
  return request<{ success: boolean }>("/auth/recovery/setup", {
    method: "POST",
    headers: { Authorization: `Bearer ${accessToken}` },
    body: JSON.stringify(payload),
  });
}

export function resetPassword(payload: {
  username: string;
  recovery_phrase: string;
  new_password: string;
  new_recovery_phrase: string;
}): Promise<{ success: boolean }> {
  return request<{ success: boolean }>("/auth/recovery/reset-password", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function me(accessToken: string): Promise<MeResponse> {
  return request<MeResponse>("/auth/me", {
    method: "GET",
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

export interface HandoffCodeResponse {
  handoff_code: string;
}

export function createHandoffCode(
  accessToken: string,
  refreshToken: string,
): Promise<HandoffCodeResponse> {
  return request<HandoffCodeResponse>("/auth/handoff", {
    method: "POST",
    headers: { Authorization: `Bearer ${accessToken}` },
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
}
