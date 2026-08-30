//! External account linking mediated by modules. (Endpoints land in step 5;
//! this holds the ticket helpers + the claim used by OIDC `userinfo`.)

use crate::state::AppState;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Sign a compact link ticket `user|module|provider|exp` with the server secret.
pub fn mint_ticket(state: &AppState, user_id: &str, module: &str, provider: &str, exp: f64) -> String {
    let payload = format!("{user_id}|{module}|{provider}|{}", exp as i64);
    let mut mac = HmacSha256::new_from_slice(state.cfg.secret_key.as_bytes()).expect("key");
    mac.update(payload.as_bytes());
    let sig: String = mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
    use base64::Engine;
    format!("{}.{sig}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload))
}

/// Returns `(user_id, module, provider)` if the ticket is valid and unexpired.
pub fn verify_ticket(state: &AppState, ticket: &str) -> Option<(String, String, String)> {
    use base64::Engine;
    let (b64, sig) = ticket.split_once('.')?;
    let payload = String::from_utf8(base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(b64).ok()?).ok()?;
    let mut mac = HmacSha256::new_from_slice(state.cfg.secret_key.as_bytes()).ok()?;
    mac.update(payload.as_bytes());
    let expect: String = mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
    if expect != sig {
        return None;
    }
    let mut parts = payload.split('|');
    let user_id = parts.next()?.to_string();
    let module = parts.next()?.to_string();
    let provider = parts.next()?.to_string();
    let exp: i64 = parts.next()?.parse().ok()?;
    if (exp as f64) < bscp_common::now_ts() {
        return None;
    }
    Some((user_id, module, provider))
}

/// `bscp_links` claim for OIDC `userinfo` / id_token.
pub async fn links_claim(state: &AppState, user_id: &str) -> Value {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
        "SELECT module, provider, display_name, profile_url FROM account_links WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    json!(rows
        .into_iter()
        .map(|(module, provider, display_name, profile_url)| json!({
            "module": module, "provider": provider,
            "display_name": display_name, "profile_url": profile_url,
        }))
        .collect::<Vec<_>>())
}
