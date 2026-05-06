use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::state::AppState;

/// Liveness vs readiness split (Kubernetes / Railway pattern):
///
/// - `/health`  liveness   — process is alive. NEVER touches dependencies.
///                            kubelet uses this to decide whether to restart.
/// - `/ready`   readiness  — process can serve requests. Probes DB and (when
///                            configured) the JWKS cache. Failure pulls the
///                            instance out of the load balancer without
///                            triggering a restart.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(liveness))
        .route("/ready", get(readiness))
}

async fn liveness() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn readiness(
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Err(e) = sqlx::query("SELECT 1").execute(state.db()).await {
        tracing::warn!(error = %e, "readiness probe: database unreachable");
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "unavailable", "dependency": "database" })),
        ));
    }

    if let Some(jwks) = state.inner.jwks.as_ref() {
        if let Err(e) = jwks.get().await {
            tracing::warn!(error = %e, "readiness probe: JWKS unreachable");
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "unavailable", "dependency": "jwks" })),
            ));
        }
    }

    Ok(Json(json!({ "status": "ok" })))
}
