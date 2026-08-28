//! `/api/admin` — server & per-user configuration (admin only).

use crate::auth::AdminUser;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use bscp_common::models::{ServerConfig, User};
use bscp_common::{now_ts, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/config", get(get_config).patch(patch_config))
        .route("/api/admin/users/:username/storage", get(get_user_storage).patch(patch_user_storage))
}

async fn ensure_config(state: &AppState) -> Result<ServerConfig, ApiError> {
    if let Some(c) = sqlx::query_as::<_, ServerConfig>("SELECT * FROM server_config WHERE id = 1")
        .fetch_optional(&state.pool)
        .await?
    {
        return Ok(c);
    }
    sqlx::query("INSERT INTO server_config (id, storage_limit_mb, updated_at) VALUES (1, 500, ?)")
        .bind(now_ts())
        .execute(&state.pool)
        .await?;
    Ok(sqlx::query_as::<_, ServerConfig>("SELECT * FROM server_config WHERE id = 1")
        .fetch_one(&state.pool)
        .await?)
}

async fn get_config(State(state): State<AppState>, _admin: AdminUser) -> Result<Json<Value>, ApiError> {
    let c = ensure_config(&state).await?;
    Ok(Json(json!({ "storage_limit_mb": c.storage_limit_mb })))
}

#[derive(Deserialize)]
struct ConfigUpdate {
    storage_limit_mb: Option<i64>,
}

async fn patch_config(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<ConfigUpdate>,
) -> Result<Json<Value>, ApiError> {
    let mut c = ensure_config(&state).await?;
    if let Some(limit) = body.storage_limit_mb {
        if limit < 1 {
            return Err(ApiError::bad_request("Storage limit must be at least 1 MB"));
        }
        sqlx::query("UPDATE server_config SET storage_limit_mb = ?, updated_at = ? WHERE id = 1")
            .bind(limit)
            .bind(now_ts())
            .execute(&state.pool)
            .await?;
        c.storage_limit_mb = limit;
    }
    Ok(Json(json!({ "storage_limit_mb": c.storage_limit_mb })))
}

async fn find_user(state: &AppState, username: &str) -> Result<User, ApiError> {
    let local = username.split('@').next().unwrap_or(username);
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(local)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))
}

async fn get_user_storage(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(username): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let user = find_user(&state, &username).await?;
    Ok(Json(json!({
        "user_id": user.id,
        "username": user.username,
        "storage_limit_mb": user.storage_limit_mb,
    })))
}

#[derive(Deserialize)]
struct StorageUpdate {
    storage_limit_mb: Option<i64>,
}

async fn patch_user_storage(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(username): Path<String>,
    Json(body): Json<StorageUpdate>,
) -> Result<Json<Value>, ApiError> {
    let mut user = find_user(&state, &username).await?;
    if let Some(limit) = body.storage_limit_mb {
        if limit < 1 {
            return Err(ApiError::bad_request("Storage limit must be at least 1 MB"));
        }
        sqlx::query("UPDATE users SET storage_limit_mb = ? WHERE id = ?")
            .bind(limit)
            .bind(&user.id)
            .execute(&state.pool)
            .await?;
        user.storage_limit_mb = limit;
    }
    Ok(Json(json!({
        "user_id": user.id,
        "username": user.username,
        "storage_limit_mb": user.storage_limit_mb,
    })))
}
