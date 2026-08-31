use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use bscp_common::call::CallManager;
use bscp_common::discovery::Discovery;
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub domain: String,
    pub public_url: String,
    pub discovery: Arc<Discovery>,
    pub cookie_key: Key,
    pub calls: Arc<CallManager>,
    /// issuer origin → (JWKS json, fetched-at)
    pub jwks: Arc<Mutex<HashMap<String, (Value, Instant)>>>,
    /// assertion `jti` → (sub, name, picture, exp) once callback-verified
    pub verified: Arc<Mutex<HashMap<String, VerifiedAssertion>>>,
}

#[derive(Clone)]
pub struct VerifiedAssertion {
    pub sub: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub exp: f64,
}

impl AppState {
    pub fn new(pool: SqlitePool, cfg: &bscp_common::config::ChannelServerConfig) -> Self {
        Self {
            pool,
            domain: cfg.domain.clone(),
            public_url: cfg.public_url.clone(),
            discovery: Arc::new(Discovery::new()),
            cookie_key: derive_key(&cfg.secret_key),
            calls: Arc::new(CallManager::new(cfg.domain.clone())),
            jwks: Arc::new(Mutex::new(HashMap::new())),
            verified: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(s: &AppState) -> Self {
        s.cookie_key.clone()
    }
}

fn derive_key(secret: &str) -> Key {
    use sha2::{Digest, Sha512};
    Key::from(&Sha512::digest(secret.as_bytes()))
}
