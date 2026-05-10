mod common;

use std::net::SocketAddr;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use chrono::Utc;
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

fn test_config() -> Config {
    Config {
        bind_addr: "0.0.0.0:8080"
            .parse::<SocketAddr>()
            .expect("static bind address parses"),
        database_url: String::from("postgresql:///test"),
        base_url: Url::parse("http://localhost:8080").expect("static url parses"),
        session_secret: String::from("0123456789abcdef0123456789abcdef"),
        title: String::from("Test Blog"),
        rss_limit: 20,
        admin_username: String::from("admin"),
        admin_password: String::from("password"),
    }
}

#[tokio::test]
async fn tag_listing_renders_published_posts_for_tag() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("tag_listing_renders_published_posts").await?;
    let post = PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("rust-post"),
            title: String::from("Rust Post"),
            body_md: String::from("rust md"),
            body_html: String::from("<p>rust html</p>"),
            excerpt: String::from("Rust excerpt"),
            status: PostStatus::Published,
            published_at: Some(Utc::now()),
        })
        .await?;
    let hidden_post = PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("draft-rust-post"),
            title: String::from("Draft Rust Post"),
            body_md: String::from("draft rust md"),
            body_html: String::from("<p>draft rust html</p>"),
            excerpt: String::from("Draft rust excerpt"),
            status: PostStatus::Draft,
            published_at: None,
        })
        .await?;

    let rust_tag = TagRepo::new(pool.clone())
        .upsert_by_slug(&NewTag {
            id: Uuid::new_v4(),
            slug: String::from("rust"),
            name: String::from("Rust"),
        })
        .await?;
    let web_tag = TagRepo::new(pool.clone())
        .upsert_by_slug(&NewTag {
            id: Uuid::new_v4(),
            slug: String::from("web"),
            name: String::from("Web"),
        })
        .await?;

    TagRepo::new(pool.clone())
        .replace_post_tags(post.id, &[rust_tag.id, web_tag.id])
        .await?;
    TagRepo::new(pool.clone())
        .replace_post_tags(hidden_post.id, &[rust_tag.id])
        .await?;

    let app = app(AppState::new(test_config(), pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/tags/rust")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("#rust"));
    assert!(body.contains("Rust Post"));
    assert!(body.contains("Rust excerpt"));
    assert!(body.contains("/posts/rust-post"));
    assert!(body.contains("/tags/web"));
    assert!(!body.contains("Draft Rust Post"));

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn tag_listing_returns_not_found_when_no_published_posts_exist() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("tag_listing_returns_not_found").await?;
    let draft_post = PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("draft-rust-post"),
            title: String::from("Draft Rust Post"),
            body_md: String::from("draft rust md"),
            body_html: String::from("<p>draft rust html</p>"),
            excerpt: String::from("Draft rust excerpt"),
            status: PostStatus::Draft,
            published_at: None,
        })
        .await?;
    let rust_tag = TagRepo::new(pool.clone())
        .upsert_by_slug(&NewTag {
            id: Uuid::new_v4(),
            slug: String::from("rust"),
            name: String::from("Rust"),
        })
        .await?;
    TagRepo::new(pool.clone())
        .replace_post_tags(draft_post.id, &[rust_tag.id])
        .await?;

    let app = app(AppState::new(test_config(), pool.clone()));
    let tagged_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/tags/rust")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let missing_response = app
        .oneshot(
            Request::builder()
                .uri("/tags/missing")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(tagged_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);

    common::reset_database(&pool).await?;
    Ok(())
}
