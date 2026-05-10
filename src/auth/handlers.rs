use axum::{
    Form,
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions_cookie_store::Session;

use crate::{
    auth::ADMIN_SESSION_KEY,
    auth::password::verify_password,
    error::AppError,
    repositories::admins::AdminRepo,
    state::AppState,
    templates::{AdminLoginTemplate, HtmlTemplate, render_template_response},
};

const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=131072,t=16,p=2$jqwcGfuQWNjaXRlD6/CTnQ$OMwAFM7qZjXvyFS7Pqq6M1AC5U6oAXNXdidFKBJ7fyc";

#[derive(Deserialize)]
pub struct LoginFormData {
    username: String,
    password: String,
}

pub async fn login_form(State(state): State<AppState>) -> HtmlTemplate<AdminLoginTemplate> {
    render_login_form(&state.config.title, None)
}

pub async fn login_submit(
    State(state): State<AppState>,
    Extension(session): Extension<Session>,
    Form(form): Form<LoginFormData>,
) -> Result<Response, AppError> {
    let repo = AdminRepo::new(state.db_pool.clone());
    let blog_title = state.config.title.clone();
    let admin = repo
        .find_by_username(&form.username)
        .await
        .map_err(|_| AppError::internal())?;

    let password_hash = admin
        .as_ref()
        .map(|admin| admin.password_hash.clone())
        .unwrap_or_else(|| DUMMY_PASSWORD_HASH.to_string());
    let verified =
        verify_password(&form.password, &password_hash).map_err(|_| AppError::internal())?;

    if let Some(admin) = admin.filter(|_| verified) {
        session
            .insert(ADMIN_SESSION_KEY, admin.id.to_string())
            .await
            .map_err(|_| AppError::internal())?;

        return Ok(Redirect::to("/admin").into_response());
    }

    Ok(login_error_response(&blog_title))
}

pub async fn logout(Extension(session): Extension<Session>) -> Result<Response, AppError> {
    session.flush().await.map_err(|_| AppError::internal())?;

    Ok(Redirect::to("/admin/login").into_response())
}

fn login_error_response(blog_title: &str) -> Response {
    render_template_response(
        StatusCode::UNAUTHORIZED,
        AdminLoginTemplate {
            blog_title: blog_title.to_string(),
            page_title: String::from("Admin Login"),
            error_message: Some(String::from("Invalid username or password.")),
        },
    )
}

fn render_login_form(blog_title: &str, error_message: Option<&str>) -> HtmlTemplate<AdminLoginTemplate> {
    HtmlTemplate(AdminLoginTemplate {
        blog_title: blog_title.to_string(),
        page_title: String::from("Admin Login"),
        error_message: error_message.map(str::to_string),
    })
}
