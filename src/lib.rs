pub mod config;
pub mod db;
pub mod state;

use axum::{Router, extract::State, routing::get};
use tower_http::trace::TraceLayer;

use crate::state::AppState;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(healthcheck))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn healthcheck(State(state): State<AppState>) -> String {
    format!("{} is running", state.config.title)
}
