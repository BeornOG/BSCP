//! `/api/user/webhooks` — personal webhook management.

use crate::auth::AuthUser;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use bscp_common::models::Webhook;
use bscp_common::{now_ts, random_token, uuid, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/user/webhooks", get(list).post(create))
        .route("/api/user/webhooks/", get(list).post(create))
        .route("/api/user/webhooks/:id", axum::routing::delete(delete_webhook))
        .route("/api/user/webhooks/:id/regenerate", post(regenerate))
}

fn webhook_url(domain: &str, id: &str, token: &str) -> String {
    format!("http://{domain}/webhooks/{id}/{token}")
}

fn serialize(w: &Webhook, domain: &str) -> Value {
    json!({
        "id": w.id,
        "name": w.name,
        "url": webhook_url(domain, &w.id, &w.token),
        "profile_pic": w.profile_pic,
        "created_at": w.created_at,
        "last_used": w.last_used,
    })
}

async fn list(State(state): State<AppState>, auth: AuthUser) -> Result<Json<Value>, ApiError> {
    let hooks = sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE user_id = ?")
        .bind(&auth.user.id)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(json!(hooks.iter().map(|w| serialize(w, state.domain())).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
    avatar_url: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let id = uuid();
    let token = random_token(32);
    sqlx::query(
        "INSERT INTO webhooks (id, user_id, name, token, profile_pic, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&auth.user.id)
    .bind(&body.name)
    .bind(&token)
    .bind(&body.avatar_url)
    .bind(now_ts())
    .execute(&state.pool)
    .await?;

    let w = sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;
    Ok((StatusCode::CREATED, Json(serialize(&w, state.domain()))))
}

async fn delete_webhook(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let res = sqlx::query("DELETE FROM webhooks WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&auth.user.id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::not_found("Webhook not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn regenerate(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM webhooks WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&auth.user.id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(ApiError::not_found("Webhook not found"));
    }
    let token = random_token(32);
    sqlx::query("UPDATE webhooks SET token = ? WHERE id = ?").bind(&token).bind(&id).execute(&state.pool).await?;
    Ok(Json(json!({ "url": webhook_url(state.domain(), &id, &token) })))
}
