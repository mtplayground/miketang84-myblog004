use std::{error::Error, fmt};

use deunicode::deunicode;
use sqlx::PgPool;
use uuid::Uuid;

use crate::repositories::posts::PostRepo;

const MAX_UNIQUENESS_ATTEMPTS: usize = 10_000;

#[derive(Clone)]
pub struct SlugService {
    repo: PostRepo,
}

impl SlugService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: PostRepo::new(pool),
        }
    }

    pub async fn resolve(
        &self,
        title: &str,
        override_slug: Option<&str>,
        exclude_post_id: Option<Uuid>,
    ) -> Result<String, SlugError> {
        let source = override_slug.unwrap_or(title);
        let base_slug = slugify(source)?;

        if self
            .slug_is_available(&base_slug, exclude_post_id)
            .await?
        {
            return Ok(base_slug);
        }

        for suffix in 2..=MAX_UNIQUENESS_ATTEMPTS {
            let candidate = format!("{base_slug}-{suffix}");

            if self
                .slug_is_available(&candidate, exclude_post_id)
                .await?
            {
                return Ok(candidate);
            }
        }

        Err(SlugError::UniquenessExhausted {
            base_slug,
            attempts: MAX_UNIQUENESS_ATTEMPTS,
        })
    }

    async fn slug_is_available(
        &self,
        candidate: &str,
        exclude_post_id: Option<Uuid>,
    ) -> Result<bool, SlugError> {
        let existing = self.repo.find_by_slug(candidate).await?;

        Ok(match existing {
            None => true,
            Some(post) => Some(post.id) == exclude_post_id,
        })
    }
}

pub fn slugify(input: &str) -> Result<String, SlugError> {
    let ascii = deunicode(input);
    let mut slug = String::with_capacity(ascii.len());
    let mut last_was_hyphen = false;

    for ch in ascii.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_hyphen = false;
        } else if !last_was_hyphen && !slug.is_empty() {
            slug.push('-');
            last_was_hyphen = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    validate_slug(&slug)?;
    Ok(slug)
}

pub fn validate_slug(slug: &str) -> Result<(), SlugError> {
    if slug.is_empty() {
        return Err(SlugError::Empty);
    }

    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(SlugError::InvalidFormat);
    }

    let mut last_was_hyphen = false;

    for ch in slug.chars() {
        match ch {
            'a'..='z' | '0'..='9' => last_was_hyphen = false,
            '-' if !last_was_hyphen => last_was_hyphen = true,
            _ => return Err(SlugError::InvalidFormat),
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum SlugError {
    Empty,
    InvalidFormat,
    Database(sqlx::Error),
    UniquenessExhausted { base_slug: String, attempts: usize },
}

impl fmt::Display for SlugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "slug cannot be empty"),
            Self::InvalidFormat => write!(f, "slug must contain only lowercase letters, numbers, and single hyphens"),
            Self::Database(_) => write!(f, "slug uniqueness lookup failed"),
            Self::UniquenessExhausted { base_slug, attempts } => write!(
                f,
                "could not find a unique slug for `{base_slug}` after {attempts} attempts"
            ),
        }
    }
}

impl Error for SlugError {}

impl From<sqlx::Error> for SlugError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{SlugError, slugify, validate_slug};

    #[test]
    fn slugify_folds_ascii_and_normalizes_spacing() {
        let slug = slugify("Café crème & croissants").expect("slugify succeeds");

        assert_eq!(slug, "cafe-creme-croissants");
    }

    #[test]
    fn slugify_rejects_empty_result() {
        let error = slugify("!!!").expect_err("slugify should fail");

        assert!(matches!(error, SlugError::Empty));
    }

    #[test]
    fn validate_slug_rejects_invalid_characters() {
        let error = validate_slug("hello_world").expect_err("validation should fail");

        assert!(matches!(error, SlugError::InvalidFormat));
    }
}
