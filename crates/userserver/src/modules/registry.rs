//! Module registration / manifest fetch + admin CRUD. (Filled in in step 4.)

use crate::modules::ModuleManifest;
use crate::state::AppState;
use bscp_common::ApiError;
use std::time::Duration;

/// Fetch and parse a module's manifest from `{base_url}/.well-known/bscp-module`.
pub async fn fetch_manifest(state: &AppState, base_url: &str) -> Result<ModuleManifest, ApiError> {
    let url = format!("{}/.well-known/bscp-module", base_url.trim_end_matches('/'));
    let resp = state
        .modules
        .client()
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|_| ApiError::bad_gateway("could not reach the module"))?;
    if !resp.status().is_success() {
        return Err(ApiError::bad_gateway("module did not serve a manifest"));
    }
    resp.json::<ModuleManifest>().await.map_err(|_| ApiError::bad_request("invalid module manifest"))
}
