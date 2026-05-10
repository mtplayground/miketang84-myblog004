mod common;

use std::net::SocketAddr;
use std::io::Cursor;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use chrono::{Duration, Utc};
use myblog004::{
    app,
    config::Config,
    repositories::posts::{NewPost, PostRepo, PostStatus},
    state::AppState,
};
use rss::Channel;
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;

fn test_config(rss_limit: usize) -> Config {
    Config {
        bind_addr: "0.0.0.0:8080"
            .parse::<SocketAddr>()
            .expect("static bind address parses"),
        database_url: String::from("postgresql:///test"),
        base_url: Url::parse("http://localhost:8080/").expect("static url parses"),
        session_secret: String::from("0123456789abcdef0123456789abcdef"),
        title: String::from("Test Blog"),
        rss_limit,
        admin_username: String::from("admin"),
        admin_password: String::from("password"),
    }
}

#[tokio::test]
async fn rss_xml_lists_latest_published_posts_with_excerpt_and_html() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("rss_xml_lists_latest_published_posts").await?;
    let repo = PostRepo::new(pool.clone());

    repo.insert(&NewPost {
        id: Uuid::new_v4(),
        slug: String::from("older-post"),
        title: String::from("Older Post"),
        body_md: String::from("older body"),
        body_html: String::from("<p>Older HTML</p>"),
        excerpt: String::from("Older excerpt"),
        status: PostStatus::Published,
        published_at: Some(Utc::now() - Duration::days(2)),
    })
    .await?;
    repo.insert(&NewPost {
        id: Uuid::new_v4(),
        slug: String::from("newer-post"),
        title: String::from("Newer Post"),
        body_md: String::from("newer body"),
        body_html: String::from("<p>Newer HTML</p>"),
        excerpt: String::from("Newer excerpt"),
        status: PostStatus::Published,
        published_at: Some(Utc::now()),
    })
    .await?;
    repo.insert(&NewPost {
        id: Uuid::new_v4(),
        slug: String::from("draft-post"),
        title: String::from("Draft Post"),
        body_md: String::from("draft body"),
        body_html: String::from("<p>Draft HTML</p>"),
        excerpt: String::from("Draft excerpt"),
        status: PostStatus::Draft,
        published_at: None,
    })
    .await?;

    let app = app(AppState::new(test_config(1), pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/rss.xml")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).and_then(|value| value.to_str().ok()),
        Some("application/rss+xml; charset=utf-8")
    );

    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("<rss"));
    let channel = Channel::read_from(Cursor::new(body.as_bytes()))?;
    assert_eq!(channel.title(), "Test Blog");
    assert_eq!(channel.link(), "http://localhost:8080/");
    assert_eq!(channel.items().len(), 1);

    let item = &channel.items()[0];
    assert_eq!(item.title(), Some("Newer Post"));
    assert_eq!(item.link(), Some("http://localhost:8080/posts/newer-post"));
    assert_eq!(item.description(), Some("Newer excerpt"));
    assert_eq!(item.content(), Some("<p>Newer HTML</p>"));

    common::reset_database(&pool).await?;
    Ok(())
}
