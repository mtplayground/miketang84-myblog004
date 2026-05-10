use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::Acquire;
use uuid::Uuid;

use crate::{
    auth::guard::AuthenticatedAdmin,
    error::AppError,
    markdown::MarkdownRenderer,
    repositories::{
        posts::{NewPost, PostRepo, PostStatus},
        tags::{NewTag, TagRepo},
    },
    slug::{SlugError, SlugService, slugify},
    state::AppState,
    templates::{AdminPostFormTemplate, HtmlTemplate, render_template_response},
};

const EXCERPT_MAX_CHARS: usize = 240;

#[derive(Debug, Deserialize)]
pub struct CreatePostFormData {
    pub(crate) title: String,
    pub(crate) slug: String,
    pub(crate) tags_csv: String,
    pub(crate) body_md: String,
    pub(crate) status: String,
}

pub async fn new_post_form(
    State(state): State<AppState>,
    _authenticated_admin: AuthenticatedAdmin,
) -> HtmlTemplate<AdminPostFormTemplate> {
    HtmlTemplate(AdminPostFormTemplate::new(
        state.config.title.clone(),
        String::from("Create Post"),
        CreatePostFormData::default(),
        None,
    ))
}

pub async fn create_post(
    State(state): State<AppState>,
    _authenticated_admin: AuthenticatedAdmin,
    Form(form): Form<CreatePostFormData>,
) -> Result<Response, AppError> {
    let title = form.title.trim();
    if title.is_empty() {
        return Ok(validation_response(
            &state,
            form,
            "Title is required.",
        ));
    }

    let body_md = form.body_md.trim();
    if body_md.is_empty() {
        return Ok(validation_response(
            &state,
            form,
            "Body Markdown is required.",
        ));
    }

    let status = match form.status.as_str() {
        "draft" => PostStatus::Draft,
        "published" => PostStatus::Published,
        _ => {
            return Ok(validation_response(
                &state,
                form,
                "Status must be either draft or published.",
            ));
        }
    };

    let slug_override = trimmed_or_none(&form.slug);
    let slug = match SlugService::new(state.db_pool.clone())
        .resolve(title, slug_override, None)
        .await
    {
        Ok(slug) => slug,
        Err(SlugError::Empty | SlugError::InvalidFormat) => {
            return Ok(validation_response(
                &state,
                form,
                "Slug must contain letters or numbers after normalization.",
            ));
        }
        Err(SlugError::UniquenessExhausted { .. }) => {
            return Ok(validation_response(
                &state,
                form,
                "Could not generate a unique slug for this post.",
            ));
        }
        Err(SlugError::Database(_)) => return Err(AppError::internal()),
    };

    let renderer = MarkdownRenderer::new();
    let body_html = renderer.render(body_md);
    let excerpt = build_excerpt(body_md);
    let tags = match parse_tags(&form.tags_csv) {
        Ok(tags) => tags,
        Err(message) => return Ok(validation_response(&state, form, message)),
    };
    let published_at = matches!(status, PostStatus::Published).then_some(Utc::now());

    let post = NewPost {
        id: Uuid::new_v4(),
        slug,
        title: title.to_string(),
        body_md: body_md.to_string(),
        body_html,
        excerpt,
        status,
        published_at,
    };

    let mut tx = state.db_pool.begin().await.map_err(|_| AppError::internal())?;
    let executor = tx.acquire().await.map_err(|_| AppError::internal())?;
    let created_post = PostRepo::insert_with(executor, &post)
        .await
        .map_err(|_| AppError::internal())?;

    let mut tag_ids = Vec::with_capacity(tags.len());
    for tag in tags {
        let executor = tx.acquire().await.map_err(|_| AppError::internal())?;
        let saved_tag = TagRepo::upsert_by_slug_with(executor, &tag)
            .await
            .map_err(|_| AppError::internal())?;
        tag_ids.push(saved_tag.id);
    }

    TagRepo::replace_post_tags_with(&mut tx, created_post.id, &tag_ids)
        .await
        .map_err(|_| AppError::internal())?;
    tx.commit().await.map_err(|_| AppError::internal())?;

    Ok(Redirect::to("/admin").into_response())
}

fn validation_response(state: &AppState, form: CreatePostFormData, message: &str) -> Response {
    render_template_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        AdminPostFormTemplate::new(
            state.config.title.clone(),
            String::from("Create Post"),
            form,
            Some(message.to_string()),
        ),
    )
}

fn trimmed_or_none(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn build_excerpt(body_md: &str) -> String {
    let collapsed = body_md.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut excerpt = String::new();

    for ch in collapsed.chars() {
        if excerpt.chars().count() >= EXCERPT_MAX_CHARS {
            break;
        }

        excerpt.push(ch);
    }

    if collapsed.chars().count() > EXCERPT_MAX_CHARS {
        excerpt.push('…');
    }

    excerpt
}

fn parse_tags(tags_csv: &str) -> Result<Vec<NewTag>, &'static str> {
    let mut tags = Vec::new();

    for raw_tag in tags_csv.split(',') {
        let name = raw_tag.trim();
        if name.is_empty() {
            continue;
        }

        let slug = slugify(name).map_err(|_| "Each tag must contain letters or numbers.")?;
        if tags.iter().any(|tag: &NewTag| tag.slug == slug) {
            continue;
        }

        tags.push(NewTag {
            id: Uuid::new_v4(),
            slug,
            name: name.to_string(),
        });
    }

    Ok(tags)
}

impl Default for CreatePostFormData {
    fn default() -> Self {
        Self {
            title: String::new(),
            slug: String::new(),
            tags_csv: String::new(),
            body_md: String::new(),
            status: String::from("draft"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CreatePostFormData, build_excerpt, parse_tags};

    #[test]
    fn excerpt_collapses_whitespace_and_limits_length() {
        let excerpt = build_excerpt(
            "First line\n\nSecond line with extra spacing that should collapse into one sentence.",
        );

        assert!(excerpt.starts_with("First line Second line"));
    }

    #[test]
    fn parse_tags_deduplicates_by_slug() {
        let tags = parse_tags("Rust, rust, Café")
            .expect("tag parsing should succeed");

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].slug, "rust");
        assert_eq!(tags[1].slug, "cafe");
    }

    #[test]
    fn create_post_form_defaults_to_draft() {
        let form = CreatePostFormData::default();

        assert_eq!(form.status, "draft");
    }
}
