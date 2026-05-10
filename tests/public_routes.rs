mod common;

use std::net::SocketAddr;

use axum_test::TestServer;
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
async fn public_routes_return_expected_statuses_and_body_fragments() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("public_routes_axum_test").await?;
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

    let server = TestServer::new(app(AppState::new(test_config(), pool.clone())))?;

    let home = server.get("/").await;
    home.assert_status_ok();
    let home_text = home.text();
    assert!(home_text.contains("Published Post"));
    assert!(!home_text.contains("Draft Post"));

    let post_detail = server.get("/posts/published-post").await;
    post_detail.assert_status_ok();
    let post_detail_text = post_detail.text();
    assert!(post_detail_text.contains("Rendered body"));
    assert!(post_detail_text.contains("Published excerpt"));

    let tag_page = server.get("/tags/rust").await;
    tag_page.assert_status_ok();
    let tag_page_text = tag_page.text();
    assert!(tag_page_text.contains("Published Post"));
    assert!(tag_page_text.contains("#rust"));

    let about = server.get("/about").await;
    about.assert_status_ok();
    assert!(about.text().contains("About"));

    let rss = server.get("/rss.xml").await;
    rss.assert_status_ok();
    let rss_text = rss.text();
    assert!(rss_text.contains("<rss"));
    assert!(rss_text.contains("Published Post"));
    assert!(!rss_text.contains("Draft Post"));

    let sitemap = server.get("/sitemap.xml").await;
    sitemap.assert_status_ok();
    let sitemap_text = sitemap.text();
    assert!(sitemap_text.contains("<urlset"));
    assert!(sitemap_text.contains("published-post"));
    assert!(!sitemap_text.contains("draft-post"));

    let robots = server.get("/robots.txt").await;
    robots.assert_status_ok();
    let robots_text = robots.text();
    assert!(robots_text.contains("Disallow: /admin/"));
    assert!(robots_text.contains("Sitemap: http://localhost:8080/sitemap.xml"));

    common::reset_database(&pool).await?;
    Ok(())
}
