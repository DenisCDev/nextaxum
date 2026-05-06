//! Generic webhook receiver, modelled on the Stripe / GitHub pattern:
//!
//! 1. Read the raw body (HMAC-SHA256 must hash the bytes the sender signed,
//!    not a re-serialised JSON value).
//! 2. Constant-time compare against `X-Signature: sha256=<hex>`.
//! 3. Insert into `webhook_events` keyed by (provider, event_id). Duplicate
//!    deliveries are dropped silently — replay-safe.
//!
//! The actual side-effects of the event (dispatching a notification,
//! updating a row) belong in the per-provider handler called below.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

const SIGNATURE_HEADER: &str = "x-signature";
const EVENT_ID_HEADER: &str = "x-event-id";
const EVENT_TYPE_HEADER: &str = "x-event-type";

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(receive_webhook))
}

/// Webhook receiver. Verifies HMAC-SHA256 against `WEBHOOK_SECRET`, dedupes
/// on (provider, event_id), persists the payload, and returns 202.
#[utoipa::path(
    post,
    path = "/webhooks/{provider}",
    tag = "webhooks",
    params(
        ("provider" = String, Path, description = "Logical sender id (stripe, github, ...)"),
        ("X-Signature" = String, Header, description = "sha256=<hex digest of the raw body>"),
        ("X-Event-Id" = String, Header, description = "Provider-issued unique id, used for idempotency"),
        ("X-Event-Type" = Option<String>, Header, description = "Optional event-type label"),
    ),
    responses(
        (status = 202, description = "Accepted — payload persisted (or already seen)"),
        (status = 401, description = "Signature missing or did not match"),
        (status = 422, description = "Required header missing"),
        (status = 503, description = "Webhook receiver disabled — WEBHOOK_SECRET not configured"),
    ),
)]
#[tracing::instrument(skip(state, body))]
async fn receive_webhook(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<impl IntoResponse> {
    let Some(secret) = state.inner.config.webhook_secret.as_deref() else {
        return Err(AppError::Validation(
            "webhook receiver disabled (WEBHOOK_SECRET not set)".into(),
        ));
    };

    let signature = headers
        .get(SIGNATURE_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("sha256="))
        .ok_or(AppError::Unauthorized)?;
    let provided = hex::decode(signature).map_err(|_| AppError::Unauthorized)?;

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| AppError::Unauthorized)?;
    mac.update(&body);
    let expected = mac.finalize().into_bytes();

    // Constant-time compare to defeat timing oracles.
    if provided.ct_eq(&expected[..]).unwrap_u8() == 0 {
        tracing::warn!(provider = %provider, "webhook signature mismatch");
        return Err(AppError::Unauthorized);
    }

    let event_id = headers
        .get(EVENT_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Validation(format!("missing {EVENT_ID_HEADER}")))?;
    let event_type = headers
        .get(EVENT_TYPE_HEADER)
        .and_then(|v| v.to_str().ok());

    let payload: serde_json::Value =
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);

    sqlx::query!(
        "INSERT INTO webhook_events (provider, event_id, event_type, payload)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (provider, event_id) DO NOTHING",
        provider,
        event_id,
        event_type,
        payload,
    )
    .execute(state.db())
    .await?;

    // Dispatch table per provider lives here. Keep handlers pure (DB writes
    // only) and small — anything slower goes on the cron in jobs/.
    // match provider.as_str() {
    //     "stripe" => stripe::handle(&state, event_type, &payload).await?,
    //     _ => {}
    // }

    Ok(StatusCode::ACCEPTED)
}
