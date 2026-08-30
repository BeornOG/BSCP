//! OAuth client resolution: registered clients (RFC 7591 dynamic registration)
//! and federation-trust clients (RP publishes `/.well-known/BSCP/relying-party`).

use super::{is_local_host, sha256_hex};
use crate::state::AppState;
use bscp_common::{now_ts, random_token, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::FromRow;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(FromRow, Clone)]
pub struct StoredClient {
    pub client_id: String,
    pub client_secret_hash: Option<String>,
    pub name: String,
    pub redirect_uris: String,
    #[allow(dead_code)]
    pub grant_types: String,
    pub scope: String,
    pub token_endpoint_auth_method: String,
    pub registration_access_token_hash: Option<String>,
    #[allow(dead_code)]
    pub created_at: f64,
    pub disabled: i64,
}

/// A client ready to run an `authorize` request against.
pub struct ResolvedClient {
    pub client_id: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub redirect_uris: Vec<String>,
    /// public ⇒ PKCE mandatory, no client secret, consent never skipped.
    pub is_public: bool,
    pub registered: bool,
    pub secret_hash: Option<String>,
    pub allowed_scope: String,
}

impl ResolvedClient {
    pub fn allows_redirect(&self, uri: &str) -> bool {
        self.redirect_uris.iter().any(|u| u == uri)
    }
}

fn parse_uris(json_text: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json_text).unwrap_or_default()
}

pub async fn load(state: &AppState, client_id: &str) -> Result<Option<StoredClient>, ApiError> {
    Ok(sqlx::query_as::<_, StoredClient>("SELECT * FROM oauth_clients WHERE client_id = ?")
        .bind(client_id)
        .fetch_optional(&state.pool)
        .await?)
}

/// Resolve the client for an `authorize`/`token` request — registered first,
/// then the federation-trust fallback.
pub async fn resolve(state: &AppState, client_id: &str) -> Result<ResolvedClient, String> {
    if let Ok(Some(c)) = load(state, client_id).await {
        if c.disabled != 0 {
            return Err("client is disabled".into());
        }
        let is_public = c.token_endpoint_auth_method == "none";
        return Ok(ResolvedClient {
            client_id: c.client_id,
            name: if c.name.is_empty() { "an application".into() } else { c.name },
            logo_url: None,
            redirect_uris: parse_uris(&c.redirect_uris),
            is_public,
            registered: true,
            secret_hash: c.client_secret_hash,
            allowed_scope: c.scope,
        });
    }
    federation_trust(state, client_id).await
}

// ── federation trust ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct RelyingPartyDoc {
    #[serde(default)]
    client_name: Option<String>,
    #[serde(default)]
    redirect_uris: Vec<String>,
    #[serde(default)]
    logo_url: Option<String>,
}

struct CacheEntry {
    doc: RelyingPartyDoc,
    at: Instant,
}
static RP_CACHE: Mutex<Option<HashMap<String, CacheEntry>>> = Mutex::new(None);
const RP_TTL: Duration = Duration::from_secs(300);

async fn federation_trust(state: &AppState, client_id: &str) -> Result<ResolvedClient, String> {
    let origin = client_id.trim_end_matches('/');
    let url = reqwest::Url::parse(origin).map_err(|_| "unknown client".to_string())?;
    let host = url.host_str().ok_or("client_id has no host")?;
    match url.scheme() {
        "https" => {}
        "http" if is_local_host(host) => {}
        _ => return Err("client_id must be an https origin".into()),
    }

    {
        let mut guard = RP_CACHE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some(e) = map.get(origin) {
            if e.at.elapsed() < RP_TTL {
                return build_from_doc(origin, &e.doc);
            }
        }
    }

    let well_known = format!("{origin}/.well-known/BSCP/relying-party");
    let resp = state
        .discovery
        .client()
        .get(&well_known)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|_| "could not reach the relying party".to_string())?;
    if !resp.status().is_success() {
        return Err("relying party did not publish a BSCP client document".into());
    }
    let doc: RelyingPartyDoc = resp.json().await.map_err(|_| "invalid relying-party document".to_string())?;
    let resolved = build_from_doc(origin, &doc)?;
    RP_CACHE
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(origin.to_string(), CacheEntry { doc, at: Instant::now() });
    Ok(resolved)
}

fn build_from_doc(origin: &str, doc: &RelyingPartyDoc) -> Result<ResolvedClient, String> {
    let redirect_uris: Vec<String> = doc
        .redirect_uris
        .iter()
        .filter(|u| u.starts_with(origin))
        .cloned()
        .collect();
    if redirect_uris.is_empty() {
        return Err("relying-party document lists no redirect URI under its own origin".into());
    }
    Ok(ResolvedClient {
        client_id: origin.to_string(),
        name: doc.client_name.clone().unwrap_or_else(|| origin.to_string()),
        logo_url: doc.logo_url.clone(),
        redirect_uris,
        is_public: true,
        registered: false,
        secret_hash: None,
        allowed_scope: "openid profile email bscp:links".into(),
    })
}

// ── dynamic registration (RFC 7591) ──────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub token_endpoint_auth_method: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

pub async fn register(state: &AppState, req: RegisterRequest) -> Result<Value, ApiError> {
    if req.redirect_uris.is_empty() {
        return Err(ApiError::bad_request("redirect_uris is required"));
    }
    for u in &req.redirect_uris {
        if reqwest::Url::parse(u).is_err() {
            return Err(ApiError::bad_request(format!("invalid redirect_uri: {u}")));
        }
    }
    let auth_method = match req.token_endpoint_auth_method.as_deref() {
        Some("none") => "none",
        Some("client_secret_post") => "client_secret_post",
        _ => "client_secret_basic",
    };
    let scope = req
        .scope
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "openid profile email".into());

    let client_id = format!("bscp_{}", bscp_common::random_hex(16));
    let reg_token = random_token(32);
    let (secret, secret_hash) = if auth_method == "none" {
        (None, None)
    } else {
        let s = random_token(32);
        let h = sha256_hex(&s);
        (Some(s), Some(h))
    };

    sqlx::query(
        "INSERT INTO oauth_clients \
         (client_id, client_secret_hash, name, redirect_uris, scope, token_endpoint_auth_method, \
          registration_access_token_hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&client_id)
    .bind(&secret_hash)
    .bind(req.client_name.clone().unwrap_or_default())
    .bind(serde_json::to_string(&req.redirect_uris).unwrap())
    .bind(&scope)
    .bind(auth_method)
    .bind(sha256_hex(&reg_token))
    .bind(now_ts())
    .execute(&state.pool)
    .await?;

    let mut out = json!({
        "client_id": client_id,
        "client_id_issued_at": now_ts() as i64,
        "redirect_uris": req.redirect_uris,
        "token_endpoint_auth_method": auth_method,
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": scope,
        "registration_access_token": reg_token,
        "registration_client_uri": format!("{}/oauth/register/{}", state.cfg.public_url, client_id),
    });
    if let Some(s) = secret {
        out["client_secret"] = json!(s);
        out["client_secret_expires_at"] = json!(0);
    }
    if let Some(n) = req.client_name {
        out["client_name"] = json!(n);
    }
    Ok(out)
}

pub async fn delete_registered(state: &AppState, client_id: &str, bearer: &str) -> Result<(), ApiError> {
    let Some(c) = load(state, client_id).await? else {
        return Err(ApiError::not_found("unknown client"));
    };
    let ok = c
        .registration_access_token_hash
        .as_deref()
        .map(|h| h == sha256_hex(bearer))
        .unwrap_or(false);
    if !ok {
        return Err(ApiError::forbidden("bad registration access token"));
    }
    sqlx::query("UPDATE oauth_clients SET disabled = 1 WHERE client_id = ?")
        .bind(client_id)
        .execute(&state.pool)
        .await?;
    Ok(())
}
