//! Integration tests for the items handlers driven through the actual axum
//! Router. `#[sqlx::test]` provisions a fresh database per test from
//! `DATABASE_URL`'s superuser connection. Each database receives platform
//! fixtures before the production migrations and application fixtures.
//!
//! These tests bypass the JWT middleware and inject a `Claims` extension
//! directly so they exercise the handler + db layer without depending on a
//! live Supabase instance. JWT verification has its own unit tests.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use backend::middleware::auth::Claims;
use backend::test_support;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn prepare_database(pool: &PgPool, with_items: bool) {
    sqlx::raw_sql(include_str!("fixtures/supabase.sql"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::migrate!().run(pool).await.unwrap();
    sqlx::raw_sql(include_str!("fixtures/users.sql"))
        .execute(pool)
        .await
        .unwrap();
    if with_items {
        sqlx::raw_sql(include_str!("fixtures/items.sql"))
            .execute(pool)
            .await
            .unwrap();
    }
}

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

#[sqlx::test(migrations = false)]
async fn list_items_returns_only_caller_rows(pool: PgPool) {
    prepare_database(&pool, true).await;
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
    assert_eq!(
        titles.len(),
        3,
        "alice has exactly 3 items, bob's row is hidden"
    );
    assert!(titles.iter().all(|t| t.starts_with("alice")));
}

#[sqlx::test(migrations = false)]
async fn create_item_persists_and_returns_201(pool: PgPool) {
    prepare_database(&pool, false).await;
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

#[sqlx::test(migrations = false)]
async fn update_item_triggers_updated_at_change(pool: PgPool) {
    prepare_database(&pool, true).await;
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

#[sqlx::test(migrations = false)]
async fn delete_item_returns_404_for_other_user_row(pool: PgPool) {
    prepare_database(&pool, true).await;
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

fn create_request(key: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/items")
        .header(header::AUTHORIZATION, "Bearer test")
        .header(header::CONTENT_TYPE, "application/json")
        .header("Idempotency-Key", key)
        .body(Body::from(r#"{"title":"concurrent"}"#))
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn concurrent_retries_create_one_item_and_replay_the_same_response(pool: PgPool) {
    prepare_database(&pool, false).await;
    // Widen the old lookup/insert race deterministically, rather than rely on scheduler timing.
    sqlx::raw_sql(
        "CREATE FUNCTION slow_item_insert() RETURNS TRIGGER LANGUAGE plpgsql AS $$
         BEGIN PERFORM pg_sleep(0.1); RETURN NEW; END; $$;
         CREATE TRIGGER slow_item_insert BEFORE INSERT ON items
         FOR EACH ROW EXECUTE FUNCTION slow_item_insert();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_support::router_for_tests(pool.clone(), alice_claims()).await;
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(12));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..12 {
        let app = app.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                app.oneshot(create_request("retry-key")),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
            let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
            serde_json::from_slice::<Value>(&bytes).unwrap()
        });
    }
    let mut bodies = Vec::new();
    while let Some(result) = tasks.join_next().await {
        bodies.push(result.unwrap());
    }
    assert!(bodies.iter().all(|body| body == &bodies[0]));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM idempotency_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = false)]
async fn cached_response_failure_rolls_back_the_item(pool: PgPool) {
    prepare_database(&pool, false).await;
    sqlx::raw_sql(
        "CREATE FUNCTION fail_cached_response() RETURNS TRIGGER LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'test storage failure'; END; $$;
         CREATE TRIGGER fail_cached_response BEFORE INSERT ON idempotency_keys
         FOR EACH ROW EXECUTE FUNCTION fail_cached_response();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_support::router_for_tests(pool.clone(), alice_claims()).await;
    let response = app.oneshot(create_request("failure-key")).await.unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = false)]
async fn idempotency_keys_are_scoped_to_the_authenticated_user(pool: PgPool) {
    prepare_database(&pool, false).await;
    let mut bob = alice_claims();
    bob.sub = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let alice_app = test_support::router_for_tests(pool.clone(), alice_claims()).await;
    let bob_app = test_support::router_for_tests(pool.clone(), bob).await;
    let alice = alice_app
        .oneshot(create_request("shared-key"))
        .await
        .unwrap();
    let bob = bob_app.oneshot(create_request("shared-key")).await.unwrap();
    assert_eq!(alice.status(), StatusCode::CREATED);
    assert_eq!(bob.status(), StatusCode::CREATED);
    let alice: Value =
        serde_json::from_slice(&to_bytes(alice.into_body(), 1 << 20).await.unwrap()).unwrap();
    let bob: Value =
        serde_json::from_slice(&to_bytes(bob.into_body(), 1 << 20).await.unwrap()).unwrap();
    assert_ne!(alice["id"], bob["id"]);
    assert_ne!(alice["user_id"], bob["user_id"]);
}
