use std::{env, error::Error, net::SocketAddr};

use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_tracing();

    let bind_addr = read_bind_addr()?;
    let listener = TcpListener::bind(bind_addr).await?;

    info!(address = %bind_addr, "listening");

    axum::serve(listener, myblog004::app()).await?;

    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("myblog004=debug,tower_http=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer())
        .init();
}

fn read_bind_addr() -> Result<SocketAddr, std::net::AddrParseError> {
    let bind_addr = env::var("BLOG_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    bind_addr.parse()
}
