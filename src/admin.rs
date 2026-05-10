use axum::{
    Form,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::Acquire;
use uuid::Uuid;

use crate::{
    error::AppError,
    markdown::MarkdownRenderer,
    repositories::{
        posts::{NewPost, Post, PostRepo, PostStatus, UpdatePost},
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
) -> HtmlTemplate<AdminPostFormTemplate> {
    HtmlTemplate(build_create_form_template(
        &state.config.title,
        CreatePostFormData::default(),
        None,
    ))
}

pub async fn create_post(
    State(state): State<AppState>,
    Form(form): Form<CreatePostFormData>,
) -> Result<Response, AppError> {
    let title = form.title.trim();
    if title.is_empty() {
        return Ok(create_validation_response(
            &state,
            form,
            "Title is required.",
        ));
    }

    let body_md = form.body_md.trim();
    if body_md.is_empty() {
        return Ok(create_validation_response(
            &state,
            form,
            "Body Markdown is required.",
        ));
    }

    let status = match form.status.as_str() {
        "draft" => PostStatus::Draft,
        "published" => PostStatus::Published,
        _ => {
            return Ok(create_validation_response(
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
            return Ok(create_validation_response(
                &state,
                form,
                "Slug must contain letters or numbers after normalization.",
            ));
        }
        Err(SlugError::UniquenessExhausted { .. }) => {
            return Ok(create_validation_response(
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
        Err(message) => return Ok(create_validation_response(&state, form, message)),
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

pub async fn edit_post_form(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
) -> Result<HtmlTemplate<AdminPostFormTemplate>, AppError> {
    let post_id = parse_post_id(&post_id)?;
    let post = load_post(&state, post_id).await?;
    let form = build_post_form_data(&state, &post).await?;

    Ok(HtmlTemplate(build_edit_form_template(
        &state.config.title,
        post_id,
        form,
        None,
    )))
}

pub async fn update_post(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
    Form(form): Form<CreatePostFormData>,
) -> Result<Response, AppError> {
    let post_id = parse_post_id(&post_id)?;
    let existing_post = load_post(&state, post_id).await?;
    let title = form.title.trim();
    if title.is_empty() {
        return Ok(edit_validation_response(
            &state,
            post_id,
            form,
            "Title is required.",
        ));
    }

    let body_md = form.body_md.trim();
    if body_md.is_empty() {
        return Ok(edit_validation_response(
            &state,
            post_id,
            form,
            "Body Markdown is required.",
        ));
    }

    let status = match form.status.as_str() {
        "draft" => PostStatus::Draft,
        "published" => PostStatus::Published,
        _ => {
            return Ok(edit_validation_response(
                &state,
                post_id,
                form,
                "Status must be either draft or published.",
            ));
        }
    };

    let slug_override = trimmed_or_none(&form.slug);
    let slug = match SlugService::new(state.db_pool.clone())
        .resolve(title, slug_override, Some(post_id))
        .await
    {
        Ok(slug) => slug,
        Err(SlugError::Empty | SlugError::InvalidFormat) => {
            return Ok(edit_validation_response(
                &state,
                post_id,
                form,
                "Slug must contain letters or numbers after normalization.",
            ));
        }
        Err(SlugError::UniquenessExhausted { .. }) => {
            return Ok(edit_validation_response(
                &state,
                post_id,
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
        Err(message) => return Ok(edit_validation_response(&state, post_id, form, message)),
    };

    let published_at = match status {
        PostStatus::Draft => None,
        PostStatus::Published => existing_post.published_at.or(Some(Utc::now())),
    };

    let post_update = UpdatePost {
        id: post_id,
        slug,
        title: title.to_string(),
        body_md: body_md.to_string(),
        body_html,
        excerpt,
    };

    let mut tx = state.db_pool.begin().await.map_err(|_| AppError::internal())?;
    let executor = tx.acquire().await.map_err(|_| AppError::internal())?;
    PostRepo::update_with(executor, &post_update)
        .await
        .map_err(|_| AppError::internal())?;

    let executor = tx.acquire().await.map_err(|_| AppError::internal())?;
    PostRepo::set_status_with(executor, post_id, status, published_at)
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

    TagRepo::replace_post_tags_with(&mut tx, post_id, &tag_ids)
        .await
        .map_err(|_| AppError::internal())?;
    tx.commit().await.map_err(|_| AppError::internal())?;

    Ok(Redirect::to("/admin").into_response())
}

fn create_validation_response(state: &AppState, form: CreatePostFormData, message: &str) -> Response {
    render_template_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        build_create_form_template(&state.config.title, form, Some(message.to_string())),
    )
}

fn edit_validation_response(
    state: &AppState,
    post_id: Uuid,
    form: CreatePostFormData,
    message: &str,
) -> Response {
    render_template_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        build_edit_form_template(
            &state.config.title,
            post_id,
            form,
            Some(message.to_string()),
        ),
    )
}

fn trimmed_or_none(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn parse_post_id(value: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(|_| AppError::not_found())
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

async fn load_post(state: &AppState, post_id: Uuid) -> Result<Post, AppError> {
    PostRepo::new(state.db_pool.clone())
        .list_all_admin()
        .await
        .map_err(|_| AppError::internal())?
        .into_iter()
        .find(|post| post.id == post_id)
        .ok_or_else(AppError::not_found)
}

async fn build_post_form_data(
    state: &AppState,
    post: &Post,
) -> Result<CreatePostFormData, AppError> {
    let tags_csv = TagRepo::new(state.db_pool.clone())
        .list_for_post(post.id)
        .await
        .map_err(|_| AppError::internal())?
        .into_iter()
        .map(|tag| tag.name)
        .collect::<Vec<_>>()
        .join(", ");

    Ok(CreatePostFormData {
        title: post.title.clone(),
        slug: post.slug.clone(),
        tags_csv,
        body_md: post.body_md.clone(),
        status: post.status.clone(),
    })
}

fn build_create_form_template(
    blog_title: &str,
    form: CreatePostFormData,
    error_message: Option<String>,
) -> AdminPostFormTemplate {
    AdminPostFormTemplate::new(
        blog_title.to_string(),
        String::from("Create Post"),
        String::from("/admin/posts"),
        String::from("Create post"),
        form,
        error_message,
    )
}

fn build_edit_form_template(
    blog_title: &str,
    post_id: Uuid,
    form: CreatePostFormData,
    error_message: Option<String>,
) -> AdminPostFormTemplate {
    AdminPostFormTemplate::new(
        blog_title.to_string(),
        String::from("Edit Post"),
        format!("/admin/posts/{post_id}"),
        String::from("Save changes"),
        form,
        error_message,
    )
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
