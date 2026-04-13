mod health;
mod items;

use std::time::Duration;

use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, Method};
use axum::middleware as axum_mw;
use axum::Router;
use tower::limit::RateLimitLayer;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::middleware::auth::require_auth;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let config = &state.inner.config;

    let cors = CorsLayer::new()
        .allow_origin(
            state
                .inner
                .frontend_url
                .parse::<HeaderValue>()
                .expect("invalid FRONTEND_URL"),
        )
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION])
        .allow_credentials(true)
        .max_age(Duration::from_secs(3600));

    // Protected routes — require valid Supabase JWT
    let protected = Router::new()
        .merge(items::router())
        .route_layer(axum_mw::from_fn_with_state(state.clone(), require_auth));

    // Public routes
    let public = Router::new().merge(health::router());

    Router::new()
        .nest("/api", protected)
        .merge(public)
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                // Request ID — outermost so all layers see it
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(CatchPanicLayer::new())
                .layer(TraceLayer::new_for_http())
                // Rate limit: 100 requests per 10 seconds globally
                .layer(RateLimitLayer::new(100, Duration::from_secs(10)))
                .layer(TimeoutLayer::new(Duration::from_secs(config.request_timeout_secs)))
                .layer(CompressionLayer::new())
                .layer(RequestBodyLimitLayer::new(config.body_limit_bytes))
                .layer(cors)
                // Security headers
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("x-content-type-options"),
                    HeaderValue::from_static("nosniff"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("x-frame-options"),
                    HeaderValue::from_static("DENY"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("x-xss-protection"),
                    HeaderValue::from_static("0"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("referrer-policy"),
                    HeaderValue::from_static("strict-origin-when-cross-origin"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("permissions-policy"),
                    HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("strict-transport-security"),
                    HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("content-security-policy"),
                    HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
                )),
        )
}
