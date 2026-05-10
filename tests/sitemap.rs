mod common;

use std::net::SocketAddr;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use chrono::Utc;
use myblog004::{
    app,
    config::Config,
    repositories::posts::{NewPost, PostRepo, PostStatus},
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
        base_url: Url::parse("http://localhost:8080/").expect("static url parses"),
        session_secret: String::from("0123456789abcdef0123456789abcdef"),
        title: String::from("Test Blog"),
        rss_limit: 20,
        admin_username: String::from("admin"),
        admin_password: String::from("password"),
    }
}

#[tokio::test]
async fn sitemap_xml_lists_fixed_routes_and_published_posts() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("sitemap_xml_lists_routes").await?;
    PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("published-post"),
            title: String::from("Published Post"),
            body_md: String::from("published body"),
            body_html: String::from("<p>published body</p>"),
            excerpt: String::from("published excerpt"),
            status: PostStatus::Published,
            published_at: Some(Utc::now()),
        })
        .await?;
    PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("draft-post"),
            title: String::from("Draft Post"),
            body_md: String::from("draft body"),
            body_html: String::from("<p>draft body</p>"),
            excerpt: String::from("draft excerpt"),
            status: PostStatus::Draft,
            published_at: None,
        })
        .await?;

    let app = app(AppState::new(test_config(), pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sitemap.xml")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).and_then(|value| value.to_str().ok()),
        Some("application/xml; charset=utf-8")
    );

    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(body.contains("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">"));
    assert!(body.contains("<loc>http://localhost:8080/</loc>"));
    assert!(body.contains("<loc>http://localhost:8080/about</loc>"));
    assert!(body.contains("<loc>http://localhost:8080/posts/published-post</loc>"));
    assert!(!body.contains("draft-post"));
    assert!(body.contains("<lastmod>"));

    common::reset_database(&pool).await?;
    Ok(())
}
