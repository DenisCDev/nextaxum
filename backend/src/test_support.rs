//! Helpers used by `tests/`. Exposed via the library target so integration
//! tests can build a real router instance with a synthetic Claims extension
//! injected by middleware (sidestepping the JWT verification path during
//! handler tests).

use axum::middleware::{from_fn, Next};
use axum::response::Response;
use axum::Router;
use sqlx::PgPool;

use crate::config::Config;
use crate::middleware::auth::Claims;
use crate::routes::create_router;
use crate::state::AppState;

/// Build the production router but swap the auth middleware for a stub that
/// inserts the supplied `Claims` into request extensions. Lets handlers run
/// the real query path while keeping the JWT layer out of the test loop.
pub async fn router_for_tests(pool: PgPool, claims: Claims) -> Router {
    let cfg = Config {
        database_url: String::new(),
        supabase_jwt_secret: "test".into(),
        supabase_jwks_url: None,
        jwks_ttl_secs: 0,
        frontend_url: "http://localhost:3000".into(),
        port: 0,
        db_max_connections: 1,
        db_min_connections: 1,
        request_timeout_secs: 30,
        body_limit_bytes: 1 << 20,
        items_page_size: 50,
        rate_limit_per_sec: 1_000_000,
        rate_limit_burst: 1_000_000,
    };

    let state = AppState::for_tests(pool, cfg);
    let claims = std::sync::Arc::new(claims);

    create_router(state).layer(from_fn(move |mut req: axum::extract::Request, next: Next| {
        let claims = claims.clone();
        async move {
            req.extensions_mut().insert((*claims).clone());
            let resp: Response = next.run(req).await;
            resp
        }
    }))
}
