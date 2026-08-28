//! Shared building blocks for the BSCP user server and channel server.

pub mod config;
pub mod db;
pub mod discovery;
pub mod error;
pub mod federation;
pub mod models;
pub mod password;
pub mod push;
pub mod totp;

pub use error::{ApiError, ApiResult};

/// Current unix time in fractional seconds (matches Python `datetime.timestamp()`).
pub fn now_ts() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Generate a URL-safe random token of `bytes` bytes of entropy (base64url, no padding).
pub fn random_token(bytes: usize) -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// Generate a random lowercase-hex string of `bytes` bytes (matches `secrets.token_hex`).
pub fn random_hex(bytes: usize) -> String {
    use rand::RngCore;
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// New random UUIDv4 string.
pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
