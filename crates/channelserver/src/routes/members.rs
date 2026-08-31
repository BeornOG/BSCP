//! `/api/guilds/:gid/members`.

use super::{guild_owner, ApiErr, ApiResult};
use crate::auth::{guard, Assertion};
use crate::perms::{KICK_MEMBERS, MANAGE_ROLES, VIEW_CHANNEL};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/guilds/:gid/members", get(list))
        .route(
            "/api/guilds/:gid/members/:uid",
            axum::routing::patch(patch).delete(remove),
        )
}

async fn list(State(state): State<AppState>, user: Assertion, Path(gid): Path<String>) -> ApiResult<Json<Value>> {
    guard(&state, &user.sub, &gid, None, VIEW_CHANNEL).await?;
    let rows: Vec<(String, Option<String>, f64)> = sqlx::query_as(
        "SELECT user_id, nickname, joined_at FROM guild_members WHERE guild_id = ? ORDER BY joined_at",
    )
    .bind(&gid)
    .fetch_all(&state.pool)
    .await?;
    let mut out = Vec::new();
    for (uid, nick, joined) in rows {
        let roles: Vec<String> = sqlx::query_scalar(
            "SELECT role_id FROM member_roles WHERE guild_id = ? AND user_id = ?",
        )
        .bind(&gid)
        .bind(&uid)
        .fetch_all(&state.pool)
        .await?;
        out.push(json!({ "user_id": uid, "nickname": nick, "joined_at": joined, "roles": roles }));
    }
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
struct PatchBody {
    roles: Vec<String>,
}

async fn patch(
    State(state): State<AppState>,
    user: Assertion,
    Path((gid, uid)): Path<(String, String)>,
    Json(b): Json<PatchBody>,
) -> ApiResult<Json<Value>> {
    guard(&state, &user.sub, &gid, None, MANAGE_ROLES).await?;
    // validate the roles exist in this guild and aren't @everyone
    let valid: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM roles WHERE guild_id = ? AND is_everyone = 0",
    )
    .bind(&gid)
    .fetch_all(&state.pool)
    .await?;
    sqlx::query("DELETE FROM member_roles WHERE guild_id = ? AND user_id = ?")
        .bind(&gid)
        .bind(&uid)
        .execute(&state.pool)
        .await?;
    for r in b.roles.iter().filter(|r| valid.contains(r)) {
        sqlx::query("INSERT OR IGNORE INTO member_roles (guild_id, user_id, role_id) VALUES (?, ?, ?)")
            .bind(&gid)
            .bind(&uid)
            .bind(r)
            .execute(&state.pool)
            .await?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn remove(
    State(state): State<AppState>,
    user: Assertion,
    Path((gid, uid)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    // self-leave is always allowed; kicking someone else needs KICK_MEMBERS
    if uid != user.sub {
        guard(&state, &user.sub, &gid, None, KICK_MEMBERS).await?;
    }
    if guild_owner(&state, &gid).await? == uid {
        return Err(ApiErr::bad("the owner cannot leave; delete or transfer the guild"));
    }
    sqlx::query("DELETE FROM guild_members WHERE guild_id = ? AND user_id = ?")
        .bind(&gid)
        .bind(&uid)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM member_roles WHERE guild_id = ? AND user_id = ?")
        .bind(&gid)
        .bind(&uid)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
