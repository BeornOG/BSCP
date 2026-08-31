//! Federation assertion claims — a short-lived RS256 JWT a user server mints for
//! one of its users so a channel server (or other federated service) can act on
//! that user's behalf without a login. Verified by the recipient against the
//! issuer's JWKS **and** an issuer callback (`POST {iss}/federation/assert/verify`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionClaims {
    /// Issuer — the user server's public URL.
    pub iss: String,
    /// Subject — `user@domain`.
    pub sub: String,
    /// Audience — the recipient service's domain.
    pub aud: String,
    /// Expiry (unix seconds).
    pub exp: i64,
    /// Issued-at (unix seconds).
    pub iat: i64,
    /// Unique token id — the issuer records this so its callback can confirm it.
    pub jti: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
}

/// Result of the issuer callback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionVerdict {
    pub valid: bool,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
}
