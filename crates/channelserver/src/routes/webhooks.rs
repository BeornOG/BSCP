//! Guild-integrated channel webhooks.
//!
//! Management lives under `/api/channels/:cid/webhooks` + `/api/webhooks/:wid`
//! and is gated by `MANAGE_CHANNELS`. Delivery is `POST /webhooks/:wid/:token`
//! (public, token-authed) and drops a message into the channel with `channel_id`
//! set so it shows up in the guild UI. Legacy `channel_path`-only webhooks (rows
//! with a NULL `channel_id`, created via `/api/channel/webhooks`) still deliver
//! through the same endpoint.

use super::{channel_ctx, ApiErr, ApiResult};
use crate::auth::{guard, Assertion};
use crate::perms::MANAGE_CHANNELS;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use bscp_common::{now_ts, random_token, uuid};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::FromRow;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/channels/:cid/webhooks", get(list).post(create))
        .route(
            "/api/webhooks/:wid",
            axum::routing::patch(patch_hook).delete(delete_hook),
        )
        .route("/api/webhooks/:wid/regenerate", post(regenerate))
        .route("/webhooks/:wid/:token", post(receive))
}

#[derive(FromRow)]
struct Webhook {
    id: String,
    channel_path: String,
    channel_id: Option<String>,
    name: String,
    token: String,
    profile_pic: Option<String>,
    created_at: f64,
    last_used: Option<f64>,
}

fn webhook_url(domain: &str, id: &str, token: &str) -> String {
    format!("http://{domain}/webhooks/{id}/{token}")
}

fn view(w: &Webhook, domain: &str) -> Value {
    json!({
        "id": w.id,
        "name": w.name,
        "channel_id": w.channel_id,
        "url": webhook_url(domain, &w.id, &w.token),
        "profile_pic": w.profile_pic,
        "created_at": w.created_at,
        "last_used": w.last_used,
    })
}

/// Load a guild webhook and the guild it belongs to (via its channel).
async fn hook_guild(state: &AppState, wid: &str) -> ApiResult<(Webhook, String)> {
    let w = sqlx::query_as::<_, Webhook>("SELECT * FROM channel_webhooks WHERE id = ?")
        .bind(wid)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiErr::not_found("webhook not found"))?;
    let cid = w
        .channel_id
        .clone()
        .ok_or_else(|| ApiErr::bad("not a guild webhook"))?;
    let (gid, _, _) = channel_ctx(state, &cid).await?;
    Ok((w, gid))
}

async fn list(
    State(state): State<AppState>,
    user: Assertion,
    Path(cid): Path<String>,
) -> ApiResult<Json<Value>> {
    let (gid, _, _) = channel_ctx(&state, &cid).await?;
    guard(&state, &user.sub, &gid, None, MANAGE_CHANNELS).await?;
    let rows = sqlx::query_as::<_, Webhook>(
        "SELECT * FROM channel_webhooks WHERE channel_id = ? ORDER BY created_at",
    )
    .bind(&cid)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(rows.iter().map(|w| view(w, &state.domain)).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
    avatar_url: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    user: Assertion,
    Path(cid): Path<String>,
    Json(b): Json<CreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (gid, path, kind) = channel_ctx(&state, &cid).await?;
    guard(&state, &user.sub, &gid, None, MANAGE_CHANNELS).await?;
    if kind != "text" {
        return Err(ApiErr::bad("webhooks can only post to text channels"));
    }
    let name = b.name.trim();
    if name.is_empty() {
        return Err(ApiErr::bad("name required"));
    }

    let id = uuid();
    let token = random_token(32);
    let created = now_ts();
    sqlx::query(
        "INSERT INTO channel_webhooks \
         (id, channel_path, channel_id, name, token, profile_pic, created_by, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&path)
    .bind(&cid)
    .bind(name)
    .bind(&token)
    .bind(&b.avatar_url)
    .bind(&user.sub)
    .bind(created)
    .execute(&state.pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "name": name,
            "channel_id": cid,
            "url": webhook_url(&state.domain, &id, &token),
            "profile_pic": b.avatar_url,
            "created_at": created,
        })),
    ))
}

#[derive(Deserialize)]
struct PatchBody {
    name: Option<String>,
    avatar_url: Option<Option<String>>,
}

async fn patch_hook(
    State(state): State<AppState>,
    user: Assertion,
    Path(wid): Path<String>,
    Json(b): Json<PatchBody>,
) -> ApiResult<Json<Value>> {
    let (_, gid) = hook_guild(&state, &wid).await?;
    guard(&state, &user.sub, &gid, None, MANAGE_CHANNELS).await?;
    if let Some(n) = b.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        sqlx::query("UPDATE channel_webhooks SET name = ? WHERE id = ?")
            .bind(n)
            .bind(&wid)
            .execute(&state.pool)
            .await?;
    }
    if let Some(a) = b.avatar_url {
        sqlx::query("UPDATE channel_webhooks SET profile_pic = ? WHERE id = ?")
            .bind(a)
            .bind(&wid)
            .execute(&state.pool)
            .await?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn delete_hook(
    State(state): State<AppState>,
    user: Assertion,
    Path(wid): Path<String>,
) -> ApiResult<StatusCode> {
    let (_, gid) = hook_guild(&state, &wid).await?;
    guard(&state, &user.sub, &gid, None, MANAGE_CHANNELS).await?;
    sqlx::query("DELETE FROM channel_webhooks WHERE id = ?")
        .bind(&wid)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn regenerate(
    State(state): State<AppState>,
    user: Assertion,
    Path(wid): Path<String>,
) -> ApiResult<Json<Value>> {
    let (_, gid) = hook_guild(&state, &wid).await?;
    guard(&state, &user.sub, &gid, None, MANAGE_CHANNELS).await?;
    let token = random_token(32);
    sqlx::query("UPDATE channel_webhooks SET token = ? WHERE id = ?")
        .bind(&token)
        .bind(&wid)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "url": webhook_url(&state.domain, &wid, &token) })))
}

// ── delivery ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Payload {
    content: Option<String>,
    /// Discord-compatible per-message display-name override.
    username: Option<String>,
    /// Accepted for Discord compatibility; not rendered yet.
    #[allow(dead_code)]
    avatar_url: Option<String>,
}

async fn receive(
    State(state): State<AppState>,
    Path((wid, token)): Path<(String, String)>,
    Json(p): Json<Payload>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let content = p.content.unwrap_or_default();
    if content.trim().is_empty() {
        return Err(ApiErr::bad("missing content"));
    }
    let w = sqlx::query_as::<_, Webhook>("SELECT * FROM channel_webhooks WHERE id = ? AND token = ?")
        .bind(&wid)
        .bind(&token)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiErr::not_found("invalid webhook"))?;

    let _ = sqlx::query("UPDATE channel_webhooks SET last_used = ? WHERE id = ?")
        .bind(now_ts())
        .bind(&w.id)
        .execute(&state.pool)
        .await;

    let sender = p
        .username
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| w.name.clone());

    let id = format!("{}/message/{}", state.domain, uuid());
    let ts = now_ts();
    sqlx::query(
        "INSERT INTO channel_messages \
         (id, channel_path, channel_id, sender, text, timestamp, via_webhook) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&w.channel_path)
    .bind(&w.channel_id)
    .bind(&sender)
    .bind(&content)
    .bind(ts)
    .bind(&w.id)
    .execute(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "message_id": id }))))
}
