use std::str::FromStr;

use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use url::Url;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    match connect_with_url(database_url).await {
        Ok(pool) => Ok(pool),
        Err(error) if should_retry_without_tls(&error) => {
            let fallback_url = force_sslmode_disable(database_url)?;
            PgPoolOptions::new()
                .connect_with(PgConnectOptions::from_str(&fallback_url)?)
                .await
        }
        Err(error) => Err(error),
    }
}

pub async fn ping(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(pool)
        .await
        .map(|_| ())
}

async fn connect_with_url(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let mut options = PgConnectOptions::from_str(database_url)?;

    if !database_url.contains("sslmode=") {
        options = options.ssl_mode(PgSslMode::Prefer);
    }

    PgPoolOptions::new().connect_with(options).await
}

fn should_retry_without_tls(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Protocol(message) => message.contains("unexpected response from SSLRequest"),
        _ => false,
    }
}

fn force_sslmode_disable(database_url: &str) -> Result<String, sqlx::Error> {
    let mut url =
        Url::parse(database_url).map_err(|source| sqlx::Error::Protocol(source.to_string()))?;
    let mut params: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    let mut replaced = false;

    for (key, value) in &mut params {
        if key == "sslmode" {
            *value = String::from("disable");
            replaced = true;
        }
    }

    if !replaced {
        params.push((String::from("sslmode"), String::from("disable")));
    }

    url.query_pairs_mut().clear().extend_pairs(params);

    Ok(url.into())
}
