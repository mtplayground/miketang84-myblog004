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
#[template(path = "tag_listing.html")]
pub struct TagListingTemplate {
    pub blog_title: String,
    pub page_title: String,
    pub tag_slug: String,
    pub posts: Vec<HomePostTemplate>,
}

#[derive(Template)]
#[template(path = "static_page.html")]
pub struct StaticPageTemplate {
    pub blog_title: String,
    pub page_title: String,
    pub title: String,
    pub body_html: String,
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
    pub posts: Vec<AdminDashboardPostTemplate>,
}

#[derive(Clone, Debug)]
pub struct AdminDashboardPostTemplate {
    pub title: String,
    pub slug: String,
    pub status_label: String,
    pub status_class: String,
    pub published_on: String,
    pub edit_url: String,
    pub toggle_url: String,
    pub toggle_label: String,
    pub delete_url: String,
}

#[derive(Template)]
#[template(path = "admin/post_form.html")]
pub struct AdminPostFormTemplate {
    pub blog_title: String,
    pub page_title: String,
    pub error_message: Option<String>,
    pub title: String,
    pub slug: String,
    pub tags_csv: String,
    pub body_md: String,
    pub is_draft: bool,
    pub is_published: bool,
}

impl AdminPostFormTemplate {
    pub fn new(
        blog_title: String,
        page_title: String,
        form: crate::admin::CreatePostFormData,
        error_message: Option<String>,
    ) -> Self {
        let is_published = form.status == "published";

        Self {
            blog_title,
            page_title,
            error_message,
            title: form.title,
            slug: form.slug,
            tags_csv: form.tags_csv,
            body_md: form.body_md,
            is_draft: !is_published,
            is_published,
        }
    }
}

pub fn render_template_response<T>(status: StatusCode, template: T) -> Response
where
    T: Template,
{
    (status, HtmlTemplate(template)).into_response()
}
