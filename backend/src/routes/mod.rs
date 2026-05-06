mod health;
mod items;
mod profile;
mod webhooks;

use std::time::Duration;

use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, Method};
use axum::middleware as axum_mw;
use axum::Router;
use tower::ServiceBuilder;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

use crate::middleware::auth::require_auth;
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "NextAxum API",
        description = "Backend API for the Next.js + Axum + Supabase template.",
        version = env!("CARGO_PKG_VERSION"),
    ),
    tags(
        (name = "items", description = "Per-user item CRUD"),
        (name = "profile", description = "Caller's public.profiles row"),
        (name = "webhooks", description = "Signed external event ingestion"),
        (name = "health", description = "Liveness and readiness probes"),
    ),
    components(),
    modifiers(&SecurityAddon),
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::new);
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some("Supabase access token (HS256 or asymmetric)"))
                    .build(),
            ),
        );
    }
}

pub fn create_router(state: AppState) -> Router {
    let config = &state.inner.config;

    // Per-IP rate limit (peer IP — for trusted-proxy deploys behind Railway/Vercel,
    // configure GovernorConfigBuilder.use_headers() to read X-Forwarded-For).
    let governor = GovernorConfigBuilder::default()
        .per_second(config.rate_limit_per_sec)
        .burst_size(config.rate_limit_burst)
        .finish()
        .expect("invalid governor config");

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
    let protected = OpenApiRouter::new()
        .merge(items::router())
        .merge(profile::router())
        .route_layer(axum_mw::from_fn_with_state(state.clone(), require_auth));

    // Public routes (health/ready + signed webhook receiver)
    let public = OpenApiRouter::new()
        .merge(health::router())
        .merge(webhooks::router());

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/api", protected)
        .merge(public)
        .split_for_parts();

    router
        // Swagger UI at /docs (mounted regardless of build profile — the spec
        // contains no secrets and the UI is read-only).
        .merge(SwaggerUi::new("/docs").url("/openapi.json", api))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                // Request ID — outermost so all layers see it
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(CatchPanicLayer::new())
                .layer(
                    TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
                        // Promote x-request-id (set upstream by SetRequestIdLayer) into the
                        // root tracing span so every log line correlates to a single request.
                        let request_id = req
                            .headers()
                            .get("x-request-id")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("");
                        tracing::info_span!(
                            "http_request",
                            method = %req.method(),
                            uri = %req.uri(),
                            request_id = %request_id,
                        )
                    }),
                )
                // Per-IP rate limit (replaces tower::limit::RateLimitLayer which is
                // a single shared bucket — see tokio-rs/axum#2634).
                .layer(GovernorLayer::new(governor))
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
