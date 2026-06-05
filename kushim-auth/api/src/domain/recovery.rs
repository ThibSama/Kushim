use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecoveryPhrase {
    pub id_user_recovery_phrase: Uuid,
    pub id_user: Uuid,
    pub phrase_hash: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
