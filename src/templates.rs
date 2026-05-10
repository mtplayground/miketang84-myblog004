use askama::Template;
use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use crate::error::AppError;

pub struct HtmlTemplate<T>(pub T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(rendered) => (
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                rendered,
            )
                .into_response(),
            Err(_) => AppError::internal().into_response(),
        }
    }
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub blog_title: String,
    pub page_title: String,
    pub heading: String,
    pub message: String,
}

#[derive(Template)]
#[template(path = "admin/login.html")]
pub struct AdminLoginTemplate {
    pub blog_title: String,
    pub page_title: String,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
pub struct AdminDashboardTemplate {
    pub blog_title: String,
    pub page_title: String,
    pub admin_id: String,
}

pub fn render_template_response<T>(status: StatusCode, template: T) -> Response
where
    T: Template,
{
    (status, HtmlTemplate(template)).into_response()
}
