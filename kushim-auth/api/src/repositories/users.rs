use crate::{domain::user::User, errors::RepositoryError};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create_user(
        &self,
        id_role: i16,
        username: &str,
        public_handle: &str,
        password_hash: &str,
    ) -> Result<User, RepositoryError> {
        let result = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (
                id_role,
                username,
                public_handle,
                password_hash
            )
            VALUES ($1, $2, $3, $4)
            RETURNING
                id_user,
                id_role,
                username,
                public_handle,
                password_hash,
                recovery_setup_completed,
                is_active,
                deleted_at,
                anonymized_at,
                created_at,
                updated_at
            "#,
        )
        .bind(id_role)
        .bind(username)
        .bind(public_handle)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(user) => Ok(user),
            Err(sqlx::Error::Database(error))
                if error.constraint() == Some("uq_users_public_handle_active") =>
            {
                Err(RepositoryError::Conflict("public_handle"))
            }
            Err(error) => Err(RepositoryError::Database(error)),
        }
    }

    pub async fn find_by_public_handle(
        &self,
        public_handle: &str,
    ) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT
                id_user,
                id_role,
                username,
                public_handle,
                password_hash,
                recovery_setup_completed,
                is_active,
                deleted_at,
                anonymized_at,
                created_at,
                updated_at
            FROM users
            WHERE public_handle = $1
            LIMIT 1
            "#,
        )
        .bind(public_handle)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn find_active_by_public_handle(
        &self,
        public_handle: &str,
    ) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT
                id_user,
                id_role,
                username,
                public_handle,
                password_hash,
                recovery_setup_completed,
                is_active,
                deleted_at,
                anonymized_at,
                created_at,
                updated_at
            FROM users
            WHERE public_handle = $1
              AND deleted_at IS NULL
              AND is_active = true
            LIMIT 1
            "#,
        )
        .bind(public_handle)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_id(&self, id_user: Uuid) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT
                id_user,
                id_role,
                username,
                public_handle,
                password_hash,
                recovery_setup_completed,
                is_active,
                deleted_at,
                anonymized_at,
                created_at,
                updated_at
            FROM users
            WHERE id_user = $1
            LIMIT 1
            "#,
        )
        .bind(id_user)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn find_active_by_id(&self, id_user: Uuid) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT
                id_user,
                id_role,
                username,
                public_handle,
                password_hash,
                recovery_setup_completed,
                is_active,
                deleted_at,
                anonymized_at,
                created_at,
                updated_at
            FROM users
            WHERE id_user = $1
              AND deleted_at IS NULL
              AND is_active = true
            LIMIT 1
            "#,
        )
        .bind(id_user)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn update_password_hash(
        &self,
        id_user: Uuid,
        password_hash: &str,
    ) -> Result<bool, RepositoryError> {
        let rows_affected = sqlx::query(
            r#"
            UPDATE users
            SET password_hash = $2
            WHERE id_user = $1
            "#,
        )
        .bind(id_user)
        .bind(password_hash)
        .execute(&self.pool)
        .await?
        .rows_affected();

        Ok(rows_affected == 1)
    }

    pub async fn mark_recovery_setup_completed(
        &self,
        id_user: Uuid,
    ) -> Result<bool, RepositoryError> {
        let rows_affected = sqlx::query(
            r#"
            UPDATE users
            SET recovery_setup_completed = true
            WHERE id_user = $1
            "#,
        )
        .bind(id_user)
        .execute(&self.pool)
        .await?
        .rows_affected();

        Ok(rows_affected == 1)
    }
}
