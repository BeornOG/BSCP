//! Authorization codes, opaque access/refresh tokens, and the signed ID token.

use super::{scope_has, sha256_hex};
use crate::state::AppState;
use bscp_common::models::User;
use bscp_common::{now_ts, random_token, uuid, ApiError};
use serde_json::{json, Map, Value};
use sqlx::FromRow;

const CODE_TTL: f64 = 60.0;

#[derive(FromRow)]
pub struct CodeRow {
    pub client_id: String,
    pub user_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub nonce: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub auth_time: f64,
}

#[allow(clippy::too_many_arguments)]
pub async fn issue_code(
    state: &AppState,
    client_id: &str,
    user_id: &str,
    redirect_uri: &str,
    scope: &str,
    nonce: Option<&str>,
    code_challenge: Option<&str>,
    code_challenge_method: Option<&str>,
    auth_time: f64,
) -> Result<String, ApiError> {
    let code = random_token(32);
    sqlx::query(
        "INSERT INTO oauth_codes \
         (code, client_id, user_id, redirect_uri, scope, nonce, code_challenge, code_challenge_method, \
          auth_time, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&code)
    .bind(client_id)
    .bind(user_id)
    .bind(redirect_uri)
    .bind(scope)
    .bind(nonce)
    .bind(code_challenge)
    .bind(code_challenge_method)
    .bind(auth_time)
    .bind(now_ts() + CODE_TTL)
    .execute(&state.pool)
    .await?;
    Ok(code)
}

/// Fetch + single-use-consume a code. `None` if missing / used / expired.
pub async fn consume_code(state: &AppState, code: &str) -> Result<Option<CodeRow>, ApiError> {
    let row = sqlx::query_as::<_, CodeRow>(
        "SELECT client_id, user_id, redirect_uri, scope, nonce, code_challenge, \
         code_challenge_method, auth_time FROM oauth_codes \
         WHERE code = ? AND used = 0 AND expires_at > ?",
    )
    .bind(code)
    .bind(now_ts())
    .fetch_optional(&state.pool)
    .await?;
    if row.is_some() {
        sqlx::query("UPDATE oauth_codes SET used = 1 WHERE code = ?").bind(code).execute(&state.pool).await?;
    }
    Ok(row)
}

/// Verify RFC 7636 S256 (or `plain`).
pub fn verify_pkce(challenge: &str, method: Option<&str>, verifier: &str) -> bool {
    match method.unwrap_or("plain") {
        "S256" => {
            use base64::Engine;
            use sha2::{Digest, Sha256};
            let h = Sha256::digest(verifier.as_bytes());
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(h) == challenge
        }
        _ => challenge == verifier,
    }
}

pub struct TokenSet {
    pub access: String,
    pub refresh: String,
    pub id_token: String,
    pub expires_in: u64,
    pub scope: String,
}

async fn store_token(
    state: &AppState,
    kind: &str,
    client_id: &str,
    user_id: &str,
    scope: &str,
    ttl: f64,
) -> Result<String, ApiError> {
    let token = random_token(32);
    sqlx::query(
        "INSERT INTO oauth_tokens (id, kind, token_hash, client_id, user_id, scope, created_at, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid())
    .bind(kind)
    .bind(sha256_hex(&token))
    .bind(client_id)
    .bind(user_id)
    .bind(scope)
    .bind(now_ts())
    .bind(now_ts() + ttl)
    .execute(&state.pool)
    .await?;
    Ok(token)
}

pub async fn issue_tokens(
    state: &AppState,
    client_id: &str,
    user: &User,
    scope: &str,
    nonce: Option<&str>,
    auth_time: f64,
) -> Result<TokenSet, ApiError> {
    let access_ttl = state.cfg.oidc_access_ttl as f64;
    let refresh_ttl = state.cfg.oidc_refresh_ttl as f64;

    let access = store_token(state, "access", client_id, &user.id, scope, access_ttl).await?;
    let refresh = store_token(state, "refresh", client_id, &user.id, scope, refresh_ttl).await?;
    let id_token = build_id_token(state, client_id, user, scope, nonce, auth_time)?;

    Ok(TokenSet {
        access,
        refresh,
        id_token,
        expires_in: state.cfg.oidc_access_ttl,
        scope: scope.to_string(),
    })
}

pub async fn rotate_refresh(
    state: &AppState,
    refresh_token: &str,
    client_id: &str,
) -> Result<Option<TokenSet>, ApiError> {
    let hash = sha256_hex(refresh_token);
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT user_id, scope FROM oauth_tokens \
         WHERE kind = 'refresh' AND token_hash = ? AND client_id = ? AND revoked = 0 AND expires_at > ?",
    )
    .bind(&hash)
    .bind(client_id)
    .bind(now_ts())
    .fetch_optional(&state.pool)
    .await?;
    let Some((user_id, scope)) = row else { return Ok(None) };

    sqlx::query("UPDATE oauth_tokens SET revoked = 1 WHERE kind = 'refresh' AND token_hash = ?")
        .bind(&hash)
        .execute(&state.pool)
        .await?;

    let Some(user) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(&user_id)
        .fetch_optional(&state.pool)
        .await?
    else {
        return Ok(None);
    };

    Ok(Some(issue_tokens(state, client_id, &user, &scope, None, now_ts()).await?))
}

/// Resolve a bearer access token to its user + granted scope (for `userinfo`).
pub async fn user_from_access(state: &AppState, token: &str) -> Result<Option<(User, String)>, ApiError> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT user_id, scope FROM oauth_tokens \
         WHERE kind = 'access' AND token_hash = ? AND revoked = 0 AND expires_at > ?",
    )
    .bind(sha256_hex(token))
    .bind(now_ts())
    .fetch_optional(&state.pool)
    .await?;
    let Some((user_id, scope)) = row else { return Ok(None) };
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(&user_id)
        .fetch_optional(&state.pool)
        .await?;
    Ok(user.map(|u| (u, scope)))
}

pub async fn revoke(state: &AppState, token: &str) -> Result<(), ApiError> {
    sqlx::query("UPDATE oauth_tokens SET revoked = 1 WHERE token_hash = ?")
        .bind(sha256_hex(token))
        .execute(&state.pool)
        .await?;
    Ok(())
}

// ── ID token ─────────────────────────────────────────────────────────

fn build_id_token(
    state: &AppState,
    client_id: &str,
    user: &User,
    scope: &str,
    nonce: Option<&str>,
    auth_time: f64,
) -> Result<String, ApiError> {
    let now = now_ts() as i64;
    let mut claims: Map<String, Value> = Map::new();
    claims.insert("iss".into(), json!(state.cfg.public_url));
    claims.insert("sub".into(), json!(state.full_id(&user.username)));
    claims.insert("aud".into(), json!(client_id));
    claims.insert("iat".into(), json!(now));
    claims.insert("exp".into(), json!(now + state.cfg.oidc_access_ttl as i64));
    claims.insert("auth_time".into(), json!(auth_time as i64));
    if let Some(n) = nonce {
        claims.insert("nonce".into(), json!(n));
    }
    add_profile_claims(&mut claims, state, user, scope);

    state.oidc.sign(&Value::Object(claims)).map_err(|e| ApiError::internal(format!("id_token: {e}")))
}

/// Shared by the ID token and `userinfo`.
pub fn add_profile_claims(claims: &mut Map<String, Value>, state: &AppState, user: &User, scope: &str) {
    if scope_has(scope, "profile") {
        claims.insert(
            "name".into(),
            json!(user.display_name.clone().unwrap_or_else(|| user.username.clone())),
        );
        claims.insert("preferred_username".into(), json!(state.full_id(&user.username)));
        if let Some(pic) = &user.profile_pic {
            claims.insert("picture".into(), json!(pic));
        }
        claims.insert("updated_at".into(), json!(user.created_at as i64));
    }
    if scope_has(scope, "email") {
        if let Some(email) = &user.email {
            claims.insert("email".into(), json!(email));
            claims.insert("email_verified".into(), json!(false));
        }
    }
}
