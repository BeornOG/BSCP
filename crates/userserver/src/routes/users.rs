//! `/api/users` — profiles, settings, push subscriptions, 2FA.

use crate::auth::{remove_cookie, set_cookie, AdminUser, AuthUser, PENDING_2FA_COOKIE};
use crate::profile::{get_profile, serialize_profile_db};
use crate::state::AppState;
use crate::util::{secure_filename, take_file_field};
use axum::extract::{Multipart, Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::PrivateCookieJar;
use bscp_common::models::{User, Webhook};
use bscp_common::{now_ts, uuid, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/users", get(list_users))
        .route("/api/users/", get(list_users))
        .route("/api/users/me", get(get_me).patch(patch_me))
        .route("/api/users/me/2fa", get(get_me))
        .route("/api/users/me/2fa/setup", post(twofa_setup))
        .route("/api/users/me/2fa/enable", post(twofa_enable))
        .route("/api/users/me/2fa/disable", post(twofa_disable))
        .route("/api/users/me/activity", post(activity))
        .route("/api/users/me/picture", post(upload_picture).delete(delete_picture))
        .route("/api/users/me/push/subscribe", post(push_subscribe).delete(push_unsubscribe))
        .route("/api/users/push/vapid_public_key", get(vapid_public_key))
        .route("/api/users/:full_id", get(get_user).delete(delete_user))
}

async fn get_me(State(state): State<AppState>, auth: AuthUser) -> Result<Json<Value>, ApiError> {
    Ok(Json(serialize_profile_db(&state, &auth.user).await))
}

#[derive(Deserialize)]
struct SettingsUpdate {
    display_name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    bio: Option<Option<String>>,
}

/// Distinguish "key absent" from "key present and null".
fn double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(de).map(Some)
}

async fn patch_me(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<SettingsUpdate>,
) -> Result<Json<Value>, ApiError> {
    if let Some(dn) = body.display_name {
        sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
            .bind(dn)
            .bind(&auth.user.id)
            .execute(&state.pool)
            .await?;
    }
    if let Some(bio) = body.bio {
        sqlx::query("UPDATE users SET bio = ? WHERE id = ?")
            .bind(bio)
            .bind(&auth.user.id)
            .execute(&state.pool)
            .await?;
    }
    let user = reload(&state, &auth.user.id).await?;
    Ok(Json(serialize_profile_db(&state, &user).await))
}

async fn reload(state: &AppState, id: &str) -> Result<User, ApiError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(Into::into)
}

async fn vapid_public_key(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "publicKey": state.vapid.public_key }))
}

// ── push subscriptions ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct PushKeys {
    p256dh: Option<String>,
    auth: Option<String>,
}
#[derive(Deserialize)]
struct PushSub {
    endpoint: Option<String>,
    keys: Option<PushKeys>,
}

async fn push_subscribe(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<PushSub>,
) -> Result<Json<Value>, ApiError> {
    let endpoint = body.endpoint.unwrap_or_default();
    let keys = body.keys.unwrap_or(PushKeys { p256dh: None, auth: None });
    let (Some(p256dh), Some(auth_key)) = (keys.p256dh, keys.auth) else {
        return Err(ApiError::bad_request("Invalid push subscription payload"));
    };
    if endpoint.is_empty() {
        return Err(ApiError::bad_request("Invalid push subscription payload"));
    }

    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM push_subscriptions WHERE endpoint = ?")
            .bind(&endpoint)
            .fetch_optional(&state.pool)
            .await?;

    if let Some(id) = existing {
        sqlx::query(
            "UPDATE push_subscriptions SET user_id = ?, p256dh = ?, auth = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&auth.user.id)
        .bind(&p256dh)
        .bind(&auth_key)
        .bind(now_ts())
        .bind(id)
        .execute(&state.pool)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO push_subscriptions (id, user_id, endpoint, p256dh, auth, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid())
        .bind(&auth.user.id)
        .bind(&endpoint)
        .bind(&p256dh)
        .bind(&auth_key)
        .bind(now_ts())
        .bind(now_ts())
        .execute(&state.pool)
        .await?;
    }
    Ok(Json(json!({ "success": true })))
}

#[derive(Deserialize)]
struct EndpointQuery {
    endpoint: Option<String>,
}

async fn push_unsubscribe(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<EndpointQuery>,
) -> Result<Json<Value>, ApiError> {
    let deleted = match q.endpoint {
        Some(ep) => {
            sqlx::query("DELETE FROM push_subscriptions WHERE user_id = ? AND endpoint = ?")
                .bind(&auth.user.id)
                .bind(ep)
                .execute(&state.pool)
                .await?
        }
        None => {
            sqlx::query("DELETE FROM push_subscriptions WHERE user_id = ?")
                .bind(&auth.user.id)
                .execute(&state.pool)
                .await?
        }
    };
    Ok(Json(json!({ "deleted": deleted.rows_affected() })))
}

// ── activity ping ────────────────────────────────────────────────────────

async fn activity(State(state): State<AppState>, auth: AuthUser) -> Json<Value> {
    let _ = sqlx::query("UPDATE user_sessions SET last_active = ? WHERE token = ? AND expires_at > ?")
        .bind(now_ts())
        .bind(&auth.token)
        .bind(now_ts())
        .execute(&state.pool)
        .await;
    if auth.user.status_type != 2 && auth.user.status_type != 3 {
        let _ = sqlx::query("UPDATE users SET status_type = 0 WHERE id = ?")
            .bind(&auth.user.id)
            .execute(&state.pool)
            .await;
    }
    Json(json!({ "success": true }))
}

// ── profile picture ──────────────────────────────────────────────────────

const ALLOWED_IMAGE: &[&str] = &[
    "image/png", "image/jpeg", "image/jpg", "image/gif", "image/webp", "image/svg+xml",
];

async fn upload_picture(
    State(state): State<AppState>,
    auth: AuthUser,
    multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    let (orig, mime, data) = take_file_field(multipart, "No file provided").await?;
    if orig.is_empty() {
        return Err(ApiError::bad_request("Invalid file"));
    }
    if !ALLOWED_IMAGE.contains(&mime.as_str()) {
        return Err(ApiError::bad_request("Unsupported file type"));
    }

    let filename = secure_filename(&format!("{}_{}", uuid(), orig));
    let path = state.cfg.upload_dir.join(&filename);
    tokio::fs::write(&path, &data).await.map_err(|e| ApiError::internal(format!("write failed: {e}")))?;

    let direct = format!("http://{}/uploads/{}", state.domain(), filename);
    let pic_url = format!("http://{}/media/proxy?url={}", state.domain(), direct);
    sqlx::query("UPDATE users SET profile_pic = ? WHERE id = ?")
        .bind(&pic_url)
        .bind(&auth.user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "profile_pic": pic_url })))
}

async fn delete_picture(State(state): State<AppState>, auth: AuthUser) -> Result<Json<Value>, ApiError> {
    sqlx::query("UPDATE users SET profile_pic = NULL WHERE id = ?")
        .bind(&auth.user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "profile_pic": null })))
}

// ── GET/DELETE /api/users/:full_id ──────────────────────────────────────

async fn get_user(State(state): State<AppState>, Path(full_id): Path<String>) -> Result<Json<Value>, ApiError> {
    let Some((username, domain)) = full_id.rsplit_once('@') else {
        return Err(ApiError::bad_request("Invalid format. Use username@domain"));
    };

    if domain == state.domain() {
        if let Some(webhook_id) = username.strip_prefix("webhook-") {
            if let Some(w) = sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE id = ?")
                .bind(webhook_id)
                .fetch_optional(&state.pool)
                .await?
            {
                return Ok(Json(json!({
                    "username": full_id,
                    "display_name": w.name,
                    "profile_pic": w.profile_pic,
                    "status": "offline",
                    "is_admin": false,
                })));
            }
        }
    }

    match get_profile(&state, &full_id).await? {
        Some(p) => Ok(Json(p)),
        None => Err(ApiError::not_found("User not found")),
    }
}

async fn delete_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(full_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let username = match full_id.rsplit_once('@') {
        Some((u, domain)) => {
            if domain != state.domain() {
                return Err(ApiError::bad_request("Cannot delete users on remote servers"));
            }
            u.to_string()
        }
        None => full_id.clone(),
    };

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&username)
        .fetch_optional(&state.pool)
        .await?;
    let Some(user) = user else {
        return Err(ApiError::not_found("User not found"));
    };
    if user.is_admin {
        return Err(ApiError::bad_request("Cannot delete admin user"));
    }

    sqlx::query("UPDATE users SET is_deleted = 1 WHERE id = ?").bind(&user.id).execute(&state.pool).await?;
    sqlx::query("DELETE FROM user_sessions WHERE user_id = ?").bind(&user.id).execute(&state.pool).await?;
    crate::modules::dispatch(&state, "user.deleted", json!({ "user": state.full_id(&user.username) }));
    Ok(Json(json!({ "message": format!("User {} has been deactivated.", user.username) })))
}

async fn list_users(State(state): State<AppState>, _admin: AdminUser) -> Result<Json<Value>, ApiError> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users").fetch_all(&state.pool).await?;
    let mut out = Vec::with_capacity(users.len());
    for u in &users {
        out.push(serialize_profile_db(&state, u).await);
    }
    Ok(Json(json!(out)))
}

// ── 2FA ──────────────────────────────────────────────────────────────────

async fn twofa_setup(
    auth: AuthUser,
    jar: PrivateCookieJar,
) -> Result<(PrivateCookieJar, Json<Value>), ApiError> {
    let secret = bscp_common::totp::random_base32();
    let uri = bscp_common::totp::provisioning_uri(&secret, &auth.user.username, "BSCP");
    let qr = bscp_common::totp::qr_png_base64(&uri).unwrap_or_default();
    let jar = set_cookie(jar, PENDING_2FA_COOKIE, secret.clone());
    Ok((jar, Json(json!({ "secret": secret, "qr_code": qr, "provisioning_uri": uri }))))
}

#[derive(Deserialize)]
struct OtpBody {
    otp: String,
}

async fn twofa_enable(
    State(state): State<AppState>,
    auth: AuthUser,
    jar: PrivateCookieJar,
    Json(body): Json<OtpBody>,
) -> Result<(PrivateCookieJar, Json<Value>), ApiError> {
    if auth.user.is_2fa_enabled {
        return Err(ApiError::bad_request("2FA is already enabled"));
    }
    let Some(temp_secret) = jar.get(PENDING_2FA_COOKIE).map(|c| c.value().to_string()) else {
        return Err(ApiError::bad_request("2FA setup not initiated. Start setup first."));
    };
    if !bscp_common::totp::verify(&temp_secret, &body.otp, 0) {
        return Ok((jar, Json(json!({ "success": false, "error": "Invalid verification code" }))));
    }
    sqlx::query("UPDATE users SET otp_secret = ?, is_2fa_enabled = 1 WHERE id = ?")
        .bind(&temp_secret)
        .bind(&auth.user.id)
        .execute(&state.pool)
        .await?;
    let jar = remove_cookie(jar, PENDING_2FA_COOKIE);
    Ok((jar, Json(json!({ "success": true, "message": "2FA enabled successfully" }))))
}

#[derive(Deserialize)]
struct PasswordBody {
    password: String,
}

async fn twofa_disable(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<PasswordBody>,
) -> Result<Json<Value>, ApiError> {
    if !bscp_common::password::verify_password(&body.password, &auth.user.password_hash) {
        return Err(ApiError::forbidden("Invalid password"));
    }
    if !auth.user.is_2fa_enabled {
        return Err(ApiError::bad_request("2FA is not enabled"));
    }
    sqlx::query("UPDATE users SET is_2fa_enabled = 0 WHERE id = ?")
        .bind(&auth.user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "success": true, "message": "2FA disabled successfully" })))
}
