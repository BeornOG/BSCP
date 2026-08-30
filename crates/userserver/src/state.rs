use crate::call::CallState;
use crate::modules::ModuleBus;
use crate::oidc::OidcKeys;
use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use bscp_common::config::UserServerConfig;
use bscp_common::discovery::Discovery;
use bscp_common::push::Vapid;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<UserServerConfig>,
    pub discovery: Arc<Discovery>,
    pub vapid: Arc<Vapid>,
    pub cookie_key: Key,
    pub calls: Arc<CallState>,
    pub oidc: Arc<OidcKeys>,
    pub modules: Arc<ModuleBus>,
}

impl AppState {
    pub fn domain(&self) -> &str {
        &self.cfg.domain
    }
    /// `username@domain`
    pub fn full_id(&self, username: &str) -> String {
        format!("{}@{}", username, self.cfg.domain)
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

/// Derive a 64-byte cookie signing key from the configured secret.
pub fn derive_cookie_key(secret: &str) -> Key {
    use sha2::{Digest, Sha512};
    let digest = Sha512::digest(secret.as_bytes());
    Key::from(&digest)
}
