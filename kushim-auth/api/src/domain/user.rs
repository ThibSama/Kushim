use crate::domain::role::UserRole;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicHandle(String);

impl PublicHandle {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id_user: Uuid,
    pub id_role: i16,
    pub username: String,
    pub public_handle: String,
    pub password_hash: String,
    pub recovery_setup_completed: bool,
    pub is_active: bool,
    pub deleted_at: Option<OffsetDateTime>,
    pub anonymized_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    #[sqlx(skip)]
    pub role: Option<UserRole>,
}
