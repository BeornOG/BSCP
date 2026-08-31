//! `/api/guilds/:gid/roles`.

use super::{ApiErr, ApiResult};
use crate::auth::{guard, Assertion};
use crate::models::Role;
use crate::perms::{self, MANAGE_ROLES};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use bscp_common::uuid;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/guilds/:gid/roles", get(list).post(create))
        .route("/api/guilds/:gid/roles/:rid", axum::routing::patch(patch).delete(delete_role))
}

async fn list(State(state): State<AppState>, user: Assertion, Path(gid): Path<String>) -> ApiResult<Json<Value>> {
    // any member may see roles
    guard(&state, &user.sub, &gid, None, perms::VIEW_CHANNEL).await.ok();
    let roles = sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE guild_id = ? ORDER BY position")
        .bind(&gid)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(json!(roles)))
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
    color: Option<String>,
    #[serde(default)]
    permissions: u64,
}

async fn create(
    State(state): State<AppState>,
    user: Assertion,
    Path(gid): Path<String>,
    Json(b): Json<CreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let caller = guard(&state, &user.sub, &gid, None, MANAGE_ROLES).await?;
    // can't grant permissions you don't have (unless ADMINISTRATOR)
    let perms = if perms::has(caller, perms::ADMINISTRATOR) { b.permissions } else { b.permissions & caller };
    let name = b.name.trim();
    if name.is_empty() {
        return Err(ApiErr::bad("name required"));
    }
    let rid = uuid();
    let pos: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position),0)+1 FROM roles WHERE guild_id = ?")
        .bind(&gid)
        .fetch_one(&state.pool)
        .await?;
    sqlx::query(
        "INSERT INTO roles (id, guild_id, name, color, position, permissions, is_everyone) \
         VALUES (?, ?, ?, ?, ?, ?, 0)",
    )
    .bind(&rid)
    .bind(&gid)
    .bind(name)
    .bind(&b.color)
    .bind(pos)
    .bind((perms & crate::perms::ALL) as i64)
    .execute(&state.pool)
    .await?;
    let role = sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE id = ?").bind(&rid).fetch_one(&state.pool).await?;
    Ok((StatusCode::CREATED, Json(json!(role))))
}

#[derive(Deserialize)]
struct PatchBody {
    name: Option<String>,
    color: Option<Option<String>>,
    permissions: Option<u64>,
    position: Option<i64>,
}

async fn patch(
    State(state): State<AppState>,
    user: Assertion,
    Path((gid, rid)): Path<(String, String)>,
    Json(b): Json<PatchBody>,
) -> ApiResult<Json<Value>> {
    let caller = guard(&state, &user.sub, &gid, None, MANAGE_ROLES).await?;
    let is_everyone: Option<i64> =
        sqlx::query_scalar("SELECT is_everyone FROM roles WHERE id = ? AND guild_id = ?")
            .bind(&rid)
            .bind(&gid)
            .fetch_optional(&state.pool)
            .await?;
    if is_everyone.is_none() {
        return Err(ApiErr::not_found("role not found"));
    }

    if let Some(n) = b.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        sqlx::query("UPDATE roles SET name = ? WHERE id = ?").bind(n).bind(&rid).execute(&state.pool).await?;
    }
    if let Some(c) = b.color {
        sqlx::query("UPDATE roles SET color = ? WHERE id = ?").bind(c).bind(&rid).execute(&state.pool).await?;
    }
    if let Some(p) = b.permissions {
        let p = if perms::has(caller, perms::ADMINISTRATOR) { p } else { p & caller };
        sqlx::query("UPDATE roles SET permissions = ? WHERE id = ?")
            .bind((p & crate::perms::ALL) as i64)
            .bind(&rid)
            .execute(&state.pool)
            .await?;
    }
    if let Some(pos) = b.position {
        sqlx::query("UPDATE roles SET position = ? WHERE id = ?").bind(pos).bind(&rid).execute(&state.pool).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn delete_role(
    State(state): State<AppState>,
    user: Assertion,
    Path((gid, rid)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    guard(&state, &user.sub, &gid, None, MANAGE_ROLES).await?;
    let is_everyone: Option<bool> =
        sqlx::query_scalar("SELECT is_everyone FROM roles WHERE id = ? AND guild_id = ?")
            .bind(&rid)
            .bind(&gid)
            .fetch_optional(&state.pool)
            .await?;
    match is_everyone {
        None => return Err(ApiErr::not_found("role not found")),
        Some(true) => return Err(ApiErr::bad("cannot delete @everyone")),
        Some(false) => {}
    }
    sqlx::query("DELETE FROM roles WHERE id = ?").bind(&rid).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}
