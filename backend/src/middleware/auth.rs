use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub role: String,
    pub exp: usize,
    pub aud: String,
    pub email: Option<String>,
}

/// Authenticates a request using Supabase JWT.
///
/// Verification path is selected by the JWT `alg` header:
/// - `HS256` → symmetric verification with `SUPABASE_JWT_SECRET` (legacy projects).
/// - `RS256`/`ES256`/`EdDSA` → asymmetric verification via JWKS cached from
///   `SUPABASE_JWKS_URL` (recommended for new projects per Supabase 2024-Q4 guidance).
///
/// Both paths can be active simultaneously; per-token alg drives which is used.
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let header = decode_header(token).map_err(|e| {
        tracing::debug!(error = %e, "jwt header decode failed");
        AppError::Unauthorized
    })?;

    let token_data = match header.alg {
        Algorithm::HS256 => {
            let mut validation = Validation::new(Algorithm::HS256);
            validation.set_audience(&["authenticated"]);
            let key = DecodingKey::from_secret(state.inner.jwt_secret.as_bytes());
            decode::<Claims>(token, &key, &validation)
        }
        // Asymmetric — requires JWKS to be configured.
        alg @ (Algorithm::RS256 | Algorithm::ES256 | Algorithm::EdDSA) => {
            let jwks = state
                .inner
                .jwks
                .as_ref()
                .ok_or_else(|| {
                    tracing::warn!("JWT uses {alg:?} but SUPABASE_JWKS_URL is not configured");
                    AppError::Unauthorized
                })?;
            let kid = header.kid.ok_or(AppError::Unauthorized)?;
            let key_set = jwks.get().await.map_err(|e| {
                tracing::error!(error = %e, "JWKS fetch failed");
                AppError::Unauthorized
            })?;
            let jwk = key_set.find(&kid).ok_or(AppError::Unauthorized)?;
            let key = DecodingKey::from_jwk(jwk).map_err(|_| AppError::Unauthorized)?;
            let mut validation = Validation::new(alg);
            validation.set_audience(&["authenticated"]);
            decode::<Claims>(token, &key, &validation)
        }
        other => {
            tracing::debug!(alg = ?other, "unsupported jwt algorithm");
            return Err(AppError::Unauthorized);
        }
    }
    .map_err(|e| {
        tracing::debug!(error = %e, "jwt verification failed");
        AppError::Unauthorized
    })?;

    req.extensions_mut().insert(token_data.claims);
    Ok(next.run(req).await)
}
