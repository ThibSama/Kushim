use crate::{
    domain::{
        token::{AuthTokens, IssuedToken, TokenClaims, TokenType},
        user::{PublicHandle, User},
    },
    services::auth::AuthServiceError,
};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode,
    errors::ErrorKind as JwtErrorKind,
};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TokenService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    access_token_ttl_seconds: i64,
    refresh_token_ttl_seconds: i64,
}

impl TokenService {
    pub fn new(
        secret: &str,
        issuer: String,
        access_token_ttl_seconds: i64,
        refresh_token_ttl_seconds: i64,
    ) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            issuer,
            access_token_ttl_seconds,
            refresh_token_ttl_seconds,
        }
    }

    pub fn issue_token_pair(&self, user: &User) -> Result<AuthTokens, AuthServiceError> {
        let access = self.issue_access_token(user)?;
        let refresh = self.issue_refresh_token(user)?;

        Ok(AuthTokens {
            access_token: access.token,
            refresh_token: refresh.token,
            access_token_expires_at: access.expires_at,
            refresh_token_expires_at: refresh.expires_at,
        })
    }

    pub fn issue_access_token(&self, user: &User) -> Result<IssuedToken, AuthServiceError> {
        self.issue_token(user, TokenType::Access, self.access_token_ttl_seconds)
    }

    pub fn issue_refresh_token(&self, user: &User) -> Result<IssuedToken, AuthServiceError> {
        self.issue_token(user, TokenType::Refresh, self.refresh_token_ttl_seconds)
    }

    pub fn decode_and_validate(
        &self,
        token: &str,
        expected_token_type: TokenType,
    ) -> Result<TokenClaims, AuthServiceError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.validate_exp = true;
        validation.leeway = 0;

        let token_data =
            decode::<TokenClaims>(token, &self.decoding_key, &validation).map_err(map_jwt_error)?;

        if token_data.claims.token_type != expected_token_type {
            return Err(AuthServiceError::InvalidTokenType);
        }

        Ok(token_data.claims)
    }

    pub fn decode_access_token(&self, token: &str) -> Result<TokenClaims, AuthServiceError> {
        self.decode_and_validate(token, TokenType::Access)
    }

    pub fn decode_refresh_token(&self, token: &str) -> Result<TokenClaims, AuthServiceError> {
        self.decode_and_validate(token, TokenType::Refresh)
    }

    fn issue_token(
        &self,
        user: &User,
        token_type: TokenType,
        ttl_seconds: i64,
    ) -> Result<IssuedToken, AuthServiceError> {
        let role = user.role.clone().ok_or(AuthServiceError::MissingUserRole)?;
        let now = OffsetDateTime::now_utc();
        let expires_at = now + Duration::seconds(ttl_seconds);
        let claims = TokenClaims {
            sub: user.id_user,
            public_handle: PublicHandle::new(user.public_handle.clone()),
            role,
            token_type,
            jti: Uuid::new_v4(),
            iat: now.unix_timestamp(),
            exp: expires_at.unix_timestamp(),
            iss: self.issuer.clone(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|_| AuthServiceError::TokenEncodingFailed)?;

        Ok(IssuedToken {
            token,
            expires_at,
            claims,
        })
    }
}

fn map_jwt_error(error: jsonwebtoken::errors::Error) -> AuthServiceError {
    match error.kind() {
        JwtErrorKind::ExpiredSignature => AuthServiceError::TokenExpired,
        _ => AuthServiceError::InvalidToken,
    }
}

#[cfg(test)]
mod tests {
    use super::TokenService;
    use crate::{
        domain::{role::UserRole, token::TokenType, user::User},
        services::auth::AuthServiceError,
    };
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn test_user() -> User {
        User {
            id_user: Uuid::new_v4(),
            id_role: 1,
            username: "Test User".to_string(),
            public_handle: "test_handle".to_string(),
            password_hash: "$argon2id$placeholder".to_string(),
            recovery_setup_completed: false,
            is_active: true,
            deleted_at: None,
            anonymized_at: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            role: Some(UserRole::User),
        }
    }

    fn token_service() -> TokenService {
        TokenService::new(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth".to_string(),
            900,
            2_592_000,
        )
    }

    #[test]
    fn generate_access_token() {
        let service = token_service();
        let token = service
            .issue_access_token(&test_user())
            .expect("issue access");

        assert!(!token.token.is_empty());
        assert_eq!(token.claims.token_type, TokenType::Access);
    }

    #[test]
    fn generate_refresh_token() {
        let service = token_service();
        let token = service
            .issue_refresh_token(&test_user())
            .expect("issue refresh");

        assert!(!token.token.is_empty());
        assert_eq!(token.claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn access_and_refresh_have_different_jti() {
        let service = token_service();
        let user = test_user();
        let access = service.issue_access_token(&user).expect("issue access");
        let refresh = service.issue_refresh_token(&user).expect("issue refresh");

        assert_ne!(access.claims.jti, refresh.claims.jti);
    }

    #[test]
    fn decode_valid_access_token() {
        let service = token_service();
        let user = test_user();
        let token = service.issue_access_token(&user).expect("issue access");

        let claims = service
            .decode_access_token(&token.token)
            .expect("decode access");

        assert_eq!(claims.sub, user.id_user);
        assert_eq!(claims.public_handle.as_str(), user.public_handle);
        assert_eq!(claims.role, UserRole::User);
        assert_eq!(claims.iss, "kushim-auth");
    }

    #[test]
    fn decode_valid_refresh_token() {
        let service = token_service();
        let user = test_user();
        let token = service.issue_refresh_token(&user).expect("issue refresh");

        let claims = service
            .decode_refresh_token(&token.token)
            .expect("decode refresh");

        assert_eq!(claims.sub, user.id_user);
        assert_eq!(claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn reject_refresh_token_when_access_expected() {
        let service = token_service();
        let token = service
            .issue_refresh_token(&test_user())
            .expect("issue refresh");

        let error = service
            .decode_access_token(&token.token)
            .expect_err("refresh token should be rejected as access");

        assert_eq!(error, AuthServiceError::InvalidTokenType);
    }

    #[test]
    fn reject_access_token_when_refresh_expected() {
        let service = token_service();
        let token = service
            .issue_access_token(&test_user())
            .expect("issue access");

        let error = service
            .decode_refresh_token(&token.token)
            .expect_err("access token should be rejected as refresh");

        assert_eq!(error, AuthServiceError::InvalidTokenType);
    }

    #[test]
    fn reject_wrong_secret() {
        let service = token_service();
        let wrong_service = TokenService::new(
            "another_secret_that_is_also_long_enough",
            "kushim-auth".to_string(),
            900,
            2_592_000,
        );
        let token = service
            .issue_access_token(&test_user())
            .expect("issue access");

        let error = wrong_service
            .decode_access_token(&token.token)
            .expect_err("token should fail with wrong secret");

        assert_eq!(error, AuthServiceError::InvalidToken);
    }

    #[test]
    fn reject_expired_token() {
        let service = TokenService::new(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth".to_string(),
            -300,
            2_592_000,
        );
        let token = service
            .issue_access_token(&test_user())
            .expect("issue access");

        let error = service
            .decode_access_token(&token.token)
            .expect_err("expired token should be rejected");

        assert_eq!(error, AuthServiceError::TokenExpired);
    }
}
