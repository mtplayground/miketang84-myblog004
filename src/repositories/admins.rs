use chrono::{DateTime, Utc};
use sqlx::PgPool;
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
        sqlx::query_as!(
            Admin,
            r#"
            SELECT id, username, password_hash, created_at, updated_at
            FROM admins
            WHERE username = $1
            "#,
            username
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn insert(&self, admin: &NewAdmin) -> Result<Admin, sqlx::Error> {
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
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_password(
        &self,
        id: Uuid,
        password_hash: &str,
    ) -> Result<Admin, sqlx::Error> {
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
        .fetch_one(&self.pool)
        .await
    }
}
