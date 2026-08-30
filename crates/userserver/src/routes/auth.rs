//! `/api/auth` — authentication & account management.

use crate::auth::{clear_session, remove_cookie, set_cookie, PENDING_USER_COOKIE, SESSION_COOKIE};
use crate::state::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::PrivateCookieJar;
use bscp_common::models::{InviteCode, User};
use bscp_common::password::{hash_password, verify_password};
use bscp_common::{now_ts, uuid, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};

const SESSION_TTL_SECS: f64 = 30.0 * 24.0 * 60.0 * 60.0;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/setup", get(setup_status).post(setup))
        .route("/api/auth/login", post(login))
        .route("/api/auth/2fa", post(two_factor))
        .route("/api/auth/register", post(register))
        .route("/api/auth/logout", post(logout))
}

fn user_agent(headers: &HeaderMap) -> String {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown")
        .to_string()
}

/// Create a device session, persist it, and store the token in the cookie jar.
async fn create_session(
    state: &AppState,
    jar: PrivateCookieJar,
    user: &User,
    device_info: String,
) -> Result<(PrivateCookieJar, String), ApiError> {
    let token = bscp_common::random_token(32);
    let now = now_ts();

    if user.status_type != 2 && user.status_type != 3 {
        sqlx::query("UPDATE users SET status_type = 0 WHERE id = ?")
            .bind(&user.id)
            .execute(&state.pool)
            .await?;
    }

    sqlx::query(
        "INSERT INTO user_sessions (id, user_id, token, device_info, last_active, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid())
    .bind(&user.id)
    .bind(&token)
    .bind(&device_info)
    .bind(now)
    .bind(now + SESSION_TTL_SECS)
    .execute(&state.pool)
    .await?;

    crate::modules::dispatch(
        state,
        "session.created",
        serde_json::json!({ "user": state.full_id(&user.username), "device_info": device_info }),
    );

    let mut jar = clear_session(jar);
    jar = set_cookie(jar, SESSION_COOKIE, token.clone());
    Ok((jar, token))
}

// ── GET/POST /api/auth/setup ─────────────────────────────────────────────

async fn setup_status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&state.pool).await?;
    Ok(Json(json!({ "needs_setup": count == 0 })))
}

#[derive(Deserialize)]
struct SetupBody {
    username: Option<String>,
    email: Option<String>,
    password: Option<String>,
    password_confirm: Option<String>,
}

async fn setup(
    State(state): State<AppState>,
    Json(body): Json<SetupBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&state.pool).await?;
    if count > 0 {
        return Err(ApiError::bad_request("Setup already complete"));
    }

    let username = body.username.unwrap_or_default().trim().to_string();
    let password = body.password.unwrap_or_default();
    let password_confirm = body.password_confirm.unwrap_or_default();
    let email = body.email.and_then(|e| {
        let e = e.trim().to_string();
        if e.is_empty() { None } else { Some(e) }
    });

    let mut errors = Vec::new();
    if username.is_empty() {
        errors.push("Username is required".to_string());
    } else if username.len() < 3 {
        errors.push("Username must be at least 3 characters".to_string());
    }
    if password.is_empty() {
        errors.push("Password is required".to_string());
    } else if password.len() < 6 {
        errors.push("Password must be at least 6 characters".to_string());
    }
    if password != password_confirm {
        errors.push("Passwords do not match".to_string());
    }
    if !errors.is_empty() {
        return Err(ApiError::with_errors(StatusCode::BAD_REQUEST, errors.join(", "), errors));
    }

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, email, otp_secret, is_admin, is_primary_admin, \
         is_2fa_enabled, created_at) VALUES (?, ?, ?, ?, ?, 1, 1, 0, ?)",
    )
    .bind(uuid())
    .bind(&username)
    .bind(hash_password(&password)?)
    .bind(&email)
    .bind(bscp_common::totp::random_base32())
    .bind(now_ts())
    .execute(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true }))))
}

// ── POST /api/auth/login ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct LoginBody {
    user: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    headers: HeaderMap,
    Json(body): Json<LoginBody>,
) -> Result<(PrivateCookieJar, Json<Value>), ApiError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&body.user)
        .fetch_optional(&state.pool)
        .await?;

    let Some(user) = user.filter(|u| verify_password(&body.password, &u.password_hash)) else {
        return Ok((jar, Json(json!({ "success": false, "error": "Invalid username or password" }))));
    };

    if user.is_2fa_enabled {
        let jar = set_cookie(jar, PENDING_USER_COOKIE, user.id.clone());
        return Ok((jar, Json(json!({ "success": false, "requires_2fa": true }))));
    }

    let (jar, token) = create_session(&state, jar, &user, user_agent(&headers)).await?;
    Ok((jar, Json(json!({ "success": true, "session_token": token }))))
}

// ── POST /api/auth/2fa ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct TwoFactorBody {
    otp: String,
}

async fn two_factor(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    headers: HeaderMap,
    Json(body): Json<TwoFactorBody>,
) -> Result<(PrivateCookieJar, Json<Value>), ApiError> {
    let pending = jar.get(PENDING_USER_COOKIE).map(|c| c.value().to_string());
    let Some(user_id) = pending else {
        return Err(ApiError::bad_request("No pending 2FA session"));
    };

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(&user_id)
        .fetch_optional(&state.pool)
        .await?;
    let Some(user) = user else {
        return Err(ApiError::bad_request("User not found"));
    };

    if !bscp_common::totp::verify(&user.otp_secret, &body.otp, 0) {
        return Ok((jar, Json(json!({ "success": false, "error": "Invalid code" }))));
    }

    let jar = remove_cookie(jar, PENDING_USER_COOKIE);
    let (jar, token) = create_session(&state, jar, &user, user_agent(&headers)).await?;
    Ok((jar, Json(json!({ "success": true, "session_token": token }))))
}

// ── POST /api/auth/register ──────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterBody {
    username: Option<String>,
    password: Option<String>,
    password_confirm: Option<String>,
    invite_code: Option<String>,
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&state.pool).await?;
    if count == 0 {
        return Err(ApiError::bad_request("Setup required first"));
    }

    let username = body.username.unwrap_or_default().trim().to_string();
    let password = body.password.unwrap_or_default();
    let password_confirm = body.password_confirm.unwrap_or_default();
    let invite_code = body.invite_code.unwrap_or_default().trim().to_string();

    let mut errors = Vec::new();
    if username.is_empty() {
        errors.push("Username is required".to_string());
    } else if username.len() < 3 {
        errors.push("Username must be at least 3 characters".to_string());
    } else {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = ?")
            .bind(&username)
            .fetch_one(&state.pool)
            .await?;
        if exists > 0 {
            errors.push("Username already exists".to_string());
        }
    }
    if password.is_empty() {
        errors.push("Password is required".to_string());
    } else if password.len() < 6 {
        errors.push("Password must be at least 6 characters".to_string());
    }
    if password != password_confirm {
        errors.push("Passwords do not match".to_string());
    }
    if invite_code.is_empty() {
        errors.push("Invite code is required".to_string());
    }

    let mut invite: Option<InviteCode> = None;
    if errors.is_empty() {
        let found = sqlx::query_as::<_, InviteCode>("SELECT * FROM invite_codes WHERE code = ?")
            .bind(&invite_code)
            .fetch_optional(&state.pool)
            .await?;
        match found {
            None => errors.push("Invalid invite code".to_string()),
            Some(i) if i.used_by.is_some() => errors.push("Invite code already used".to_string()),
            Some(i) if i.expires_at.is_some_and(|e| e < now_ts()) => {
                errors.push("Invite code has expired".to_string())
            }
            Some(i) => invite = Some(i),
        }
    }

    if !errors.is_empty() {
        return Err(ApiError::with_errors(StatusCode::BAD_REQUEST, errors.join(", "), errors));
    }
    let invite = invite.expect("invite present when no errors");

    let user_id = uuid();
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, otp_secret, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&user_id)
    .bind(&username)
    .bind(hash_password(&password)?)
    .bind(bscp_common::totp::random_base32())
    .bind(now_ts())
    .execute(&state.pool)
    .await?;

    sqlx::query("UPDATE invite_codes SET used_by = ?, used_at = ? WHERE id = ?")
        .bind(&user_id)
        .bind(now_ts())
        .bind(invite.id)
        .execute(&state.pool)
        .await?;

    crate::modules::dispatch(
        &state,
        "user.registered",
        json!({ "user": state.full_id(&username), "username": username, "created_at": now_ts() }),
    );

    Ok((StatusCode::CREATED, Json(json!({ "success": true }))))
}

// ── POST /api/auth/logout ────────────────────────────────────────────────

async fn logout(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<(PrivateCookieJar, Json<Value>), ApiError> {
    if let Some(token) = jar.get(SESSION_COOKIE).map(|c| c.value().to_string()) {
        if let Some(session) = sqlx::query_as::<_, bscp_common::models::UserSession>(
            "SELECT * FROM user_sessions WHERE token = ?",
        )
        .bind(&token)
        .fetch_optional(&state.pool)
        .await?
        {
            let user_id = session.user_id.clone();
            sqlx::query("DELETE FROM user_sessions WHERE id = ?").bind(&session.id).execute(&state.pool).await?;
            let remaining: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM user_sessions WHERE user_id = ? AND expires_at > ?",
            )
            .bind(&user_id)
            .bind(now_ts())
            .fetch_one(&state.pool)
            .await?;
            if remaining == 0 {
                sqlx::query("UPDATE users SET status_type = 1 WHERE id = ?")
                    .bind(&user_id)
                    .execute(&state.pool)
                    .await?;
            }
        }
    }
    Ok((clear_session(jar), Json(json!({ "success": true }))))
}
