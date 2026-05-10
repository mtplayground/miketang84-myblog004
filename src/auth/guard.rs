use axum::{
    extract::{Extension, FromRequestParts, Request},
    http::request::Parts,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions_cookie_store::Session;
use uuid::Uuid;

use crate::auth::ADMIN_SESSION_KEY;

#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedAdmin {
    pub admin_id: Uuid,
}

impl<S> FromRequestParts<S> for AuthenticatedAdmin
where
    S: Sync + Send,
{
    type Rejection = Redirect;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .copied()
            .ok_or_else(login_redirect)
    }
}

pub async fn require_admin_auth(
    Extension(session): Extension<Session>,
    mut request: Request,
    next: Next,
) -> Response {
    match session.get::<String>(ADMIN_SESSION_KEY).await {
        Ok(Some(admin_id)) => match Uuid::parse_str(&admin_id) {
            Ok(admin_id) => {
                request
                    .extensions_mut()
                    .insert(AuthenticatedAdmin { admin_id });
                next.run(request).await
            }
            Err(_) => login_redirect().into_response(),
        },
        Ok(None) | Err(_) => login_redirect().into_response(),
    }
}

fn login_redirect() -> Redirect {
    Redirect::to("/admin/login")
}
