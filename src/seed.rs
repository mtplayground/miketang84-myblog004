use std::{error::Error, fmt};

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::password::{PasswordError, hash_password},
    repositories::admins::{AdminRepo, NewAdmin},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminSeedOutcome {
    Created,
    SkippedExisting,
}

#[derive(Debug)]
pub enum AdminSeedError {
    Database(sqlx::Error),
    Password(PasswordError),
}

impl fmt::Display for AdminSeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => write!(f, "admin seeding database operation failed"),
            Self::Password(_) => write!(f, "admin seeding password operation failed"),
        }
    }
}

impl Error for AdminSeedError {}

impl From<sqlx::Error> for AdminSeedError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl From<PasswordError> for AdminSeedError {
    fn from(value: PasswordError) -> Self {
        Self::Password(value)
    }
}

pub async fn seed_admin(
    pool: &PgPool,
    username: &str,
    password: &str,
) -> Result<AdminSeedOutcome, AdminSeedError> {
    let admin_exists = sqlx::query_scalar!(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM admins
        ) AS "exists!"
        "#
    )
    .fetch_one(pool)
    .await?;

    if admin_exists {
        return Ok(AdminSeedOutcome::SkippedExisting);
    }

    let password_hash = hash_password(password)?;
    let repo = AdminRepo::new(pool.clone());

    repo.insert(&NewAdmin {
        id: Uuid::new_v4(),
        username: username.to_string(),
        password_hash,
    })
    .await?;

    Ok(AdminSeedOutcome::Created)
}
