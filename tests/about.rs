use std::net::SocketAddr;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use myblog004::{app, config::Config, state::AppState};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use url::Url;

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
async fn about_page_renders_markdown_file() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgresql:///test")?;
    let app = app(AppState::new(test_config(), pool));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/about")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("About This Blog"));
    assert!(body.contains("small Rust application"));
    assert!(body.contains("<li>Written in Rust</li>"));

    Ok(())
}
