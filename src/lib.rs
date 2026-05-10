pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod markdown;
pub mod repositories;
pub mod seed;
pub mod session;
pub mod slug;
pub mod state;
pub mod templates;

use axum::middleware;
use axum::{
    extract::{Path, Query},
    Router,
    extract::State,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{
    auth::{
        guard::{AuthenticatedAdmin, require_admin_auth},
        handlers::{login_form, login_submit, logout},
    },
    error::{AppError, AppResult},
    markdown::MarkdownRenderer,
    repositories::{posts::PostRepo, tags::TagRepo},
    session::session_layer,
    state::AppState,
    templates::{
        AdminDashboardPostTemplate, AdminDashboardTemplate, HomePostTemplate, HomeTemplate,
        HtmlTemplate, PostDetailTemplate, StaticPageTemplate, TagChipTemplate, TagListingTemplate,
    },
};

const HOME_PAGE_SIZE: i64 = 10;
const ABOUT_CONTENT_PATH: &str = "content/about.md";

#[derive(Debug, Deserialize)]
struct HomePageParams {
    page: Option<i64>,
}

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
        .route("/about", get(about_page))
        .route("/posts/{slug}", get(post_detail))
        .route("/tags/{tag}", get(tag_listing))
        .nest("/admin", admin_routes)
        .nest_service("/static", ServeDir::new("static"))
        .fallback(not_found)
        .layer(session_layer)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn healthcheck(
    State(state): State<AppState>,
    Query(params): Query<HomePageParams>,
) -> AppResult<HtmlTemplate<HomeTemplate>> {
    let current_page = params.page.unwrap_or(1).max(1);
    let posts = PostRepo::new(state.db_pool.clone())
        .list_published(current_page, HOME_PAGE_SIZE)
        .await
        .map_err(|_| AppError::internal())?;
    let tag_repo = TagRepo::new(state.db_pool.clone());
    let mut home_posts = Vec::with_capacity(posts.len());

    for post in posts {
        let tags = tag_repo
            .list_for_post(post.id)
            .await
            .map_err(|_| AppError::internal())?;

        home_posts.push(HomePostTemplate {
            slug: post.slug,
            title: post.title,
            published_on: display_timestamp(post.published_at.unwrap_or(post.created_at)),
            excerpt: post.excerpt,
            tags: tags
                .into_iter()
                .map(|tag| TagChipTemplate {
                    name: tag.name,
                    slug: tag.slug,
                })
                .collect(),
        });
    }

    let next_page = if home_posts.len() as i64 == HOME_PAGE_SIZE {
        Some(current_page + 1)
    } else {
        None
    };

    Ok(HtmlTemplate(HomeTemplate {
        blog_title: state.config.title.clone(),
        page_title: String::from("Home"),
        posts: home_posts,
        current_page,
        previous_page: (current_page > 1).then_some(current_page - 1),
        next_page,
    }))
}

async fn post_detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<HtmlTemplate<PostDetailTemplate>> {
    let post = PostRepo::new(state.db_pool.clone())
        .find_by_slug(&slug)
        .await
        .map_err(|_| AppError::internal())?;
    let Some(post) = post else {
        return Err(AppError::not_found());
    };

    if post.status != "published" {
        return Err(AppError::not_found());
    }

    let tags = TagRepo::new(state.db_pool.clone())
        .list_for_post(post.id)
        .await
        .map_err(|_| AppError::internal())?;
    let canonical_url = state
        .config
        .base_url
        .join(&format!("posts/{}", post.slug))
        .map_err(|_| AppError::internal())?;

    Ok(HtmlTemplate(PostDetailTemplate {
        blog_title: state.config.title.clone(),
        page_title: post.title.clone(),
        seo_description: post.excerpt.clone(),
        canonical_url: canonical_url.to_string(),
        title: post.title,
        published_on: display_timestamp(post.published_at.unwrap_or(post.created_at)),
        body_html: post.body_html,
        tags: tags
            .into_iter()
            .map(|tag| TagChipTemplate {
                name: tag.name,
                slug: tag.slug,
            })
            .collect(),
    }))
}

async fn tag_listing(
    State(state): State<AppState>,
    Path(tag): Path<String>,
) -> AppResult<HtmlTemplate<TagListingTemplate>> {
    let posts = TagRepo::new(state.db_pool.clone())
        .posts_by_tag_slug(&tag)
        .await
        .map_err(|_| AppError::internal())?;

    if posts.is_empty() {
        return Err(AppError::not_found());
    }

    let tag_repo = TagRepo::new(state.db_pool.clone());
    let mut listed_posts = Vec::with_capacity(posts.len());

    for post in posts {
        let tags = tag_repo
            .list_for_post(post.id)
            .await
            .map_err(|_| AppError::internal())?;

        listed_posts.push(HomePostTemplate {
            slug: post.slug,
            title: post.title,
            published_on: display_timestamp(post.published_at.unwrap_or(post.created_at)),
            excerpt: post.excerpt,
            tags: tags
                .into_iter()
                .map(|tag| TagChipTemplate {
                    name: tag.name,
                    slug: tag.slug,
                })
                .collect(),
        });
    }

    Ok(HtmlTemplate(TagListingTemplate {
        blog_title: state.config.title.clone(),
        page_title: format!("Tag: {tag}"),
        tag_slug: tag,
        posts: listed_posts,
    }))
}

async fn admin_home(
    State(state): State<AppState>,
    _authenticated_admin: AuthenticatedAdmin,
) -> AppResult<HtmlTemplate<AdminDashboardTemplate>> {
    let posts = PostRepo::new(state.db_pool.clone())
        .list_all_admin()
        .await
        .map_err(|_| AppError::internal())?;
    let dashboard_posts = posts
        .into_iter()
        .map(|post| {
            let is_published = post.status == "published";
            AdminDashboardPostTemplate {
                title: post.title,
                slug: post.slug.clone(),
                status_label: if is_published {
                    String::from("Published")
                } else {
                    String::from("Draft")
                },
                status_class: if is_published {
                    String::from("status-badge--published")
                } else {
                    String::from("status-badge--draft")
                },
                published_on: post
                    .published_at
                    .map(display_timestamp)
                    .unwrap_or_else(|| String::from("Not published")),
                edit_url: format!("/admin/posts/{}/edit", post.slug),
                toggle_url: format!(
                    "/admin/posts/{}/{}",
                    post.slug,
                    if is_published { "unpublish" } else { "publish" }
                ),
                toggle_label: if is_published {
                    String::from("Unpublish")
                } else {
                    String::from("Publish")
                },
                delete_url: format!("/admin/posts/{}/delete", post.slug),
            }
        })
        .collect();

    Ok(HtmlTemplate(AdminDashboardTemplate {
        blog_title: state.config.title.clone(),
        page_title: String::from("Admin Dashboard"),
        posts: dashboard_posts,
    }))
}

async fn about_page(State(state): State<AppState>) -> AppResult<HtmlTemplate<StaticPageTemplate>> {
    let markdown = tokio::fs::read_to_string(ABOUT_CONTENT_PATH)
        .await
        .map_err(|_| AppError::internal())?;
    let body_html = MarkdownRenderer::new().render(&markdown);

    Ok(HtmlTemplate(StaticPageTemplate {
        blog_title: state.config.title.clone(),
        page_title: String::from("About"),
        title: String::from("About"),
        body_html,
    }))
}

async fn not_found() -> AppError {
    AppError::not_found()
}

fn display_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.format("%b %d, %Y").to_string()
}
