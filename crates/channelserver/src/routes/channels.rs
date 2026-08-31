//! `/api/guilds/:gid/channels` + `/api/channels/:cid/overrides`.

use super::{channel_ctx, ApiErr, ApiResult};
use crate::auth::{guard, Assertion};
use crate::models::Channel;
use crate::perms::{self, MANAGE_CHANNELS, MANAGE_ROLES};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use bscp_common::uuid;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/guilds/:gid/channels", post(create))
        .route(
            "/api/guilds/:gid/channels/:cid",
            axum::routing::patch(patch).delete(delete_channel),
        )
        .route("/api/channels/:cid/overrides", get(list_overrides))
        .route("/api/channels/:cid/overrides/:target", axum::routing::put(put_override))
}

/// The raw override rows for a channel — powers the per-channel permission editor.
async fn list_overrides(
    State(state): State<AppState>,
    user: Assertion,
    Path(cid): Path<String>,
) -> ApiResult<Json<Value>> {
    let (gid, _, _) = channel_ctx(&state, &cid).await?;
    guard(&state, &user.sub, &gid, None, MANAGE_ROLES).await?;
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT target_type, target_id, allow, deny FROM channel_overrides WHERE channel_id = ?",
    )
    .bind(&cid)
    .fetch_all(&state.pool)
    .await?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(|(tt, tid, a, d)| json!({ "target_type": tt, "target_id": tid, "allow": a as u64, "deny": d as u64 }))
        .collect();
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
    #[serde(default = "default_kind")]
    kind: String,
    parent_id: Option<String>,
    topic: Option<String>,
}
fn default_kind() -> String {
    "text".into()
}

async fn create(
    State(state): State<AppState>,
    user: Assertion,
    Path(gid): Path<String>,
    Json(body): Json<CreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    guard(&state, &user.sub, &gid, None, MANAGE_CHANNELS).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiErr::bad("name required"));
    }
    if !matches!(body.kind.as_str(), "text" | "voice" | "category") {
        return Err(ApiErr::bad("kind must be text|voice|category"));
    }

    let parent_path = match &body.parent_id {
        Some(pid) => {
            let (pg, ppath, _) = channel_ctx(&state, pid).await?;
            if pg != gid {
                return Err(ApiErr::bad("parent is in another guild"));
            }
            Some(ppath)
        }
        None => None,
    };

    let cid = uuid();
    let path = match parent_path {
        Some(pp) => format!("{pp}#{cid}"),
        None => format!("{}#{}#{}", state.domain, gid, cid),
    };
    let pos: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position),0)+1 FROM channels WHERE guild_id = ?")
        .bind(&gid)
        .fetch_one(&state.pool)
        .await?;
    sqlx::query(
        "INSERT INTO channels (id, guild_id, parent_id, name, kind, topic, position, path) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&cid)
    .bind(&gid)
    .bind(&body.parent_id)
    .bind(name)
    .bind(&body.kind)
    .bind(&body.topic)
    .bind(pos)
    .bind(&path)
    .execute(&state.pool)
    .await?;

    let ch = sqlx::query_as::<_, Channel>("SELECT * FROM channels WHERE id = ?")
        .bind(&cid)
        .fetch_one(&state.pool)
        .await?;
    Ok((StatusCode::CREATED, Json(json!(ch))))
}

#[derive(Deserialize)]
struct PatchBody {
    name: Option<String>,
    topic: Option<Option<String>>,
    position: Option<i64>,
}

async fn patch(
    State(state): State<AppState>,
    user: Assertion,
    Path((gid, cid)): Path<(String, String)>,
    Json(body): Json<PatchBody>,
) -> ApiResult<Json<Value>> {
    guard(&state, &user.sub, &gid, None, MANAGE_CHANNELS).await?;
    if let Some(n) = body.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        sqlx::query("UPDATE channels SET name = ? WHERE id = ? AND guild_id = ?")
            .bind(n).bind(&cid).bind(&gid).execute(&state.pool).await?;
    }
    if let Some(t) = body.topic {
        sqlx::query("UPDATE channels SET topic = ? WHERE id = ? AND guild_id = ?")
            .bind(t).bind(&cid).bind(&gid).execute(&state.pool).await?;
    }
    if let Some(p) = body.position {
        sqlx::query("UPDATE channels SET position = ? WHERE id = ? AND guild_id = ?")
            .bind(p).bind(&cid).bind(&gid).execute(&state.pool).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn delete_channel(
    State(state): State<AppState>,
    user: Assertion,
    Path((gid, cid)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    guard(&state, &user.sub, &gid, None, MANAGE_CHANNELS).await?;
    sqlx::query("DELETE FROM channels WHERE id = ? AND guild_id = ?")
        .bind(&cid)
        .bind(&gid)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct OverrideBody {
    target_type: String, // role | member
    #[serde(default)]
    allow: u64,
    #[serde(default)]
    deny: u64,
}

async fn put_override(
    State(state): State<AppState>,
    user: Assertion,
    Path((cid, target)): Path<(String, String)>,
    Json(body): Json<OverrideBody>,
) -> ApiResult<Json<Value>> {
    let (gid, _, _) = channel_ctx(&state, &cid).await?;
    guard(&state, &user.sub, &gid, None, MANAGE_ROLES).await?;
    if !matches!(body.target_type.as_str(), "role" | "member") {
        return Err(ApiErr::bad("target_type must be role|member"));
    }
    let (allow, deny) = (body.allow & perms::ALL, body.deny & perms::ALL);
    if allow == 0 && deny == 0 {
        sqlx::query("DELETE FROM channel_overrides WHERE channel_id = ? AND target_type = ? AND target_id = ?")
            .bind(&cid).bind(&body.target_type).bind(&target).execute(&state.pool).await?;
    } else {
        sqlx::query(
            "INSERT INTO channel_overrides (channel_id, target_type, target_id, allow, deny) \
             VALUES (?, ?, ?, ?, ?) ON CONFLICT(channel_id,target_type,target_id) \
             DO UPDATE SET allow = excluded.allow, deny = excluded.deny",
        )
        .bind(&cid)
        .bind(&body.target_type)
        .bind(&target)
        .bind(allow as i64)
        .bind(deny as i64)
        .execute(&state.pool)
        .await?;
    }
    Ok(Json(json!({ "ok": true })))
}
