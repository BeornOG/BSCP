//! Background maintenance tasks.

use crate::media::{load_metadata, metadata_path, save_metadata, METADATA_FILE};
use bscp_common::config::UserServerConfig;
use bscp_common::now_ts;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Periodically drop cached media files older than `CACHE_TIME`. Port of the
/// cleanup thread in `app.py`.
pub fn spawn_cache_cleanup(cfg: Arc<UserServerConfig>) {
    tokio::spawn(async move {
        tracing::info!(cache_time = cfg.cache_time, dir = %cfg.cache_dir.display(), "[CACHE] cleanup task started");
        loop {
            if let Err(e) = cleanup_once(&cfg).await {
                tracing::warn!(error = %e, "[CACHE] cleanup error");
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}

async fn cleanup_once(cfg: &UserServerConfig) -> std::io::Result<()> {
    let dir = &cfg.cache_dir;
    if !dir.exists() {
        return Ok(());
    }
    let meta_path = metadata_path(dir);
    let mut meta = load_metadata(&meta_path);
    let now = now_ts();
    let mut deleted = 0u32;
    let mut scanned = 0u32;

    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == METADATA_FILE {
            continue;
        }
        scanned += 1;

        match meta.get(&name).and_then(|v| v.as_f64()) {
            None => {
                meta.insert(name, Value::from(now));
            }
            Some(created) => {
                if now - created > cfg.cache_time as f64
                    && tokio::fs::remove_file(entry.path()).await.is_ok()
                {
                    meta.remove(&name);
                    deleted += 1;
                }
            }
        }
    }

    meta.retain(|k, _| dir.join(k).exists());
    save_metadata(&meta_path, &meta);
    tracing::debug!(scanned, deleted, "[CACHE] scan complete");
    Ok(())
}
