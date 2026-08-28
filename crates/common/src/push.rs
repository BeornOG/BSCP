//! Web Push (VAPID / RFC 8291) support. Best-effort: every failure is logged,
//! never propagated. Uses the pure-Rust `web-push-native` for payload encryption
//! and `reqwest` for delivery.

use crate::models::PushSubscription;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;
use web_push_native::jwt_simple::algorithms::ES256KeyPair;
use web_push_native::p256::PublicKey;
use web_push_native::{Auth, WebPushBuilder};

const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

#[derive(Serialize, Deserialize)]
struct VapidKeyFile {
    private_key: String,
    public_key: String,
}

#[derive(Clone)]
pub struct Vapid {
    /// base64url raw 32-byte private scalar.
    private_b64: String,
    /// base64url uncompressed public point (what the browser needs).
    pub public_key: String,
    pub contact: String,
}

impl Vapid {
    fn key_pair(&self) -> Option<ES256KeyPair> {
        let bytes = B64.decode(&self.private_b64).ok()?;
        ES256KeyPair::from_bytes(&bytes).ok()
    }
}

/// Resolve the VAPID keypair: env vars → `vapid_keys.json` → freshly generated
/// (and persisted). Mirrors the intent of the old `app.py` logic.
pub fn load_or_generate(env_private: &str, env_public: &str, contact: &str, keys_file: &Path) -> Vapid {
    if !env_private.is_empty() && !env_public.is_empty() {
        return Vapid {
            private_b64: env_private.to_string(),
            public_key: env_public.to_string(),
            contact: contact.to_string(),
        };
    }
    if let Ok(text) = std::fs::read_to_string(keys_file) {
        if let Ok(k) = serde_json::from_str::<VapidKeyFile>(&text) {
            tracing::info!("[VAPID] loaded persistent keys");
            return Vapid { private_b64: k.private_key, public_key: k.public_key, contact: contact.to_string() };
        }
    }

    let (private_b64, public_key) = generate_keypair();
    let _ = std::fs::write(
        keys_file,
        serde_json::to_string(&VapidKeyFile { private_key: private_b64.clone(), public_key: public_key.clone() })
            .unwrap(),
    );
    tracing::info!("[VAPID] generated and saved new keypair");
    Vapid { private_b64, public_key, contact: contact.to_string() }
}

fn generate_keypair() -> (String, String) {
    use web_push_native::p256::elliptic_curve::sec1::ToEncodedPoint;
    let secret = web_push_native::p256::SecretKey::random(&mut rand::rngs::OsRng);
    let scalar = secret.to_bytes();
    let point = secret.public_key().to_encoded_point(false);
    (B64.encode(scalar), B64.encode(point.as_bytes()))
}

#[derive(Serialize)]
struct PushPayload<'a> {
    title: &'a str,
    body: &'a str,
    url: &'a str,
}

/// Send `title`/`body` to every push subscription belonging to `user_id`.
/// Subscriptions rejected with 404/410 are deleted.
pub async fn send_to_user(
    pool: &SqlitePool,
    client: &reqwest::Client,
    vapid: &Vapid,
    user_id: &str,
    title: &str,
    body: &str,
    url: &str,
) {
    let Some(key_pair) = vapid.key_pair() else {
        tracing::warn!("[PUSH] VAPID key unavailable; skipping");
        return;
    };

    let subs: Vec<PushSubscription> =
        match sqlx::query_as::<_, PushSubscription>("SELECT * FROM push_subscriptions WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(pool)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "[PUSH] could not load subscriptions");
                return;
            }
        };

    let payload = serde_json::to_vec(&PushPayload { title, body, url }).unwrap_or_default();

    for sub in subs {
        match deliver(client, &key_pair, &vapid.contact, &sub, &payload).await {
            Ok(()) => {}
            Err(status) => {
                tracing::warn!(endpoint = %sub.endpoint, status, "[PUSH] delivery failed");
                if status == 404 || status == 410 {
                    let _ = sqlx::query("DELETE FROM push_subscriptions WHERE id = ?")
                        .bind(&sub.id)
                        .execute(pool)
                        .await;
                }
            }
        }
    }
}

async fn deliver(
    client: &reqwest::Client,
    key_pair: &ES256KeyPair,
    contact: &str,
    sub: &PushSubscription,
    payload: &[u8],
) -> Result<(), u16> {
    let p256dh = B64.decode(&sub.p256dh).map_err(|_| 400u16)?;
    let auth = B64.decode(&sub.auth).map_err(|_| 400u16)?;
    if auth.len() != 16 {
        return Err(400);
    }
    let ua_public = PublicKey::from_sec1_bytes(&p256dh).map_err(|_| 400u16)?;

    let mut auth_arr = Auth::default();
    auth_arr.copy_from_slice(&auth);
    let builder = WebPushBuilder::new(sub.endpoint.parse().map_err(|_| 400u16)?, ua_public, auth_arr)
        .with_vapid(key_pair, contact);
    let request = builder.build(payload.to_vec()).map_err(|_| 500u16)?;

    let (parts, body) = request.into_parts();
    let mut rb = client.post(parts.uri.to_string());
    for (name, value) in parts.headers.iter() {
        rb = rb.header(name, value);
    }
    match rb.body(body).send().await {
        Ok(resp) => {
            let code = resp.status().as_u16();
            if (200..300).contains(&code) {
                Ok(())
            } else {
                Err(code)
            }
        }
        Err(_) => Err(0),
    }
}
