use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{CreateItem, Item, UpdateItem};

pub async fn get_items_by_user(
    pool: &PgPool,
    user_id: Uuid,
    cursor: Option<(DateTime<Utc>, Uuid)>,
    limit: i64,
) -> AppResult<Vec<Item>> {
    let items = match cursor {
        Some((created_at, id)) => {
            sqlx::query_as::<_, Item>(
                "SELECT id, user_id, title, description, created_at, updated_at
                 FROM items
                 WHERE user_id = $1
                   AND (created_at, id) < ($2, $3)
                 ORDER BY created_at DESC, id DESC
                 LIMIT $4",
            )
            .bind(user_id)
            .bind(created_at)
            .bind(id)
            .bind(limit + 1)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, Item>(
                "SELECT id, user_id, title, description, created_at, updated_at
                 FROM items
                 WHERE user_id = $1
                 ORDER BY created_at DESC, id DESC
                 LIMIT $2",
            )
            .bind(user_id)
            .bind(limit + 1)
            .fetch_all(pool)
            .await?
        }
    };

    Ok(items)
}

pub async fn get_item(pool: &PgPool, id: Uuid, user_id: Uuid) -> AppResult<Item> {
    sqlx::query_as::<_, Item>(
        "SELECT id, user_id, title, description, created_at, updated_at
         FROM items
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("item not found".into()))
}

pub async fn create_item(
    executor: impl PgExecutor<'_>,
    user_id: Uuid,
    input: &CreateItem,
) -> AppResult<Item> {
    let item = sqlx::query_as::<_, Item>(
        "INSERT INTO items (id, user_id, title, description, created_at, updated_at)
         VALUES ($1, $2, $3, $4, now(), now())
         RETURNING id, user_id, title, description, created_at, updated_at",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(&input.title)
    .bind(&input.description)
    .fetch_one(executor)
    .await?;

    Ok(item)
}

pub async fn update_item(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    input: &UpdateItem,
) -> AppResult<Item> {
    let item = sqlx::query_as::<_, Item>(
        // updated_at is bumped by the items_updated_at trigger (moddatetime).
        "UPDATE items
         SET title = COALESCE($3, title),
             description = COALESCE($4, description)
         WHERE id = $1 AND user_id = $2
         RETURNING id, user_id, title, description, created_at, updated_at",
    )
    .bind(id)
    .bind(user_id)
    .bind(&input.title)
    .bind(&input.description)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("item not found".into()))?;

    Ok(item)
}

pub async fn delete_item(pool: &PgPool, id: Uuid, user_id: Uuid) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM items WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("item not found".into()));
    }

    Ok(())
}
