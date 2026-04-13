use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

// -- User (maps to Supabase auth.users) --

#[derive(Debug, Serialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

// -- Example domain entity --

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Item {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateItem {
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateItem {
    #[validate(length(min = 1, max = 255))]
    pub title: Option<String>,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
}

// -- Pagination --

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

impl PaginationParams {
    /// Decode cursor from "rfc3339,uuid" format.
    pub fn decode_cursor(&self) -> Option<(DateTime<Utc>, Uuid)> {
        let cursor = self.cursor.as_ref()?;
        let (ts_str, id_str) = cursor.split_once(',')?;
        let ts = ts_str.parse::<DateTime<Utc>>().ok()?;
        let id = id_str.parse::<Uuid>().ok()?;
        Some((ts, id))
    }

    /// Encode a cursor from created_at and id.
    pub fn encode_cursor(created_at: &DateTime<Utc>, id: &Uuid) -> String {
        format!("{},{}", created_at.to_rfc3339(), id)
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}
