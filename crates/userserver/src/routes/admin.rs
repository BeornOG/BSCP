//! `/api/admin` — server & per-user configuration (admin only).

use crate::auth::AdminUser;
use crate::modules::registry;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use bscp_common::models::{ServerConfig, User};
use bscp_common::{now_ts, random_token, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/config", get(get_config).patch(patch_config))
        .route("/api/admin/users/:username/storage", get(get_user_storage).patch(patch_user_storage))
        .route("/api/admin/oauth/config", get(get_oauth_config).patch(patch_oauth_config))
        .route("/api/admin/oauth/clients", get(list_oauth_clients))
        .route("/api/admin/oauth/clients/:client_id", axum::routing::delete(revoke_oauth_client))
        .route("/api/admin/modules", get(list_modules).post(add_module))
        .route("/api/admin/modules/:name", axum::routing::delete(remove_module).patch(patch_module))
}

// ── OIDC clients / config ────────────────────────────────────────────

async fn get_oauth_config(State(state): State<AppState>, _a: AdminUser) -> Result<Json<Value>, ApiError> {
    let enabled: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT oidc_enabled FROM server_config WHERE id = 1), 1)",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({ "oidc_enabled": enabled != 0 })))
}

#[derive(Deserialize)]
struct OauthConfig {
    oidc_enabled: Option<bool>,
}

async fn patch_oauth_config(
    State(state): State<AppState>,
    _a: AdminUser,
    Json(body): Json<OauthConfig>,
) -> Result<Json<Value>, ApiError> {
    ensure_config(&state).await?;
    if let Some(en) = body.oidc_enabled {
        sqlx::query("UPDATE server_config SET oidc_enabled = ?, updated_at = ? WHERE id = 1")
            .bind(en as i64)
            .bind(now_ts())
            .execute(&state.pool)
            .await?;
    }
    get_oauth_config(State(state), _a).await
}

async fn list_oauth_clients(State(state): State<AppState>, _a: AdminUser) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, String, String, f64, i64)>(
        "SELECT client_id, name, redirect_uris, token_endpoint_auth_method, created_at, disabled \
         FROM oauth_clients ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, uris, method, created, disabled)| {
            json!({
                "client_id": id,
                "name": name,
                "redirect_uris": serde_json::from_str::<Value>(&uris).unwrap_or(json!([])),
                "token_endpoint_auth_method": method,
                "created_at": created,
                "disabled": disabled != 0,
            })
        })
        .collect();
    Ok(Json(json!(out)))
}

async fn revoke_oauth_client(
    State(state): State<AppState>,
    _a: AdminUser,
    Path(client_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    for sql in [
        "DELETE FROM oauth_tokens WHERE client_id = ?",
        "DELETE FROM oauth_codes WHERE client_id = ?",
        "DELETE FROM oauth_consents WHERE client_id = ?",
        "DELETE FROM oauth_clients WHERE client_id = ?",
    ] {
        sqlx::query(sql).bind(&client_id).execute(&state.pool).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

// ── modules ──────────────────────────────────────────────────────────

async fn list_modules(State(state): State<AppState>, _a: AdminUser) -> Json<Value> {
    let out: Vec<Value> = state
        .modules
        .enabled_and_disabled()
        .into_iter()
        .map(|m| {
            json!({
                "name": m.name,
                "base_url": m.base_url,
                "enabled": m.enabled,
                "manifest": {
                    "description": m.manifest.description,
                    "version": m.manifest.version,
                    "events": m.manifest.events,
                    "link_providers": m.manifest.link_providers,
                    "admin_url": m.manifest.admin_url,
                },
            })
        })
        .collect();
    Json(json!(out))
}

#[derive(Deserialize)]
struct AddModule {
    base_url: String,
}

async fn add_module(
    State(state): State<AppState>,
    _a: AdminUser,
    Json(body): Json<AddModule>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let base_url = body.base_url.trim().trim_end_matches('/').to_string();
    if reqwest::Url::parse(&base_url).is_err() {
        return Err(ApiError::bad_request("base_url must be a valid URL"));
    }
    let manifest = registry::fetch_manifest(&state, &base_url).await?;
    let name = if manifest.name.is_empty() {
        return Err(ApiError::bad_request("module manifest has no name"));
    } else {
        manifest.name.clone()
    };
    let secret = random_token(32);
    sqlx::query(
        "INSERT INTO modules (name, base_url, secret, manifest, created_at) VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(name) DO UPDATE SET base_url = excluded.base_url, secret = excluded.secret, \
         manifest = excluded.manifest",
    )
    .bind(&name)
    .bind(&base_url)
    .bind(&secret)
    .bind(serde_json::to_string(&manifest_value(&manifest)).unwrap())
    .bind(now_ts())
    .execute(&state.pool)
    .await?;
    state.modules.reload(&state.pool).await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "name": name,
            "base_url": base_url,
            "secret": secret,
            "events": manifest.events,
            "link_providers": manifest.link_providers,
        })),
    ))
}

fn manifest_value(m: &crate::modules::ModuleManifest) -> Value {
    json!({
        "name": m.name, "version": m.version, "description": m.description,
        "events": m.events,
        "link_providers": m.link_providers,
        "admin_url": m.admin_url,
    })
}

async fn remove_module(
    State(state): State<AppState>,
    _a: AdminUser,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    sqlx::query("DELETE FROM modules WHERE name = ?").bind(&name).execute(&state.pool).await?;
    state.modules.reload(&state.pool).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ModulePatch {
    enabled: Option<bool>,
}

async fn patch_module(
    State(state): State<AppState>,
    _a: AdminUser,
    Path(name): Path<String>,
    Json(body): Json<ModulePatch>,
) -> Result<StatusCode, ApiError> {
    if let Some(en) = body.enabled {
        sqlx::query("UPDATE modules SET enabled = ? WHERE name = ?")
            .bind(en as i64)
            .bind(&name)
            .execute(&state.pool)
            .await?;
        state.modules.reload(&state.pool).await;
    }
    Ok(StatusCode::NO_CONTENT)
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
