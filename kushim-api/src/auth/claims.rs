use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Access,
    Refresh,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    User,
    Admin,
    Support,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
            Self::Support => "support",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: Uuid,
    pub public_handle: String,
    pub role: UserRole,
    pub token_type: TokenType,
    pub jti: Uuid,
    pub iat: i64,
    pub exp: i64,
    pub iss: String,
}
