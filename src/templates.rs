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
    pub posts: Vec<HomePostTemplate>,
    pub current_page: i64,
    pub previous_page: Option<i64>,
    pub next_page: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct HomePostTemplate {
    pub slug: String,
    pub title: String,
    pub published_on: String,
    pub excerpt: String,
    pub tags: Vec<TagChipTemplate>,
}

#[derive(Clone, Debug)]
pub struct TagChipTemplate {
    pub name: String,
    pub slug: String,
}

#[derive(Template)]
#[template(path = "post_detail.html")]
pub struct PostDetailTemplate {
    pub blog_title: String,
    pub page_title: String,
    pub seo_description: String,
    pub canonical_url: String,
    pub title: String,
    pub published_on: String,
    pub body_html: String,
    pub tags: Vec<TagChipTemplate>,
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
