mod common;

use chrono::Utc;
use myblog004::{
    repositories::posts::{NewPost, PostRepo, PostStatus},
    slug::SlugService,
};
use uuid::Uuid;

#[tokio::test]
async fn slug_service_generates_unique_slug_from_title() -> Result<(), Box<dyn std::error::Error>> {
    let pool = common::test_pool("slug_service_generates_unique_slug").await?;
    PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("hello-world"),
            title: String::from("Hello World"),
            body_md: String::from("body"),
            body_html: String::from("<p>body</p>"),
            excerpt: String::from("excerpt"),
            status: PostStatus::Published,
            published_at: Some(Utc::now()),
        })
        .await?;

    let slug = SlugService::new(pool.clone())
        .resolve("Hello World", None, None)
        .await?;

    assert_eq!(slug, "hello-world-2");

    common::reset_database(&pool).await?;
    Ok(())
}

#[tokio::test]
async fn slug_service_accepts_admin_override_and_excludes_current_post() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = common::test_pool("slug_service_accepts_override").await?;
    let existing_post = PostRepo::new(pool.clone())
        .insert(&NewPost {
            id: Uuid::new_v4(),
            slug: String::from("custom-slug"),
            title: String::from("Existing Post"),
            body_md: String::from("body"),
            body_html: String::from("<p>body</p>"),
            excerpt: String::from("excerpt"),
            status: PostStatus::Published,
            published_at: Some(Utc::now()),
        })
        .await?;

    let same_post_slug = SlugService::new(pool.clone())
        .resolve("Ignored Title", Some("custom slug"), Some(existing_post.id))
        .await?;
    let next_unique_slug = SlugService::new(pool.clone())
        .resolve("Ignored Title", Some("custom slug"), None)
        .await?;

    assert_eq!(same_post_slug, "custom-slug");
    assert_eq!(next_unique_slug, "custom-slug-2");

    common::reset_database(&pool).await?;
    Ok(())
}
