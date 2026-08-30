//! Out-of-process modules: separate HTTP services that receive signed event
//! webhooks and can mediate external-account linking. No routes into the user
//! server, no SPA injection.

pub mod links;
pub mod registry;

use crate::state::AppState;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use std::sync::RwLock;
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Deserialize)]
pub struct ModuleManifest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub link_providers: Vec<LinkProvider>,
    #[serde(default)]
    pub admin_url: Option<String>,
}

#[derive(Clone, Deserialize, serde::Serialize)]
pub struct LinkProvider {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

#[derive(Clone)]
pub struct Module {
    pub name: String,
    pub base_url: String,
    pub secret: String,
    pub manifest: ModuleManifest,
    pub enabled: bool,
}

pub struct ModuleBus {
    client: reqwest::Client,
    cache: RwLock<Vec<Module>>,
}

pub fn hmac_hex(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(body);
    mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

impl ModuleBus {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder().timeout(Duration::from_secs(5)).build().expect("client"),
            cache: RwLock::new(Vec::new()),
        }
    }

    /// Reload the in-memory module list from the DB.
    pub async fn reload(&self, pool: &sqlx::SqlitePool) {
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(
            "SELECT name, base_url, secret, manifest, enabled FROM modules",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mods = rows
            .into_iter()
            .map(|(name, base_url, secret, manifest, enabled)| Module {
                name,
                base_url,
                secret,
                manifest: manifest
                    .and_then(|m| serde_json::from_str(&m).ok())
                    .unwrap_or_else(|| serde_json::from_str("{}").unwrap()),
                enabled: enabled != 0,
            })
            .collect();
        *self.cache.write().unwrap() = mods;
    }

    pub fn enabled(&self) -> Vec<Module> {
        self.cache.read().unwrap().iter().filter(|m| m.enabled).cloned().collect()
    }

    pub fn get(&self, name: &str) -> Option<Module> {
        self.cache.read().unwrap().iter().find(|m| m.name == name).cloned()
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Fire an event to every enabled module that subscribed to it. Best-effort.
    pub fn dispatch(self: &std::sync::Arc<Self>, event: &str, data: Value) {
        let targets: Vec<Module> = self
            .enabled()
            .into_iter()
            .filter(|m| m.manifest.events.iter().any(|e| e == event))
            .collect();
        if targets.is_empty() {
            return;
        }
        let event = event.to_string();
        let body = serde_json::to_vec(&json!({
            "id": bscp_common::uuid(),
            "type": event,
            "ts": bscp_common::now_ts(),
            "data": data,
        }))
        .unwrap_or_default();

        let bus = self.clone();
        tokio::spawn(async move {
            for m in targets {
                let url = format!("{}/events", m.base_url.trim_end_matches('/'));
                let sig = format!("sha256={}", hmac_hex(&m.secret, &body));
                for attempt in 0..3u32 {
                    let res = bus
                        .client
                        .post(&url)
                        .header("x-bscp-module", &m.name)
                        .header("x-bscp-signature", &sig)
                        .header("content-type", "application/json")
                        .body(body.clone())
                        .send()
                        .await;
                    match res {
                        Ok(r) if r.status().is_success() => break,
                        _ if attempt < 2 => {
                            tokio::time::sleep(Duration::from_millis(300 * (attempt as u64 + 1))).await;
                        }
                        _ => tracing::warn!(module = %m.name, event, "module event delivery failed"),
                    }
                }
            }
        });
    }
}

impl Default for ModuleBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience for event data payloads.
pub fn dispatch(state: &AppState, event: &str, data: Value) {
    state.modules.dispatch(event, data);
}
