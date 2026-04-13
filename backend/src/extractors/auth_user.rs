use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::auth::Claims;

/// Extracts the authenticated user from request extensions.
/// Must be used on routes behind the `require_auth` middleware.
pub struct AuthUser(pub Claims);

impl AuthUser {
    pub fn id(&self) -> Uuid {
        self.0.sub
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .map(AuthUser)
            .ok_or(AppError::Unauthorized)
    }
}
