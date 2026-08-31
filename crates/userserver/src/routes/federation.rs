//! Federation protocol endpoints — port of `federation.py`.

use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use bscp_common::models::User;
use bscp_common::now_ts;
use serde::Deserialize;
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/federation/receive", post(receive))
        .route("/federation/validate", get(validate))
        .route("/federation/assert/verify", post(assert_verify))
}

#[derive(Deserialize)]
struct AssertVerifyReq {
    token: String,
}

/// Callback from a channel server: confirm we minted this assertion and it is
/// still valid. Returns `{ valid, name, picture }`.
async fn assert_verify(State(state): State<AppState>, Json(req): Json<AssertVerifyReq>) -> impl IntoResponse {
    let claims: bscp_common::assertion::AssertionClaims =
        match state.oidc.verify(&req.token, None) {
            Ok(c) => c,
            Err(_) => return Json(json!({ "valid": false })),
        };
    if claims.iss != state.cfg.public_url {
        return Json(json!({ "valid": false }));
    }
    if state.domain_blocked(&claims.aud).await {
        return Json(json!({ "valid": false }));
    }
    match crate::guilds::assert::verify_issued(&state, &claims.jti, &claims.sub, &claims.aud).await {
        Some((name, picture)) => Json(json!({ "valid": true, "name": name, "picture": picture })),
        None => Json(json!({ "valid": false })),
    }
}

#[derive(Deserialize)]
struct IncomingMessage {
    id: String,
    sender: String,
    receiver: String,
    text: String,
    #[serde(rename = "validationKey")]
    validation_key: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    metadata: Option<String>,
}

async fn receive(State(state): State<AppState>, Json(data): Json<IncomingMessage>) -> impl IntoResponse {
    if !data.sender.contains('@') {
        return (StatusCode::BAD_REQUEST, "Invalid sender format").into_response();
    }
    let sender_domain = data.sender.rsplit('@').next().unwrap_or_default().to_string();

    if crate::moderation::is_domain_blocked(&state.pool, &sender_domain).await {
        tracing::info!(domain = %sender_domain, "[FEDERATION] rejected message from blocked domain");
        return (StatusCode::FORBIDDEN, "Sender domain is blocked").into_response();
    }

    let val_key = data.validation_key.clone().unwrap_or_default();

    let valid = bscp_common::federation::validate_remote(
        &state.discovery,
        &sender_domain,
        &data.id,
        &val_key,
        &data.sender,
        &data.receiver,
    )
    .await;

    if !valid {
        return (StatusCode::UNAUTHORIZED, "Invalid").into_response();
    }

    let kind = data.kind.clone().unwrap_or_else(|| "text".to_string());
    let insert = sqlx::query(
        "INSERT INTO messages (id, sender, receiver, text, validation_key, timestamp, is_read, kind, metadata) \
         VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(&data.id)
    .bind(&data.sender)
    .bind(&data.receiver)
    .bind(&data.text)
    .bind(&data.validation_key)
    .bind(now_ts())
    .bind(&kind)
    .bind(&data.metadata)
    .execute(&state.pool)
    .await;

    if let Err(e) = insert {
        tracing::error!(error = %e, "[FEDERATION] failed to store message");
        return (StatusCode::UNAUTHORIZED, "Invalid").into_response();
    }

    // Notify local recipient.
    if let Some(local) = data.receiver.split('@').next() {
        if let Ok(Some(recipient)) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
            .bind(local)
            .fetch_optional(&state.pool)
            .await
        {
            let rid = recipient.id.clone();
            let (pool, vapid, disc) = (state.pool.clone(), state.vapid.clone(), state.discovery.clone());
            let title = if kind == "call_invite" {
                format!("Incoming call from {}", data.sender)
            } else {
                format!("New message from {}", data.sender)
            };
            let (text, push_rid) = (data.text.clone(), rid.clone());
            tokio::spawn(async move {
                bscp_common::push::send_to_user(&pool, disc.client(), &vapid, &push_rid, &title, &text, "/").await;
            });

            // Call signaling.
            if let Some(meta) = data.metadata.as_deref() {
                match kind.as_str() {
                    "call_invite" => crate::call::ws::on_call_invite(&state, &rid, &data.sender, meta),
                    "call_end" => crate::call::ws::on_call_end(&state, &rid, meta),
                    _ => {}
                }
            }
        }
    }

    crate::modules::dispatch(
        &state,
        "message.received",
        serde_json::json!({
            "id": data.id, "sender": data.sender, "receiver": data.receiver, "kind": kind, "text": data.text,
        }),
    );

    (StatusCode::OK, "OK").into_response()
}

#[derive(Deserialize)]
struct ValidateQuery {
    #[serde(rename = "messageId")]
    message_id: Option<String>,
    #[serde(rename = "validationKey")]
    validation_key: Option<String>,
    sender: Option<String>,
    receiver: Option<String>,
}

async fn validate(State(state): State<AppState>, Query(q): Query<ValidateQuery>) -> impl IntoResponse {
    let row = sqlx::query_as::<_, bscp_common::models::Message>("SELECT * FROM messages WHERE id = ?")
        .bind(q.message_id.unwrap_or_default())
        .fetch_optional(&state.pool)
        .await;

    match row {
        Ok(Some(msg)) => {
            let ok = msg.validation_key.as_deref() == q.validation_key.as_deref()
                && Some(msg.sender.as_str()) == q.sender.as_deref()
                && Some(msg.receiver.as_str()) == q.receiver.as_deref();
            Json(json!({ "valid": ok })).into_response()
        }
        Ok(None) => Json(json!({ "valid": false })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "validate error");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "valid": false, "error": e.to_string() }))).into_response()
        }
    }
}
