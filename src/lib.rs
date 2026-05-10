pub mod admin;
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
    http::header,
    response::IntoResponse,
    Router,
    extract::State,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use rss::{ChannelBuilder, ItemBuilder};
use serde::Deserialize;
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{
    admin::{
        create_post, delete_post, delete_post_confirm, edit_post_form, new_post_form,
        publish_post, unpublish_post, update_post,
    },
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
        HtmlTemplate, PostDetailTemplate, SeoMeta, StaticPageTemplate, TagChipTemplate,
        TagListingTemplate,
    },
};

const HOME_PAGE_SIZE: i64 = 10;
const ABOUT_CONTENT_PATH: &str = "content/about.md";
const SEO_DESCRIPTION_MAX_CHARS: usize = 160;

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
                .route("/posts/new", get(new_post_form))
                .route("/posts", post(create_post))
                .route("/posts/{id}/edit", get(edit_post_form))
                .route("/posts/{id}", post(update_post))
                .route("/posts/{id}/publish", post(publish_post))
                .route("/posts/{id}/unpublish", post(unpublish_post))
                .route("/posts/{id}/delete", get(delete_post_confirm).post(delete_post))
                .route("/logout", post(logout))
                .route_layer(middleware::from_fn(require_admin_auth)),
        );

    Router::new()
        .route("/", get(healthcheck))
        .route("/about", get(about_page))
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap_xml))
        .route("/rss.xml", get(rss_xml))
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
        seo: build_home_seo(&state, current_page)?,
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
        seo: SeoMeta {
            title: format!("{} | {}", post.title, state.config.title),
            description: post.excerpt.clone(),
            canonical_url: canonical_url.to_string(),
            og_type: String::from("article"),
        },
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
        seo: SeoMeta {
            title: format!("Tag: #{tag} | {}", state.config.title),
            description: format!("Published posts tagged #{tag} on {}.", state.config.title),
            canonical_url: state
                .config
                .base_url
                .join(&format!("tags/{tag}"))
                .map_err(|_| AppError::internal())?
                .to_string(),
            og_type: String::from("website"),
        },
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
                edit_url: format!("/admin/posts/{}/edit", post.id),
                toggle_url: format!(
                    "/admin/posts/{}/{}",
                    post.id,
                    if is_published { "unpublish" } else { "publish" }
                ),
                toggle_label: if is_published {
                    String::from("Unpublish")
                } else {
                    String::from("Publish")
                },
                delete_url: format!("/admin/posts/{}/delete", post.id),
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
        seo: SeoMeta {
            title: format!("About | {}", state.config.title),
            description: summarize_text(&markdown, SEO_DESCRIPTION_MAX_CHARS),
            canonical_url: state
                .config
                .base_url
                .join("about")
                .map_err(|_| AppError::internal())?
                .to_string(),
            og_type: String::from("website"),
        },
        title: String::from("About"),
        body_html,
    }))
}

async fn robots_txt(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let sitemap_url = state
        .config
        .base_url
        .join("sitemap.xml")
        .map_err(|_| AppError::internal())?;
    let body = format!(
        "User-agent: *\nAllow: /\nDisallow: /admin/\n\nSitemap: {sitemap_url}\n"
    );

    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body))
}

async fn sitemap_xml(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let posts = PostRepo::new(state.db_pool.clone())
        .list_all_admin()
        .await
        .map_err(|_| AppError::internal())?;
    let published_posts = posts
        .into_iter()
        .filter(|post| post.status == "published")
        .collect::<Vec<_>>();
    let home_lastmod = published_posts
        .iter()
        .map(|post| post.updated_at)
        .max()
        .unwrap_or_else(Utc::now);
    let about_lastmod = tokio::fs::metadata(ABOUT_CONTENT_PATH)
        .await
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(DateTime::<Utc>::from)
        .unwrap_or(home_lastmod);

    let mut body = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    );
    body.push_str(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
    body.push_str(&render_sitemap_entry(
        state.config.base_url.as_str(),
        home_lastmod,
    ));
    body.push_str(&render_sitemap_entry(
        state
            .config
            .base_url
            .join("about")
            .map_err(|_| AppError::internal())?
            .as_str(),
        about_lastmod,
    ));

    for post in published_posts {
        let loc = state
            .config
            .base_url
            .join(&format!("posts/{}", post.slug))
            .map_err(|_| AppError::internal())?;
        body.push_str(&render_sitemap_entry(loc.as_str(), post.updated_at));
    }

    body.push_str("</urlset>");

    Ok(([(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body))
}

async fn rss_xml(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let posts = PostRepo::new(state.db_pool.clone())
        .list_published(1, state.config.rss_limit as i64)
        .await
        .map_err(|_| AppError::internal())?;
    let items = posts
        .into_iter()
        .map(|post| {
            let link = state
                .config
                .base_url
                .join(&format!("posts/{}", post.slug))
                .map_err(|_| AppError::internal())?;
            let pub_date = post.published_at.unwrap_or(post.created_at).to_rfc2822();

            Ok::<_, AppError>(
                ItemBuilder::default()
                    .title(Some(post.title))
                    .link(Some(link.to_string()))
                    .guid(Some(rss::Guid {
                        value: link.to_string(),
                        permalink: true,
                    }))
                    .pub_date(Some(pub_date))
                    .description(Some(post.excerpt))
                    .content(Some(post.body_html))
                    .build(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let channel = ChannelBuilder::default()
        .title(state.config.title.clone())
        .link(state.config.base_url.to_string())
        .description(format!("Latest posts from {}.", state.config.title))
        .items(items)
        .build();

    Ok(([(header::CONTENT_TYPE, "application/rss+xml; charset=utf-8")], channel.to_string()))
}

async fn not_found() -> AppError {
    AppError::not_found()
}

fn display_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.format("%b %d, %Y").to_string()
}

fn build_home_seo(state: &AppState, current_page: i64) -> Result<SeoMeta, AppError> {
    let canonical_url = if current_page <= 1 {
        state.config.base_url.clone()
    } else {
        url::Url::parse(&format!("{}?page={current_page}", state.config.base_url))
            .map_err(|_| AppError::internal())?
    };
    let title = if current_page <= 1 {
        state.config.title.clone()
    } else {
        format!("Page {current_page} | {}", state.config.title)
    };
    let description = if current_page <= 1 {
        format!("Recent published posts from {}.", state.config.title)
    } else {
        format!("Published posts page {current_page} from {}.", state.config.title)
    };

    Ok(SeoMeta {
        title,
        description,
        canonical_url: canonical_url.to_string(),
        og_type: String::from("website"),
    })
}

fn summarize_text(input: &str, max_chars: usize) -> String {
    let collapsed = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut summary = String::new();

    for ch in collapsed.chars() {
        if summary.chars().count() >= max_chars {
            break;
        }

        summary.push(ch);
    }

    if collapsed.chars().count() > max_chars {
        summary.push('…');
    }

    summary
}

fn render_sitemap_entry(loc: &str, lastmod: DateTime<Utc>) -> String {
    format!(
        "<url><loc>{}</loc><lastmod>{}</lastmod></url>",
        escape_xml(loc),
        escape_xml(&lastmod.to_rfc3339()),
    )
}

fn escape_xml(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());

    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }

    escaped
}
