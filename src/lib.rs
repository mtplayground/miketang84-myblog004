pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod repositories;
pub mod seed;
pub mod session;
pub mod state;

use axum::{Router, extract::State, routing::{get, post}};
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{
    auth::handlers::{login_form, login_submit, logout},
    error::{AppError, AppResult},
    session::session_layer,
    state::AppState,
};

pub fn app(state: AppState) -> Router {
    let session_layer = session_layer(&state.config.session_secret);

    Router::new()
        .route("/", get(healthcheck))
        .route("/admin/login", get(login_form).post(login_submit))
        .route("/admin/logout", post(logout))
        .nest_service("/static", ServeDir::new("static"))
        .fallback(not_found)
        .layer(session_layer)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn healthcheck(State(state): State<AppState>) -> AppResult<String> {
    Ok(format!("{} is running", state.config.title))
}

async fn not_found() -> AppError {
    AppError::not_found()
}
