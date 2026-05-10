pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod repositories;
pub mod seed;
pub mod session;
pub mod state;

use axum::{Router, extract::State, routing::{get, post}};
use axum::middleware;
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{
    auth::{
        guard::{AuthenticatedAdmin, require_admin_auth},
        handlers::{login_form, login_submit, logout},
    },
    error::{AppError, AppResult},
    session::session_layer,
    state::AppState,
};

pub fn app(state: AppState) -> Router {
    let session_layer = session_layer(&state.config.session_secret);
    let admin_routes = Router::new()
        .route("/login", get(login_form).post(login_submit))
        .merge(
            Router::new()
                .route("/", get(admin_home))
                .route("/logout", post(logout))
                .route_layer(middleware::from_fn(require_admin_auth)),
        );

    Router::new()
        .route("/", get(healthcheck))
        .nest("/admin", admin_routes)
        .nest_service("/static", ServeDir::new("static"))
        .fallback(not_found)
        .layer(session_layer)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn healthcheck(State(state): State<AppState>) -> AppResult<String> {
    Ok(format!("{} is running", state.config.title))
}

async fn admin_home(authenticated_admin: AuthenticatedAdmin) -> AppResult<String> {
    Ok(format!("Authenticated admin {}", authenticated_admin.admin_id))
}

async fn not_found() -> AppError {
    AppError::not_found()
}
