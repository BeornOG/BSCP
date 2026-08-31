//! Mint (and cache) federation assertions for local users.

use crate::state::AppState;
use bscp_common::assertion::AssertionClaims;
use bscp_common::models::User;
use bscp_common::{now_ts, uuid};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const TTL: f64 = 240.0;
/// Re-mint when the cached token has less than this many seconds left.
const REFRESH_MARGIN: f64 = 60.0;

/// `(user_id, aud)` → `(token, exp)`
type Cache = Mutex<HashMap<(String, String), (String, f64)>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A short-lived RS256 assertion for `user` addressed to channel server `aud`.
pub async fn assertion_for(state: &AppState, user: &User, aud: &str) -> anyhow::Result<String> {
    let key = (user.id.clone(), aud.to_string());
    let now = now_ts();

    if let Some((tok, exp)) = cache().lock().unwrap().get(&key) {
        if *exp - now > REFRESH_MARGIN {
            return Ok(tok.clone());
        }
    }

    let jti = uuid();
    let exp = now + TTL;
    let claims = AssertionClaims {
        iss: state.cfg.public_url.clone(),
        sub: state.full_id(&user.username),
        aud: aud.to_string(),
        exp: exp as i64,
        iat: now as i64,
        jti: jti.clone(),
        name: user.display_name.clone().or_else(|| Some(user.username.clone())),
        picture: user.profile_pic.clone(),
    };
    let token = state.oidc.sign(&claims)?;

    sqlx::query("INSERT INTO issued_assertions (jti, user_id, sub, aud, exp) VALUES (?, ?, ?, ?, ?)")
        .bind(&jti)
        .bind(&user.id)
        .bind(&claims.sub)
        .bind(aud)
        .bind(exp)
        .execute(&state.pool)
        .await?;

    cache().lock().unwrap().insert(key, (token.clone(), exp));
    Ok(token)
}

/// Answer the recipient's callback: did we issue `jti` for this `sub`/`aud`, still valid?
pub async fn verify_issued(
    state: &AppState,
    jti: &str,
    sub: &str,
    aud: &str,
) -> Option<(Option<String>, Option<String>)> {
    let row = sqlx::query_as::<_, (String, String, f64)>(
        "SELECT sub, aud, exp FROM issued_assertions WHERE jti = ?",
    )
    .bind(jti)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()?;

    if row.0 != sub || row.1 != aud || row.2 <= now_ts() {
        return None;
    }
    // (name / picture come from the current user record, not the stale claim)
    let np = sqlx::query_as::<_, (Option<String>, Option<String>, String)>(
        "SELECT display_name, profile_pic, username FROM users WHERE id = \
         (SELECT user_id FROM issued_assertions WHERE jti = ?)",
    )
    .bind(jti)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    match np {
        Some((dn, pic, uname)) => Some((dn.or(Some(uname)), pic)),
        None => Some((None, None)),
    }
}

/// Best-effort GC of expired rows.
pub async fn gc(state: &AppState) {
    let _ = sqlx::query("DELETE FROM issued_assertions WHERE exp <= ?")
        .bind(now_ts())
        .execute(&state.pool)
        .await;
}
