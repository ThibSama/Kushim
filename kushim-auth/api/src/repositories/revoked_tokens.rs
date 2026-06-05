use crate::{domain::token::RevokedToken, errors::RepositoryError};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct RevokedTokenRepository {
    pool: PgPool,
}

impl RevokedTokenRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn revoke_token(
        &self,
        jti: &str,
        token_type: &str,
        expires_at: OffsetDateTime,
        id_user: Option<Uuid>,
    ) -> Result<RevokedToken, RepositoryError> {
        let result = sqlx::query_as::<_, RevokedToken>(
            r#"
            INSERT INTO revoked_tokens (
                id_user,
                jti,
                token_type,
                expires_at
            )
            VALUES ($1, $2, $3, $4)
            RETURNING
                id_revoked_token,
                id_user,
                jti,
                token_type,
                expires_at,
                revoked_at
            "#,
        )
        .bind(id_user)
        .bind(jti)
        .bind(token_type)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(token) => Ok(token),
            Err(sqlx::Error::Database(error))
                if error.constraint() == Some("uq_revoked_tokens_jti") =>
            {
                Err(RepositoryError::Conflict("revoked_token_jti"))
            }
            Err(error) => Err(RepositoryError::Database(error)),
        }
    }

    pub async fn is_revoked(&self, jti: &str) -> Result<bool, RepositoryError> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM revoked_tokens
                WHERE jti = $1
            )
            "#,
        )
        .bind(jti)
        .fetch_one(&self.pool)
        .await?;

        Ok(exists)
    }

    pub async fn delete_expired_tokens(&self) -> Result<u64, RepositoryError> {
        let rows_affected = sqlx::query(
            r#"
            DELETE FROM revoked_tokens
            WHERE expires_at < now()
            "#,
        )
        .execute(&self.pool)
        .await?
        .rows_affected();

        Ok(rows_affected)
    }
}
