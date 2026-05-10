mod common;

use chrono::Utc;
use myblog004::repositories::{
    admins::{AdminRepo, NewAdmin},
    posts::{NewPost, PostRepo, PostStatus, UpdatePost},
    tags::{NewTag, TagRepo},
};
use uuid::Uuid;

#[tokio::test]
async fn admin_repo_crud_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = common::test_transaction().await?;

    let admin = NewAdmin {
        id: Uuid::new_v4(),
        username: String::from("alice"),
        password_hash: String::from("hash-1"),
    };

    let inserted = AdminRepo::insert_with(&mut *tx, &admin).await?;
    assert_eq!(inserted.username, "alice");

    let fetched = AdminRepo::find_by_username_with(&mut *tx, "alice")
        .await?
        .expect("admin should exist");
    assert_eq!(fetched.id, inserted.id);

    let updated = AdminRepo::update_password_with(&mut *tx, inserted.id, "hash-2").await?;
    assert_eq!(updated.password_hash, "hash-2");

    tx.rollback().await?;
    Ok(())
}

#[tokio::test]
async fn post_repo_crud_and_pagination() -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = common::test_transaction().await?;

    let published_post = NewPost {
        id: Uuid::new_v4(),
        slug: String::from("published-post"),
        title: String::from("Published Post"),
        body_md: String::from("published body"),
        body_html: String::from("<p>published body</p>"),
        excerpt: String::from("published excerpt"),
        status: PostStatus::Published,
        published_at: Some(Utc::now()),
    };

    let draft_post = NewPost {
        id: Uuid::new_v4(),
        slug: String::from("draft-post"),
        title: String::from("Draft Post"),
        body_md: String::from("draft body"),
        body_html: String::from("<p>draft body</p>"),
        excerpt: String::from("draft excerpt"),
        status: PostStatus::Draft,
        published_at: None,
    };

    let inserted_published = PostRepo::insert_with(&mut *tx, &published_post).await?;
    let inserted_draft = PostRepo::insert_with(&mut *tx, &draft_post).await?;

    let published_page = PostRepo::list_published_with(&mut *tx, 1, 10).await?;
    assert_eq!(published_page.len(), 1);
    assert_eq!(published_page[0].slug, inserted_published.slug);

    let all_posts = PostRepo::list_all_admin_with(&mut *tx).await?;
    assert_eq!(all_posts.len(), 2);

    let fetched_by_slug = PostRepo::find_by_slug_with(&mut *tx, "draft-post")
        .await?
        .expect("draft post should exist");
    assert_eq!(fetched_by_slug.id, inserted_draft.id);

    let updated = PostRepo::update_with(
        &mut *tx,
        &UpdatePost {
            id: inserted_draft.id,
            slug: String::from("draft-post-updated"),
            title: String::from("Draft Post Updated"),
            body_md: String::from("updated md"),
            body_html: String::from("<p>updated html</p>"),
            excerpt: String::from("updated excerpt"),
        },
    )
    .await?;
    assert_eq!(updated.slug, "draft-post-updated");

    let status_changed = PostRepo::set_status_with(
        &mut *tx,
        inserted_draft.id,
        PostStatus::Published,
        Some(Utc::now()),
    )
    .await?;
    assert_eq!(status_changed.status, "published");

    PostRepo::delete_with(&mut *tx, inserted_published.id).await?;
    let remaining = PostRepo::list_all_admin_with(&mut *tx).await?;
    assert_eq!(remaining.len(), 1);

    tx.rollback().await?;
    Ok(())
}

#[tokio::test]
async fn tag_repo_round_trip_and_post_lookup() -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = common::test_transaction().await?;

    let post = PostRepo::insert_with(
        &mut *tx,
        &NewPost {
            id: Uuid::new_v4(),
            slug: String::from("tagged-post"),
            title: String::from("Tagged Post"),
            body_md: String::from("tagged md"),
            body_html: String::from("<p>tagged html</p>"),
            excerpt: String::from("tagged excerpt"),
            status: PostStatus::Published,
            published_at: Some(Utc::now()),
        },
    )
    .await?;

    let rust_tag = TagRepo::upsert_by_slug_with(
        &mut *tx,
        &NewTag {
            id: Uuid::new_v4(),
            slug: String::from("rust"),
            name: String::from("Rust"),
        },
    )
    .await?;

    let databases_tag = TagRepo::upsert_by_slug_with(
        &mut *tx,
        &NewTag {
            id: Uuid::new_v4(),
            slug: String::from("databases"),
            name: String::from("Databases"),
        },
    )
    .await?;

    let updated_rust_tag = TagRepo::upsert_by_slug_with(
        &mut *tx,
        &NewTag {
            id: Uuid::new_v4(),
            slug: String::from("rust"),
            name: String::from("Rust Language"),
        },
    )
    .await?;
    assert_eq!(updated_rust_tag.id, rust_tag.id);
    assert_eq!(updated_rust_tag.name, "Rust Language");

    TagRepo::replace_post_tags_with(&mut tx, post.id, &[rust_tag.id, databases_tag.id]).await?;

    let tags_for_post = TagRepo::list_for_post_with(&mut *tx, post.id).await?;
    assert_eq!(tags_for_post.len(), 2);

    let posts_for_tag = TagRepo::posts_by_tag_slug_with(&mut *tx, "rust").await?;
    assert_eq!(posts_for_tag.len(), 1);
    assert_eq!(posts_for_tag[0].id, post.id);

    tx.rollback().await?;
    Ok(())
}
