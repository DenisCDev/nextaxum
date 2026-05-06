//! Integration tests for the items handlers driven through the actual axum
//! Router. `#[sqlx::test]` provisions a fresh database per test from
//! `DATABASE_URL`'s superuser connection, applies all `migrations/`, then
//! loads the named fixtures.
//!
//! These tests bypass the JWT middleware and inject a `Claims` extension
//! directly so they exercise the handler + db layer without depending on a
//! live Supabase instance. JWT verification has its own unit tests.

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use backend::middleware::auth::Claims;
use backend::test_support;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: Uuid = match Uuid::try_parse("11111111-1111-1111-1111-111111111111") {
    Ok(u) => u,
    Err(_) => unreachable!(),
};

fn alice_claims() -> Claims {
    Claims {
        sub: ALICE,
        role: "authenticated".into(),
        exp: usize::MAX / 2,
        aud: "authenticated".into(),
        email: Some("alice@test.local".into()),
    }
}

#[sqlx::test(fixtures("users", "items"))]
async fn list_items_returns_only_caller_rows(pool: PgPool) {
    let app = test_support::router_for_tests(pool, alice_claims()).await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/items")
                .header(header::AUTHORIZATION, "Bearer test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let titles: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles.len(), 3, "alice has exactly 3 items, bob's row is hidden");
    assert!(titles.iter().all(|t| t.starts_with("alice")));
}

#[sqlx::test(fixtures("users"))]
async fn create_item_persists_and_returns_201(pool: PgPool) {
    let app = test_support::router_for_tests(pool.clone(), alice_claims()).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/items")
                .header(header::AUTHORIZATION, "Bearer test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"title":"new"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE user_id = $1")
        .bind(ALICE)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(fixtures("users", "items"))]
async fn update_item_triggers_updated_at_change(pool: PgPool) {
    let item_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1").unwrap();
    let before: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT updated_at FROM items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let app = test_support::router_for_tests(pool.clone(), alice_claims()).await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/items/{item_id}"))
                .header(header::AUTHORIZATION, "Bearer test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"title":"renamed"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let after: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT updated_at FROM items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        after > before,
        "moddatetime trigger must bump updated_at without the app passing it"
    );
}

#[sqlx::test(fixtures("users", "items"))]
async fn delete_item_returns_404_for_other_user_row(pool: PgPool) {
    let bob_item = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbb1";
    let app = test_support::router_for_tests(pool, alice_claims()).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/items/{bob_item}"))
                .header(header::AUTHORIZATION, "Bearer test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
