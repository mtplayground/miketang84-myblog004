pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod markdown;
pub mod repositories;
pub mod seed;
pub mod session;
pub mod state;
pub mod templates;

use axum::middleware;
use axum::{
    Router,
    extract::State,
    routing::{get, post},
};
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{
    auth::{
        guard::{AuthenticatedAdmin, require_admin_auth},
        handlers::{login_form, login_submit, logout},
    },
    error::{AppError, AppResult},
    session::session_layer,
    state::AppState,
    templates::{AdminDashboardTemplate, HomeTemplate, HtmlTemplate},
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

async fn healthcheck(State(state): State<AppState>) -> AppResult<HtmlTemplate<HomeTemplate>> {
    Ok(HtmlTemplate(HomeTemplate {
        blog_title: state.config.title.clone(),
        page_title: String::from("Home"),
        heading: state.config.title.clone(),
        message: String::from("The blog server is running and ready for content."),
    }))
}

async fn admin_home(
    State(state): State<AppState>,
    authenticated_admin: AuthenticatedAdmin,
) -> AppResult<HtmlTemplate<AdminDashboardTemplate>> {
    Ok(HtmlTemplate(AdminDashboardTemplate {
        blog_title: state.config.title.clone(),
        page_title: String::from("Admin Dashboard"),
        admin_id: authenticated_admin.admin_id.to_string(),
    }))
}

async fn not_found() -> AppError {
    AppError::not_found()
}
