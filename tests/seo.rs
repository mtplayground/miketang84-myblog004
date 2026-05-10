mod common;

use std::net::SocketAddr;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use myblog004::{
    app,
    config::Config,
    repositories::{
        posts::{NewPost, PostRepo, PostStatus},
        tags::{NewTag, TagRepo},
    },
    state::AppState,
};
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;

fn test_config(database_url: String) -> Config {
    Config {
        bind_addr: "0.0.0.0:8080"
            .parse::<SocketAddr>()
            .expect("static bind address parses"),
        database_url,
        base_url: Url::parse("http://localhost:8080/").expect("static url parses"),
        session_secret: String::from("0123456789abcdef0123456789abcdef"),
        title: String::from("Test Blog"),
        admin_username: String::from("admin"),
        admin_password: String::from("password"),
    }
}

async fn seed_published_post(
    pool: &sqlx::PgPool,
    slug: &str,
    title: &str,
    excerpt: &str,
) -> Result<myblog004::repositories::posts::Post, Box<dyn std::error::Error>> {
    let post = PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: slug.to_string(),
            title: title.to_string(),
            body_md: String::from("body"),
            body_html: String::from("<p>body</p>"),
            excerpt: excerpt.to_string(),
            status: PostStatus::Published,
            published_at: Some(chrono::Utc::now()),
        })
        .await?;

    Ok(post)
}

#[tokio::test]
async fn home_page_renders_seo_meta_tags() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("home_page_renders_seo_meta_tags").await?;
    let state = AppState::new(test_config(String::from("postgresql:///test")), pool.clone());
    let app = app(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/?page=2")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("<title>Page 2 | Test Blog</title>"));
    assert!(body.contains("meta name=\"description\" content=\"Published posts page 2 from Test Blog.\""));
    assert!(body.contains("link rel=\"canonical\" href=\"http://localhost:8080/?page=2\""));
    assert!(body.contains("meta property=\"og:type\" content=\"website\""));
    assert!(body.contains("meta property=\"og:url\" content=\"http://localhost:8080/?page=2\""));

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn post_detail_renders_article_seo_meta_tags() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("post_detail_renders_article_seo_meta_tags").await?;
    seed_published_post(&pool, "hello-rust", "Hello Rust", "A compact excerpt for SEO").await?;
    let state = AppState::new(test_config(String::from("postgresql:///test")), pool.clone());
    let app = app(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/posts/hello-rust")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("<title>Hello Rust | Test Blog</title>"));
    assert!(body.contains("meta name=\"description\" content=\"A compact excerpt for SEO\""));
    assert!(body.contains("link rel=\"canonical\" href=\"http://localhost:8080/posts/hello-rust\""));
    assert!(body.contains("meta property=\"og:type\" content=\"article\""));
    assert!(body.contains("meta property=\"og:url\" content=\"http://localhost:8080/posts/hello-rust\""));

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn tag_and_about_pages_render_seo_meta_tags() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("tag_and_about_pages_render_seo_meta_tags").await?;
    let post = seed_published_post(&pool, "tagged-post", "Tagged Post", "Tagged excerpt").await?;
    let tag_repo = TagRepo::new(pool.clone());
    let rust_tag = tag_repo
        .upsert_by_slug(&NewTag {
            id: Uuid::new_v4(),
            slug: String::from("rust"),
            name: String::from("Rust"),
        })
        .await?;
    tag_repo.replace_post_tags(post.id, &[rust_tag.id]).await?;

    let state = AppState::new(test_config(String::from("postgresql:///test")), pool.clone());
    let app = app(state);

    let tag_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tags/rust")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let about_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/about")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(tag_response.status(), StatusCode::OK);
    let tag_body = String::from_utf8(to_bytes(tag_response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(tag_body.contains("<title>Tag: #rust | Test Blog</title>"));
    assert!(tag_body.contains("meta name=\"description\" content=\"Published posts tagged #rust on Test Blog.\""));
    assert!(tag_body.contains("link rel=\"canonical\" href=\"http://localhost:8080/tags/rust\""));
    assert!(tag_body.contains("meta property=\"og:type\" content=\"website\""));

    assert_eq!(about_response.status(), StatusCode::OK);
    let about_body = String::from_utf8(to_bytes(about_response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(about_body.contains("<title>About | Test Blog</title>"));
    assert!(about_body.contains("link rel=\"canonical\" href=\"http://localhost:8080/about\""));
    assert!(about_body.contains("meta property=\"og:type\" content=\"website\""));
    assert!(about_body.contains("meta name=\"description\""));

    common::reset_database(&pool).await?;
    Ok(())
}
