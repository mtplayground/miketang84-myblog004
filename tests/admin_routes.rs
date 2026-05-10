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
    repositories::{
        posts::PostRepo,
        tags::TagRepo,
    },
    seed::seed_admin,
    state::AppState,
};
use tower::ServiceExt;
use url::{Url, form_urlencoded};

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
async fn admin_guard_and_login_flow_render_dashboard() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("admin_guard_and_login_flow").await?;
    seed_admin(&pool, "admin", "password").await?;
    let app = app(AppState::new(test_config(), pool.clone()));

    let redirect_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    assert_eq!(redirect_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        redirect_response.headers().get(header::LOCATION).and_then(|value| value.to_str().ok()),
        Some("/admin/login")
    );

    let login_page = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/login")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    assert_eq!(login_page.status(), StatusCode::OK);
    let login_page_body = String::from_utf8(to_bytes(login_page.into_body(), usize::MAX).await?.to_vec())?;
    assert!(login_page_body.contains("Sign in"));

    let session_cookie = login_cookie(app.clone(), "admin", "password").await?;
    let dashboard = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    assert_eq!(dashboard.status(), StatusCode::OK);
    let dashboard_body = String::from_utf8(to_bytes(dashboard.into_body(), usize::MAX).await?.to_vec())?;
    assert!(dashboard_body.contains("Authenticated admin dashboard"));
    assert!(dashboard_body.contains("New post"));

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn admin_can_create_edit_publish_and_delete_post() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("admin_can_create_edit_publish_delete").await?;
    seed_admin(&pool, "admin", "password").await?;
    let app = app(AppState::new(test_config(), pool.clone()));
    let session_cookie = login_cookie(app.clone(), "admin", "password").await?;

    let create_body = form_urlencoded::Serializer::new(String::new())
        .append_pair("title", "Draft Route Test")
        .append_pair("slug", "")
        .append_pair("tags_csv", "Rust, Testing")
        .append_pair("body_md", "Draft **content**")
        .append_pair("status", "draft")
        .finish();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/posts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, &session_cookie)
                .body(Body::from(create_body))
                .expect("request builds"),
        )
        .await?;
    assert_eq!(create_response.status(), StatusCode::SEE_OTHER);

    let repo = PostRepo::new(pool.clone());
    let created_post = repo
        .find_by_slug("draft-route-test")
        .await?
        .expect("created draft exists");
    assert_eq!(created_post.status, "draft");

    let home_before_publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let home_before_publish_body =
        String::from_utf8(to_bytes(home_before_publish.into_body(), usize::MAX).await?.to_vec())?;
    assert!(!home_before_publish_body.contains("Draft Route Test"));

    let dashboard = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin")
                .header(header::COOKIE, &session_cookie)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let dashboard_body = String::from_utf8(to_bytes(dashboard.into_body(), usize::MAX).await?.to_vec())?;
    assert!(dashboard_body.contains("Draft Route Test"));
    assert!(dashboard_body.contains("Draft"));

    let edit_body = form_urlencoded::Serializer::new(String::new())
        .append_pair("title", "Edited Route Test")
        .append_pair("slug", "edited-route-test")
        .append_pair("tags_csv", "Rust")
        .append_pair("body_md", "Edited body")
        .append_pair("status", "draft")
        .finish();

    let edit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/posts/{}", created_post.id))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, &session_cookie)
                .body(Body::from(edit_body))
                .expect("request builds"),
        )
        .await?;
    assert_eq!(edit_response.status(), StatusCode::SEE_OTHER);

    let edited_post = repo
        .find_by_slug("edited-route-test")
        .await?
        .expect("edited post exists");
    assert_eq!(edited_post.title, "Edited Route Test");

    let publish_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/posts/{}/publish", edited_post.id))
                .header(header::COOKIE, &session_cookie)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    assert_eq!(publish_response.status(), StatusCode::SEE_OTHER);

    let published_post = repo
        .find_by_slug("edited-route-test")
        .await?
        .expect("published post exists");
    assert_eq!(published_post.status, "published");
    assert!(published_post.published_at.is_some());

    let home_after_publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let home_after_publish_body =
        String::from_utf8(to_bytes(home_after_publish.into_body(), usize::MAX).await?.to_vec())?;
    assert!(home_after_publish_body.contains("Edited Route Test"));

    let tag_names = TagRepo::new(pool.clone())
        .list_for_post(published_post.id)
        .await?
        .into_iter()
        .map(|tag| tag.name)
        .collect::<Vec<_>>();
    assert_eq!(tag_names, vec![String::from("Rust")]);

    let delete_confirm = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/admin/posts/{}/delete", published_post.id))
                .header(header::COOKIE, &session_cookie)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let delete_confirm_body =
        String::from_utf8(to_bytes(delete_confirm.into_body(), usize::MAX).await?.to_vec())?;
    assert!(delete_confirm_body.contains("Delete Post"));
    assert!(delete_confirm_body.contains("Edited Route Test"));

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/posts/{}/delete", published_post.id))
                .header(header::COOKIE, &session_cookie)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    assert_eq!(delete_response.status(), StatusCode::SEE_OTHER);
    assert!(repo.find_by_slug("edited-route-test").await?.is_none());

    common::reset_database(&pool).await?;
    Ok(())
}
