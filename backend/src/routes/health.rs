use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

/// Liveness vs readiness split (Kubernetes / Railway pattern):
///
/// - `/health`  liveness   — process is alive. NEVER touches dependencies.
/// - `/ready`   readiness  — process can serve requests. Probes DB and JWKS.
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(liveness))
        .routes(routes!(readiness))
}

/// Liveness probe. Always returns 200 — used by orchestrators to decide
/// whether to RESTART the process.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses((status = 200, description = "Process is alive")),
)]
async fn liveness() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// Readiness probe. 200 only when the DB and (optionally) the JWKS endpoint
/// are reachable. Used by orchestrators to decide whether to ROUTE traffic.
#[utoipa::path(
    get,
    path = "/ready",
    tag = "health",
    responses(
        (status = 200, description = "Ready to serve traffic"),
        (status = 503, description = "A required dependency is unavailable"),
    ),
)]
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
