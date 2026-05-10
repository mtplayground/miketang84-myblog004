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
}

impl AppError {
    pub fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            title: "Internal Server Error",
            message: "The server could not complete this request.",
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            title: "Page Not Found",
            message: "The page you requested could not be found.",
        }
    }

    fn render_html(&self) -> String {
        let title = escape_html(self.title);
        let message = escape_html(self.message);

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
                "<body>",
                "<main>",
                "<h1>{title}</h1>",
                "<p>{message}</p>",
                "</main>",
                "</body>",
                "</html>"
            ),
            title = title,
            message = message,
        )
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

fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());

    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }

    escaped
}
