//! `/api/channels/:cid/messages`.

use super::{channel_ctx, ApiErr, ApiResult};
use crate::auth::{guard, Assertion};
use crate::perms::{MANAGE_MESSAGES, SEND_MESSAGES, VIEW_CHANNEL};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use bscp_common::{now_ts, uuid};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/channels/:cid/messages", get(list).post(send))
        .route("/api/channels/:cid/messages/:mid", axum::routing::delete(delete_msg))
}

#[derive(Deserialize)]
struct ListQuery {
    since: Option<f64>,
    before: Option<f64>,
    #[serde(default = "default_limit")]
    limit: i64,
}
fn default_limit() -> i64 {
    50
}

async fn list(
    State(state): State<AppState>,
    user: Assertion,
    Path(cid): Path<String>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    let (gid, _path, _kind) = channel_ctx(&state, &cid).await?;
    guard(&state, &user.sub, &gid, Some(&cid), VIEW_CHANNEL).await?;

    let mut sql = String::from(
        "SELECT id, sender, text, timestamp, via_webhook FROM channel_messages \
         WHERE channel_id = ? AND deleted = 0",
    );
    if q.since.is_some() {
        sql.push_str(" AND timestamp > ?");
    }
    if q.before.is_some() {
        sql.push_str(" AND timestamp < ?");
    }
    sql.push_str(" ORDER BY timestamp DESC LIMIT ?");

    let mut query =
        sqlx::query_as::<_, (String, Option<String>, Option<String>, f64, Option<String>)>(&sql).bind(&cid);
    if let Some(s) = q.since {
        query = query.bind(s);
    }
    if let Some(b) = q.before {
        query = query.bind(b);
    }
    let mut rows = query.bind(q.limit.clamp(1, 200)).fetch_all(&state.pool).await?;
    rows.reverse();
    let out: Vec<Value> = rows
        .into_iter()
        .map(|(id, sender, text, ts, via)| {
            json!({ "id": id, "sender": sender, "text": text, "timestamp": ts, "via_webhook": via })
        })
        .collect();
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
struct SendBody {
    text: String,
}

async fn send(
    State(state): State<AppState>,
    user: Assertion,
    Path(cid): Path<String>,
    Json(b): Json<SendBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (gid, path, kind) = channel_ctx(&state, &cid).await?;
    if kind != "text" {
        return Err(ApiErr::bad("not a text channel"));
    }
    guard(&state, &user.sub, &gid, Some(&cid), SEND_MESSAGES).await?;
    let text = b.text.trim();
    if text.is_empty() {
        return Err(ApiErr::bad("empty message"));
    }

    let id = format!("{}/message/{}", state.domain, uuid());
    let ts = now_ts();
    sqlx::query(
        "INSERT INTO channel_messages (id, channel_path, channel_id, sender, text, timestamp) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&path)
    .bind(&cid)
    .bind(&user.sub)
    .bind(text)
    .bind(ts)
    .execute(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "sender": user.sub, "text": text, "timestamp": ts }))))
}

async fn delete_msg(
    State(state): State<AppState>,
    user: Assertion,
    Path((cid, mid)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let (gid, _, _) = channel_ctx(&state, &cid).await?;
    let sender: Option<String> =
        sqlx::query_scalar("SELECT sender FROM channel_messages WHERE id = ? AND channel_id = ?")
            .bind(&mid)
            .bind(&cid)
            .fetch_optional(&state.pool)
            .await?;
    let Some(sender) = sender else {
        return Err(ApiErr::not_found("message not found"));
    };
    if sender != user.sub {
        guard(&state, &user.sub, &gid, Some(&cid), MANAGE_MESSAGES).await?;
    }
    sqlx::query("UPDATE channel_messages SET deleted = 1 WHERE id = ?").bind(&mid).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}
