//! Idempotency-Key support (RFC 9637 / Stripe convention).
//!
//! When a request carries `Idempotency-Key`, the extractor returns the key
//! and a cached response (if any) so the handler can short-circuit. After
//! the handler runs, the route helper persists `(user, key, status, body)`
//! so a retried request returns byte-for-byte the same payload without
//! creating a duplicate resource.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

const HEADER: &str = "idempotency-key";

#[derive(Debug, Clone)]
pub struct IdempotencyKey(pub Option<String>);

impl<S: Send + Sync> FromRequestParts<S> for IdempotencyKey {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get(HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        if let Some(value) = raw {
            // Reject pathological keys before they reach the DB.
            if value.len() > 255 {
                return Err(AppError::Validation(
                    "Idempotency-Key longer than 255 chars".into(),
                ));
            }
            return Ok(IdempotencyKey(Some(value.to_string())));
        }
        Ok(IdempotencyKey(None))
    }
}

pub struct CachedResponse {
    pub status: u16,
    pub body: Value,
}

pub async fn lookup(
    pool: &PgPool,
    user_id: Uuid,
    key: &str,
) -> Result<Option<CachedResponse>, sqlx::Error> {
    sqlx::query!(
        "SELECT response_status, response_body
         FROM idempotency_keys
         WHERE user_id = $1 AND key = $2",
        user_id,
        key,
    )
    .fetch_optional(pool)
    .await
    .map(|row| {
        row.map(|r| CachedResponse {
            status: r.response_status as u16,
            body: r.response_body,
        })
    })
}

pub async fn store(
    pool: &PgPool,
    user_id: Uuid,
    key: &str,
    method: &str,
    path: &str,
    status: u16,
    body: &Value,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO idempotency_keys
            (user_id, key, request_method, request_path, response_status, response_body)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (user_id, key) DO NOTHING",
        user_id,
        key,
        method,
        path,
        status as i16,
        body,
    )
    .execute(pool)
    .await
    .map(|_| ())
}

pub async fn cleanup_older_than(
    pool: &PgPool,
    keep_for: chrono::Duration,
) -> Result<u64, sqlx::Error> {
    let cutoff = chrono::Utc::now() - keep_for;
    let result = sqlx::query!(
        "DELETE FROM idempotency_keys WHERE created_at < $1",
        cutoff,
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
