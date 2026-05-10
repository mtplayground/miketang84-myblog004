use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostStatus {
    Draft,
    Published,
}

impl PostStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Post {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub body_md: String,
    pub body_html: String,
    pub excerpt: String,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewPost {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub body_md: String,
    pub body_html: String,
    pub excerpt: String,
    pub status: PostStatus,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct UpdatePost {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub body_md: String,
    pub body_html: String,
    pub excerpt: String,
}

#[derive(Clone)]
pub struct PostRepo {
    pool: PgPool,
}

impl PostRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_published(&self, page: i64, per_page: i64) -> Result<Vec<Post>, sqlx::Error> {
        Self::list_published_with(&self.pool, page, per_page).await
    }

    pub async fn list_published_with<'e, E>(
        executor: E,
        page: i64,
        per_page: i64,
    ) -> Result<Vec<Post>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let page = page.max(1);
        let per_page = per_page.max(1);
        let offset = (page - 1) * per_page;

        sqlx::query_as!(
            Post,
            r#"
            SELECT
                id,
                slug,
                title,
                body_md,
                body_html,
                excerpt,
                status,
                published_at,
                created_at,
                updated_at
            FROM posts
            WHERE status = 'published'
            ORDER BY published_at DESC NULLS LAST, created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            per_page,
            offset
        )
        .fetch_all(executor)
        .await
    }

    pub async fn list_all_admin(&self) -> Result<Vec<Post>, sqlx::Error> {
        Self::list_all_admin_with(&self.pool).await
    }

    pub async fn list_all_admin_with<'e, E>(executor: E) -> Result<Vec<Post>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            Post,
            r#"
            SELECT
                id,
                slug,
                title,
                body_md,
                body_html,
                excerpt,
                status,
                published_at,
                created_at,
                updated_at
            FROM posts
            ORDER BY updated_at DESC, created_at DESC
            "#
        )
        .fetch_all(executor)
        .await
    }

    pub async fn find_by_slug(&self, slug: &str) -> Result<Option<Post>, sqlx::Error> {
        Self::find_by_slug_with(&self.pool, slug).await
    }

    pub async fn find_by_slug_with<'e, E>(
        executor: E,
        slug: &str,
    ) -> Result<Option<Post>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            Post,
            r#"
            SELECT
                id,
                slug,
                title,
                body_md,
                body_html,
                excerpt,
                status,
                published_at,
                created_at,
                updated_at
            FROM posts
            WHERE slug = $1
            "#,
            slug
        )
        .fetch_optional(executor)
        .await
    }

    pub async fn insert(&self, post: &NewPost) -> Result<Post, sqlx::Error> {
        Self::insert_with(&self.pool, post).await
    }

    pub async fn insert_with<'e, E>(executor: E, post: &NewPost) -> Result<Post, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            Post,
            r#"
            INSERT INTO posts (
                id,
                slug,
                title,
                body_md,
                body_html,
                excerpt,
                status,
                published_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id,
                slug,
                title,
                body_md,
                body_html,
                excerpt,
                status,
                published_at,
                created_at,
                updated_at
            "#,
            post.id,
            post.slug,
            post.title,
            post.body_md,
            post.body_html,
            post.excerpt,
            post.status.as_str(),
            post.published_at
        )
        .fetch_one(executor)
        .await
    }

    pub async fn update(&self, post: &UpdatePost) -> Result<Post, sqlx::Error> {
        Self::update_with(&self.pool, post).await
    }

    pub async fn update_with<'e, E>(executor: E, post: &UpdatePost) -> Result<Post, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            Post,
            r#"
            UPDATE posts
            SET
                slug = $2,
                title = $3,
                body_md = $4,
                body_html = $5,
                excerpt = $6,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                slug,
                title,
                body_md,
                body_html,
                excerpt,
                status,
                published_at,
                created_at,
                updated_at
            "#,
            post.id,
            post.slug,
            post.title,
            post.body_md,
            post.body_html,
            post.excerpt
        )
        .fetch_one(executor)
        .await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), sqlx::Error> {
        Self::delete_with(&self.pool, id).await
    }

    pub async fn delete_with<'e, E>(executor: E, id: Uuid) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query!(
            r#"
            DELETE FROM posts
            WHERE id = $1
            "#,
            id
        )
        .execute(executor)
        .await
        .map(|_| ())
    }

    pub async fn set_status(
        &self,
        id: Uuid,
        status: PostStatus,
        published_at: Option<DateTime<Utc>>,
    ) -> Result<Post, sqlx::Error> {
        Self::set_status_with(&self.pool, id, status, published_at).await
    }

    pub async fn set_status_with<'e, E>(
        executor: E,
        id: Uuid,
        status: PostStatus,
        published_at: Option<DateTime<Utc>>,
    ) -> Result<Post, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            Post,
            r#"
            UPDATE posts
            SET
                status = $2,
                published_at = $3,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                slug,
                title,
                body_md,
                body_html,
                excerpt,
                status,
                published_at,
                created_at,
                updated_at
            "#,
            id,
            status.as_str(),
            published_at
        )
        .fetch_one(executor)
        .await
    }
}
