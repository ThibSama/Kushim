export type ApiError = {
  code: string;
  message: string;
  status: number;
};

export class ApiRequestError extends Error {
  readonly code: string;
  readonly status: number;

  constructor(error: ApiError) {
    super(error.message);
    this.name = "ApiRequestError";
    this.code = error.code;
    this.status = error.status;
  }
}

// Stable, sanitized error codes for transport-level failures. They deliberately
// carry `status: 0` so they can never be mistaken for an HTTP 401 — the
// authenticated wrapper must NOT refresh tokens or clear the session when a
// request times out or the network is unreachable.
export const TIMEOUT_ERROR_CODE = "request_timeout";
export const NETWORK_ERROR_CODE = "network_error";
export const ABORTED_ERROR_CODE = "request_aborted";

// Default ceiling for a single business API request. The slowest current
// business request is the synchronous operation POST, which the worker offloads
// (the API responds as soon as the operation row + refresh request are written),
// so 10s is comfortably above the legitimate worst case while still bounding a
// stuck network request that would otherwise pin a store in `loading`.
export const DEFAULT_REQUEST_TIMEOUT_MS = 10_000;

type RequestOptions = {
  method?: string;
  body?: unknown;
  headers?: Record<string, string>;
  token?: string | null;
  // Per-request override of the default timeout. Pass a larger value only for a
  // request the audit proves needs it; pass `0` to disable the timeout.
  timeoutMs?: number;
  // Optional caller-owned abort signal (e.g. component unmount). Combined with
  // the internal timeout controller — whichever fires first wins.
  signal?: AbortSignal;
};

export function isTimeoutError(error: unknown): boolean {
  return error instanceof ApiRequestError && error.code === TIMEOUT_ERROR_CODE;
}

export async function apiRequest<T>(
  baseUrl: string,
  path: string,
  options: RequestOptions = {},
): Promise<T> {
  const {
    method = "GET",
    body,
    headers = {},
    token,
    timeoutMs = DEFAULT_REQUEST_TIMEOUT_MS,
    signal,
  } = options;

  const requestHeaders: Record<string, string> = {
    ...headers,
  };

  if (body !== undefined) {
    requestHeaders["Content-Type"] = "application/json";
  }

  if (token) {
    requestHeaders["Authorization"] = `Bearer ${token}`;
  }

  // Internal controller drives the timeout. We track whether the abort came
  // from our timer (→ timeout error) or from the caller's signal (→ aborted
  // error) so the surfaced error is accurate and stable.
  const controller = new AbortController();
  let timedOut = false;
  const timer =
    timeoutMs > 0
      ? setTimeout(() => {
          timedOut = true;
          controller.abort();
        }, timeoutMs)
      : null;

  if (signal) {
    if (signal.aborted) {
      controller.abort();
    } else {
      signal.addEventListener("abort", () => controller.abort(), {
        once: true,
      });
    }
  }

  let response: Response;
  try {
    response = await fetch(`${baseUrl}${path}`, {
      method,
      headers: requestHeaders,
      body: body !== undefined ? JSON.stringify(body) : undefined,
      signal: controller.signal,
    });
  } catch {
    if (timedOut) {
      throw new ApiRequestError({
        code: TIMEOUT_ERROR_CODE,
        message: "La requête a expiré. Réessayez dans quelques instants.",
        status: 0,
      });
    }
    if (controller.signal.aborted) {
      throw new ApiRequestError({
        code: ABORTED_ERROR_CODE,
        message: "La requête a été interrompue.",
        status: 0,
      });
    }
    // A genuine network failure (DNS, connection refused, offline). Sanitized:
    // no upstream detail is surfaced.
    throw new ApiRequestError({
      code: NETWORK_ERROR_CODE,
      message: "Le service est injoignable. Réessayez dans quelques instants.",
      status: 0,
    });
  } finally {
    if (timer) clearTimeout(timer);
  }

  if (!response.ok) {
    let apiError: ApiError;
    try {
      const json = await response.json();
      apiError = {
        code: json.error?.code ?? "unknown_error",
        message: json.error?.message ?? response.statusText,
        status: response.status,
      };
    } catch {
      apiError = {
        code: "unknown_error",
        message: response.statusText || `HTTP ${response.status}`,
        status: response.status,
      };
    }
    throw new ApiRequestError(apiError);
  }

  return response.json();
}

export function isUnauthorized(error: unknown): boolean {
  return error instanceof ApiRequestError && error.status === 401;
}
