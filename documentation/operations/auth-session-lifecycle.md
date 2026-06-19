# Authentication session lifecycle

## Scope and source of truth

This document describes the currently implemented Kushim MVP behavior. It is not a production TTL recommendation or a promise of a future session design.

The executable sources are authoritative:

- `kushim-auth/api/src/config.rs` and `services/token.rs` for issuance and validation;
- `services/auth.rs` and `repositories/revoked_tokens.rs` for rotation and logout;
- `kushim-app/src/lib/api/tokenStorage.ts`, `sessionGate.ts`, and `authenticatedRequest.ts` for app behavior;
- the auth frontend and handoff service for the cross-origin transfer.

## Current default lifetimes

| Token | Environment variable | Seconds | Human duration | Validation |
|---|---|---:|---|---|
| Access | `ACCESS_TOKEN_TTL_SECONDS` | 900 | 15 minutes | Integer greater than zero |
| Refresh | `REFRESH_TOKEN_TTL_SECONDS` | 2,592,000 | 30 days | Integer greater than zero |

Both defaults are present in `kushim-auth/api/.env.example` and are also fallbacks in `Config::from_env`.

Docker Compose explicitly sets `ACCESS_TOKEN_TTL_SECONDS` to `${ACCESS_TOKEN_TTL_SECONDS:-900}`, so a PowerShell environment override is interpolated when the container is recreated. The refresh TTL comes from the auth service `env_file` in the current Compose configuration. These local defaults are not production security recommendations.

## JWT issuance and validation

The auth service issues two JWT types: `access` and `refresh`. Each token contains:

- `sub`: user UUID;
- `public_handle`;
- `role`;
- `token_type`;
- `jti`: a new UUID for every token;
- `iat`: issuance Unix timestamp;
- `exp`: expiration Unix timestamp;
- `iss`: configured issuer (`JWT_ISSUER`, default `kushim-auth`).

Current cryptographic and validation behavior:

- algorithm: HS256;
- signature key: `AUTH_JWT_SECRET`;
- issuer validation: enabled;
- expiration validation: enabled;
- validation leeway/clock skew allowance: zero seconds;
- expected token type is checked after JWT decoding.

The secret must contain at least 32 characters. Additional placeholder checks apply when `APP_ENV=production`.

## Login, signup, and handoff

`POST /auth/login` and `POST /auth/signup` issue both tokens. Their responses include `access_token`, `refresh_token`, `access_token_expires_at`, and `refresh_token_expires_at`.

Raw tokens must not be placed in URLs. The implemented auth-to-app transfer is:

1. the auth frontend receives and stores the tokens on the auth frontend origin;
2. it sends the access token as Bearer authentication and the refresh token in the body to create a handoff;
3. Redis stores the token pair behind a random, 60-second handoff code;
4. only `handoff_code` appears in the redirect URL;
5. `kushim-app` exchanges the code once; Redis uses `GETDEL`, so the code cannot be exchanged twice;
6. the app stores the returned tokens on the app origin.

The handoff exchange response contains the two tokens but not their expiration timestamps. The app currently does not persist token expiration metadata.

## Refresh rotation on the server

`POST /auth/refresh` receives `{ "refresh_token": "<redacted>" }` in the JSON request body.

The implemented sequence is:

1. validate the request and JWT signature, issuer, expiration, and `refresh` token type;
2. reject the token if its `jti` already exists in `revoked_tokens`;
3. load the active user and role;
4. insert the old refresh-token `jti`, token type, expiry, and user ID into `revoked_tokens`;
5. issue a new access token and a new refresh token;
6. return both new tokens and their explicit expiration timestamps.

The revocation table stores a `jti`, metadata, and expiry. It does not store the raw JWT. A sequential reuse of the old refresh token is rejected as `refresh_token_revoked`; this behavior is covered by the backend integration test.

### Atomicity limitation

Rotation is **not transactionally atomic**. Revocation and new-token issuance do not share a database transaction, and revocation happens before issuance. A failure after revocation can therefore leave the client without a usable replacement pair.

Concurrent use of the same refresh token is not protected by a token-family transaction or row lock. The repository treats a duplicate revocation insert as an idempotent success. Sequential reuse is validated; a strict one-winner guarantee for simultaneous refreshes is not implemented or tested.

## Reactive frontend refresh

`kushim-app` does not decode `exp` to schedule renewal. There is no timer and no early-renewal margin.

The implemented request path is:

1. read the access token from storage at request time;
2. send the authenticated request once;
3. after a `401`, request a refresh through `sessionGate`;
4. concurrent `401` responses in the same JavaScript runtime share one in-flight refresh promise;
5. after successful rotation, replace both stored tokens and synchronize the Zustand access-token mirror;
6. retry the original request exactly once with the new access token;
7. if the retry also returns `401`, clear the local session and do not refresh again;
8. if refresh fails or no refresh token exists, clear the local session.

Method, path, JSON body, query string, and caller headers are retained for the one retry. The stale Authorization value is replaced.

This is **reactive refresh after a protected request receives `401`**, not proactive renewal.

## Logout race guard

The in-page session coordinator maintains a monotonically increasing generation. Login/handoff and session clearing advance the generation.

If logout occurs while refresh is in flight, the late response is ignored when its captured generation no longer matches. It cannot rewrite storage or update the Zustand token mirror. Tests cover this logout/refresh race.

The single-flight promise and generation counter exist only within one loaded JavaScript runtime.

## Browser storage

On the `kushim-app` origin, `localStorage` uses:

- `kushim_access_token`;
- `kushim_refresh_token`;
- legacy read fallback `kushim_token`, removed on the next write or cleanup.

Token helpers do not intentionally log tokens or copy them to query parameters. Successful rotation replaces both tokens. Refresh failure, a second `401`, and logout remove all three keys.

The auth frontend also stores `kushim_access_token` and `kushim_refresh_token` in its own origin-local `localStorage` while completing login/signup and handoff. It clears them when handoff creation fails, but the current successful handoff path does not clear them. App-origin logout cannot directly clear storage belonging to the auth-frontend origin.

**Known limitation:** `localStorage` is readable by any script executing in that origin and persists beyond page reloads. It is an explicit MVP limitation, not the intended final production security design.

## Logout and revocation

App logout is best effort:

1. read the current refresh token;
2. call `POST /auth/logout` with that token in the body when present;
3. clear app-origin tokens and reset local domain/session state regardless of the API result.

Backend logout:

- validates signature, issuer, expiration, and refresh token type;
- revokes only the supplied refresh token `jti`;
- returns success when that structurally valid token is already revoked;
- rejects malformed, expired, or wrong-type tokens;
- does not revoke the access token, which remains cryptographically valid until its own expiration;
- does not revoke other refresh tokens belonging to the same user.

There is no global “log out all sessions” implementation. Password reset updates the password and rotates the recovery phrase, but does not revoke existing access or refresh tokens.

## Multiple tabs

`localStorage` itself is shared by tabs on the same origin, so later reads can observe writes or removals made by another tab. However, the app has no `storage` event listener, `BroadcastChannel`, Web Lock, or equivalent session coordinator.

Consequences:

- single-flight refresh applies only inside one tab/runtime;
- simultaneous 401s in different tabs may initiate separate refresh requests with the same refresh token;
- in-memory Zustand state and redirects are not proactively synchronized across tabs;
- another tab normally observes logout only when later code rereads storage or receives an authorization failure.

Cross-tab session coordination is **Deferred**.

## Controlled short-access-TTL validation

Use this only for local manual validation of reactive refresh. Do not shorten the refresh-token TTL and do not commit the override.

The `$env:` form below is PowerShell syntax. It fails if pasted directly into `cmd.exe`.

```powershell
cd E:\Kushim

# Apply a temporary 10-second access-token TTL.
$env:ACCESS_TOKEN_TTL_SECONDS = "10"
docker compose up -d --force-recreate kushim-auth-api
docker compose exec kushim-auth-api printenv ACCESS_TOKEN_TTL_SECONDS
# Expected: 10

# Manual scenario:
# 1. sign in and complete the handoff;
# 2. wait more than 10 seconds;
# 3. trigger a protected request in kushim-app;
# 4. verify one refresh request, one original-request retry, and a usable session;
# 5. verify that no token value is printed or copied into the report.

# Remove the temporary PowerShell override and restore the Compose default.
Remove-Item Env:ACCESS_TOKEN_TTL_SECONDS
docker compose up -d --force-recreate kushim-auth-api
docker compose exec kushim-auth-api printenv ACCESS_TOKEN_TTL_SECONDS
# Expected: 900
```

Do not set or shorten `REFRESH_TOKEN_TTL_SECONDS` for this scenario.

## Implementation status and limitations

| Status | Behavior |
|---|---|
| Implemented and validated | 15-minute access and 30-day refresh defaults; HS256 issuer/type/expiration validation; zero leeway |
| Implemented and validated | Sequential refresh rotation, old-`jti` rejection, idempotent logout for an already revoked valid refresh token |
| Implemented and validated | In-tab single-flight refresh, retry exactly once, failed-refresh cleanup, logout generation guard |
| Implemented | One-time 60-second Redis handoff code; tokens absent from the redirect URL |
| Known limitation | Reactive refresh only; no proactive renewal or early margin |
| Known limitation | Tokens in per-origin `localStorage`; successful auth-frontend handoff retains its origin-local copy |
| Known limitation | Rotation is not transactionally atomic; strict simultaneous-use exclusion is not implemented |
| Deferred | Cross-tab refresh/logout coordination |
| Deferred | Token families, global session revocation, and revoke-all on password reset |
