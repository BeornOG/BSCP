//! OpenID Connect provider. This server is its own issuer and only ever
//! authenticates its **local** users (via the session cookie); cross-domain
//! identity is the relying party's job.

pub mod clients;
pub mod consent;
pub mod keys;
pub mod tokens;

pub use keys::OidcKeys;

use sha2::{Digest, Sha256};

pub(crate) fn sha256_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// True for hosts we allow over plain `http` (dev / loopback).
pub(crate) fn is_local_host(host: &str) -> bool {
    host.starts_with("localhost")
        || host.starts_with("127.")
        || host.starts_with("[::1]")
        || host == "0.0.0.0"
}

/// Standard OIDC scopes this provider understands.
pub(crate) fn known_scopes() -> &'static [&'static str] {
    &["openid", "profile", "email", "bscp:links"]
}

pub(crate) fn scope_has(scope: &str, want: &str) -> bool {
    scope.split_whitespace().any(|s| s == want)
}
