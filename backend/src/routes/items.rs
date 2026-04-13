use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::db;
use crate::error::AppResult;
use crate::extractors::auth_user::AuthUser;
use crate::extractors::validated::ValidatedJson;
use crate::models::{CreateItem, Item, PaginatedResponse, PaginationParams, UpdateItem};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/items", get(list_items).post(create_item))
        .route(
            "/items/{id}",
            get(get_item).put(update_item).delete(delete_item),
        )
}

#[tracing::instrument(skip(state))]
async fn list_items(
    State(state): State<AppState>,
    user: AuthUser,
    Query(params): Query<PaginationParams>,
) -> AppResult<Json<PaginatedResponse<Item>>> {
    let page_size = state.inner.config.items_page_size;
    let limit = params.limit.unwrap_or(page_size).min(page_size);
    let cursor = params.decode_cursor();

    let mut items = if cursor.is_none() {
        // First page: check cache
        if let Some(cached) = state.inner.items_cache.get(&user.id()).await {
            if cached.len() as i64 <= limit + 1 {
                cached
            } else {
                cached.into_iter().take((limit + 1) as usize).collect()
            }
        } else {
            let fetched = db::get_items_by_user(state.db(), user.id(), None, limit).await?;
            state.inner.items_cache.insert(user.id(), fetched.clone()).await;
            fetched
        }
    } else {
        db::get_items_by_user(state.db(), user.id(), cursor, limit).await?
    };

    let has_more = items.len() as i64 > limit;
    if has_more {
        items.pop();
    }

    let next_cursor = if has_more {
        items
            .last()
            .map(|i| PaginationParams::encode_cursor(&i.created_at, &i.id))
    } else {
        None
    };

    Ok(Json(PaginatedResponse {
        data: items,
        next_cursor,
        has_more,
    }))
}

#[tracing::instrument(skip(state, input))]
async fn create_item(
    State(state): State<AppState>,
    user: AuthUser,
    ValidatedJson(input): ValidatedJson<CreateItem>,
) -> AppResult<(StatusCode, Json<Item>)> {
    let item = db::create_item(state.db(), user.id(), &input).await?;
    state.inner.items_cache.invalidate(&user.id()).await;
    Ok((StatusCode::CREATED, Json(item)))
}

#[tracing::instrument(skip(state))]
async fn get_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let item = db::get_item(state.db(), id, user.id()).await?;
    let etag = format!("W/\"{}\"", item.updated_at.timestamp_millis());

    if let Some(if_none_match) = headers.get(IF_NONE_MATCH).and_then(|v| v.to_str().ok()) {
        if if_none_match == etag {
            return Ok(StatusCode::NOT_MODIFIED.into_response());
        }
    }

    Ok((
        [
            (ETAG, etag),
            (
                CACHE_CONTROL,
                "private, max-age=0, must-revalidate".to_string(),
            ),
        ],
        Json(item),
    )
        .into_response())
}

#[tracing::instrument(skip(state, input))]
async fn update_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateItem>,
) -> AppResult<Json<Item>> {
    let item = db::update_item(state.db(), id, user.id(), &input).await?;
    state.inner.items_cache.invalidate(&user.id()).await;
    Ok(Json(item))
}

#[tracing::instrument(skip(state))]
async fn delete_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    db::delete_item(state.db(), id, user.id()).await?;
    state.inner.items_cache.invalidate(&user.id()).await;
    Ok(StatusCode::NO_CONTENT)
}
