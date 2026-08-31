//! Local + federated user profile resolution — port of `services/users.py`.

use crate::state::AppState;
use crate::status::user_status;
use bscp_common::models::{User, UserSession};
use bscp_common::ApiError;
use serde_json::{json, Value};

pub async fn load_sessions(state: &AppState, user_id: &str) -> Vec<UserSession> {
    sqlx::query_as::<_, UserSession>("SELECT * FROM user_sessions WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
}

/// Public profile JSON. Never exposes internal IDs.
pub fn serialize_profile(state: &AppState, user: &User, sessions: &[UserSession]) -> Value {
    json!({
        "username": state.full_id(&user.username),
        "display_name": user.display_name.clone().unwrap_or_else(|| user.username.clone()),
        "profile_pic": user.profile_pic,
        "status": user_status(user, sessions),
        "is_admin": user.is_admin,
        "is_primary_admin": user.is_primary_admin,
        "is_2fa_enabled": user.is_2fa_enabled,
        "bio": user.bio,
        "storage_limit_mb": user.storage_limit_mb,
    })
}

pub async fn serialize_profile_db(state: &AppState, user: &User) -> Value {
    let sessions = load_sessions(state, &user.id).await;
    serialize_profile(state, user, &sessions)
}

/// Resolve a `username@domain` profile. `Ok(None)` = not found;
/// `Err(502)` = remote server unreachable.
pub async fn get_profile(state: &AppState, full_id: &str) -> Result<Option<Value>, ApiError> {
    let Some((username, domain)) = full_id.rsplit_once('@') else {
        return Ok(None);
    };

    if domain == state.domain() {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ? AND is_deleted = 0")
            .bind(username)
            .fetch_optional(&state.pool)
            .await?;
        return Ok(match user {
            Some(u) => Some(serialize_profile_db(state, &u).await),
            None => None,
        });
    }

    if state.domain_blocked(domain).await {
        return Ok(None);
    }

    match bscp_common::federation::fetch_remote_profile(&state.discovery, domain, full_id).await {
        Ok(v) => Ok(v),
        Err(()) => Err(ApiError::bad_gateway("Failed to reach remote server")),
    }
}
