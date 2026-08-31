//! Minimal OIDC *client* — used only for the operator console "Sign in with BSCP".

use crate::state::AppState;
use anyhow::{anyhow, Context};
use base64::Engine;
use bscp_common::{now_ts, random_token};
use serde_json::Value;

const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

pub struct IdpMeta {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
}

/// Resolve an IdP from a `user@domain` or bare `domain` string.
pub async fn discover(state: &AppState, idp_input: &str) -> anyhow::Result<IdpMeta> {
    let host = idp_input.rsplit('@').next().unwrap_or(idp_input).trim();
    // try BSCP well-known first (it points at the openid-configuration), then direct
    let candidates = [
        format!("http://{host}/.well-known/openid-configuration"),
        format!("https://{host}/.well-known/openid-configuration"),
    ];
    let client = state.discovery.client();
    for url in candidates {
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                let v: Value = resp.json().await.context("openid-configuration json")?;
                return Ok(IdpMeta {
                    issuer: s(&v, "issuer").ok_or_else(|| anyhow!("no issuer"))?,
                    authorization_endpoint: s(&v, "authorization_endpoint")
                        .ok_or_else(|| anyhow!("no authorization_endpoint"))?,
                    token_endpoint: s(&v, "token_endpoint").ok_or_else(|| anyhow!("no token_endpoint"))?,
                    registration_endpoint: s(&v, "registration_endpoint"),
                });
            }
        }
    }
    Err(anyhow!("could not reach an OIDC provider at {host}"))
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(String::from)
}

/// Get or dynamically-register client credentials for `meta.issuer`.
async fn client_creds(state: &AppState, meta: &IdpMeta) -> anyhow::Result<(String, String)> {
    if let Some(row) = sqlx::query_as::<_, (String, String)>(
        "SELECT client_id, client_secret FROM idp_clients WHERE idp = ?",
    )
    .bind(&meta.issuer)
    .fetch_optional(&state.pool)
    .await?
    {
        return Ok(row);
    }
    let reg = meta
        .registration_endpoint
        .as_ref()
        .ok_or_else(|| anyhow!("issuer has no dynamic registration"))?;
    let redirect = format!("{}/oauth/callback", state.public_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "client_name": format!("BSCP channel server {}", state.domain),
        "redirect_uris": [redirect],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "client_secret_basic",
        "scope": "openid profile",
    });
    let v: Value = state
        .discovery
        .client()
        .post(reg)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let cid = s(&v, "client_id").ok_or_else(|| anyhow!("registration: no client_id"))?;
    let secret = s(&v, "client_secret").unwrap_or_default();
    sqlx::query("INSERT INTO idp_clients (idp, client_id, client_secret, registered_at) VALUES (?, ?, ?, ?)")
        .bind(&meta.issuer)
        .bind(&cid)
        .bind(&secret)
        .bind(now_ts())
        .execute(&state.pool)
        .await?;
    Ok((cid, secret))
}

/// Build the authorize redirect URL and persist the PKCE state.
pub async fn begin(state: &AppState, idp_input: &str) -> anyhow::Result<String> {
    let meta = discover(state, idp_input).await?;
    let (client_id, _secret) = client_creds(state, &meta).await?;

    let verifier = random_token(48);
    let challenge = {
        use sha2::{Digest, Sha256};
        B64.encode(Sha256::digest(verifier.as_bytes()))
    };
    let st = random_token(24);
    sqlx::query("INSERT INTO oidc_states (state, idp, code_verifier, created_at) VALUES (?, ?, ?, ?)")
        .bind(&st)
        .bind(&meta.issuer)
        .bind(&verifier)
        .bind(now_ts())
        .execute(&state.pool)
        .await?;

    let redirect = format!("{}/oauth/callback", state.public_url.trim_end_matches('/'));
    let q = serde_urlencoded::to_string([
        ("response_type", "code"),
        ("client_id", &client_id),
        ("redirect_uri", &redirect),
        ("scope", "openid profile"),
        ("state", &st),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ])?;
    Ok(format!("{}?{q}", meta.authorization_endpoint))
}

pub struct OperatorIdentity {
    pub sub: String,
    #[allow(dead_code)]
    pub name: Option<String>,
}

/// Exchange the code, decode the id_token, return the identity.
pub async fn complete(state: &AppState, code: &str, st: &str) -> anyhow::Result<OperatorIdentity> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT idp, code_verifier FROM oidc_states WHERE state = ?",
    )
    .bind(st)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| anyhow!("unknown or expired state"))?;
    sqlx::query("DELETE FROM oidc_states WHERE state = ?").bind(st).execute(&state.pool).await.ok();
    let (issuer, verifier) = row;

    let meta = discover(state, &issuer).await?;
    let (client_id, secret) = sqlx::query_as::<_, (String, String)>(
        "SELECT client_id, client_secret FROM idp_clients WHERE idp = ?",
    )
    .bind(&issuer)
    .fetch_one(&state.pool)
    .await?;

    let redirect = format!("{}/oauth/callback", state.public_url.trim_end_matches('/'));
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &redirect),
        ("code_verifier", &verifier),
        ("client_id", &client_id),
    ];
    let tok: Value = state
        .discovery
        .client()
        .post(&meta.token_endpoint)
        .basic_auth(&client_id, Some(&secret))
        .form(&form)
        .send()
        .await?
        .error_for_status()
        .context("token endpoint")?
        .json()
        .await?;
    let id_token = s(&tok, "id_token").ok_or_else(|| anyhow!("no id_token"))?;

    // decode payload (received over TLS straight from the token endpoint)
    let payload = id_token.split('.').nth(1).ok_or_else(|| anyhow!("bad id_token"))?;
    let claims: Value = serde_json::from_slice(&B64.decode(payload)?)?;
    let sub = s(&claims, "sub").ok_or_else(|| anyhow!("id_token has no sub"))?;
    Ok(OperatorIdentity { sub, name: s(&claims, "name").or_else(|| s(&claims, "preferred_username")) })
}

/// GC stale PKCE states.
pub async fn gc_states(state: &AppState) {
    let _ = sqlx::query("DELETE FROM oidc_states WHERE created_at < ?")
        .bind(now_ts() - 600.0)
        .execute(&state.pool)
        .await;
}
