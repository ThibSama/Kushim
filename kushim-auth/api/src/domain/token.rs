use crate::domain::{role::UserRole, user::PublicHandle};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Access,
    Refresh,
}

impl TokenType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Access => "access",
            Self::Refresh => "refresh",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: OffsetDateTime,
    pub refresh_token_expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuedToken {
    pub token: String,
    pub expires_at: OffsetDateTime,
    pub claims: TokenClaims,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: Uuid,
    pub public_handle: PublicHandle,
    pub role: UserRole,
    pub token_type: TokenType,
    pub jti: Uuid,
    pub iat: i64,
    pub exp: i64,
    pub iss: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RevokedToken {
    pub id_revoked_token: Uuid,
    pub id_user: Option<Uuid>,
    pub jti: String,
    pub token_type: String,
    pub expires_at: OffsetDateTime,
    pub revoked_at: OffsetDateTime,
}
