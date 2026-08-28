//! Server discovery via `/.well-known/BSCP/<type>.json`, port of `json_discovery.py`.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(60);

pub struct Discovery {
    client: reqwest::Client,
    cache: Mutex<HashMap<(String, String), (Value, Instant)>>,
}

impl Default for Discovery {
    fn default() -> Self {
        Self::new()
    }
}

impl Discovery {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client");
        Self { client, cache: Mutex::new(HashMap::new()) }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Fetch and cache the discovery document for `domain` / `server_type`.
    pub async fn discover(&self, domain: &str, server_type: &str) -> Option<Value> {
        let key = (domain.to_string(), server_type.to_string());
        if let Some((cfg, ts)) = self.cache.lock().unwrap().get(&key) {
            if ts.elapsed() < CACHE_TTL {
                return Some(cfg.clone());
            }
        }

        let url = format!("http://{domain}/.well-known/BSCP/{server_type}.json");
        let resp = self.client.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let text = resp.text().await.ok()?;
        let cfg: Value = serde_json::from_str(&text).ok()?;
        if cfg.is_null() {
            return None;
        }
        self.cache.lock().unwrap().insert(key, (cfg.clone(), Instant::now()));
        Some(cfg)
    }

    /// Resolve a named endpoint to a full URL, mirroring `get_endpoint`.
    pub async fn get_endpoint(&self, domain: &str, server_type: &str, endpoint_name: &str) -> Option<String> {
        let cfg = self.discover(domain, server_type).await?;
        let api = cfg.get("api")?;
        let path = api.get("endpoints")?.get(endpoint_name)?.as_str()?;
        let base = api
            .get("base")
            .and_then(|b| b.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("http://{domain}"));

        let mut base = base;
        let mut path = path.to_string();
        if base.ends_with('/') && path.starts_with('/') {
            path.remove(0);
        } else if !base.ends_with('/') && !path.starts_with('/') {
            base.push('/');
        }
        Some(format!("{base}{path}").trim_end_matches('/').to_string())
    }
}
