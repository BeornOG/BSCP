//! Constrained reverse proxy: browser → home user server → channel server.
//! The user only ever talks to us; we attach a federation assertion and forward.

use crate::state::AppState;
use bscp_common::models::User;
use serde_json::Value;

pub struct GwResponse {
    pub status: u16,
    pub body: Value,
}

/// Resolve a channel server's API base URL (must be a discoverable channel server).
pub async fn channel_base(state: &AppState, cs: &str) -> Option<String> {
    let cfg = state.discovery.discover(cs, "channelserver").await?;
    cfg.get("api")
        .and_then(|a| a.get("base"))
        .and_then(|b| b.as_str())
        .map(|s| s.trim_end_matches('/').to_string())
        .or_else(|| Some(format!("http://{cs}")))
}

/// Forward `method {cs}/api/{path}?{query}` on behalf of `user`.
pub async fn forward(
    state: &AppState,
    user: &User,
    cs: &str,
    method: reqwest::Method,
    path: &str,
    query: Option<&str>,
    body: Option<Value>,
) -> anyhow::Result<GwResponse> {
    if path.contains("..") {
        anyhow::bail!("bad path");
    }
    let base = channel_base(state, cs)
        .await
        .ok_or_else(|| anyhow::anyhow!("not a known channel server"))?;
    let assertion = super::assert::assertion_for(state, user, cs).await?;

    let mut url = format!("{base}/api/{}", path.trim_start_matches('/'));
    if let Some(q) = query.filter(|q| !q.is_empty()) {
        url.push('?');
        url.push_str(q);
    }

    let mut req = state.discovery.client().request(method, &url).bearer_auth(assertion);
    if let Some(b) = body {
        req = req.json(&b);
    }
    let resp = req.timeout(std::time::Duration::from_secs(10)).send().await?;
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();
    let json = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok(GwResponse { status, body: json })
}
