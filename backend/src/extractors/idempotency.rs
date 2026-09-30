//! Idempotency-Key support (RFC 9637 / Stripe convention).
//!
//! When a request carries `Idempotency-Key`, the extractor returns the key
//! to the handler. A transaction-scoped lock serializes requests for the
//! same user and key; the item and cached response commit together.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use serde_json::Value;
use sqlx::{PgExecutor, PgPool, Postgres, Transaction};
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

pub async fn begin_locked<'p>(
    pool: &'p PgPool,
    user_id: Uuid,
    key: &str,
) -> Result<Transaction<'p, Postgres>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '10s'")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '3s'")
        .execute(&mut *transaction)
        .await?;
    // Hash collisions only serialize unrelated requests; lookup still uses the full key.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("{user_id}:{key}"))
        .execute(&mut *transaction)
        .await?;
    Ok(transaction)
}

#[derive(sqlx::FromRow)]
struct CachedResponseRow {
    response_status: i16,
    response_body: Value,
}

pub async fn lookup(
    executor: impl PgExecutor<'_>,
    user_id: Uuid,
    key: &str,
) -> Result<Option<CachedResponse>, sqlx::Error> {
    sqlx::query_as::<_, CachedResponseRow>(
        "SELECT response_status, response_body
         FROM idempotency_keys
         WHERE user_id = $1 AND key = $2",
    )
    .bind(user_id)
    .bind(key)
    .fetch_optional(executor)
    .await
    .map(|row| {
        row.map(|r| CachedResponse {
            status: r.response_status as u16,
            body: r.response_body,
        })
    })
}

pub async fn store(
    executor: impl PgExecutor<'_>,
    user_id: Uuid,
    key: &str,
    method: &str,
    path: &str,
    status: u16,
    body: &Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO idempotency_keys
            (user_id, key, request_method, request_path, response_status, response_body)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(key)
    .bind(method)
    .bind(path)
    .bind(status as i16)
    .bind(body)
    .execute(executor)
    .await
    .map(|_| ())
}

pub async fn cleanup_older_than(
    pool: &PgPool,
    keep_for: chrono::Duration,
) -> Result<u64, sqlx::Error> {
    let cutoff = chrono::Utc::now() - keep_for;
    let result = sqlx::query("DELETE FROM idempotency_keys WHERE created_at < $1")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
