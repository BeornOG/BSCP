//! Voice channels — the channel server is the call manager (signaling only).

use super::{channel_ctx, ApiErr, ApiResult};
use crate::auth::{guard, Assertion};
use crate::perms::{CONNECT, VIEW_CHANNEL};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/channels/:cid/voice-token", post(voice_token))
        .route("/api/channels/:cid/voice-roster", get(voice_roster))
}

async fn voice_token(
    State(state): State<AppState>,
    user: Assertion,
    Path(cid): Path<String>,
) -> ApiResult<Json<Value>> {
    let (gid, path, kind) = channel_ctx(&state, &cid).await?;
    if kind != "voice" {
        return Err(ApiErr::bad("not a voice channel"));
    }
    guard(&state, &user.sub, &gid, Some(&cid), CONNECT).await?;

    let call_id = state.calls.room(&path);
    let user_domain = user.sub.rsplit('@').next().unwrap_or_default().to_string();
    let token = state
        .calls
        .mint_token(&call_id, &user_domain)
        .ok_or_else(|| ApiErr::not_found("room unavailable"))?;

    Ok(Json(json!({
        "call_id": call_id,
        "manager_ws_url": format!("{}/calls/manager/ws", ws_base(&state)),
        "token": token,
        "channel_path": path,
    })))
}

async fn voice_roster(
    State(state): State<AppState>,
    user: Assertion,
    Path(cid): Path<String>,
) -> ApiResult<Json<Value>> {
    let (gid, path, _) = channel_ctx(&state, &cid).await?;
    guard(&state, &user.sub, &gid, Some(&cid), VIEW_CHANNEL).await?;
    let call_id = state.calls.room(&path);
    let roster = state.calls.roster(&call_id).unwrap_or_default();
    Ok(Json(json!(roster)))
}

fn ws_base(state: &AppState) -> String {
    let base = state.public_url.trim_end_matches('/');
    base.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1)
}
