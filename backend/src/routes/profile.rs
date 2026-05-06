use axum::extract::State;
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::{AppError, AppResult};
use crate::extractors::auth_user::AuthUser;
use crate::extractors::validated::ValidatedJson;
use crate::models::{Profile, UpdateProfile};
use crate::state::AppState;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_profile, update_profile))
}

/// Fetch the caller's profile row. Auto-created by the on_auth_user_created
/// trigger when the auth.users row first appears.
#[utoipa::path(
    get,
    path = "/profile",
    tag = "profile",
    responses(
        (status = 200, body = Profile),
        (status = 404, description = "Profile row missing — should not happen"),
    ),
    security(("bearer" = [])),
)]
#[tracing::instrument(skip(state))]
async fn get_profile(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Profile>> {
    let profile = sqlx::query_as!(
        Profile,
        "SELECT id, display_name, avatar_url, updated_at
         FROM profiles
         WHERE id = $1",
        user.id(),
    )
    .fetch_optional(state.db())
    .await?
    .ok_or_else(|| AppError::NotFound("profile not found".into()))?;
    Ok(Json(profile))
}

/// Patch the caller's profile. Fields left out keep their value.
#[utoipa::path(
    put,
    path = "/profile",
    tag = "profile",
    request_body = UpdateProfile,
    responses(
        (status = 200, body = Profile),
        (status = 400, description = "Validation failure"),
    ),
    security(("bearer" = [])),
)]
#[tracing::instrument(skip(state, input))]
async fn update_profile(
    State(state): State<AppState>,
    user: AuthUser,
    ValidatedJson(input): ValidatedJson<UpdateProfile>,
) -> AppResult<Json<Profile>> {
    let profile = sqlx::query_as!(
        Profile,
        "UPDATE profiles
         SET display_name = COALESCE($2, display_name),
             avatar_url   = COALESCE($3, avatar_url)
         WHERE id = $1
         RETURNING id, display_name, avatar_url, updated_at",
        user.id(),
        input.display_name,
        input.avatar_url,
    )
    .fetch_one(state.db())
    .await?;
    Ok(Json(profile))
}
