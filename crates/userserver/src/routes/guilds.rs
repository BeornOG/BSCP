//! `/api/guilds` + `/api/gw/:cs/*path` — guild access proxied through the user's
//! own server (the browser never talks to a channel server directly).

use crate::auth::AuthUser;
use crate::guilds::gateway::{self, GwResponse};
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use bscp_common::{now_ts, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/guilds", get(list))
        .route("/api/guilds/join", post(join))
        .route("/api/guilds/create", post(create))
        .route("/api/guilds/leave", post(leave))
        .route("/api/gw/:cs/*path", any(gw))
}

fn gw_resp(r: GwResponse) -> Response {
    let code = StatusCode::from_u16(r.status).unwrap_or(StatusCode::BAD_GATEWAY);
    (code, Json(r.body)).into_response()
}

/// Raw gateway — forward any guild API call to the channel server.
async fn gw(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((cs, path)): Path<(String, String)>,
    RawQuery(query): RawQuery,
    method: Method,
    body: Bytes,
) -> Response {
    let json_body: Option<Value> = if body.is_empty() {
        None
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => Some(v),
            Err(_) => return (StatusCode::BAD_REQUEST, "body must be JSON").into_response(),
        }
    };
    let rm = match reqwest::Method::from_bytes(method.as_str().as_bytes()) {
        Ok(m) => m,
        Err(_) => return StatusCode::METHOD_NOT_ALLOWED.into_response(),
    };
    match gateway::forward(&state, &auth.user, &cs, rm, &path, query.as_deref(), json_body).await {
        Ok(r) => gw_resp(r),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

// ── membership cache ─────────────────────────────────────────────────

async fn upsert_membership(
    state: &AppState,
    user_id: &str,
    cs: &str,
    guild_id: &str,
    name: Option<&str>,
    icon: Option<&str>,
) {
    let _ = sqlx::query(
        "INSERT INTO guild_memberships (user_id, channel_server, guild_id, name, icon, joined_at) \
         VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(user_id,channel_server,guild_id) \
         DO UPDATE SET name = COALESCE(excluded.name, name), icon = COALESCE(excluded.icon, icon)",
    )
    .bind(user_id)
    .bind(cs)
    .bind(guild_id)
    .bind(name)
    .bind(icon)
    .bind(now_ts())
    .execute(&state.pool)
    .await;
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    refresh: bool,
}

#[derive(sqlx::FromRow)]
struct MembershipRow {
    channel_server: String,
    guild_id: String,
    name: Option<String>,
    icon: Option<String>,
    joined_at: f64,
}

async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Query(q): axum::extract::Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    if q.refresh {
        let servers: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT channel_server FROM guild_memberships WHERE user_id = ?",
        )
        .bind(&auth.user.id)
        .fetch_all(&state.pool)
        .await?;
        for cs in servers {
            if let Ok(r) = gateway::forward(
                &state, &auth.user, &cs, reqwest::Method::GET, "guilds/mine", None, None,
            )
            .await
            {
                if let Some(arr) = r.body.as_array() {
                    for g in arr {
                        if let Some(id) = g.get("id").and_then(|v| v.as_str()) {
                            upsert_membership(
                                &state, &auth.user.id, &cs, id,
                                g.get("name").and_then(|v| v.as_str()),
                                g.get("icon").and_then(|v| v.as_str()),
                            )
                            .await;
                        }
                    }
                }
            }
        }
    }

    let rows = sqlx::query_as::<_, MembershipRow>(
        "SELECT channel_server, guild_id, name, icon, joined_at FROM guild_memberships \
         WHERE user_id = ? ORDER BY joined_at",
    )
    .bind(&auth.user.id)
    .fetch_all(&state.pool)
    .await?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({ "channel_server": r.channel_server, "guild_id": r.guild_id,
                    "name": r.name, "icon": r.icon, "joined_at": r.joined_at })
        })
        .collect();
    Ok(Json(json!(out)))
}

/// Parse an invite link (`http://cs/invite/CODE`, `cs/invite/CODE`, or `cs/CODE`).
fn parse_invite(input: &str) -> Option<(String, String)> {
    let s = input.trim().trim_start_matches("https://").trim_start_matches("http://");
    let s = s.trim_end_matches('/');
    if let Some((host, rest)) = s.split_once('/') {
        let code = rest.rsplit('/').next()?;
        if !host.is_empty() && !code.is_empty() {
            return Some((host.to_string(), code.to_string()));
        }
    }
    None
}

#[derive(Deserialize)]
struct JoinBody {
    invite: String,
}

async fn join(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(b): Json<JoinBody>,
) -> Result<Json<Value>, ApiError> {
    let (cs, code) = parse_invite(&b.invite).ok_or_else(|| ApiError::bad_request("bad invite link"))?;
    let r = gateway::forward(
        &state, &auth.user, &cs, reqwest::Method::POST,
        &format!("invites/{code}/accept"), None, Some(json!({})),
    )
    .await
    .map_err(|e| ApiError::bad_gateway(e.to_string()))?;

    if !(200..300).contains(&r.status) {
        return Ok(Json(json!({ "ok": false, "status": r.status, "error": r.body })));
    }
    let gid = r.body.get("guild_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let name = r.body.get("name").and_then(|v| v.as_str());
    upsert_membership(&state, &auth.user.id, &cs, &gid, name, None).await;
    Ok(Json(json!({ "ok": true, "channel_server": cs, "guild_id": gid, "name": name })))
}

#[derive(Deserialize)]
struct CreateBody {
    channel_server: String,
    name: String,
    icon: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(b): Json<CreateBody>,
) -> Result<Json<Value>, ApiError> {
    let cs = b.channel_server.trim().to_string();
    let r = gateway::forward(
        &state, &auth.user, &cs, reqwest::Method::POST, "guilds", None,
        Some(json!({ "name": b.name, "icon": b.icon })),
    )
    .await
    .map_err(|e| ApiError::bad_gateway(e.to_string()))?;
    if !(200..300).contains(&r.status) {
        return Ok(Json(json!({ "ok": false, "status": r.status, "error": r.body })));
    }
    let gid = r.body.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    upsert_membership(&state, &auth.user.id, &cs, &gid, Some(b.name.trim()), b.icon.as_deref()).await;
    Ok(Json(json!({ "ok": true, "channel_server": cs, "guild_id": gid })))
}

#[derive(Deserialize)]
struct LeaveBody {
    channel_server: String,
    guild_id: String,
}

async fn leave(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(b): Json<LeaveBody>,
) -> Result<Json<Value>, ApiError> {
    let me = state.full_id(&auth.user.username);
    let _ = gateway::forward(
        &state, &auth.user, &b.channel_server, reqwest::Method::DELETE,
        &format!("guilds/{}/members/{}", b.guild_id, urlencode(&me)), None, None,
    )
    .await;
    sqlx::query("DELETE FROM guild_memberships WHERE user_id = ? AND channel_server = ? AND guild_id = ?")
        .bind(&auth.user.id)
        .bind(&b.channel_server)
        .bind(&b.guild_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
