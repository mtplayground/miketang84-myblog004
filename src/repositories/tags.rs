use sqlx::PgPool;
use uuid::Uuid;

use crate::repositories::posts::Post;

#[derive(Debug, Clone)]
pub struct Tag {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct NewTag {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

#[derive(Clone)]
pub struct TagRepo {
    pool: PgPool,
}

impl TagRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_by_slug(&self, tag: &NewTag) -> Result<Tag, sqlx::Error> {
        sqlx::query_as!(
            Tag,
            r#"
            INSERT INTO tags (id, slug, name)
            VALUES ($1, $2, $3)
            ON CONFLICT (slug) DO UPDATE
            SET name = EXCLUDED.name
            RETURNING id, slug, name
            "#,
            tag.id,
            tag.slug,
            tag.name
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_for_post(&self, post_id: Uuid) -> Result<Vec<Tag>, sqlx::Error> {
        sqlx::query_as!(
            Tag,
            r#"
            SELECT t.id, t.slug, t.name
            FROM tags t
            INNER JOIN post_tags pt ON pt.tag_id = t.id
            WHERE pt.post_id = $1
            ORDER BY t.name ASC
            "#,
            post_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn posts_by_tag_slug(&self, slug: &str) -> Result<Vec<Post>, sqlx::Error> {
        sqlx::query_as!(
            Post,
            r#"
            SELECT
                p.id,
                p.slug,
                p.title,
                p.body_md,
                p.body_html,
                p.excerpt,
                p.status,
                p.published_at,
                p.created_at,
                p.updated_at
            FROM posts p
            INNER JOIN post_tags pt ON pt.post_id = p.id
            INNER JOIN tags t ON t.id = pt.tag_id
            WHERE t.slug = $1 AND p.status = 'published'
            ORDER BY p.published_at DESC NULLS LAST, p.created_at DESC
            "#,
            slug
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn replace_post_tags(&self, post_id: Uuid, tag_ids: &[Uuid]) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            r#"
            DELETE FROM post_tags
            WHERE post_id = $1
            "#,
            post_id
        )
        .execute(&mut *tx)
        .await?;

        if !tag_ids.is_empty() {
            sqlx::query!(
                r#"
                INSERT INTO post_tags (post_id, tag_id)
                SELECT $1, input.tag_id
                FROM UNNEST($2::uuid[]) AS input(tag_id)
                ON CONFLICT (post_id, tag_id) DO NOTHING
                "#,
                post_id,
                tag_ids
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await
    }
}
