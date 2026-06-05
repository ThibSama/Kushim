use crate::{domain::role::Role, errors::RepositoryError};
use sqlx::PgPool;

#[derive(Clone)]
pub struct RoleRepository {
    pool: PgPool,
}

impl RoleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn find_by_label(&self, label: &str) -> Result<Option<Role>, RepositoryError> {
        let role = sqlx::query_as::<_, Role>(
            r#"
            SELECT id_role, label
            FROM roles
            WHERE label = $1
            LIMIT 1
            "#,
        )
        .bind(label)
        .fetch_optional(&self.pool)
        .await?;

        Ok(role)
    }

    pub async fn find_user_role(&self) -> Result<Option<Role>, RepositoryError> {
        self.find_by_label("user").await
    }

    pub async fn list_roles(&self) -> Result<Vec<Role>, RepositoryError> {
        let roles = sqlx::query_as::<_, Role>(
            r#"
            SELECT id_role, label
            FROM roles
            ORDER BY id_role
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(roles)
    }
}
