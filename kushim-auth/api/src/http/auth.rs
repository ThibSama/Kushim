use crate::{
    domain::{role::UserRole, token::AuthTokens, user::User},
    errors::ApiError,
    services::{
        auth::AuthServiceError,
        password::PasswordService,
        rate_limit::{
            global_auth_ip_rule, login_handle_rule, login_ip_rule, recovery_reset_handle_rule,
            recovery_reset_ip_rule, recovery_setup_ip_rule, recovery_setup_user_rule,
            refresh_ip_rule, signup_ip_rule,
        },
        recovery::RecoveryService,
    },
    state::AppState,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

static PUBLIC_HANDLE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9_][a-z0-9_-]{2,39}$").expect("valid public_handle regex"));

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignupRequest {
    pub username: String,
    pub public_handle: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub public_handle: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySetupRequest {
    pub current_password: String,
    pub recovery_phrase: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetPasswordRequest {
    pub public_handle: String,
    pub recovery_phrase: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserResponse {
    pub id_user: Uuid,
    pub username: String,
    pub public_handle: String,
    pub role: String,
    pub recovery_setup_completed: bool,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignupResponse {
    pub user: UserResponse,
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: OffsetDateTime,
    pub refresh_token_expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub user: UserResponse,
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: OffsetDateTime,
    pub refresh_token_expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: OffsetDateTime,
    pub refresh_token_expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogoutResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenericSuccessResponse {
    pub success: bool,
}

impl SignupRequest {
    pub fn validate(&self, password_service: &PasswordService) -> Result<(), ApiError> {
        validate_username(&self.username)?;
        validate_public_handle(&self.public_handle)?;
        password_service
            .validate_password_policy(&self.password)
            .map_err(map_password_validation_error)?;
        Ok(())
    }
}

impl LoginRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        validate_public_handle(&self.public_handle)?;
        validate_non_blank(&self.password, "password", "password must not be blank")
    }
}

impl RefreshRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        validate_non_blank(
            &self.refresh_token,
            "refresh_token",
            "refresh_token must not be blank",
        )
    }
}

impl LogoutRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        validate_non_blank(
            &self.refresh_token,
            "refresh_token",
            "refresh_token must not be blank",
        )
    }
}

impl RecoverySetupRequest {
    pub fn validate(&self, recovery_service: &RecoveryService) -> Result<(), ApiError> {
        validate_non_blank(
            &self.current_password,
            "current_password",
            "current_password must not be blank",
        )?;
        recovery_service
            .validate_recovery_phrase(&self.recovery_phrase)
            .map_err(map_recovery_validation_error)?;
        Ok(())
    }
}

impl ResetPasswordRequest {
    pub fn validate(
        &self,
        password_service: &PasswordService,
        recovery_service: &RecoveryService,
    ) -> Result<(), ApiError> {
        validate_public_handle(&self.public_handle)?;
        recovery_service
            .validate_recovery_phrase(&self.recovery_phrase)
            .map_err(map_recovery_validation_error)?;
        password_service
            .validate_password_policy(&self.new_password)
            .map_err(map_password_validation_error)?;
        Ok(())
    }
}

impl UserResponse {
    pub fn from_user(user: &User) -> Result<Self, ApiError> {
        let role = user
            .role
            .as_ref()
            .map(UserRole::as_str)
            .ok_or(ApiError::Internal {
                code: "missing_user_role",
                message: "user role is missing",
            })?;

        Ok(Self {
            id_user: user.id_user,
            username: user.username.clone(),
            public_handle: user.public_handle.clone(),
            role: role.to_string(),
            recovery_setup_completed: user.recovery_setup_completed,
            created_at: user.created_at,
        })
    }
}

impl SignupResponse {
    pub fn from_auth_tokens(user: UserResponse, tokens: AuthTokens) -> Self {
        Self {
            user,
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            access_token_expires_at: tokens.access_token_expires_at,
            refresh_token_expires_at: tokens.refresh_token_expires_at,
        }
    }
}

impl LoginResponse {
    pub fn from_auth_tokens(user: UserResponse, tokens: AuthTokens) -> Self {
        Self {
            user,
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            access_token_expires_at: tokens.access_token_expires_at,
            refresh_token_expires_at: tokens.refresh_token_expires_at,
        }
    }
}

impl RefreshResponse {
    pub fn from_auth_tokens(tokens: AuthTokens) -> Self {
        Self {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            access_token_expires_at: tokens.access_token_expires_at,
            refresh_token_expires_at: tokens.refresh_token_expires_at,
        }
    }
}

pub async fn signup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SignupRequest>,
) -> Result<(StatusCode, Json<SignupResponse>), ApiError> {
    enforce_global_auth_rate_limit(&state, &headers).await?;
    enforce_ip_rate_limit(&state, signup_ip_rule(), &headers).await?;
    request.validate(&PasswordService::new())?;
    let response = state
        .auth_service
        .signup(request)
        .await
        .map_err(map_auth_service_error)?;

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    enforce_global_auth_rate_limit(&state, &headers).await?;
    enforce_ip_rate_limit(&state, login_ip_rule(), &headers).await?;
    enforce_handle_rate_limit(&state, login_handle_rule(), &request.public_handle).await?;
    request.validate()?;
    let response = state
        .auth_service
        .login(request)
        .await
        .map_err(map_auth_service_error)?;

    Ok(Json(response))
}

pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, ApiError> {
    enforce_global_auth_rate_limit(&state, &headers).await?;
    enforce_ip_rate_limit(&state, refresh_ip_rule(), &headers).await?;
    request.validate()?;
    let response = state
        .auth_service
        .refresh(request)
        .await
        .map_err(map_auth_service_error)?;

    Ok(Json(response))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LogoutRequest>,
) -> Result<Json<LogoutResponse>, ApiError> {
    enforce_global_auth_rate_limit(&state, &headers).await?;
    request.validate()?;
    let response = state
        .auth_service
        .logout(request)
        .await
        .map_err(map_auth_service_error)?;

    Ok(Json(response))
}

pub async fn setup_recovery_phrase(
    State(state): State<AppState>,
    authenticated: crate::http::extractors::AuthenticatedAccessToken,
    headers: HeaderMap,
    Json(request): Json<RecoverySetupRequest>,
) -> Result<Json<GenericSuccessResponse>, ApiError> {
    enforce_global_auth_rate_limit(&state, &headers).await?;
    enforce_ip_rate_limit(&state, recovery_setup_ip_rule(), &headers).await?;
    enforce_user_rate_limit(
        &state,
        recovery_setup_user_rule(),
        &authenticated.claims.sub.to_string(),
    )
    .await?;
    request.validate(&RecoveryService::new())?;
    let response = state
        .auth_service
        .setup_recovery_phrase(&authenticated.raw_token, request)
        .await
        .map_err(map_auth_service_error)?;

    Ok(Json(response))
}

pub async fn reset_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ResetPasswordRequest>,
) -> Result<Json<GenericSuccessResponse>, ApiError> {
    enforce_global_auth_rate_limit(&state, &headers).await?;
    enforce_ip_rate_limit(&state, recovery_reset_ip_rule(), &headers).await?;
    enforce_handle_rate_limit(&state, recovery_reset_handle_rule(), &request.public_handle).await?;
    request.validate(&PasswordService::new(), &RecoveryService::new())?;
    let response = state
        .auth_service
        .reset_password(request)
        .await
        .map_err(map_auth_service_error)?;

    Ok(Json(response))
}

async fn enforce_global_auth_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    enforce_ip_rate_limit(state, global_auth_ip_rule(), headers).await
}

async fn enforce_ip_rate_limit(
    state: &AppState,
    rule: crate::services::rate_limit::LimitRule,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let Some(rate_limiter) = &state.rate_limiter else {
        return Ok(());
    };

    let identifier = client_identifier(headers);
    rate_limiter.enforce(rule, &identifier).await
}

async fn enforce_handle_rate_limit(
    state: &AppState,
    rule: crate::services::rate_limit::LimitRule,
    public_handle: &str,
) -> Result<(), ApiError> {
    let Some(rate_limiter) = &state.rate_limiter else {
        return Ok(());
    };

    rate_limiter.enforce(rule, public_handle).await
}

async fn enforce_user_rate_limit(
    state: &AppState,
    rule: crate::services::rate_limit::LimitRule,
    user_identifier: &str,
) -> Result<(), ApiError> {
    let Some(rate_limiter) = &state.rate_limiter else {
        return Ok(());
    };

    rate_limiter.enforce(rule, user_identifier).await
}

fn client_identifier(headers: &HeaderMap) -> String {
    if let Some(forwarded_for) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return forwarded_for.to_string();
    }

    headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "unknown".to_string())
}

fn validate_username(username: &str) -> Result<(), ApiError> {
    validate_non_blank(username, "username", "username must not be blank")?;

    if username.chars().count() > 50 {
        return Err(ApiError::Validation {
            code: "username_too_long",
            message: "username must be at most 50 characters long".to_string(),
        });
    }

    Ok(())
}

pub fn validate_public_handle(public_handle: &str) -> Result<(), ApiError> {
    if public_handle.chars().count() > 40 {
        return Err(ApiError::Validation {
            code: "public_handle_too_long",
            message: "public_handle must be at most 40 characters long".to_string(),
        });
    }

    if !PUBLIC_HANDLE_REGEX.is_match(public_handle) {
        return Err(ApiError::Validation {
            code: "invalid_public_handle",
            message: "public_handle format is invalid".to_string(),
        });
    }

    Ok(())
}

fn validate_non_blank(
    value: &str,
    code: &'static str,
    message: &'static str,
) -> Result<(), ApiError> {
    if value.trim().is_empty() {
        return Err(ApiError::Validation {
            code,
            message: message.to_string(),
        });
    }

    Ok(())
}

fn map_password_validation_error(
    error: crate::services::password::PasswordServiceError,
) -> ApiError {
    match error {
        crate::services::password::PasswordServiceError::BlankPassword => ApiError::Validation {
            code: "blank_password",
            message: "password must not be blank".to_string(),
        },
        crate::services::password::PasswordServiceError::TooShort { minimum } => {
            ApiError::Validation {
                code: "password_too_short",
                message: format!("password must be at least {minimum} characters long"),
            }
        }
        crate::services::password::PasswordServiceError::TooLong { maximum } => {
            ApiError::Validation {
                code: "password_too_long",
                message: format!("password must be at most {maximum} characters long"),
            }
        }
        crate::services::password::PasswordServiceError::InvalidStoredHash
        | crate::services::password::PasswordServiceError::HashingFailed => ApiError::Internal {
            code: "password_service_error",
            message: "password service failed",
        },
    }
}

fn map_recovery_validation_error(
    error: crate::services::recovery::RecoveryServiceError,
) -> ApiError {
    match error {
        crate::services::recovery::RecoveryServiceError::BlankRecoveryPhrase => {
            ApiError::Validation {
                code: "blank_recovery_phrase",
                message: "recovery_phrase must not be blank".to_string(),
            }
        }
        crate::services::recovery::RecoveryServiceError::TooShort { minimum } => {
            ApiError::Validation {
                code: "recovery_phrase_too_short",
                message: format!("recovery_phrase must be at least {minimum} characters long"),
            }
        }
        crate::services::recovery::RecoveryServiceError::InvalidStoredHash
        | crate::services::recovery::RecoveryServiceError::HashingFailed => ApiError::Internal {
            code: "recovery_service_error",
            message: "recovery service failed",
        },
    }
}

pub(crate) fn map_auth_service_error(error: AuthServiceError) -> ApiError {
    match error {
        AuthServiceError::InvalidToken => ApiError::Unauthorized {
            code: "invalid_token",
            message: "token is invalid",
        },
        AuthServiceError::TokenExpired => ApiError::Unauthorized {
            code: "token_expired",
            message: "token has expired",
        },
        AuthServiceError::InvalidTokenType => ApiError::Unauthorized {
            code: "invalid_token_type",
            message: "token type is invalid",
        },
        AuthServiceError::InvalidCredentials => ApiError::Unauthorized {
            code: "invalid_credentials",
            message: "invalid credentials",
        },
        AuthServiceError::PublicHandleAlreadyExists => ApiError::Conflict {
            code: "public_handle_conflict",
            message: "public_handle is already in use",
        },
        AuthServiceError::RefreshTokenRevoked => ApiError::Unauthorized {
            code: "refresh_token_revoked",
            message: "refresh token has been revoked",
        },
        AuthServiceError::InvalidRecoveryPhrase | AuthServiceError::RecoveryPhraseNotConfigured => {
            ApiError::Unauthorized {
                code: "invalid_recovery_phrase",
                message: "recovery phrase is invalid",
            }
        }
        AuthServiceError::UserNotFound => ApiError::Unauthorized {
            code: "user_not_available",
            message: "user is not available",
        },
        AuthServiceError::MissingUserRole
        | AuthServiceError::TokenEncodingFailed
        | AuthServiceError::RoleNotFound
        | AuthServiceError::Repository
        | AuthServiceError::ResponseMapping => ApiError::Internal {
            code: "auth_service_error",
            message: "authentication service failed",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LoginRequest, LogoutRequest, RecoverySetupRequest, RefreshRequest, ResetPasswordRequest,
        SignupRequest, UserResponse, validate_public_handle,
    };
    use crate::{
        domain::{role::UserRole, user::User},
        errors::ApiError,
        services::{password::PasswordService, recovery::RecoveryService},
    };
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn user_fixture() -> User {
        User {
            id_user: Uuid::new_v4(),
            id_role: 1,
            username: "Alice".to_string(),
            public_handle: "alice_handle".to_string(),
            password_hash: "$argon2id$placeholder".to_string(),
            recovery_setup_completed: true,
            is_active: true,
            deleted_at: None,
            anonymized_at: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            role: Some(UserRole::User),
        }
    }

    #[test]
    fn dto_user_response_conversion_is_safe() {
        let user = user_fixture();
        let response = UserResponse::from_user(&user).expect("convert user response");

        assert_eq!(response.id_user, user.id_user);
        assert_eq!(response.username, "Alice");
        assert_eq!(response.public_handle, "alice_handle");
        assert_eq!(response.role, "user");
        assert!(response.recovery_setup_completed);
    }

    #[test]
    fn dto_signup_request_validation_accepts_valid_payload() {
        let request = SignupRequest {
            username: "Alice".to_string(),
            public_handle: "alice_handle".to_string(),
            password: "correct horse battery".to_string(),
        };

        request
            .validate(&PasswordService::new())
            .expect("signup request should be valid");
    }

    #[test]
    fn validation_rejects_invalid_public_handle() {
        let error =
            validate_public_handle("Invalid Handle").expect_err("invalid handle should fail");

        assert!(matches!(error, ApiError::Validation { .. }));
    }

    #[test]
    fn validation_rejects_blank_username() {
        let request = SignupRequest {
            username: "   ".to_string(),
            public_handle: "alice_handle".to_string(),
            password: "correct horse battery".to_string(),
        };

        let error = request
            .validate(&PasswordService::new())
            .expect_err("blank username should fail");

        assert!(matches!(error, ApiError::Validation { .. }));
    }

    #[test]
    fn validation_rejects_blank_refresh_token() {
        let request = RefreshRequest {
            refresh_token: "   ".to_string(),
        };

        let error = request
            .validate()
            .expect_err("blank refresh token should fail");

        assert!(matches!(error, ApiError::Validation { .. }));
    }

    #[test]
    fn validation_login_request_accepts_valid_payload() {
        let request = LoginRequest {
            public_handle: "alice_handle".to_string(),
            password: "correct horse battery".to_string(),
        };

        request.validate().expect("login request should be valid");
    }

    #[test]
    fn validation_logout_request_rejects_blank_refresh_token() {
        let request = LogoutRequest {
            refresh_token: String::new(),
        };

        let error = request
            .validate()
            .expect_err("blank refresh token should fail");

        assert!(matches!(error, ApiError::Validation { .. }));
    }

    #[test]
    fn validation_recovery_setup_request_rejects_short_phrase() {
        let request = RecoverySetupRequest {
            current_password: "correct horse battery".to_string(),
            recovery_phrase: "too short".to_string(),
        };

        let error = request
            .validate(&RecoveryService::new())
            .expect_err("short recovery phrase should fail");

        assert!(matches!(error, ApiError::Validation { .. }));
    }

    #[test]
    fn validation_reset_password_request_accepts_valid_payload() {
        let request = ResetPasswordRequest {
            public_handle: "alice_handle".to_string(),
            recovery_phrase: "this is a long recovery phrase".to_string(),
            new_password: "correct horse battery".to_string(),
        };

        request
            .validate(&PasswordService::new(), &RecoveryService::new())
            .expect("reset password request should be valid");
    }
}
