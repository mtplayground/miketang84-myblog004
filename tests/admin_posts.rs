mod common;

use std::net::SocketAddr;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use myblog004::{
    app,
    config::Config,
    repositories::{posts::PostRepo, tags::TagRepo},
    seed::seed_admin,
    state::AppState,
};
use tower::ServiceExt;
use url::{Url, form_urlencoded};

fn test_config(database_url: String) -> Config {
    Config {
        bind_addr: "0.0.0.0:8080"
            .parse::<SocketAddr>()
            .expect("static bind address parses"),
        database_url,
        base_url: Url::parse("http://localhost:8080").expect("static url parses"),
        session_secret: String::from("0123456789abcdef0123456789abcdef"),
        title: String::from("Test Blog"),
        admin_username: String::from("admin"),
        admin_password: String::from("password"),
    }
}

async fn login_cookie(
    app: Router,
    username: &str,
    password: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("username", username)
        .append_pair("password", password)
        .finish();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await?;

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .expect("login should set a session cookie")
        .to_string();

    Ok(cookie)
}

#[tokio::test]
async fn new_post_form_renders_for_authenticated_admin() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("new_post_form_renders").await?;
    seed_admin(&pool, "admin", "password").await?;
    let app = app(AppState::new(
        test_config(String::from("postgresql:///test")),
        pool.clone(),
    ));
    let cookie = login_cookie(app.clone(), "admin", "password").await?;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/posts/new")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("Create a new post"));
    assert!(body.contains("action=\"/admin/posts\""));
    assert!(body.contains("name=\"title\""));
    assert!(body.contains("name=\"tags_csv\""));
    assert!(body.contains("name=\"body_md\""));

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn create_post_persists_slug_markdown_and_tags() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("create_post_persists").await?;
    seed_admin(&pool, "admin", "password").await?;
    let app = app(AppState::new(
        test_config(String::from("postgresql:///test")),
        pool.clone(),
    ));
    let cookie = login_cookie(app.clone(), "admin", "password").await?;

    let form_body = form_urlencoded::Serializer::new(String::new())
        .append_pair("title", "Café crème & Rust")
        .append_pair("slug", "")
        .append_pair("tags_csv", "Rust, Web Notes, Café")
        .append_pair("body_md", "# Heading\n\nHello **world**")
        .append_pair("status", "published")
        .finish();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/posts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, cookie)
                .body(Body::from(form_body))
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(header::LOCATION).and_then(|value| value.to_str().ok()),
        Some("/admin")
    );

    let repo = PostRepo::new(pool.clone());
    let post = repo
        .find_by_slug("cafe-creme-rust")
        .await?
        .expect("post should be created");
    assert_eq!(post.title, "Café crème & Rust");
    assert_eq!(post.status, "published");
    assert!(post.published_at.is_some());
    assert!(post.body_html.contains("<h1>Heading</h1>"));
    assert!(post.body_html.contains("<strong>world</strong>"));

    let tag_repo = TagRepo::new(pool.clone());
    let mut tag_slugs = tag_repo
        .list_for_post(post.id)
        .await?
        .into_iter()
        .map(|tag| tag.slug)
        .collect::<Vec<_>>();
    tag_slugs.sort();

    assert_eq!(tag_slugs, vec!["cafe", "rust", "web-notes"]);

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn create_post_validation_rerenders_form() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("create_post_validation").await?;
    seed_admin(&pool, "admin", "password").await?;
    let app = app(AppState::new(
        test_config(String::from("postgresql:///test")),
        pool.clone(),
    ));
    let cookie = login_cookie(app.clone(), "admin", "password").await?;

    let form_body = form_urlencoded::Serializer::new(String::new())
        .append_pair("title", "   ")
        .append_pair("slug", "custom slug")
        .append_pair("tags_csv", "Rust")
        .append_pair("body_md", "Body stays on the form")
        .append_pair("status", "draft")
        .finish();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/posts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, cookie)
                .body(Body::from(form_body))
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("Title is required."));
    assert!(body.contains("Body stays on the form"));
    assert!(body.contains("custom slug"));
    assert!(PostRepo::new(pool.clone()).list_all_admin().await?.is_empty());

    common::reset_database(&pool).await?;
    Ok(())
}
