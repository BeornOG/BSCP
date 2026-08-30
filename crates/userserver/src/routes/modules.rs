//! `/api/modules/*` — external account linking mediated by out-of-process modules.

use crate::auth::AuthUser;
use crate::modules::{hmac_hex, links};
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bscp_common::{now_ts, uuid, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/modules/providers", get(list_providers))
        .route("/api/modules/:module/link/:provider/start", get(link_start))
        .route("/api/modules/:module/links", post(link_callback))
        .route("/api/modules/:module/links/:provider", axum::routing::delete(unlink))
        .route("/api/users/me/links", get(my_links))
}

#[derive(sqlx::FromRow)]
struct LinkRow {
    module: String,
    provider: String,
    display_name: Option<String>,
    profile_url: Option<String>,
    avatar_url: Option<String>,
    created_at: f64,
}

fn link_json(r: &LinkRow) -> Value {
    json!({
        "module": r.module, "provider": r.provider,
        "display_name": r.display_name, "profile_url": r.profile_url,
        "avatar_url": r.avatar_url, "created_at": r.created_at,
    })
}

async fn user_links(state: &AppState, user_id: &str) -> Vec<LinkRow> {
    sqlx::query_as::<_, LinkRow>(
        "SELECT module, provider, display_name, profile_url, avatar_url, created_at \
         FROM account_links WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default()
}

async fn my_links(State(state): State<AppState>, auth: AuthUser) -> Json<Value> {
    let rows = user_links(&state, &auth.user.id).await;
    Json(json!(rows.iter().map(link_json).collect::<Vec<_>>()))
}

/// Every link provider offered by every enabled module, annotated with the
/// caller's current link.
async fn list_providers(State(state): State<AppState>, auth: AuthUser) -> Json<Value> {
    let links = user_links(&state, &auth.user.id).await;
    let mut out = Vec::new();
    for m in state.modules.enabled() {
        for p in &m.manifest.link_providers {
            let existing = links.iter().find(|l| l.module == m.name && l.provider == p.id);
            out.push(json!({
                "module": m.name,
                "id": p.id,
                "name": if p.name.is_empty() { p.id.clone() } else { p.name.clone() },
                "icon_url": p.icon_url,
                "linked": existing.is_some(),
                "link": existing.map(link_json),
            }));
        }
    }
    Json(json!(out))
}

async fn link_start(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((module, provider)): Path<(String, String)>,
) -> Result<Redirect, ApiError> {
    let m = state
        .modules
        .get(&module)
        .filter(|m| m.enabled)
        .ok_or_else(|| ApiError::not_found("unknown module"))?;
    if !m.manifest.link_providers.iter().any(|p| p.id == provider) {
        return Err(ApiError::not_found("unknown provider"));
    }

    let ticket = links::mint_ticket(&state, &auth.user.id, &module, &provider, now_ts() + 300.0);
    let callback = format!("{}/api/modules/{}/links", state.cfg.public_url, module);
    let mut url = reqwest::Url::parse(&format!("{}/link/{}", m.base_url.trim_end_matches('/'), provider))
        .map_err(|_| ApiError::internal("bad module url"))?;
    url.query_pairs_mut().append_pair("ticket", &ticket).append_pair("callback", &callback);
    Ok(Redirect::to(url.as_str()))
}

#[derive(Deserialize)]
struct LinkCallback {
    ticket: String,
    external_id: Option<String>,
    display_name: Option<String>,
    profile_url: Option<String>,
    avatar_url: Option<String>,
}

/// Called by the module (HMAC-signed) once the external OAuth completed.
async fn link_callback(
    State(state): State<AppState>,
    Path(module): Path<String>,
    headers: HeaderMap,
    raw: Bytes,
) -> Response {
    let Some(m) = state.modules.get(&module) else {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": "unknown module" }))).into_response();
    };
    let sig = headers.get("x-bscp-signature").and_then(|v| v.to_str().ok()).unwrap_or_default();
    let expected = format!("sha256={}", hmac_hex(&m.secret, &raw));
    if !constant_eq(sig, &expected) {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": "bad signature" }))).into_response();
    }
    let Ok(body) = serde_json::from_slice::<LinkCallback>(&raw) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid body" }))).into_response();
    };
    let Some((user_id, tmod, tprov)) = links::verify_ticket(&state, &body.ticket) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid or expired ticket" }))).into_response();
    };
    if tmod != module {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "ticket/module mismatch" }))).into_response();
    }

    let res = sqlx::query(
        "INSERT INTO account_links \
         (id, user_id, module, provider, external_id, display_name, profile_url, avatar_url, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(user_id, module, provider) DO UPDATE SET \
           external_id = excluded.external_id, display_name = excluded.display_name, \
           profile_url = excluded.profile_url, avatar_url = excluded.avatar_url",
    )
    .bind(uuid())
    .bind(&user_id)
    .bind(&tmod)
    .bind(&tprov)
    .bind(&body.external_id)
    .bind(&body.display_name)
    .bind(&body.profile_url)
    .bind(&body.avatar_url)
    .bind(now_ts())
    .execute(&state.pool)
    .await;

    match res {
        Ok(_) => Json(json!({ "ok": true, "redirect": format!("{}/settings", state.cfg.public_url) })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "account_links upsert failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "store failed" }))).into_response()
        }
    }
}

async fn unlink(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((module, provider)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    sqlx::query("DELETE FROM account_links WHERE user_id = ? AND module = ? AND provider = ?")
        .bind(&auth.user.id)
        .bind(&module)
        .bind(&provider)
        .execute(&state.pool)
        .await?;

    if let Some(m) = state.modules.get(&module) {
        let body = json!({ "user": state.full_id(&auth.user.username), "provider": provider }).to_string();
        let sig = format!("sha256={}", hmac_hex(&m.secret, body.as_bytes()));
        let url = format!("{}/links/removed", m.base_url.trim_end_matches('/'));
        let client = state.modules.client().clone();
        tokio::spawn(async move {
            let _ = client
                .post(&url)
                .header("x-bscp-signature", sig)
                .header("content-type", "application/json")
                .body(body)
                .send()
                .await;
        });
    }
    Ok(StatusCode::NO_CONTENT)
}

fn constant_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
