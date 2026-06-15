// Authenticated request layer. Wraps the raw `apiRequest` with:
//   1. read the latest access token at call time;
//   2. send the request once;
//   3. on 401, ask the session gate for a refresh — single-flight; concurrent
//      401s share one /auth/refresh call;
//   4. retry the original request exactly once with the rotated access token;
//   5. a second 401 (or a failed refresh) clears the local session and the
//      original ApiRequestError is propagated to the caller.
//
// Method, path, JSON body, query string and caller-provided headers are all
// preserved on retry. The stale Authorization header is replaced with the
// rotated one — the caller cannot accidentally reuse the expired token.

import { ApiRequestError, apiRequest, isUnauthorized } from "./httpClient";
import { clearSession, refreshAccessToken } from "./sessionGate";
import { readAccessToken } from "./tokenStorage";

export type AuthenticatedRequestOptions = {
  method?: string;
  body?: unknown;
  headers?: Record<string, string>;
};

export async function authenticatedRequest<T>(
  baseUrl: string,
  path: string,
  options: AuthenticatedRequestOptions = {},
): Promise<T> {
  const token = readAccessToken();

  try {
    return await apiRequest<T>(baseUrl, path, { ...options, token });
  } catch (error) {
    if (!isUnauthorized(error)) {
      throw error;
    }

    const rotated = await refreshAccessToken();
    if (!rotated) {
      // refreshAccessToken already cleared the local session.
      throw error;
    }

    try {
      return await apiRequest<T>(baseUrl, path, { ...options, token: rotated });
    } catch (retryError) {
      if (isUnauthorized(retryError)) {
        // A second 401 after a fresh access token means the server explicitly
        // rejected the rotated identity. Clear the session and never refresh
        // again as a workaround.
        clearSession("retry_unauthorized");
      }
      throw retryError;
    }
  }
}

export { ApiRequestError, isUnauthorized };
