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
}

#[derive(Deserialize)]
struct IncomingMessage {
    id: String,
    sender: String,
    receiver: String,
    text: String,
    #[serde(rename = "validationKey")]
    validation_key: Option<String>,
}

async fn receive(State(state): State<AppState>, Json(data): Json<IncomingMessage>) -> impl IntoResponse {
    if !data.sender.contains('@') {
        return (StatusCode::BAD_REQUEST, "Invalid sender format").into_response();
    }
    let sender_domain = data.sender.rsplit('@').next().unwrap_or_default().to_string();
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

    let insert = sqlx::query(
        "INSERT INTO messages (id, sender, receiver, text, validation_key, timestamp, is_read) \
         VALUES (?, ?, ?, ?, ?, ?, 0)",
    )
    .bind(&data.id)
    .bind(&data.sender)
    .bind(&data.receiver)
    .bind(&data.text)
    .bind(&data.validation_key)
    .bind(now_ts())
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
            let (pool, vapid, disc) = (state.pool.clone(), state.vapid.clone(), state.discovery.clone());
            let (title, text) = (format!("New message from {}", data.sender), data.text.clone());
            tokio::spawn(async move {
                bscp_common::push::send_to_user(&pool, disc.client(), &vapid, &recipient.id, &title, &text, "/").await;
            });
        }
    }

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
