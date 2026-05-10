use sqlx::PgPool;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db_pool: PgPool,
}

impl AppState {
    pub fn new(config: Config, db_pool: PgPool) -> Self {
        Self { config, db_pool }
    }
}
