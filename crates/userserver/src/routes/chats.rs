//! `/api/chats` — conversation list, message history, sending, deletion.

use crate::auth::AuthUser;
use crate::profile::get_profile;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use bscp_common::federation::{self, FedMessage};
use bscp_common::models::{Message, User, Webhook};
use bscp_common::{now_ts, uuid, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chats", get(chat_list))
        .route("/api/chats/", get(chat_list))
        .route("/api/chats/:target/messages", get(get_messages).post(send_message))
        .route("/api/chats/:target/messages/:message_id", axum::routing::delete(delete_message))
}

fn serialize_message(m: &Message) -> Value {
    json!({
        "id": m.id,
        "sender": m.sender,
        "receiver": m.receiver,
        "text": m.text,
        "timestamp": m.timestamp,
        "is_read": m.is_read,
    })
}

// ── GET /api/chats ───────────────────────────────────────────────────────

async fn chat_list(State(state): State<AppState>, auth: AuthUser) -> Result<Json<Value>, ApiError> {
    let me = state.full_id(&auth.user.username);
    let domain_suffix = format!("@{}", state.domain());

    let sent: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT receiver FROM messages WHERE sender = ?")
            .bind(&me)
            .fetch_all(&state.pool)
            .await?;
    let recv: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT sender FROM messages WHERE receiver = ?")
            .bind(&me)
            .fetch_all(&state.pool)
            .await?;

    let partners: BTreeSet<String> = sent.into_iter().chain(recv).collect();
    let mut chats = Vec::with_capacity(partners.len());

    for partner in partners {
        let mut display_name = partner.split('@').next().unwrap_or(&partner).to_string();
        let mut profile_pic: Option<String> = None;
        let mut status = "offline".to_string();

        let webhook_local = partner
            .strip_suffix(&domain_suffix)
            .and_then(|local| local.strip_prefix("webhook-"));

        if let Some(webhook_id) = webhook_local {
            let w = sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE id = ? AND user_id = ?")
                .bind(webhook_id)
                .bind(&auth.user.id)
                .fetch_optional(&state.pool)
                .await?;
            if let Some(w) = w {
                display_name = w.name;
                profile_pic = w.profile_pic;
            }
        } else if let Ok(Some(profile)) = get_profile(&state, &partner).await {
            if let Some(v) = profile.get("display_name").and_then(|v| v.as_str()) {
                display_name = v.to_string();
            }
            profile_pic = profile.get("profile_pic").and_then(|v| v.as_str()).map(String::from);
            if let Some(v) = profile.get("status").and_then(|v| v.as_str()) {
                status = v.to_string();
            }
        }

        let unread: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM messages WHERE sender = ? AND receiver = ? AND is_read = 0",
        )
        .bind(&partner)
        .bind(&me)
        .fetch_one(&state.pool)
        .await?;

        let last: Option<(String, String)> = sqlx::query_as(
            "SELECT text, sender FROM messages \
             WHERE (sender = ? AND receiver = ?) OR (sender = ? AND receiver = ?) \
             ORDER BY timestamp DESC LIMIT 1",
        )
        .bind(&partner)
        .bind(&me)
        .bind(&me)
        .bind(&partner)
        .fetch_optional(&state.pool)
        .await?;

        chats.push(json!({
            "id": partner,
            "display_name": display_name,
            "profile_pic": profile_pic,
            "status": status,
            "unread_count": unread,
            "last_message_text": last.as_ref().map(|(t, _)| t.clone()),
            "last_message_sender": last.as_ref().map(|(_, s)| s.clone()),
        }));
    }

    Ok(Json(json!(chats)))
}

// ── GET /api/chats/:target/messages ─────────────────────────────────────

#[derive(Deserialize)]
struct MessagesQuery {
    since: Option<f64>,
    before: Option<f64>,
    #[serde(default = "default_limit")]
    limit: i64,
}
fn default_limit() -> i64 {
    50
}

async fn get_messages(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(target): Path<String>,
    Query(q): Query<MessagesQuery>,
) -> Result<Json<Value>, ApiError> {
    let me = state.full_id(&auth.user.username);

    if target.contains('#') {
        let target_domain = target.split('#').next().unwrap_or_default();
        let data = federation::poll_channel(&state.discovery, target_domain, &target, q.limit, q.since, q.before).await;
        return Ok(Json(data));
    }

    let target_full = if target.contains('@') { target.clone() } else { format!("{target}@{}", state.domain()) };

    let mut sql = String::from(
        "SELECT * FROM messages WHERE ((sender = ? AND receiver = ?) OR (sender = ? AND receiver = ?))",
    );
    if q.since.is_some() {
        sql.push_str(" AND timestamp > ?");
    }
    if q.before.is_some() {
        sql.push_str(" AND timestamp < ?");
    }
    sql.push_str(" ORDER BY timestamp DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, Message>(&sql)
        .bind(&me)
        .bind(&target_full)
        .bind(&target_full)
        .bind(&me);
    if let Some(s) = q.since {
        query = query.bind(s);
    }
    if let Some(b) = q.before {
        query = query.bind(b);
    }
    query = query.bind(q.limit.clamp(1, 500));
    let mut msgs = query.fetch_all(&state.pool).await?;

    if target_full != me {
        sqlx::query("UPDATE messages SET is_read = 1 WHERE sender = ? AND receiver = ? AND is_read = 0")
            .bind(&target_full)
            .bind(&me)
            .execute(&state.pool)
            .await?;
    }

    msgs.reverse();
    Ok(Json(json!(msgs.iter().map(serialize_message).collect::<Vec<_>>())))
}

// ── POST /api/chats/:target/messages ───────────────────────────────────

#[derive(Deserialize)]
struct SendBody {
    text: String,
}

async fn send_message(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(target): Path<String>,
    Json(body): Json<SendBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sender = state.full_id(&auth.user.username);
    let receiver = if target.contains('@') { target.clone() } else { format!("{target}@{}", state.domain()) };

    let recv_username = receiver.split('@').next().unwrap_or_default();
    if recv_username.starts_with("webhook-") {
        return Err(ApiError::forbidden("Cannot send messages to webhooks"));
    }

    let is_channel = target.contains('#');
    if !is_channel {
        match get_profile(&state, &receiver).await? {
            Some(_) => {}
            None => return Err(ApiError::not_found("User not found")),
        }
    }

    let msg_uuid = uuid();
    let full_id = format!("{}/{}", state.domain(), msg_uuid);
    let val_key = format!("key-{}", &msg_uuid[..8]);
    let ts = now_ts();

    sqlx::query(
        "INSERT INTO messages (id, sender, receiver, text, validation_key, timestamp, is_read) \
         VALUES (?, ?, ?, ?, ?, ?, 0)",
    )
    .bind(&full_id)
    .bind(&sender)
    .bind(&receiver)
    .bind(&body.text)
    .bind(&val_key)
    .bind(ts)
    .execute(&state.pool)
    .await?;

    // Local push (background).
    if let Some(local) = receiver.strip_suffix(&format!("@{}", state.domain())) {
        if let Some(recipient) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
            .bind(local)
            .fetch_optional(&state.pool)
            .await?
        {
            if recipient.id != auth.user.id {
                let (pool, vapid, disc) =
                    (state.pool.clone(), state.vapid.clone(), state.discovery.clone());
                let (title, text) = (format!("New message from {}", auth.user.username), body.text.clone());
                tokio::spawn(async move {
                    bscp_common::push::send_to_user(&pool, disc.client(), &vapid, &recipient.id, &title, &text, "/").await;
                });
            }
        }
    }

    // Federate (background, best-effort).
    let payload = FedMessage {
        id: full_id.clone(),
        sender: sender.clone(),
        receiver: receiver.clone(),
        text: body.text.clone(),
        validation_key: val_key.clone(),
    };
    let disc = state.discovery.clone();
    let target_clone = target.clone();
    let receiver_clone = receiver.clone();
    tokio::spawn(async move {
        if let Some(dom) = target_clone.contains('#').then(|| target_clone.split('#').next().unwrap_or_default().to_string()) {
            federation::deliver_channel(&disc, &dom, &payload).await;
        } else if let Some(dom) = receiver_clone.rsplit('@').next() {
            federation::deliver_dm(&disc, dom, &payload).await;
        }
    });

    let msg = Message {
        id: full_id,
        sender,
        receiver,
        text: body.text,
        validation_key: Some(val_key),
        timestamp: ts,
        is_read: false,
    };
    Ok((StatusCode::CREATED, Json(serialize_message(&msg))))
}

// ── DELETE /api/chats/:target/messages/:message_id ─────────────────────

async fn delete_message(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((_target, message_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let me = state.full_id(&auth.user.username);
    let msg = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = ?")
        .bind(&message_id)
        .fetch_optional(&state.pool)
        .await?;
    let Some(msg) = msg else {
        return Err(ApiError::not_found("Message not found"));
    };
    if msg.sender != me {
        return Err(ApiError::forbidden("Cannot delete other user's messages"));
    }
    sqlx::query("DELETE FROM messages WHERE id = ?").bind(&message_id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}
