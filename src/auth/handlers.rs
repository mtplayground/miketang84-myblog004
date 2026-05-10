use axum::{
    Form,
    extract::{Extension, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions_cookie_store::Session;

use crate::{
    auth::password::verify_password,
    error::AppError,
    repositories::admins::AdminRepo,
    state::AppState,
};

const ADMIN_SESSION_KEY: &str = "admin_id";
const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=131072,t=16,p=2$jqwcGfuQWNjaXRlD6/CTnQ$OMwAFM7qZjXvyFS7Pqq6M1AC5U6oAXNXdidFKBJ7fyc";

#[derive(Deserialize)]
pub struct LoginFormData {
    username: String,
    password: String,
}

pub async fn login_form() -> Html<String> {
    render_login_form(None)
}

pub async fn login_submit(
    State(state): State<AppState>,
    Extension(session): Extension<Session>,
    Form(form): Form<LoginFormData>,
) -> Result<Response, AppError> {
    let repo = AdminRepo::new(state.db_pool.clone());
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

    Ok(login_error_response())
}

pub async fn logout(Extension(session): Extension<Session>) -> Result<Response, AppError> {
    session.flush().await.map_err(|_| AppError::internal())?;

    Ok(Redirect::to("/admin/login").into_response())
}

fn login_error_response() -> Response {
    (StatusCode::UNAUTHORIZED, render_login_form(Some("Invalid username or password.")))
        .into_response()
}

fn render_login_form(error_message: Option<&str>) -> Html<String> {
    let error_block = error_message
        .map(|message| format!(r#"<p class="error">{message}</p>"#))
        .unwrap_or_default();

    Html(format!(
        concat!(
            "<!DOCTYPE html>",
            "<html lang=\"en\">",
            "<head>",
            "<meta charset=\"utf-8\">",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">",
            "<title>Admin Login</title>",
            "<link rel=\"stylesheet\" href=\"/static/css/site.css\">",
            "</head>",
            "<body>",
            "<main>",
            "<h1>Admin Login</h1>",
            "{error_block}",
            "<form method=\"post\" action=\"/admin/login\">",
            "<label for=\"username\">Username</label>",
            "<input id=\"username\" name=\"username\" type=\"text\" autocomplete=\"username\" required>",
            "<label for=\"password\">Password</label>",
            "<input id=\"password\" name=\"password\" type=\"password\" autocomplete=\"current-password\" required>",
            "<button type=\"submit\">Sign in</button>",
            "</form>",
            "</main>",
            "</body>",
            "</html>"
        ),
        error_block = error_block,
    ))
}
