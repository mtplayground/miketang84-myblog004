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
async fn post_detail_page_renders_published_post_with_tags_and_seo() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = common::test_pool("post_detail_page_renders").await?;
    let published_post = PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("published-post"),
            title: String::from("Published Post"),
            body_md: String::from("published body"),
            body_html: String::from("<p>Rendered body</p>"),
            excerpt: String::from("Published excerpt"),
            status: PostStatus::Published,
            published_at: Some(Utc::now()),
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
                .uri("/posts/published-post")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("Published Post"));
    assert!(body.contains("Rendered body"));
    assert!(body.contains("Published excerpt"));
    assert!(body.contains("/tags/rust"));
    assert!(body.contains("meta name=\"description\" content=\"Published excerpt\""));
    assert!(body.contains("property=\"og:title\" content=\"Published Post | Test Blog\""));

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn post_detail_returns_not_found_for_draft_or_missing_post() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = common::test_pool("post_detail_returns_not_found").await?;
    PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("draft-post"),
            title: String::from("Draft Post"),
            body_md: String::from("draft body"),
            body_html: String::from("<p>draft body</p>"),
            excerpt: String::from("Draft excerpt"),
            status: PostStatus::Draft,
            published_at: None,
        })
        .await?;

    let app = app(AppState::new(test_config(), pool.clone()));
    let draft_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/posts/draft-post")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let missing_response = app
        .oneshot(
            Request::builder()
                .uri("/posts/missing-post")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(draft_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);

    common::reset_database(&pool).await?;
    Ok(())
}
