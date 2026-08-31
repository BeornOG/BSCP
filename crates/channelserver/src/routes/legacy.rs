use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bscp_common::{now_ts, random_token, uuid};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::FromRow;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/channel/send", post(channel_send))
        .route("/api/channel/poll", get(channel_poll))
        .route("/api/channel/webhooks", get(list_webhooks).post(create_webhook))
        .route("/api/channel/webhooks/:id", delete(delete_webhook))
        .route("/api/channel/webhooks/:id/regenerate", post(regenerate_webhook))
        .route("/webhooks/:id/:token", post(receive_webhook))
        .route("/.well-known/BSCP/channelserver", get(wellknown))
        .route("/.well-known/BSCP/channelserver.json", get(wellknown))

}

#[derive(FromRow)]
struct ChannelMessage {
    id: String,
    sender: Option<String>,
    text: Option<String>,
    timestamp: f64,
}

#[derive(FromRow)]
struct ChannelWebhook {
    id: String,
    channel_path: String,
    name: String,
    token: String,
    profile_pic: Option<String>,
    created_at: f64,
    last_used: Option<f64>,
}

fn webhook_url(domain: &str, id: &str, token: &str) -> String {
    format!("http://{domain}/webhooks/{id}/{token}")
}

// ── POST /api/channel/send ────────────────────────────────────────────────

#[derive(Deserialize)]
struct IncomingMessage {
    id: String,
    sender: String,
    receiver: String,
    text: String,
    #[serde(rename = "validationKey")]
    validation_key: Option<String>,
}

async fn channel_send(State(st): State<AppState>, Json(data): Json<IncomingMessage>) -> impl IntoResponse {
    let Some(sender_domain) = data.sender.rsplit('@').next() else {
        return (StatusCode::UNAUTHORIZED, "Invalid").into_response();
    };
    let val_key = data.validation_key.clone().unwrap_or_default();
    let valid = bscp_common::federation::validate_remote(
        &st.discovery,
        sender_domain,
        &data.id,
        &val_key,
        &data.sender,
        &data.receiver,
    )
    .await;
    if !valid {
        return (StatusCode::UNAUTHORIZED, "Invalid").into_response();
    }

    let full_id = format!("{}/message/{}", st.domain, data.id);
    let res = sqlx::query(
        "INSERT INTO channel_messages (id, channel_path, sender, text, timestamp) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&full_id)
    .bind(&data.receiver)
    .bind(&data.sender)
    .bind(&data.text)
    .bind(now_ts())
    .execute(&st.pool)
    .await;

    match res {
        Ok(_) => Json(json!({ "status": "stored", "id": full_id })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "channel_send insert failed");
            (StatusCode::UNAUTHORIZED, "Invalid").into_response()
        }
    }
}

// ── GET /api/channel/poll ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct PollQuery {
    path: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
    since: Option<f64>,
    before: Option<f64>,
}
fn default_limit() -> i64 {
    50
}

async fn channel_poll(State(st): State<AppState>, Query(q): Query<PollQuery>) -> impl IntoResponse {
    let path = q.path.unwrap_or_default();
    let mut sql = String::from("SELECT id, sender, text, timestamp FROM channel_messages WHERE channel_path = ?");
    if q.since.is_some() {
        sql.push_str(" AND timestamp > ?");
    }
    if q.before.is_some() {
        sql.push_str(" AND timestamp < ?");
    }
    sql.push_str(" ORDER BY timestamp DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, ChannelMessage>(&sql).bind(path);
    if let Some(s) = q.since {
        query = query.bind(s);
    }
    if let Some(b) = q.before {
        query = query.bind(b);
    }
    query = query.bind(q.limit.clamp(1, 500));

    match query.fetch_all(&st.pool).await {
        Ok(mut rows) => {
            rows.reverse();
            let out: Vec<Value> = rows
                .into_iter()
                .map(|m| json!({ "id": m.id, "sender": m.sender, "text": m.text, "time": m.timestamp }))
                .collect();
            Json(out).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "channel_poll failed");
            Json(json!([])).into_response()
        }
    }
}

// ── channel webhook management ────────────────────────────────────────────

#[derive(Deserialize)]
struct WebhookPathQuery {
    path: Option<String>,
}

fn serialize_webhook(w: &ChannelWebhook, domain: &str) -> Value {
    json!({
        "id": w.id,
        "name": w.name,
        "url": webhook_url(domain, &w.id, &w.token),
        "profile_pic": w.profile_pic,
        "created_at": w.created_at,
        "last_used": w.last_used,
    })
}

async fn list_webhooks(State(st): State<AppState>, Query(q): Query<WebhookPathQuery>) -> impl IntoResponse {
    let Some(path) = q.path else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Missing channel path" }))).into_response();
    };
    match sqlx::query_as::<_, ChannelWebhook>("SELECT * FROM channel_webhooks WHERE channel_path = ?")
        .bind(path)
        .fetch_all(&st.pool)
        .await
    {
        Ok(rows) => {
            let out: Vec<Value> = rows.iter().map(|w| serialize_webhook(w, &st.domain)).collect();
            Json(out).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "list_webhooks failed");
            Json(json!([])).into_response()
        }
    }
}

#[derive(Deserialize)]
struct CreateWebhookBody {
    path: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
}

async fn create_webhook(State(st): State<AppState>, Json(body): Json<CreateWebhookBody>) -> impl IntoResponse {
    let (Some(path), Some(name)) = (body.path, body.name) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Missing channel path or name" }))).into_response();
    };
    let id = uuid();
    let token = random_token(32);
    let created = now_ts();
    let res = sqlx::query(
        "INSERT INTO channel_webhooks (id, channel_path, name, token, profile_pic, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&path)
    .bind(&name)
    .bind(&token)
    .bind(&body.avatar_url)
    .bind(created)
    .execute(&st.pool)
    .await;
    match res {
        Ok(_) => (
            StatusCode::CREATED,
            Json(json!({
                "id": id,
                "name": name,
                "url": webhook_url(&st.domain, &id, &token),
                "profile_pic": body.avatar_url,
                "created_at": created,
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "create_webhook failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "insert failed" }))).into_response()
        }
    }
}

async fn delete_webhook(State(st): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let r = sqlx::query("DELETE FROM channel_webhooks WHERE id = ?").bind(&id).execute(&st.pool).await;
    match r {
        Ok(res) if res.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({ "error": "Webhook not found" }))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "delete_webhook failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "delete failed" }))).into_response()
        }
    }
}

async fn regenerate_webhook(State(st): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let token = random_token(32);
    let r = sqlx::query("UPDATE channel_webhooks SET token = ? WHERE id = ?")
        .bind(&token)
        .bind(&id)
        .execute(&st.pool)
        .await;
    match r {
        Ok(res) if res.rows_affected() > 0 => {
            Json(json!({ "url": webhook_url(&st.domain, &id, &token) })).into_response()
        }
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({ "error": "Webhook not found" }))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "regenerate_webhook failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "update failed" }))).into_response()
        }
    }
}

// ── POST /webhooks/:id/:token ────────────────────────────────────────────

#[derive(Deserialize)]
struct WebhookPayload {
    content: Option<String>,
}

async fn receive_webhook(
    State(st): State<AppState>,
    Path((id, token)): Path<(String, String)>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    let Some(content) = payload.content else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Missing content" }))).into_response();
    };

    let webhook = sqlx::query_as::<_, ChannelWebhook>("SELECT * FROM channel_webhooks WHERE id = ? AND token = ?")
        .bind(&id)
        .bind(&token)
        .fetch_optional(&st.pool)
        .await
        .ok()
        .flatten();
    let Some(webhook) = webhook else {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": "Invalid webhook" }))).into_response();
    };

    let _ = sqlx::query("UPDATE channel_webhooks SET last_used = ? WHERE id = ?")
        .bind(now_ts())
        .bind(&webhook.id)
        .execute(&st.pool)
        .await;

    let full_id = format!("{}/message/{}", st.domain, uuid());
    let sender = format!("webhook-{}@{}", webhook.id, st.domain);
    let res = sqlx::query(
        "INSERT INTO channel_messages (id, channel_path, sender, text, timestamp) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&full_id)
    .bind(&webhook.channel_path)
    .bind(&sender)
    .bind(&content)
    .bind(now_ts())
    .execute(&st.pool)
    .await;

    match res {
        Ok(_) => (StatusCode::CREATED, Json(json!({ "success": true, "message_id": full_id }))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "receive_webhook insert failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "insert failed" }))).into_response()
        }
    }
}

// ── GET /.well-known/BSCP/channelserver ──────────────────────────────────

async fn wellknown(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "server": { "name": "BSCP Channel Server", "version": "1.0", "type": "channelserver" },
        "api": {
            "base": st.public_url.clone(),
            "endpoints": {
                "channel_send": "/api/channel/send",
                "channel_poll": "/api/channel/poll",
                "channel_webhooks": "/api/channel/webhooks",
                "guilds": "/api/guilds",
                "guilds_mine": "/api/guilds/mine",
                "invite_accept": "/api/invites/{code}/accept",
                "voice_token": "/api/channels/{id}/voice-token",
                "calls_manager_ws": "/calls/manager/ws"
            }
        },
        "capabilities": {
            "federation": true,
            "channels": true,
            "guilds": true,
            "direct_messaging": false,
            "media_upload": false,
            "webhooks": true
        }
    }))
}
