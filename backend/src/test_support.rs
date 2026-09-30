//! Helpers used by `tests/`. Exposed via the library target so integration
//! tests can build a real router instance with signed test credentials and
//! connection metadata while keeping the production middleware enabled.

use axum::Router;
use axum::extract::ConnectInfo;
use axum::http::{HeaderValue, header};
use axum::middleware::{Next, from_fn};
use axum::response::Response;
use jsonwebtoken::{EncodingKey, Header, encode};
use sqlx::PgPool;

use crate::config::Config;
use crate::middleware::auth::Claims;
use crate::routes::create_router;
use crate::state::AppState;

/// Build the production router and replace the fixture bearer token with a
/// signed JWT. A synthetic peer address supplies the host metadata required
/// by the production rate limiter.
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
        rate_limit_per_sec: 100,
        rate_limit_burst: 100,
        webhook_secret: None,
    };

    let token = encode(
        &Header::default(),
        &serde_json::json!({
            "sub": claims.sub,
            "role": claims.role,
            "exp": claims.exp,
            "aud": claims.aud,
            "email": claims.email,
        }),
        &EncodingKey::from_secret(cfg.supabase_jwt_secret.as_bytes()),
    )
    .expect("test claims must encode");
    let authorization =
        HeaderValue::from_str(&format!("Bearer {token}")).expect("test JWT must be a valid header");
    let state = AppState::for_tests(pool, cfg);

    create_router(state).layer(from_fn(
        move |mut req: axum::extract::Request, next: Next| {
            let authorization = authorization.clone();
            async move {
                req.extensions_mut()
                    .insert(ConnectInfo(std::net::SocketAddr::from((
                        [127, 0, 0, 1],
                        12345,
                    ))));
                if req
                    .headers()
                    .get(header::AUTHORIZATION)
                    .is_some_and(|value| value == "Bearer test")
                {
                    req.headers_mut()
                        .insert(header::AUTHORIZATION, authorization);
                }
                let resp: Response = next.run(req).await;
                resp
            }
        },
    ))
}
