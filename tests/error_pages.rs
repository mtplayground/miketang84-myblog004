mod common;

use std::net::SocketAddr;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    response::IntoResponse,
};
use myblog004::{
    app,
    config::Config,
    error::AppError,
    state::AppState,
};
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
        rss_limit: 20,
        admin_username: String::from("admin"),
        admin_password: String::from("password"),
    }
}

async fn body_text(response: axum::response::Response) -> Result<String, Box<dyn std::error::Error>> {
    Ok(String::from_utf8(
        to_bytes(response.into_body(), usize::MAX).await?.to_vec(),
    )?)
}

#[tokio::test]
async fn friendly_not_found_pages_cover_routes_and_missing_content() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("friendly_not_found_pages").await?;
    let app = app(AppState::new(test_config(), pool.clone()));

    for path in ["/missing-route", "/posts/missing-post", "/tags/missing-tag"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await?;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = body_text(response).await?;
        assert!(body.contains("Page Not Found"));
        assert!(body.contains("We couldn&#39;t find the page, post, or tag you requested."));
        assert!(body.contains("Back home"));
        assert!(body.contains("About this site"));
    }

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn internal_error_page_is_generic_and_safe() -> Result<(), Box<dyn std::error::Error>> {
    let response = AppError::internal().into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = body_text(response).await?;
    assert!(body.contains("Internal Server Error"));
    assert!(body.contains("Something went wrong on our side."));
    assert!(body.contains("Please try again in a moment."));
    assert!(!body.contains("sqlx"));
    assert!(!body.contains("panic"));

    Ok(())
}
