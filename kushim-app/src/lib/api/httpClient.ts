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

type RequestOptions = {
  method?: string;
  body?: unknown;
  headers?: Record<string, string>;
  token?: string | null;
};

export async function apiRequest<T>(
  baseUrl: string,
  path: string,
  options: RequestOptions = {},
): Promise<T> {
  const { method = "GET", body, headers = {}, token } = options;

  const requestHeaders: Record<string, string> = {
    ...headers,
  };

  if (body !== undefined) {
    requestHeaders["Content-Type"] = "application/json";
  }

  if (token) {
    requestHeaders["Authorization"] = `Bearer ${token}`;
  }

  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers: requestHeaders,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });

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
