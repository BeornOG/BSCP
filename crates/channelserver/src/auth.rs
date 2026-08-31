//! Request auth for the channel server.
//!
//! * `Assertion` — a federated user, proven by an RS256 assertion from their home
//!   user server: JWKS-verified **and** confirmed by an issuer callback.
//! * `Operator` — the channel-server operator, via the console session cookie.

use crate::state::{AppState, VerifiedAssertion};
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::PrivateCookieJar;
use bscp_common::assertion::{AssertionClaims, AssertionVerdict};
use bscp_common::now_ts;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use std::time::{Duration, Instant};

const JWKS_TTL: Duration = Duration::from_secs(60);
pub const OP_COOKIE: &str = "bscp_operator";

pub struct AuthError(pub StatusCode, pub &'static str);
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}
fn err(code: StatusCode, msg: &'static str) -> AuthError {
    AuthError(code, msg)
}

// ── Assertion ─────────────────────────────────────────────────────────

pub struct Assertion {
    pub sub: String,
    #[allow(dead_code)]
    pub name: Option<String>,
    #[allow(dead_code)]
    pub picture: Option<String>,
}

#[async_trait]
impl FromRequestParts<AppState> for Assertion {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "missing assertion"))?
            .to_string();

        verify_assertion(state, &token).await.map(|v| Assertion {
            sub: v.sub,
            name: v.name,
            picture: v.picture,
        })
    }
}

/// Full two-way verification of an assertion token. Cached by `jti` until `exp`.
pub async fn verify_assertion(state: &AppState, token: &str) -> Result<VerifiedAssertion, AuthError> {
    // read (unverified) claims to get iss / jti / aud
    let header = jsonwebtoken::decode_header(token).map_err(|_| err(StatusCode::UNAUTHORIZED, "bad token"))?;
    let kid = header.kid.ok_or_else(|| err(StatusCode::UNAUTHORIZED, "no kid"))?;
    let unverified: AssertionClaims = {
        let mut v = Validation::new(Algorithm::RS256);
        v.insecure_disable_signature_validation();
        v.validate_aud = false;
        v.set_required_spec_claims(&["exp"]);
        jsonwebtoken::decode(token, &DecodingKey::from_secret(b"x"), &v)
            .map_err(|_| err(StatusCode::UNAUTHORIZED, "bad claims"))?
            .claims
    };

    if unverified.aud != state.domain {
        return Err(err(StatusCode::UNAUTHORIZED, "wrong audience"));
    }
    if (unverified.exp as f64) <= now_ts() {
        return Err(err(StatusCode::UNAUTHORIZED, "expired"));
    }

    // cache hit?
    if let Some(v) = state.verified.lock().unwrap().get(&unverified.jti) {
        if v.exp > now_ts() {
            return Ok(v.clone());
        }
    }

    // 1. signature against issuer JWKS
    let jwks = fetch_jwks(state, &unverified.iss).await?;
    let jwk = jwks
        .get("keys")
        .and_then(|k| k.as_array())
        .and_then(|arr| arr.iter().find(|k| k.get("kid").and_then(|x| x.as_str()) == Some(&kid)))
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "unknown key"))?;
    let (n, e) = (
        jwk.get("n").and_then(|x| x.as_str()).ok_or_else(|| err(StatusCode::UNAUTHORIZED, "bad jwk"))?,
        jwk.get("e").and_then(|x| x.as_str()).ok_or_else(|| err(StatusCode::UNAUTHORIZED, "bad jwk"))?,
    );
    let key = DecodingKey::from_rsa_components(n, e).map_err(|_| err(StatusCode::UNAUTHORIZED, "bad jwk"))?;
    let mut val = Validation::new(Algorithm::RS256);
    val.set_audience(&[&state.domain]);
    val.set_required_spec_claims(&["exp"]);
    let claims: AssertionClaims = jsonwebtoken::decode(token, &key, &val)
        .map_err(|_| err(StatusCode::UNAUTHORIZED, "signature check failed"))?
        .claims;

    // 2. issuer callback
    let verdict: AssertionVerdict = state
        .discovery
        .client()
        .post(format!("{}/federation/assert/verify", claims.iss.trim_end_matches('/')))
        .json(&serde_json::json!({ "token": token }))
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .map_err(|_| err(StatusCode::BAD_GATEWAY, "issuer unreachable"))?
        .json()
        .await
        .map_err(|_| err(StatusCode::BAD_GATEWAY, "issuer bad response"))?;
    if !verdict.valid {
        return Err(err(StatusCode::UNAUTHORIZED, "issuer rejected assertion"));
    }

    let v = VerifiedAssertion {
        sub: claims.sub,
        name: verdict.name.or(claims.name),
        picture: verdict.picture.or(claims.picture),
        exp: claims.exp as f64,
    };
    state.verified.lock().unwrap().insert(claims.jti, v.clone());
    Ok(v)
}

async fn fetch_jwks(state: &AppState, iss: &str) -> Result<serde_json::Value, AuthError> {
    let iss = iss.trim_end_matches('/').to_string();
    if let Some((j, at)) = state.jwks.lock().unwrap().get(&iss) {
        if at.elapsed() < JWKS_TTL {
            return Ok(j.clone());
        }
    }
    let jwks: serde_json::Value = state
        .discovery
        .client()
        .get(format!("{iss}/oauth/jwks"))
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .map_err(|_| err(StatusCode::BAD_GATEWAY, "jwks unreachable"))?
        .json()
        .await
        .map_err(|_| err(StatusCode::BAD_GATEWAY, "jwks bad response"))?;
    state.jwks.lock().unwrap().insert(iss, (jwks.clone(), Instant::now()));
    Ok(jwks)
}

// ── Operator ──────────────────────────────────────────────────────────

pub struct Operator {
    pub sub: String,
}

#[async_trait]
impl FromRequestParts<AppState> for Operator {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let jar = PrivateCookieJar::from_headers(&parts.headers, state.cookie_key.clone());
        let sid = jar.get(OP_COOKIE).map(|c| c.value().to_string());
        let Some(sid) = sid else {
            return Err(err(StatusCode::UNAUTHORIZED, "operator sign-in required"));
        };
        let row: Option<(String, f64)> =
            sqlx::query_as("SELECT sub, expires_at FROM operator_sessions WHERE id = ?")
                .bind(&sid)
                .fetch_optional(&state.pool)
                .await
                .ok()
                .flatten();
        let Some((sub, exp)) = row else {
            return Err(err(StatusCode::UNAUTHORIZED, "operator sign-in required"));
        };
        if exp <= now_ts() {
            return Err(err(StatusCode::UNAUTHORIZED, "session expired"));
        }
        let operator_sub: Option<String> =
            sqlx::query_scalar::<_, Option<String>>("SELECT operator_sub FROM operator_config WHERE id = 1")
                .fetch_optional(&state.pool)
                .await
                .ok()
                .flatten()
                .flatten();
        if operator_sub.as_deref() != Some(sub.as_str()) {
            return Err(err(StatusCode::FORBIDDEN, "not the operator"));
        }
        Ok(Operator { sub })
    }
}

// ── permission guard ─────────────────────────────────────────────────

pub async fn guard(
    state: &AppState,
    user: &str,
    guild_id: &str,
    channel_id: Option<&str>,
    need: u64,
) -> Result<u64, AuthError> {
    let perms = crate::perms::effective(state, guild_id, user, channel_id).await;
    if crate::perms::has(perms, need) {
        Ok(perms)
    } else {
        Err(err(StatusCode::FORBIDDEN, "insufficient permissions"))
    }
}
