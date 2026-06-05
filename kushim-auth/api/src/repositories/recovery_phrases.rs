use crate::{domain::recovery::RecoveryPhrase, errors::RepositoryError};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct RecoveryPhraseRepository {
    pool: PgPool,
}

impl RecoveryPhraseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert_for_user(
        &self,
        id_user: Uuid,
        phrase_hash: &str,
    ) -> Result<RecoveryPhrase, RepositoryError> {
        let recovery_phrase = sqlx::query_as::<_, RecoveryPhrase>(
            r#"
            INSERT INTO user_recovery_phrases (
                id_user,
                phrase_hash
            )
            VALUES ($1, $2)
            ON CONFLICT (id_user)
            DO UPDATE
            SET phrase_hash = EXCLUDED.phrase_hash
            RETURNING
                id_user_recovery_phrase,
                id_user,
                phrase_hash,
                created_at,
                updated_at
            "#,
        )
        .bind(id_user)
        .bind(phrase_hash)
        .fetch_one(&self.pool)
        .await?;

        Ok(recovery_phrase)
    }

    pub async fn find_by_user_id(
        &self,
        id_user: Uuid,
    ) -> Result<Option<RecoveryPhrase>, RepositoryError> {
        let recovery_phrase = sqlx::query_as::<_, RecoveryPhrase>(
            r#"
            SELECT
                id_user_recovery_phrase,
                id_user,
                phrase_hash,
                created_at,
                updated_at
            FROM user_recovery_phrases
            WHERE id_user = $1
            LIMIT 1
            "#,
        )
        .bind(id_user)
        .fetch_optional(&self.pool)
        .await?;

        Ok(recovery_phrase)
    }
}
