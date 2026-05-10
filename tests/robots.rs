mod common;

use std::net::SocketAddr;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use myblog004::{app, config::Config, state::AppState};
use tower::ServiceExt;
use url::Url;

fn test_config() -> Config {
    Config {
        bind_addr: "0.0.0.0:8080"
            .parse::<SocketAddr>()
            .expect("static bind address parses"),
        database_url: String::from("postgresql:///test"),
        base_url: Url::parse("http://localhost:8080/").expect("static url parses"),
        session_secret: String::from("0123456789abcdef0123456789abcdef"),
        title: String::from("Test Blog"),
        admin_username: String::from("admin"),
        admin_password: String::from("password"),
    }
}

#[tokio::test]
async fn robots_txt_disallows_admin_and_references_sitemap() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("robots_txt_disallows_admin").await?;
    let app = app(AppState::new(test_config(), pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/robots.txt")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).and_then(|value| value.to_str().ok()),
        Some("text/plain; charset=utf-8")
    );

    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert_eq!(
        body,
        "User-agent: *\nAllow: /\nDisallow: /admin/\n\nSitemap: http://localhost:8080/sitemap.xml\n"
    );

    common::reset_database(&pool).await?;
    Ok(())
}
