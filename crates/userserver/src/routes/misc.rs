//! Non-API routes: incoming webhooks, `.well-known`, media, static uploads, SPA.

use crate::media;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bscp_common::models::{User, Webhook};
use bscp_common::{now_ts, uuid};
use serde::Deserialize;
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/webhooks/:webhook_id/:webhook_token", post(receive_webhook))
        .route("/media/proxy", get(media::proxy))
        .route("/uploads/:filename", get(serve_upload))
        .route("/.well-known/BSCP/userserver", get(wellknown))
        .route("/.well-known/BSCP/userserver.json", get(wellknown))
}

// ── POST /webhooks/:id/:token ────────────────────────────────────────────

#[derive(Deserialize)]
struct WebhookPayload {
    content: Option<String>,
    #[allow(dead_code)]
    username: Option<String>,
    #[allow(dead_code)]
    avatar_url: Option<String>,
}

async fn receive_webhook(
    State(state): State<AppState>,
    Path((webhook_id, webhook_token)): Path<(String, String)>,
    Json(payload): Json<WebhookPayload>,
) -> Response {
    let Some(content) = payload.content.filter(|c| !c.is_empty()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "message": "content is required" }))).into_response();
    };

    let webhook = sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE id = ? AND token = ?")
        .bind(&webhook_id)
        .bind(&webhook_token)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();
    let Some(webhook) = webhook else {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": "Invalid webhook" }))).into_response();
    };

    let _ = sqlx::query("UPDATE webhooks SET last_used = ? WHERE id = ?")
        .bind(now_ts())
        .bind(&webhook.id)
        .execute(&state.pool)
        .await;

    let owner = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(&webhook.user_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();
    let Some(owner) = owner else {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": "Invalid webhook" }))).into_response();
    };

    let msg_uuid = uuid();
    let full_id = format!("{}/{}", state.domain(), msg_uuid);
    let val_key = format!("key-{}", &msg_uuid[..8]);
    let sender = format!("webhook-{}@{}", webhook.id, state.domain());
    let receiver = format!("{}@{}", owner.username, state.domain());

    let res = sqlx::query(
        "INSERT INTO messages (id, sender, receiver, text, validation_key, timestamp, is_read) \
         VALUES (?, ?, ?, ?, ?, ?, 0)",
    )
    .bind(&full_id)
    .bind(&sender)
    .bind(&receiver)
    .bind(&content)
    .bind(&val_key)
    .bind(now_ts())
    .execute(&state.pool)
    .await;

    if let Err(e) = res {
        tracing::error!(error = %e, "webhook message insert failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "insert failed" }))).into_response();
    }

    let (pool, vapid, disc) = (state.pool.clone(), state.vapid.clone(), state.discovery.clone());
    let (title, body) = (format!("Message from {}", webhook.name), content.clone());
    tokio::spawn(async move {
        bscp_common::push::send_to_user(&pool, disc.client(), &vapid, &owner.id, &title, &body, "/").await;
    });

    (StatusCode::CREATED, Json(json!({ "success": true, "message_id": full_id }))).into_response()
}

// ── GET /uploads/:filename ──────────────────────────────────────────────

async fn serve_upload(State(state): State<AppState>, Path(filename): Path<String>) -> Response {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = state.cfg.upload_dir.join(&filename);
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&filename).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref().to_string())], bytes).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

// ── GET /.well-known/BSCP/userserver ───────────────────────────────────

async fn wellknown(State(state): State<AppState>) -> impl IntoResponse {
    let base = state.cfg.public_url.clone();
    Json(json!({
        "server": { "name": "BSCP User Server", "version": "1.0", "type": "userserver" },
        "oidc": {
            "issuer": base,
            "configuration": format!("{base}/.well-known/openid-configuration"),
        },
        "api": {
            "base": base,
            "docs": "/api/docs/",
            "openapi": "/api/docs/openapi.json",
            "endpoints": {
                "chats": "/api/chats/",
                "messages": "/api/messages/",
                "send_message": "/api/messages/",
                "users_me": "/api/users/me",
                "users": "/api/users/",
                "invites": "/api/invites/",
                "upload": "/api/upload/",
                "auth_login": "/api/auth/login",
                "auth_register": "/api/auth/register",
                "auth_setup": "/api/auth/setup",
                "webhooks": "/api/user/webhooks",
                "federation_receive": "/federation/receive",
                "federation_validate": "/federation/validate",
                "media_proxy": "/media/proxy"
            }
        },
        "capabilities": {
            "federation": true,
            "channels": false,
            "direct_messaging": true,
            "media_upload": true,
            "webhooks": true,
            "oidc": true
        }
    }))
}

// ── SPA fallback ────────────────────────────────────────────────────────

pub async fn spa_fallback(State(state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path();
    if path.starts_with("/api/") || path.starts_with("/federation/") {
        return (StatusCode::NOT_FOUND, Json(json!({ "message": "Not Found" }))).into_response();
    }

    let rel = path.trim_start_matches('/');
    if !rel.is_empty() && !rel.contains("..") {
        let candidate = state.cfg.static_dir.join(rel);
        if candidate.is_file() {
            if let Ok(bytes) = tokio::fs::read(&candidate).await {
                let mime = mime_guess::from_path(&candidate).first_or_octet_stream();
                return ([(header::CONTENT_TYPE, mime.as_ref().to_string())], bytes).into_response();
            }
        }
    }

    match tokio::fs::read(state.cfg.static_dir.join("index.html")).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], bytes).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Frontend not built").into_response(),
    }
}
