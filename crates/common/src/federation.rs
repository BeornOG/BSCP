//! Federation HTTP helpers (port of `federation.py` + the outbound calls in
//! `routes/chats.py`).

use crate::discovery::Discovery;
use serde_json::{json, Value};
use std::time::Duration;

/// Outbound DM/channel message payload.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FedMessage {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub text: String,
    #[serde(rename = "validationKey")]
    pub validation_key: String,
    /// `text` (default) or a special kind such as `call_invite` / `call_end`.
    #[serde(default = "default_kind", skip_serializing_if = "is_text_kind")]
    pub kind: String,
    /// JSON string with kind-specific fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
}

fn default_kind() -> String {
    "text".to_string()
}
fn is_text_kind(k: &str) -> bool {
    k == "text"
}

impl FedMessage {
    pub fn text(id: String, sender: String, receiver: String, text: String, validation_key: String) -> Self {
        Self { id, sender, receiver, text, validation_key, kind: "text".into(), metadata: None }
    }
}

/// Ask the sender's origin server to confirm a message really originated there.
pub async fn validate_remote(
    disc: &Discovery,
    sender_domain: &str,
    message_id: &str,
    validation_key: &str,
    sender: &str,
    receiver: &str,
) -> bool {
    let url = disc
        .get_endpoint(sender_domain, "userserver", "federation_validate")
        .await
        .unwrap_or_else(|| format!("http://{sender_domain}/federation/validate"));

    let resp = disc
        .client()
        .get(&url)
        .query(&[
            ("messageId", message_id),
            ("validationKey", validation_key),
            ("sender", sender),
            ("receiver", receiver),
        ])
        .timeout(Duration::from_secs(3))
        .send()
        .await;

    match resp {
        Ok(r) => r
            .json::<Value>()
            .await
            .ok()
            .and_then(|v| v.get("valid").and_then(|b| b.as_bool()))
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Best-effort deliver a DM to a remote user server.
pub async fn deliver_dm(disc: &Discovery, receiver_domain: &str, payload: &FedMessage) {
    let url = disc
        .get_endpoint(receiver_domain, "userserver", "federation_receive")
        .await
        .unwrap_or_else(|| format!("http://{receiver_domain}/federation/receive"));
    let _ = disc
        .client()
        .post(&url)
        .json(payload)
        .timeout(Duration::from_secs(5))
        .send()
        .await;
}

/// Best-effort deliver a message to a channel server.
pub async fn deliver_channel(disc: &Discovery, target_domain: &str, payload: &FedMessage) {
    let url = disc
        .get_endpoint(target_domain, "channelserver", "channel_send")
        .await
        .unwrap_or_else(|| format!("http://{target_domain}/api/channel/send"));
    let _ = disc
        .client()
        .post(&url)
        .json(payload)
        .timeout(Duration::from_secs(3))
        .send()
        .await;
}

/// Poll a remote channel server for messages. Returns `[]` on any failure.
pub async fn poll_channel(
    disc: &Discovery,
    target_domain: &str,
    path: &str,
    limit: i64,
    since: Option<f64>,
    before: Option<f64>,
) -> Value {
    let url = disc
        .get_endpoint(target_domain, "channelserver", "channel_poll")
        .await
        .unwrap_or_else(|| format!("http://{target_domain}/api/channel/poll"));

    let mut q: Vec<(String, String)> = vec![("path".into(), path.into()), ("limit".into(), limit.to_string())];
    if let Some(s) = since {
        q.push(("since".into(), s.to_string()));
    }
    if let Some(b) = before {
        q.push(("before".into(), b.to_string()));
    }

    match disc.client().get(&url).query(&q).timeout(Duration::from_secs(5)).send().await {
        Ok(r) => r.json::<Value>().await.unwrap_or_else(|_| json!([])),
        Err(_) => json!([]),
    }
}

/// Fetch a remote user profile. `Ok(None)` = not found, `Err(())` = unreachable.
pub async fn fetch_remote_profile(
    disc: &Discovery,
    domain: &str,
    full_id: &str,
) -> Result<Option<Value>, ()> {
    let base = disc
        .get_endpoint(domain, "userserver", "users")
        .await
        .unwrap_or_else(|| format!("http://{domain}/api/users"));

    match disc
        .client()
        .get(format!("{base}/{full_id}"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => {
            let mut profile = r.json::<Value>().await.map_err(|_| ())?;
            profile["is_admin"] = json!(false);
            Ok(Some(profile))
        }
        Ok(_) => Ok(None),
        Err(_) => Err(()),
    }
}
