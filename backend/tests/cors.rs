use axum::body::Body;
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN,
};
use axum::http::{Method, Request, StatusCode};
use backend::middleware::auth::Claims;
use backend::test_support;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn cors_allows_patch_and_idempotency_key_from_frontend() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgresql://postgres:postgres@localhost/nextaxum_test")
        .expect("test database URL must be valid");
    let claims = Claims {
        sub: Uuid::nil(),
        role: "authenticated".into(),
        exp: usize::MAX / 2,
        aud: "authenticated".into(),
        email: None,
    };
    let app = test_support::router_for_tests(pool, claims).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/items/00000000-0000-0000-0000-000000000000")
                .header(ORIGIN, "http://localhost:3000")
                .header(ACCESS_CONTROL_REQUEST_METHOD, "PATCH")
                .header(ACCESS_CONTROL_REQUEST_HEADERS, "idempotency-key")
                .body(Body::empty())
                .expect("preflight request must be valid"),
        )
        .await
        .expect("router must answer preflight");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
        "http://localhost:3000"
    );
    assert!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_METHODS)
            .unwrap()
            .to_str()
            .unwrap()
            .split(',')
            .any(|method| method.trim() == "PATCH")
    );
    assert!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_HEADERS)
            .unwrap()
            .to_str()
            .unwrap()
            .split(',')
            .any(|header| header.trim().eq_ignore_ascii_case("idempotency-key"))
    );
}
