use std::error::Error;

use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use myblog004::{config::Config, db, state::AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_tracing();

    let config = Config::from_env()?;
    let db_pool = db::connect(&config.database_url).await?;
    db::ping(&db_pool).await?;
    db::run_migrations(&db_pool).await?;
    let bind_addr = config.bind_addr;
    let listener = TcpListener::bind(bind_addr).await?;

    info!(
        address = %bind_addr,
        base_url = %config.base_url,
        title = %config.title,
        "listening"
    );

    axum::serve(listener, myblog004::app(AppState::new(config, db_pool))).await?;

    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_env("BLOG_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info,myblog004=info,tower_http=info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer())
        .init();
}
