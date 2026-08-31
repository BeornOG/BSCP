//! Guild invites + shareable links.

use super::{ApiErr, ApiResult};
use crate::auth::{guard, Assertion};
use crate::perms::{CREATE_INVITE, EVERYONE_DEFAULT, MANAGE_GUILD};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use bscp_common::now_ts;
use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/guilds/:gid/invites", get(list).post(create))
        .route("/api/guilds/:gid/invites/:code", axum::routing::delete(delete_invite))
        .route("/api/invites/:code", get(preview))
        .route("/api/invites/:code/accept", post(accept))
}

fn short_code() -> String {
    const A: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut r = rand::thread_rng();
    (0..8).map(|_| A[r.gen_range(0..A.len())] as char).collect()
}

#[derive(Deserialize)]
struct CreateBody {
    max_uses: Option<i64>,
    expires_in_secs: Option<f64>,
}

async fn create(
    State(state): State<AppState>,
    user: Assertion,
    Path(gid): Path<String>,
    Json(b): Json<CreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    guard(&state, &user.sub, &gid, None, CREATE_INVITE).await?;
    let code = short_code();
    let expires_at = b.expires_in_secs.map(|s| now_ts() + s);
    sqlx::query(
        "INSERT INTO guild_invites (code, guild_id, created_by, max_uses, expires_at, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&code)
    .bind(&gid)
    .bind(&user.sub)
    .bind(b.max_uses)
    .bind(expires_at)
    .bind(now_ts())
    .execute(&state.pool)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "code": code,
            "url": format!("{}/invite/{}", state.public_url.trim_end_matches('/'), code),
            "max_uses": b.max_uses,
            "expires_at": expires_at,
        })),
    ))
}

#[derive(sqlx::FromRow)]
struct InviteRow {
    code: String,
    created_by: String,
    uses: i64,
    max_uses: Option<i64>,
    expires_at: Option<f64>,
    created_at: f64,
}

async fn list(State(state): State<AppState>, user: Assertion, Path(gid): Path<String>) -> ApiResult<Json<Value>> {
    guard(&state, &user.sub, &gid, None, MANAGE_GUILD).await?;
    let rows = sqlx::query_as::<_, InviteRow>(
        "SELECT code, created_by, uses, max_uses, expires_at, created_at FROM guild_invites \
         WHERE guild_id = ? ORDER BY created_at DESC",
    )
    .bind(&gid)
    .fetch_all(&state.pool)
    .await?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "code": r.code,
                "url": format!("{}/invite/{}", state.public_url.trim_end_matches('/'), r.code),
                "created_by": r.created_by, "uses": r.uses, "max_uses": r.max_uses,
                "expires_at": r.expires_at, "created_at": r.created_at,
            })
        })
        .collect();
    Ok(Json(json!(out)))
}

async fn delete_invite(
    State(state): State<AppState>,
    user: Assertion,
    Path((gid, code)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    guard(&state, &user.sub, &gid, None, MANAGE_GUILD).await?;
    sqlx::query("DELETE FROM guild_invites WHERE code = ? AND guild_id = ?")
        .bind(&code)
        .bind(&gid)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn resolve_invite(state: &AppState, code: &str) -> ApiResult<(String, String, Option<String>, i64)> {
    let row: Option<(String, i64, Option<i64>, Option<f64>)> = sqlx::query_as(
        "SELECT guild_id, uses, max_uses, expires_at FROM guild_invites WHERE code = ?",
    )
    .bind(code)
    .fetch_optional(&state.pool)
    .await?;
    let Some((gid, uses, max, exp)) = row else {
        return Err(ApiErr::not_found("invite not found"));
    };
    if exp.is_some_and(|e| e < now_ts()) || max.is_some_and(|m| uses >= m) {
        return Err(ApiErr::new(StatusCode::GONE, "invite is no longer valid"));
    }
    let g: (String, String, Option<String>) =
        sqlx::query_as("SELECT id, name, icon FROM guilds WHERE id = ?")
            .bind(&gid)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiErr::not_found("guild gone"))?;
    let members: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guild_members WHERE guild_id = ?")
        .bind(&gid)
        .fetch_one(&state.pool)
        .await?;
    Ok((g.0, g.1, g.2, members))
}

async fn preview(State(state): State<AppState>, Path(code): Path<String>) -> ApiResult<Json<Value>> {
    let (gid, name, icon, members) = resolve_invite(&state, &code).await?;
    Ok(Json(json!({ "guild_id": gid, "name": name, "icon": icon, "member_count": members })))
}

async fn accept(
    State(state): State<AppState>,
    user: Assertion,
    Path(code): Path<String>,
) -> ApiResult<Json<Value>> {
    let (gid, name, _icon, _members) = resolve_invite(&state, &code).await?;

    let already: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM guild_members WHERE guild_id = ? AND user_id = ?")
            .bind(&gid)
            .bind(&user.sub)
            .fetch_optional(&state.pool)
            .await?;
    if already.is_none() {
        sqlx::query("INSERT INTO guild_members (guild_id, user_id, joined_at) VALUES (?, ?, ?)")
            .bind(&gid)
            .bind(&user.sub)
            .bind(now_ts())
            .execute(&state.pool)
            .await?;
        // ensure an @everyone role exists (older guilds)
        let has_everyone: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM roles WHERE guild_id = ? AND is_everyone = 1")
                .bind(&gid)
                .fetch_optional(&state.pool)
                .await?;
        if has_everyone.is_none() {
            sqlx::query(
                "INSERT INTO roles (id, guild_id, name, position, permissions, is_everyone) \
                 VALUES (?, ?, '@everyone', 0, ?, 1)",
            )
            .bind(bscp_common::uuid())
            .bind(&gid)
            .bind(EVERYONE_DEFAULT as i64)
            .execute(&state.pool)
            .await?;
        }
        sqlx::query("UPDATE guild_invites SET uses = uses + 1 WHERE code = ?")
            .bind(&code)
            .execute(&state.pool)
            .await?;
    }

    Ok(Json(json!({ "guild_id": gid, "name": name })))
}
