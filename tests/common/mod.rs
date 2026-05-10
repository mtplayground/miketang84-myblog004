#![allow(dead_code)]

use std::env;

use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};

pub async fn test_transaction() -> Result<Transaction<'static, Postgres>, Box<dyn std::error::Error>> {
    let database_url = env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| String::from("postgresql:///myblog004_repo_tests"));
    let admin_database_url = env::var("TEST_DATABASE_ADMIN_URL")
        .unwrap_or_else(|_| String::from("postgresql:///postgres"));

    ensure_test_database(&admin_database_url, &database_url).await?;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    myblog004::db::run_migrations(&pool).await?;

    let pool = Box::leak(Box::new(pool));
    let tx = pool.begin().await?;

    Ok(tx)
}

pub async fn test_pool(test_name: &str) -> Result<PgPool, Box<dyn std::error::Error>> {
    let database_url = test_database_url(test_name)?;
    let admin_database_url = env::var("TEST_DATABASE_ADMIN_URL")
        .unwrap_or_else(|_| String::from("postgresql:///postgres"));

    ensure_test_database(&admin_database_url, &database_url).await?;

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;

    myblog004::db::run_migrations(&pool).await?;
    reset_database(&pool).await?;

    Ok(pool)
}

pub async fn reset_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        TRUNCATE TABLE post_tags, tags, posts, admins RESTART IDENTITY CASCADE
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_test_database(
    admin_database_url: &str,
    database_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let database_name = database_url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or("missing database name in TEST_DATABASE_URL")?;

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(admin_database_url)
        .await?;

    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM pg_database
            WHERE datname = $1
        )
        "#,
    )
    .bind(database_name)
    .fetch_one(&admin_pool)
    .await?;

    if !exists {
        let statement = format!(r#"CREATE DATABASE "{database_name}""#);
        if let Err(error) = sqlx::query(&statement).execute(&admin_pool).await {
            if !is_duplicate_database_error(&error) {
                return Err(Box::new(error));
            }
        }
    }

    Ok(())
}

fn test_database_url(test_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let base_database_url = env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| String::from("postgresql:///myblog004_repo_tests"));
    let database_name = base_database_url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or("missing database name in TEST_DATABASE_URL")?;
    let prefix = base_database_url
        .rsplit_once('/')
        .map(|(prefix, _)| prefix)
        .ok_or("missing database URL prefix in TEST_DATABASE_URL")?;
    let sanitized_test_name: String = test_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();

    Ok(format!("{prefix}/{database_name}_{sanitized_test_name}"))
}

fn is_duplicate_database_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db_error) => {
            let code = db_error.code().map(|code| code.into_owned());
            code.as_deref() == Some("42P04") || code.as_deref() == Some("23505")
        }
        _ => false,
    }
}
