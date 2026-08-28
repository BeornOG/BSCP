//! Session cookie handling and request extractors.

use crate::state::AppState;
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};
use bscp_common::models::User;
use bscp_common::{now_ts, ApiError};

pub const SESSION_COOKIE: &str = "bscp_session";
pub const PENDING_USER_COOKIE: &str = "bscp_pending_user";
pub const PENDING_2FA_COOKIE: &str = "bscp_pending_2fa";

fn base_cookie(name: &'static str, value: String) -> Cookie<'static> {
    let mut c = Cookie::new(name, value);
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_path("/");
    c
}

pub fn set_cookie(jar: PrivateCookieJar, name: &'static str, value: impl Into<String>) -> PrivateCookieJar {
    jar.add(base_cookie(name, value.into()))
}

pub fn remove_cookie(jar: PrivateCookieJar, name: &'static str) -> PrivateCookieJar {
    let mut c = Cookie::from(name);
    c.set_path("/");
    jar.remove(c)
}

pub fn clear_session(mut jar: PrivateCookieJar) -> PrivateCookieJar {
    for name in [SESSION_COOKIE, PENDING_USER_COOKIE, PENDING_2FA_COOKIE] {
        jar = remove_cookie(jar, name);
    }
    jar
}

/// Resolve the session token from the private cookie or the `X-Session-Token`
/// header (used by embedded / mobile clients).
pub fn session_token(jar: &PrivateCookieJar, parts: &Parts) -> Option<String> {
    if let Some(c) = jar.get(SESSION_COOKIE) {
        return Some(c.value().to_string());
    }
    parts
        .headers
        .get("x-session-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

async fn load_user_by_token(state: &AppState, token: &str) -> Option<User> {
    let session = sqlx::query_as::<_, bscp_common::models::UserSession>(
        "SELECT * FROM user_sessions WHERE token = ?",
    )
    .bind(token)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()?;

    if session.expires_at <= now_ts() {
        return None;
    }

    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(&session.user_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten()
}

/// Authenticated user + the token that identified them.
pub struct AuthUser {
    pub user: User,
    pub token: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let jar: PrivateCookieJar = PrivateCookieJar::from_headers(&parts.headers, state.cookie_key.clone());
        let token = session_token(&jar, parts).ok_or_else(ApiError::unauthorized)?;
        let user = load_user_by_token(state, &token).await.ok_or_else(ApiError::unauthorized)?;
        Ok(AuthUser { user, token })
    }
}

/// Authenticated user that must be an admin.
pub struct AdminUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let AuthUser { user, .. } = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(ApiError::forbidden("Admin access required"));
        }
        Ok(AdminUser(user))
    }
}
