//! Server discovery via `/.well-known/BSCP/<type>.json`, port of `json_discovery.py`.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(60);
/// First back-off after a failed lookup; doubles on each consecutive failure.
const BACKOFF_BASE: Duration = Duration::from_secs(15);
/// Ceiling for the back-off so a long-dead server is still retried occasionally.
const BACKOFF_MAX: Duration = Duration::from_secs(300);

type Key = (String, String);

struct Failure {
    /// Skip network lookups for this key until this instant.
    until: Instant,
    /// Consecutive failure count, used to grow the back-off.
    strikes: u32,
}

pub struct Discovery {
    client: reqwest::Client,
    cache: Mutex<HashMap<Key, (Value, Instant)>>,
    /// Negative cache: keys whose last lookup failed, with an expiring back-off
    /// so an unreachable peer doesn't get hit (and logged) on every poll.
    failures: Mutex<HashMap<Key, Failure>>,
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
        Self {
            client,
            cache: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
        }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Record a failed lookup and return the back-off that now applies.
    fn note_failure(&self, key: &Key, reason: &str) {
        let mut failures = self.failures.lock().unwrap();
        let entry = failures.entry(key.clone()).or_insert(Failure {
            until: Instant::now(),
            strikes: 0,
        });
        entry.strikes = entry.strikes.saturating_add(1);
        let backoff = BACKOFF_BASE
            .saturating_mul(1u32 << entry.strikes.min(5).saturating_sub(1))
            .min(BACKOFF_MAX);
        entry.until = Instant::now() + backoff;
        // Only log the first strike so a persistently-down peer stays quiet.
        if entry.strikes == 1 {
            tracing::warn!(
                domain = %key.0,
                server_type = %key.1,
                %reason,
                "discovery lookup failed; backing off {backoff:?} before retrying"
            );
        }
    }

    /// Fetch and cache the discovery document for `domain` / `server_type`.
    pub async fn discover(&self, domain: &str, server_type: &str) -> Option<Value> {
        let key = (domain.to_string(), server_type.to_string());
        if let Some((cfg, ts)) = self.cache.lock().unwrap().get(&key) {
            if ts.elapsed() < CACHE_TTL {
                return Some(cfg.clone());
            }
        }

        // Still inside the back-off window from a previous failure: don't touch
        // the network (and don't log) — just report the peer as undiscoverable.
        if let Some(f) = self.failures.lock().unwrap().get(&key) {
            if Instant::now() < f.until {
                return None;
            }
        }

        let url = format!("http://{domain}/.well-known/BSCP/{server_type}.json");
        let resp = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                self.note_failure(&key, &e.to_string());
                return None;
            }
        };
        if !resp.status().is_success() {
            self.note_failure(&key, &format!("HTTP {}", resp.status()));
            return None;
        }
        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                self.note_failure(&key, &e.to_string());
                return None;
            }
        };
        let cfg: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                self.note_failure(&key, &e.to_string());
                return None;
            }
        };
        if cfg.is_null() {
            self.note_failure(&key, "discovery document is null");
            return None;
        }

        self.failures.lock().unwrap().remove(&key);
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
