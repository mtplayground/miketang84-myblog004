use std::env;

use sqlx::{Postgres, Transaction, postgres::PgPoolOptions};

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

fn is_duplicate_database_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db_error) => {
            let code = db_error.code().map(|code| code.into_owned());
            code.as_deref() == Some("42P04") || code.as_deref() == Some("23505")
        }
        _ => false,
    }
}
