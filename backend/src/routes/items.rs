use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::db;
use crate::error::AppResult;
use crate::extractors::auth_user::AuthUser;
use crate::extractors::validated::ValidatedJson;
use crate::models::{CreateItem, Item, PaginatedItems, PaginationParams, UpdateItem};
use crate::state::AppState;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_items, create_item))
        .routes(routes!(get_item, update_item, delete_item))
}

/// List the caller's items, ordered newest-first, with cursor pagination.
#[utoipa::path(
    get,
    path = "/items",
    tag = "items",
    params(
        ("cursor" = Option<String>, Query, description = "Opaque cursor returned in the previous response"),
        ("limit" = Option<i64>, Query, description = "Page size; capped at the server's items_page_size"),
    ),
    responses(
        (status = 200, body = PaginatedItems),
        (status = 401, description = "Missing or invalid Supabase JWT"),
    ),
    security(("bearer" = [])),
)]
#[tracing::instrument(skip(state))]
async fn list_items(
    State(state): State<AppState>,
    user: AuthUser,
    Query(params): Query<PaginationParams>,
) -> AppResult<Json<PaginatedItems>> {
    let page_size = state.inner.config.items_page_size;
    let limit = params.limit.unwrap_or(page_size).min(page_size);
    let cursor = params.decode_cursor();

    let mut items = db::get_items_by_user(state.db(), user.id(), cursor, limit).await?;

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

    Ok(Json(PaginatedItems {
        data: items,
        next_cursor,
        has_more,
    }))
}

/// Create a new item owned by the caller.
#[utoipa::path(
    post,
    path = "/items",
    tag = "items",
    request_body = CreateItem,
    responses(
        (status = 201, body = Item),
        (status = 400, description = "Validation failure"),
        (status = 401, description = "Missing or invalid Supabase JWT"),
    ),
    security(("bearer" = [])),
)]
#[tracing::instrument(skip(state, input))]
async fn create_item(
    State(state): State<AppState>,
    user: AuthUser,
    ValidatedJson(input): ValidatedJson<CreateItem>,
) -> AppResult<(StatusCode, Json<Item>)> {
    let item = db::create_item(state.db(), user.id(), &input).await?;
    Ok((StatusCode::CREATED, Json(item)))
}

/// Fetch a single item with weak ETag-based conditional GET.
#[utoipa::path(
    get,
    path = "/items/{id}",
    tag = "items",
    params(("id" = Uuid, Path, description = "Item id")),
    responses(
        (status = 200, body = Item),
        (status = 304, description = "Cached copy still fresh"),
        (status = 401, description = "Missing or invalid Supabase JWT"),
        (status = 404, description = "Item not found or owned by another user"),
    ),
    security(("bearer" = [])),
)]
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

/// Patch an item. Fields left out keep their current value.
#[utoipa::path(
    put,
    path = "/items/{id}",
    tag = "items",
    params(("id" = Uuid, Path, description = "Item id")),
    request_body = UpdateItem,
    responses(
        (status = 200, body = Item),
        (status = 400, description = "Validation failure"),
        (status = 401, description = "Missing or invalid Supabase JWT"),
        (status = 404, description = "Item not found or owned by another user"),
    ),
    security(("bearer" = [])),
)]
#[tracing::instrument(skip(state, input))]
async fn update_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateItem>,
) -> AppResult<Json<Item>> {
    let item = db::update_item(state.db(), id, user.id(), &input).await?;
    Ok(Json(item))
}

/// Delete one of the caller's items.
#[utoipa::path(
    delete,
    path = "/items/{id}",
    tag = "items",
    params(("id" = Uuid, Path, description = "Item id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Missing or invalid Supabase JWT"),
        (status = 404, description = "Item not found or owned by another user"),
    ),
    security(("bearer" = [])),
)]
#[tracing::instrument(skip(state))]
async fn delete_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    db::delete_item(state.db(), id, user.id()).await?;
    Ok(StatusCode::NO_CONTENT)
}
