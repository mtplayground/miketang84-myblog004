mod common;

use std::net::SocketAddr;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
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
        admin_username: String::from("admin"),
        admin_password: String::from("password"),
    }
}

#[tokio::test]
async fn home_page_renders_published_posts_with_tags_and_excerpt() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = common::test_pool("home_page_renders_published_posts").await?;
    let now = Utc::now();

    let published_post = PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("published-post"),
            title: String::from("Published Post"),
            body_md: String::from("published md"),
            body_html: String::from("<p>published html</p>"),
            excerpt: String::from("Published excerpt"),
            status: PostStatus::Published,
            published_at: Some(now),
        })
        .await?;

    PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("draft-post"),
            title: String::from("Draft Post"),
            body_md: String::from("draft md"),
            body_html: String::from("<p>draft html</p>"),
            excerpt: String::from("Draft excerpt"),
            status: PostStatus::Draft,
            published_at: None,
        })
        .await?;

    let tag = TagRepo::new(pool.clone())
        .upsert_by_slug(&NewTag {
            id: Uuid::new_v4(),
            slug: String::from("rust"),
            name: String::from("Rust"),
        })
        .await?;
    TagRepo::new(pool.clone())
        .replace_post_tags(published_post.id, &[tag.id])
        .await?;

    let app = app(AppState::new(test_config(), pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("Published Post"));
    assert!(body.contains("Published excerpt"));
    assert!(body.contains("Rust"));
    assert!(body.contains("/tags/rust"));
    assert!(!body.contains("Draft Post"));

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn home_page_supports_second_page_of_published_posts() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("home_page_supports_second_page").await?;
    let repo = PostRepo::new(pool.clone());
    let base_time = Utc::now();

    for index in 0..11 {
        let title = format!("Post {index:02}");
        repo.insert(&NewPost {
            id: Uuid::new_v4(),
            slug: format!("post-{index:02}"),
            title,
            body_md: String::from("body"),
            body_html: String::from("<p>body</p>"),
            excerpt: format!("Excerpt {index:02}"),
            status: PostStatus::Published,
            published_at: Some(base_time - Duration::days(index.into())),
        })
        .await?;
    }

    let app = app(AppState::new(test_config(), pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/?page=2")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("Post 10"));
    assert!(!body.contains("Post 00"));
    assert!(body.contains("/?page=1"));
    assert!(body.contains("Page 2"));

    common::reset_database(&pool).await?;
    Ok(())
}
