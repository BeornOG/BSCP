//! Media proxy + shared cache-metadata helpers (`/media/proxy`).

use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use bscp_common::now_ts;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const METADATA_FILE: &str = ".cache_metadata.json";

pub fn metadata_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join(METADATA_FILE)
}

pub fn load_metadata(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Map<String, Value>>(&s).ok())
        .unwrap_or_default()
}

pub fn save_metadata(path: &Path, meta: &Map<String, Value>) {
    if let Ok(s) = serde_json::to_string(meta) {
        let _ = std::fs::write(path, s);
    }
}

fn guess_mime(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    mime_guess::from_path(path).first_raw().unwrap_or("image/jpeg").to_string()
}

fn md5_hex(input: &str) -> String {
    use md5::{Digest, Md5};
    let digest = Md5::digest(input.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Deserialize)]
pub struct ProxyQuery {
    url: Option<String>,
}

pub async fn proxy(State(state): State<AppState>, Query(q): Query<ProxyQuery>) -> Response {
    let Some(url) = q.url.filter(|u| !u.is_empty()) else {
        return (StatusCode::BAD_REQUEST, "Missing URL").into_response();
    };

    let hash = md5_hex(&url);
    let file_path = state.cfg.cache_dir.join(&hash);
    let meta_path = metadata_path(&state.cfg.cache_dir);
    let mime = guess_mime(&url);

    if file_path.exists() {
        let meta = load_metadata(&meta_path);
        if let Some(created) = meta.get(&hash).and_then(|v| v.as_f64()) {
            if now_ts() - created < state.cfg.cache_time as f64 {
                if let Ok(bytes) = tokio::fs::read(&file_path).await {
                    return ([(header::CONTENT_TYPE, mime)], bytes).into_response();
                }
            }
        }
    }

    let resp = state
        .discovery
        .client()
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let bytes = match r.bytes().await {
                Ok(b) => b,
                Err(_) => return (StatusCode::BAD_GATEWAY, "Failed to fetch image").into_response(),
            };
            let _ = tokio::fs::write(&file_path, &bytes).await;
            let mut meta = load_metadata(&meta_path);
            meta.insert(hash, Value::from(now_ts()));
            save_metadata(&meta_path, &meta);
            ([(header::CONTENT_TYPE, mime)], bytes.to_vec()).into_response()
        }
        _ => (StatusCode::BAD_GATEWAY, "Failed to fetch image").into_response(),
    }
}
