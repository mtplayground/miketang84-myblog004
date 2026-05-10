mod common;

use std::{
    env,
    ffi::OsString,
    sync::{LazyLock, Mutex},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use myblog004::{
    app,
    config::Config,
    repositories::posts::PostRepo,
    seed::{AdminSeedOutcome, seed_admin},
    state::AppState,
};
use tower::ServiceExt;
use url::form_urlencoded;

static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

const TEST_DATABASE_URL_FALLBACK: &str = "postgresql:///postgres";
const ENV_VAR_NAMES: [&str; 8] = [
    "BLOG_BIND_ADDR",
    "BLOG_DATABASE_URL",
    "BLOG_BASE_URL",
    "BLOG_SESSION_SECRET",
    "BLOG_TITLE",
    "BLOG_RSS_LIMIT",
    "ADMIN_USERNAME",
    "ADMIN_PASSWORD",
];

struct TestEnvGuard {
    previous: Vec<(&'static str, Option<OsString>)>,
}

impl TestEnvGuard {
    fn set(database_url: &str) -> Self {
        let previous = ENV_VAR_NAMES
            .into_iter()
            .map(|name| (name, env::var_os(name)))
            .collect::<Vec<_>>();

        unsafe {
            env::set_var("BLOG_BIND_ADDR", "0.0.0.0:8080");
            env::set_var("BLOG_DATABASE_URL", database_url);
            env::set_var("BLOG_BASE_URL", "http://localhost:8080/");
            env::set_var(
                "BLOG_SESSION_SECRET",
                "0123456789abcdef0123456789abcdef",
            );
            env::set_var("BLOG_TITLE", "End-to-End Test Blog");
            env::set_var("BLOG_RSS_LIMIT", "10");
            env::set_var("ADMIN_USERNAME", "env-admin");
            env::set_var("ADMIN_PASSWORD", "env-password");
        }

        Self { previous }
    }
}

impl Drop for TestEnvGuard {
    fn drop(&mut self) {
        for (name, value) in self.previous.drain(..) {
            unsafe {
                match value {
                    Some(value) => env::set_var(name, value),
                    None => env::remove_var(name),
                }
            }
        }
    }
}

async fn response_body(response: axum::response::Response) -> Result<String, Box<dyn std::error::Error>> {
    Ok(String::from_utf8(
        to_bytes(response.into_body(), usize::MAX).await?.to_vec(),
    )?)
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

    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .expect("login should set a session cookie")
        .to_string();

    Ok(cookie)
}

fn fallback_database_url() -> String {
    env::var("TEST_DATABASE_URL").unwrap_or_else(|_| String::from(TEST_DATABASE_URL_FALLBACK))
}

#[tokio::test]
async fn end_to_end_happy_path_exercises_admin_and_public_flows() -> Result<(), Box<dyn std::error::Error>> {
    let _env_lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let pool = common::test_pool("end_to_end_happy_path").await?;
    let _env_guard = TestEnvGuard::set(&fallback_database_url());

    let config = Config::from_env()?;
    assert_eq!(
        seed_admin(&pool, &config.admin_username, &config.admin_password).await?,
        AdminSeedOutcome::Created
    );

    let app = app(AppState::new(config.clone(), pool.clone()));
    let session_cookie = login_cookie(app.clone(), &config.admin_username, &config.admin_password).await?;

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
    assert_eq!(dashboard.status(), StatusCode::OK);
    let dashboard_body = response_body(dashboard).await?;
    assert!(dashboard_body.contains("Authenticated admin dashboard"));

    let create_body = form_urlencoded::Serializer::new(String::new())
        .append_pair("title", "End To End Happy Path")
        .append_pair("slug", "")
        .append_pair("tags_csv", "Rust, Testing")
        .append_pair("body_md", "Draft **body** for the end-to-end flow.")
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
    let draft_post = repo
        .find_by_slug("end-to-end-happy-path")
        .await?
        .expect("draft post should exist");
    assert_eq!(draft_post.status, "draft");

    let draft_dashboard = app
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
    let draft_dashboard_body = response_body(draft_dashboard).await?;
    assert!(draft_dashboard_body.contains("End To End Happy Path"));
    assert!(draft_dashboard_body.contains("Draft"));

    let public_home_before_publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let public_home_before_publish_body = response_body(public_home_before_publish).await?;
    assert!(!public_home_before_publish_body.contains("End To End Happy Path"));

    let rss_before_publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/rss.xml")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let rss_before_publish_body = response_body(rss_before_publish).await?;
    assert!(!rss_before_publish_body.contains("End To End Happy Path"));

    let edit_form = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/admin/posts/{}/edit", draft_post.id))
                .header(header::COOKIE, &session_cookie)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    assert_eq!(edit_form.status(), StatusCode::OK);
    let edit_form_body = response_body(edit_form).await?;
    assert!(edit_form_body.contains("End To End Happy Path"));
    assert!(edit_form_body.contains("Draft **body** for the end-to-end flow."));

    let edit_body = form_urlencoded::Serializer::new(String::new())
        .append_pair("title", "End To End Published Post")
        .append_pair("slug", "end-to-end-published-post")
        .append_pair("tags_csv", "Rust, RSS")
        .append_pair("body_md", "# Published heading\n\nFinal body for RSS and home.")
        .append_pair("status", "draft")
        .finish();

    let edit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/posts/{}", draft_post.id))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, &session_cookie)
                .body(Body::from(edit_body))
                .expect("request builds"),
        )
        .await?;
    assert_eq!(edit_response.status(), StatusCode::SEE_OTHER);

    let edited_post = repo
        .find_by_slug("end-to-end-published-post")
        .await?
        .expect("edited post should exist");
    assert_eq!(edited_post.title, "End To End Published Post");
    assert_eq!(edited_post.status, "draft");

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
        .find_by_slug("end-to-end-published-post")
        .await?
        .expect("published post should exist");
    assert_eq!(published_post.status, "published");
    assert!(published_post.published_at.is_some());

    let public_home = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let public_home_body = response_body(public_home).await?;
    assert!(public_home_body.contains("End To End Published Post"));

    let rss_feed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/rss.xml")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    assert_eq!(rss_feed.status(), StatusCode::OK);
    let rss_feed_body = response_body(rss_feed).await?;
    assert!(rss_feed_body.contains("End To End Published Post"));
    assert!(rss_feed_body.contains("Final body for RSS and home."));
    assert!(rss_feed_body.contains("/posts/end-to-end-published-post"));

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
    assert_eq!(delete_confirm.status(), StatusCode::OK);
    let delete_confirm_body = response_body(delete_confirm).await?;
    assert!(delete_confirm_body.contains("Delete Post"));
    assert!(delete_confirm_body.contains("End To End Published Post"));

    let delete_response = app
        .clone()
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
    assert!(repo.find_by_slug("end-to-end-published-post").await?.is_none());

    let public_home_after_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let public_home_after_delete_body = response_body(public_home_after_delete).await?;
    assert!(!public_home_after_delete_body.contains("End To End Published Post"));

    let rss_after_delete = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/rss.xml")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;
    let rss_after_delete_body = response_body(rss_after_delete).await?;
    assert!(!rss_after_delete_body.contains("End To End Published Post"));

    common::reset_database(&pool).await?;
    Ok(())
}
