pub mod claims;
pub mod extractor;

use claims::{AuthClaims, TokenType};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, errors::ErrorKind as JwtErrorKind};

pub use extractor::AuthenticatedUser;

#[derive(Debug, Clone)]
pub struct JwtValidator {
    decoding_key: DecodingKey,
    issuer: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JwtValidationError {
    TokenExpired,
    InvalidToken,
    InvalidTokenType,
}

impl JwtValidator {
    pub fn new(secret: &str, issuer: String) -> Self {
        Self {
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            issuer,
        }
    }

    pub fn decode_access_token(&self, token: &str) -> Result<AuthClaims, JwtValidationError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.validate_exp = true;
        validation.leeway = 0;

        let token_data =
            decode::<AuthClaims>(token, &self.decoding_key, &validation).map_err(map_jwt_error)?;

        if token_data.claims.token_type != TokenType::Access {
            return Err(JwtValidationError::InvalidTokenType);
        }

        Ok(token_data.claims)
    }
}

fn map_jwt_error(error: jsonwebtoken::errors::Error) -> JwtValidationError {
    match error.kind() {
        JwtErrorKind::ExpiredSignature => JwtValidationError::TokenExpired,
        _ => JwtValidationError::InvalidToken,
    }
}

#[cfg(test)]
mod tests {
    use super::{JwtValidationError, JwtValidator};
    use crate::auth::claims::{AuthClaims, TokenType, UserRole};
    use jsonwebtoken::{EncodingKey, Header, encode};
    use time::{Duration, OffsetDateTime};
    use uuid::Uuid;

    fn build_token(secret: &str, issuer: &str, token_type: TokenType, exp_offset: i64) -> String {
        let now = OffsetDateTime::now_utc();
        let claims = AuthClaims {
            sub: Uuid::new_v4(),
            public_handle: "test_handle".to_string(),
            role: UserRole::User,
            token_type,
            jti: Uuid::new_v4(),
            iat: now.unix_timestamp(),
            exp: (now + Duration::seconds(exp_offset)).unix_timestamp(),
            iss: issuer.to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("token should be encoded")
    }

    #[test]
    fn valid_access_token_is_accepted() {
        let validator = JwtValidator::new(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth".to_string(),
        );
        let token = build_token(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth",
            TokenType::Access,
            900,
        );

        let claims = validator
            .decode_access_token(&token)
            .expect("access token should be valid");

        assert_eq!(claims.token_type, TokenType::Access);
        assert_eq!(claims.iss, "kushim-auth");
    }

    #[test]
    fn refresh_token_is_rejected() {
        let validator = JwtValidator::new(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth".to_string(),
        );
        let token = build_token(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth",
            TokenType::Refresh,
            900,
        );

        let error = validator
            .decode_access_token(&token)
            .expect_err("refresh token should be rejected");

        assert_eq!(error, JwtValidationError::InvalidTokenType);
    }

    #[test]
    fn wrong_issuer_is_rejected() {
        let validator = JwtValidator::new(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth".to_string(),
        );
        let token = build_token(
            "dev_only_change_me_minimum_32_chars",
            "other-issuer",
            TokenType::Access,
            900,
        );

        let error = validator
            .decode_access_token(&token)
            .expect_err("wrong issuer should be rejected");

        assert_eq!(error, JwtValidationError::InvalidToken);
    }

    #[test]
    fn expired_token_is_rejected() {
        let validator = JwtValidator::new(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth".to_string(),
        );
        let token = build_token(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth",
            TokenType::Access,
            -300,
        );

        let error = validator
            .decode_access_token(&token)
            .expect_err("expired token should be rejected");

        assert_eq!(error, JwtValidationError::TokenExpired);
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let validator = JwtValidator::new(
            "another_secret_that_is_also_long_enough",
            "kushim-auth".to_string(),
        );
        let token = build_token(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth",
            TokenType::Access,
            900,
        );

        let error = validator
            .decode_access_token(&token)
            .expect_err("wrong secret should be rejected");

        assert_eq!(error, JwtValidationError::InvalidToken);
    }
}
