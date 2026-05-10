mod common;

use std::net::SocketAddr;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use myblog004::{
    app,
    auth::password::verify_password,
    config::Config,
    repositories::{
        admins::AdminRepo,
        posts::{NewPost, PostRepo, PostStatus},
    },
    seed::{AdminSeedOutcome, seed_admin},
    state::AppState,
};
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;

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

async fn login_request(
    app: Router,
    username: &str,
    password: &str,
) -> Result<axum::response::Response, Box<dyn std::error::Error>> {
    let body = format!("username={username}&password={password}");

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

    Ok(response)
}

#[tokio::test]
async fn admin_seeder_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("admin_seeder_is_idempotent").await?;

    let first_seed = seed_admin(&pool, "seed-admin", "correct horse battery staple").await?;
    let second_seed = seed_admin(&pool, "seed-admin", "correct horse battery staple").await?;

    assert_eq!(first_seed, AdminSeedOutcome::Created);
    assert_eq!(second_seed, AdminSeedOutcome::SkippedExisting);

    let admin = AdminRepo::new(pool.clone())
        .find_by_username("seed-admin")
        .await?
        .expect("seeded admin exists");
    assert!(verify_password("correct horse battery staple", &admin.password_hash)?);

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn login_success_sets_session_and_allows_guarded_route() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = common::test_pool("login_success_sets_session").await?;
    let password = "correct horse battery staple";
    seed_admin(&pool, "admin", password).await?;
    let repo = PostRepo::new(pool.clone());
    let published_post = repo.insert(&NewPost {
        id: Uuid::new_v4(),
        slug: String::from("published-post"),
        title: String::from("Published Post"),
        body_md: String::from("published body"),
        body_html: String::from("<p>published body</p>"),
        excerpt: String::from("published excerpt"),
        status: PostStatus::Published,
        published_at: Some(chrono::Utc::now()),
    })
    .await?;
    let draft_post = repo.insert(&NewPost {
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

    let state = AppState::new(test_config(String::from("postgresql:///test")), pool.clone());
    let app = app(state);

    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("username=admin&password={password}")))
                .expect("request builds"),
        )
        .await?;

    assert_eq!(login_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        login_response.headers().get(header::LOCATION).and_then(|value| value.to_str().ok()),
        Some("/admin")
    );

    let session_cookie = login_response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::to_owned)
        .expect("session cookie is set");

    let guarded_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(guarded_response.status(), StatusCode::OK);
    let guarded_body = to_bytes(guarded_response.into_body(), usize::MAX).await?;
    let guarded_body = String::from_utf8(guarded_body.to_vec())?;
    assert!(guarded_body.contains("Authenticated admin dashboard"));
    assert!(guarded_body.contains("Published Post"));
    assert!(guarded_body.contains("Draft Post"));
    assert!(guarded_body.contains("Published"));
    assert!(guarded_body.contains("Draft"));
    assert!(guarded_body.contains(&format!("/admin/posts/{}/edit", published_post.id)));
    assert!(guarded_body.contains(&format!("action=\"/admin/posts/{}/unpublish\"", published_post.id)));
    assert!(guarded_body.contains(&format!("action=\"/admin/posts/{}/publish\"", draft_post.id)));
    assert!(guarded_body.contains(&format!("/admin/posts/{}/delete", draft_post.id)));

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn login_failure_responses_are_indistinguishable() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("login_failure_responses").await?;
    seed_admin(&pool, "admin", "correct horse battery staple").await?;
    let state = AppState::new(test_config(String::from("postgresql:///test")), pool.clone());
    let app = app(state);

    let wrong_password_response = login_request(app.clone(), "admin", "wrong password").await?;
    let unknown_user_response = login_request(app, "missing-admin", "wrong password").await?;

    assert_eq!(wrong_password_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_user_response.status(), StatusCode::UNAUTHORIZED);
    assert!(wrong_password_response.headers().get(header::SET_COOKIE).is_none());
    assert!(unknown_user_response.headers().get(header::SET_COOKIE).is_none());

    let wrong_password_body =
        String::from_utf8(to_bytes(wrong_password_response.into_body(), usize::MAX).await?.to_vec())?;
    let unknown_user_body =
        String::from_utf8(to_bytes(unknown_user_response.into_body(), usize::MAX).await?.to_vec())?;

    assert_eq!(wrong_password_body, unknown_user_body);
    assert!(wrong_password_body.contains("Invalid username or password."));

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn guard_redirects_unauthenticated_requests() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("guard_redirects_unauthenticated").await?;
    let state = AppState::new(test_config(String::from("postgresql:///test")), pool.clone());
    let app = app(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(header::LOCATION).and_then(|value| value.to_str().ok()),
        Some("/admin/login")
    );

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn login_form_renders() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("login_form_renders").await?;
    let state = AppState::new(test_config(String::from("postgresql:///test")), pool.clone());
    let app = app(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/login")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("<form"));
    assert!(body.contains("method=\"post\""));
    assert!(body.contains("action=\"/admin/login\""));

    common::reset_database(&pool).await?;
    Ok(())
}
