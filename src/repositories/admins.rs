use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Admin {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewAdmin {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
}

#[derive(Clone)]
pub struct AdminRepo {
    pool: PgPool,
}

impl AdminRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<Admin>, sqlx::Error> {
        Self::find_by_username_with(&self.pool, username).await
    }

    pub async fn find_by_username_with<'e, E>(
        executor: E,
        username: &str,
    ) -> Result<Option<Admin>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            Admin,
            r#"
            SELECT id, username, password_hash, created_at, updated_at
            FROM admins
            WHERE username = $1
            "#,
            username
        )
        .fetch_optional(executor)
        .await
    }

    pub async fn insert(&self, admin: &NewAdmin) -> Result<Admin, sqlx::Error> {
        Self::insert_with(&self.pool, admin).await
    }

    pub async fn insert_with<'e, E>(executor: E, admin: &NewAdmin) -> Result<Admin, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            Admin,
            r#"
            INSERT INTO admins (id, username, password_hash)
            VALUES ($1, $2, $3)
            RETURNING id, username, password_hash, created_at, updated_at
            "#,
            admin.id,
            admin.username,
            admin.password_hash
        )
        .fetch_one(executor)
        .await
    }

    pub async fn update_password(
        &self,
        id: Uuid,
        password_hash: &str,
    ) -> Result<Admin, sqlx::Error> {
        Self::update_password_with(&self.pool, id, password_hash).await
    }

    pub async fn update_password_with<'e, E>(
        executor: E,
        id: Uuid,
        password_hash: &str,
    ) -> Result<Admin, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            Admin,
            r#"
            UPDATE admins
            SET password_hash = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, username, password_hash, created_at, updated_at
            "#,
            id,
            password_hash
        )
        .fetch_one(executor)
        .await
    }
}
