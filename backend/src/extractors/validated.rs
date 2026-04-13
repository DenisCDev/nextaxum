use axum::extract::rejection::JsonRejection;
use axum::extract::FromRequest;
use axum::http::Request;
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::error::AppError;

/// JSON extractor that validates the payload with the `validator` crate.
/// Returns 400 with validation details on failure.
pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = AppError;

    async fn from_request(req: Request<axum::body::Body>, state: &S) -> Result<Self, Self::Rejection> {
        let axum::Json(value) = axum::Json::<T>::from_request(req, state)
            .await
            .map_err(|e: JsonRejection| AppError::Validation(e.body_text()))?;

        value
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        Ok(ValidatedJson(value))
    }
}
