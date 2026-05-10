use askama::Template;
use axum::{
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone)]
pub struct AppError {
    status: StatusCode,
    title: &'static str,
    message: &'static str,
    detail: &'static str,
}

impl AppError {
    pub fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            title: "Internal Server Error",
            message: "Something went wrong on our side.",
            detail: "Please try again in a moment. If the problem keeps happening, return to the home page and retry from there.",
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            title: "Page Not Found",
            message: "We couldn't find the page, post, or tag you requested.",
            detail: "The link may be outdated, the content may have been removed, or the address may have been mistyped.",
        }
    }

    fn render_html(&self) -> String {
        let template = ErrorPageTemplate {
            status_code: self.status.as_u16(),
            title: self.title,
            message: self.message,
            detail: self.detail,
        };

        match template.render() {
            Ok(rendered) => rendered,
            Err(_) => render_fallback_html(self),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Html(self.render_html());

        (
            self.status,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            body,
        )
            .into_response()
    }
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPageTemplate {
    status_code: u16,
    title: &'static str,
    message: &'static str,
    detail: &'static str,
}

fn render_fallback_html(error: &AppError) -> String {
    format!(
        concat!(
            "<!DOCTYPE html>",
            "<html lang=\"en\">",
            "<head>",
            "<meta charset=\"utf-8\">",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">",
            "<title>{title}</title>",
            "<link rel=\"stylesheet\" href=\"/static/css/site.css\">",
            "</head>",
            "<body class=\"site-shell\">",
            "<main class=\"site-main\">",
            "<section class=\"hero-card\">",
            "<p class=\"eyebrow\">Error {status_code}</p>",
            "<h1>{title}</h1>",
            "<p>{message}</p>",
            "</section>",
            "<section class=\"panel\">",
            "<p>{detail}</p>",
            "<p><a href=\"/\">Back home</a></p>",
            "</section>",
            "</main>",
            "</body>",
            "</html>"
        ),
        status_code = error.status.as_u16(),
        title = error.title,
        message = error.message,
        detail = error.detail,
    )
}
