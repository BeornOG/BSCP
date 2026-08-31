//! `/api/guilds` — guild lifecycle + detail.

use super::{guild_owner, ApiErr, ApiResult};
use crate::auth::{guard, Assertion};
use crate::models::{Channel, Guild, Role};
use crate::perms;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use bscp_common::{now_ts, uuid};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/guilds", axum::routing::post(create))
        .route("/api/guilds/mine", get(mine))
        .route("/api/guilds/:gid", get(detail).patch(patch).delete(delete_guild))
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
    icon: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    user: Assertion,
    Json(body): Json<CreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let allowed: Option<i64> = sqlx::query_scalar("SELECT 1 FROM guild_creators WHERE user_id = ?")
        .bind(&user.sub)
        .fetch_optional(&state.pool)
        .await?;
    if allowed.is_none() {
        return Err(ApiErr::forbidden("not allowed to create guilds on this server"));
    }
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiErr::bad("name required"));
    }

    let gid = uuid();
    let now = now_ts();
    sqlx::query("INSERT INTO guilds (id, name, icon, owner, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&gid)
        .bind(name)
        .bind(&body.icon)
        .bind(&user.sub)
        .bind(now)
        .execute(&state.pool)
        .await?;
    sqlx::query("INSERT INTO guild_members (guild_id, user_id, joined_at) VALUES (?, ?, ?)")
        .bind(&gid)
        .bind(&user.sub)
        .bind(now)
        .execute(&state.pool)
        .await?;
    sqlx::query(
        "INSERT INTO roles (id, guild_id, name, position, permissions, is_everyone) \
         VALUES (?, ?, '@everyone', 0, ?, 1)",
    )
    .bind(uuid())
    .bind(&gid)
    .bind(perms::EVERYONE_DEFAULT as i64)
    .execute(&state.pool)
    .await?;

    for (name, kind, pos) in [("general", "text", 0), ("General", "voice", 1)] {
        let cid = uuid();
        sqlx::query(
            "INSERT INTO channels (id, guild_id, name, kind, position, path) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&cid)
        .bind(&gid)
        .bind(name)
        .bind(kind)
        .bind(pos)
        .bind(format!("{}#{}#{}", state.domain, gid, cid))
        .execute(&state.pool)
        .await?;
    }

    Ok((StatusCode::CREATED, Json(json!({ "id": gid, "name": name, "icon": body.icon, "owner": user.sub }))))
}

async fn mine(State(state): State<AppState>, user: Assertion) -> ApiResult<Json<Value>> {
    let rows = sqlx::query_as::<_, Guild>(
        "SELECT g.* FROM guilds g JOIN guild_members m ON m.guild_id = g.id WHERE m.user_id = ? \
         ORDER BY g.created_at",
    )
    .bind(&user.sub)
    .fetch_all(&state.pool)
    .await?;
    let mut out = Vec::new();
    for g in rows {
        let members: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guild_members WHERE guild_id = ?")
            .bind(&g.id)
            .fetch_one(&state.pool)
            .await?;
        out.push(json!({ "id": g.id, "name": g.name, "icon": g.icon, "owner": g.owner, "member_count": members }));
    }
    Ok(Json(json!(out)))
}

async fn detail(
    State(state): State<AppState>,
    user: Assertion,
    Path(gid): Path<String>,
) -> ApiResult<Json<Value>> {
    let guild = sqlx::query_as::<_, Guild>("SELECT * FROM guilds WHERE id = ?")
        .bind(&gid)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiErr::not_found("guild not found"))?;

    let my_guild_perms = perms::effective(&state, &gid, &user.sub, None).await;
    let is_member: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM guild_members WHERE guild_id = ? AND user_id = ?")
            .bind(&gid)
            .bind(&user.sub)
            .fetch_optional(&state.pool)
            .await?;
    if is_member.is_none() && guild.owner != user.sub {
        return Err(ApiErr::forbidden("not a member"));
    }

    let all_channels = sqlx::query_as::<_, Channel>(
        "SELECT * FROM channels WHERE guild_id = ? ORDER BY position, name",
    )
    .bind(&gid)
    .fetch_all(&state.pool)
    .await?;
    let mut channels = Vec::new();
    for c in all_channels {
        let p = perms::effective(&state, &gid, &user.sub, Some(&c.id)).await;
        if perms::has(p, perms::VIEW_CHANNEL) {
            channels.push(json!({
                "id": c.id, "name": c.name, "kind": c.kind, "parent_id": c.parent_id,
                "topic": c.topic, "position": c.position, "path": c.path,
                "my_permissions": p,
            }));
        }
    }

    let roles = sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE guild_id = ? ORDER BY position")
        .bind(&gid)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(json!({
        "id": guild.id, "name": guild.name, "icon": guild.icon, "owner": guild.owner,
        "my_permissions": my_guild_perms,
        "channels": channels,
        "roles": roles,
    })))
}

#[derive(Deserialize)]
struct PatchBody {
    name: Option<String>,
    icon: Option<Option<String>>,
}

async fn patch(
    State(state): State<AppState>,
    user: Assertion,
    Path(gid): Path<String>,
    Json(body): Json<PatchBody>,
) -> ApiResult<Json<Value>> {
    guard(&state, &user.sub, &gid, None, perms::MANAGE_GUILD).await?;
    if let Some(name) = body.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        sqlx::query("UPDATE guilds SET name = ? WHERE id = ?").bind(name).bind(&gid).execute(&state.pool).await?;
    }
    if let Some(icon) = body.icon {
        sqlx::query("UPDATE guilds SET icon = ? WHERE id = ?").bind(icon).bind(&gid).execute(&state.pool).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn delete_guild(
    State(state): State<AppState>,
    user: Assertion,
    Path(gid): Path<String>,
) -> ApiResult<StatusCode> {
    if guild_owner(&state, &gid).await? != user.sub {
        return Err(ApiErr::forbidden("only the owner can delete a guild"));
    }
    sqlx::query("DELETE FROM guilds WHERE id = ?").bind(&gid).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}
