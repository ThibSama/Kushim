use crate::{
    domain::{role::UserRole, token::TokenClaims, user::User},
    errors::RepositoryError,
    http::{
        auth::{
            GenericSuccessResponse, LoginRequest, LoginResponse, LogoutRequest, LogoutResponse,
            RecoverySetupRequest, RefreshRequest, RefreshResponse, ResetPasswordRequest,
            SignupRequest, SignupResponse, UserResponse,
        },
        me::MeResponse,
    },
    repositories::{
        recovery_phrases::RecoveryPhraseRepository, revoked_tokens::RevokedTokenRepository,
        roles::RoleRepository, users::UserRepository,
    },
    services::{password::PasswordService, recovery::RecoveryService, token::TokenService},
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthServiceError {
    #[error("invalid token")]
    InvalidToken,
    #[error("token has expired")]
    TokenExpired,
    #[error("token type is invalid")]
    InvalidTokenType,
    #[error("user role is missing")]
    MissingUserRole,
    #[error("failed to encode token")]
    TokenEncodingFailed,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("username already exists")]
    UsernameAlreadyExists,
    #[error("role not found")]
    RoleNotFound,
    #[error("user not found")]
    UserNotFound,
    #[error("refresh token has been revoked")]
    RefreshTokenRevoked,
    #[error("recovery phrase is invalid")]
    InvalidRecoveryPhrase,
    #[error("recovery phrase is not configured")]
    RecoveryPhraseNotConfigured,
    #[error("repository error")]
    Repository,
    #[error("response mapping failed")]
    ResponseMapping,
}

#[derive(Clone)]
pub struct AuthService {
    roles: RoleRepository,
    users: UserRepository,
    recovery_phrases: RecoveryPhraseRepository,
    revoked_tokens: RevokedTokenRepository,
    password_service: PasswordService,
    recovery_service: RecoveryService,
    token_service: TokenService,
}

impl AuthService {
    pub fn new(
        roles: RoleRepository,
        users: UserRepository,
        recovery_phrases: RecoveryPhraseRepository,
        revoked_tokens: RevokedTokenRepository,
        password_service: PasswordService,
        recovery_service: RecoveryService,
        token_service: TokenService,
    ) -> Self {
        Self {
            roles,
            users,
            recovery_phrases,
            revoked_tokens,
            password_service,
            recovery_service,
            token_service,
        }
    }

    pub async fn signup(&self, request: SignupRequest) -> Result<SignupResponse, AuthServiceError> {
        request
            .validate(&self.password_service)
            .map_err(|_| AuthServiceError::Repository)?;

        let role = self
            .roles
            .find_user_role()
            .await
            .map_err(map_repository_error)?
            .ok_or(AuthServiceError::RoleNotFound)?;
        let password_hash = self
            .password_service
            .hash_password(&request.password)
            .map_err(|_| AuthServiceError::Repository)?;

        let mut user = self
            .users
            .create_user(
                role.id_role,
                &request.username,
                &request.username,
                &password_hash,
            )
            .await
            .map_err(map_repository_error)?;
        user.role = Some(UserRole::User);

        let tokens = self.token_service.issue_token_pair(&user)?;
        let user_response =
            UserResponse::from_user(&user).map_err(|_| AuthServiceError::ResponseMapping)?;

        tracing::info!(
            event = "signup_success",
            username = redact_username(&user.username),
            "user signup succeeded"
        );

        Ok(SignupResponse::from_auth_tokens(user_response, tokens))
    }

    pub async fn login(&self, request: LoginRequest) -> Result<LoginResponse, AuthServiceError> {
        request
            .validate()
            .map_err(|_| AuthServiceError::InvalidCredentials)?;

        let mut user = self
            .users
            .find_active_by_username(&request.username)
            .await
            .map_err(map_repository_error)?
            .ok_or_else(|| {
                tracing::warn!(
                    event = "login_failed",
                    reason = "invalid_credentials",
                    username = redact_username(&request.username),
                    "login rejected"
                );
                AuthServiceError::InvalidCredentials
            })?;

        let is_valid = self
            .password_service
            .verify_password(&request.password, &user.password_hash)
            .map_err(|_| AuthServiceError::InvalidCredentials)?;

        if !is_valid {
            tracing::warn!(
                event = "login_failed",
                reason = "invalid_credentials",
                username = redact_username(&request.username),
                "login rejected"
            );
            return Err(AuthServiceError::InvalidCredentials);
        }

        user.role = Some(self.load_user_role(&user).await?);
        let tokens = self.token_service.issue_token_pair(&user)?;
        let user_response =
            UserResponse::from_user(&user).map_err(|_| AuthServiceError::ResponseMapping)?;

        tracing::info!(
            event = "login_success",
            user_id = %user.id_user,
            username = redact_username(&user.username),
            "login succeeded"
        );

        Ok(LoginResponse::from_auth_tokens(user_response, tokens))
    }

    pub async fn refresh(
        &self,
        request: RefreshRequest,
    ) -> Result<RefreshResponse, AuthServiceError> {
        request
            .validate()
            .map_err(|_| AuthServiceError::InvalidToken)?;

        let claims = self
            .token_service
            .decode_refresh_token(&request.refresh_token)
            .inspect_err(|error| {
                tracing::warn!(
                    event = "refresh_failed",
                    reason = error_reason(error),
                    "refresh rejected"
                );
            })?;
        if self
            .revoked_tokens
            .is_revoked(&claims.jti.to_string())
            .await
            .map_err(map_repository_error)?
        {
            tracing::warn!(
                event = "refresh_failed",
                reason = "refresh_token_revoked",
                user_id = %claims.sub,
                "refresh rejected"
            );
            return Err(AuthServiceError::RefreshTokenRevoked);
        }

        let mut user = self
            .users
            .find_active_by_id(claims.sub)
            .await
            .map_err(map_repository_error)?
            .ok_or(AuthServiceError::UserNotFound)?;
        user.role = Some(self.load_user_role(&user).await?);

        self.revoke_refresh_claims(&claims).await?;
        let tokens = self.token_service.issue_token_pair(&user)?;

        tracing::info!(
            event = "refresh_success",
            user_id = %user.id_user,
            "refresh token rotation succeeded"
        );

        Ok(RefreshResponse::from_auth_tokens(tokens))
    }

    pub async fn logout(&self, request: LogoutRequest) -> Result<LogoutResponse, AuthServiceError> {
        request
            .validate()
            .map_err(|_| AuthServiceError::InvalidToken)?;

        let claims = match self
            .token_service
            .decode_refresh_token(&request.refresh_token)
        {
            Ok(claims) => claims,
            Err(error) => {
                tracing::warn!(
                    event = "logout",
                    reason = error_reason(&error),
                    "logout rejected"
                );
                return Err(error);
            }
        };

        let jti = claims.jti.to_string();
        if self
            .revoked_tokens
            .is_revoked(&jti)
            .await
            .map_err(map_repository_error)?
        {
            tracing::info!(
                event = "logout",
                reason = "already_revoked",
                user_id = %claims.sub,
                "logout accepted for already revoked refresh token"
            );
            return Ok(LogoutResponse { success: true });
        }

        self.revoke_refresh_claims(&claims).await?;
        tracing::info!(
            event = "logout",
            reason = "refresh_revoked",
            user_id = %claims.sub,
            "logout succeeded"
        );

        Ok(LogoutResponse { success: true })
    }

    pub async fn me(&self, access_token: &str) -> Result<MeResponse, AuthServiceError> {
        let claims = self.token_service.decode_access_token(access_token)?;
        let mut user = self
            .users
            .find_active_by_id(claims.sub)
            .await
            .map_err(map_repository_error)?
            .ok_or(AuthServiceError::UserNotFound)?;
        user.role = Some(self.load_user_role(&user).await?);

        let user_response =
            UserResponse::from_user(&user).map_err(|_| AuthServiceError::ResponseMapping)?;
        Ok(MeResponse {
            user: user_response,
        })
    }

    pub fn decode_access_token(&self, access_token: &str) -> Result<TokenClaims, AuthServiceError> {
        self.token_service.decode_access_token(access_token)
    }

    pub async fn setup_recovery_phrase(
        &self,
        access_token: &str,
        request: RecoverySetupRequest,
    ) -> Result<GenericSuccessResponse, AuthServiceError> {
        request
            .validate(&self.recovery_service)
            .map_err(|_| AuthServiceError::Repository)?;

        let claims = self.token_service.decode_access_token(access_token)?;
        let user = self
            .users
            .find_active_by_id(claims.sub)
            .await
            .map_err(map_repository_error)?
            .ok_or_else(|| {
                tracing::warn!(
                    event = "recovery_setup_failed",
                    reason = "user_not_found",
                    user_id = %claims.sub,
                    "recovery setup rejected"
                );
                AuthServiceError::UserNotFound
            })?;

        let password_is_valid = self
            .password_service
            .verify_password(&request.current_password, &user.password_hash)
            .map_err(|_| AuthServiceError::InvalidCredentials)?;
        if !password_is_valid {
            tracing::warn!(
                event = "recovery_setup_failed",
                reason = "invalid_credentials",
                user_id = %user.id_user,
                "recovery setup rejected"
            );
            return Err(AuthServiceError::InvalidCredentials);
        }

        let phrase_hash = self
            .recovery_service
            .hash_recovery_phrase(&request.recovery_phrase)
            .map_err(|_| AuthServiceError::Repository)?;

        self.recovery_phrases
            .upsert_for_user(user.id_user, &phrase_hash)
            .await
            .map_err(map_repository_error)?;

        let updated = self
            .users
            .mark_recovery_setup_completed(user.id_user)
            .await
            .map_err(map_repository_error)?;
        if !updated {
            return Err(AuthServiceError::UserNotFound);
        }

        tracing::info!(
            event = "recovery_setup_success",
            user_id = %user.id_user,
            "recovery phrase setup succeeded"
        );

        Ok(GenericSuccessResponse { success: true })
    }

    pub async fn reset_password(
        &self,
        request: ResetPasswordRequest,
    ) -> Result<GenericSuccessResponse, AuthServiceError> {
        request
            .validate(&self.password_service, &self.recovery_service)
            .map_err(|_| AuthServiceError::Repository)?;

        let user = self
            .users
            .find_active_by_username(&request.username)
            .await
            .map_err(map_repository_error)?
            .ok_or_else(|| {
                tracing::warn!(
                    event = "reset_password_failed",
                    reason = "invalid_recovery_phrase",
                    username = redact_username(&request.username),
                    "password reset rejected"
                );
                AuthServiceError::InvalidRecoveryPhrase
            })?;

        let recovery_phrase = self
            .recovery_phrases
            .find_by_user_id(user.id_user)
            .await
            .map_err(map_repository_error)?
            .ok_or_else(|| {
                tracing::warn!(
                    event = "reset_password_failed",
                    reason = "recovery_not_configured",
                    user_id = %user.id_user,
                    "password reset rejected"
                );
                AuthServiceError::RecoveryPhraseNotConfigured
            })?;

        let phrase_is_valid = self
            .recovery_service
            .verify_recovery_phrase(&request.recovery_phrase, &recovery_phrase.phrase_hash)
            .map_err(|_| AuthServiceError::InvalidRecoveryPhrase)?;
        if !phrase_is_valid {
            tracing::warn!(
                event = "reset_password_failed",
                reason = "invalid_recovery_phrase",
                user_id = %user.id_user,
                "password reset rejected"
            );
            return Err(AuthServiceError::InvalidRecoveryPhrase);
        }

        let password_hash = self
            .password_service
            .hash_password(&request.new_password)
            .map_err(|_| AuthServiceError::Repository)?;

        let updated = self
            .users
            .update_password_hash(user.id_user, &password_hash)
            .await
            .map_err(map_repository_error)?;
        if !updated {
            return Err(AuthServiceError::UserNotFound);
        }

        let new_phrase_hash = self
            .recovery_service
            .hash_recovery_phrase(&request.new_recovery_phrase)
            .map_err(|_| AuthServiceError::Repository)?;
        self.recovery_phrases
            .upsert_for_user(user.id_user, &new_phrase_hash)
            .await
            .map_err(map_repository_error)?;

        tracing::info!(
            event = "reset_password_success",
            user_id = %user.id_user,
            "password reset with phrase rotation succeeded"
        );

        Ok(GenericSuccessResponse { success: true })
    }

    async fn load_user_role(&self, user: &User) -> Result<UserRole, AuthServiceError> {
        let role = self
            .roles
            .list_roles()
            .await
            .map_err(map_repository_error)?
            .into_iter()
            .find(|role| role.id_role == user.id_role)
            .ok_or(AuthServiceError::RoleNotFound)?;

        UserRole::try_from(role.label.as_str()).map_err(|_| AuthServiceError::RoleNotFound)
    }

    async fn revoke_refresh_claims(&self, claims: &TokenClaims) -> Result<(), AuthServiceError> {
        match self
            .revoked_tokens
            .revoke_token(
                &claims.jti.to_string(),
                claims.token_type.as_str(),
                time::OffsetDateTime::from_unix_timestamp(claims.exp)
                    .map_err(|_| AuthServiceError::InvalidToken)?,
                Some(claims.sub),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(RepositoryError::Conflict("revoked_token_jti")) => Ok(()),
            Err(error) => Err(map_repository_error(error)),
        }
    }
}

fn redact_username(username: &str) -> String {
    let visible: String = username.chars().take(3).collect();
    format!("{visible}***")
}

fn error_reason(error: &AuthServiceError) -> &'static str {
    match error {
        AuthServiceError::InvalidToken => "invalid_token",
        AuthServiceError::TokenExpired => "token_expired",
        AuthServiceError::InvalidTokenType => "invalid_token_type",
        AuthServiceError::MissingUserRole => "missing_user_role",
        AuthServiceError::TokenEncodingFailed => "token_encoding_failed",
        AuthServiceError::InvalidCredentials => "invalid_credentials",
        AuthServiceError::UsernameAlreadyExists => "username_conflict",
        AuthServiceError::RoleNotFound => "role_not_found",
        AuthServiceError::UserNotFound => "user_not_found",
        AuthServiceError::RefreshTokenRevoked => "refresh_token_revoked",
        AuthServiceError::InvalidRecoveryPhrase => "invalid_recovery_phrase",
        AuthServiceError::RecoveryPhraseNotConfigured => "recovery_not_configured",
        AuthServiceError::Repository => "repository_error",
        AuthServiceError::ResponseMapping => "response_mapping_failed",
    }
}

fn map_repository_error(error: RepositoryError) -> AuthServiceError {
    match error {
        RepositoryError::Conflict("username") => AuthServiceError::UsernameAlreadyExists,
        RepositoryError::Conflict("revoked_token_jti") => AuthServiceError::RefreshTokenRevoked,
        RepositoryError::Conflict(_) | RepositoryError::Database(_) => AuthServiceError::Repository,
    }
}

#[cfg(test)]
mod tests {
    use super::redact_username;

    #[test]
    fn redact_username_shows_first_three_chars() {
        assert_eq!(redact_username("camille_durand"), "cam***");
    }

    #[test]
    fn redact_username_short_input() {
        assert_eq!(redact_username("ab"), "ab***");
    }
}
