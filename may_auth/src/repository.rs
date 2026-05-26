use crate::error::AuthError;
use crate::models::{Role, User};
use async_trait::async_trait;
use sqlx::PgPool;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_username(&self, username: &str) -> Result<User, AuthError>;
    async fn create(
        &self,
        username: &str,
        password_hash: &str,
        role: Role,
    ) -> Result<User, AuthError>;
    async fn list(&self, page: u32, per_page: u32) -> Result<Vec<User>, AuthError>;
    async fn deactivate(&self, id: uuid::Uuid) -> Result<(), AuthError>;
}

pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn find_by_username(&self, username: &str) -> Result<User, AuthError> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT id, username, password_hash, role as "role: Role", is_active, created_at, updated_at
            FROM users
            WHERE username = $1 AND is_active = true
            "#,
            username
        )
        .fetch_optional(&self.pool)
        .await?;

        match user {
            Some(u) => Ok(u),
            None => Err(AuthError::UserNotFound),
        }
    }

    async fn create(
        &self,
        username: &str,
        password_hash: &str,
        role: Role,
    ) -> Result<User, AuthError> {
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (username, password_hash, role)
            VALUES ($1, $2, $3::user_role)
            RETURNING id, username, password_hash, role as "role: Role", is_active, created_at, updated_at
            "#,
            username,
            password_hash,
            role as _
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    async fn list(&self, page: u32, per_page: u32) -> Result<Vec<User>, AuthError> {
        let limit = per_page as i64;
        let offset = (page.saturating_sub(1) * per_page) as i64;

        let users = sqlx::query_as!(
            User,
            r#"
            SELECT id, username, password_hash, role as "role: Role", is_active, created_at, updated_at
            FROM users
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    async fn deactivate(&self, id: uuid::Uuid) -> Result<(), AuthError> {
        let result = sqlx::query!(
            "UPDATE users SET is_active = false, updated_at = now() WHERE id = $1 AND is_active = true",
            id
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AuthError::UserNotFound);
        }

        Ok(())
    }
}
