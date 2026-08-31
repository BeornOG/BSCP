//! Per-server RSA signing key for OIDC ID tokens. Auto-generated on first run,
//! persisted next to the other server key files.

use base64::Engine as _;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rsa::pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey, EncodeRsaPublicKey, LineEnding};
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

#[derive(Serialize, Deserialize)]
struct KeyFile {
    kid: String,
    pkcs1_pem: String,
}

pub struct OidcKeys {
    pub kid: String,
    encoding: EncodingKey,
    decoding: DecodingKey,
    jwk_n: String,
    jwk_e: String,
}

impl OidcKeys {
    pub fn load_or_generate(path: &Path) -> anyhow::Result<Self> {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(kf) = serde_json::from_str::<KeyFile>(&text) {
                let key = RsaPrivateKey::from_pkcs1_pem(&kf.pkcs1_pem)?;
                tracing::info!("[OIDC] loaded signing key");
                return Self::from_parts(kf.kid, kf.pkcs1_pem, &key);
            }
        }

        tracing::info!("[OIDC] generating RSA signing key (one-time)");
        let key = RsaPrivateKey::new(&mut rand::thread_rng(), 2048)?;
        let pem = key.to_pkcs1_pem(LineEnding::LF)?.to_string();
        let kid = kid_for(&key);
        let _ = std::fs::write(
            path,
            serde_json::to_string(&KeyFile { kid: kid.clone(), pkcs1_pem: pem.clone() })?,
        );
        Self::from_parts(kid, pem, &key)
    }

    fn from_parts(kid: String, pkcs1_pem: String, key: &RsaPrivateKey) -> anyhow::Result<Self> {
        let encoding = EncodingKey::from_rsa_pem(pkcs1_pem.as_bytes())?;
        let pubk = key.to_public_key();
        let pub_pem = pubk.to_pkcs1_pem(LineEnding::LF)?;
        let decoding = DecodingKey::from_rsa_pem(pub_pem.as_bytes())?;
        Ok(Self {
            kid,
            encoding,
            decoding,
            jwk_n: B64.encode(pubk.n().to_bytes_be()),
            jwk_e: B64.encode(pubk.e().to_bytes_be()),
        })
    }

    pub fn sign<T: Serialize>(&self, claims: &T) -> anyhow::Result<String> {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        Ok(jsonwebtoken::encode(&header, claims, &self.encoding)?)
    }

    /// Verify a token this server signed (RS256 + `exp`); `aud` optionally checked.
    pub fn verify<T: DeserializeOwned>(&self, token: &str, aud: Option<&str>) -> anyhow::Result<T> {
        let mut v = Validation::new(Algorithm::RS256);
        v.set_required_spec_claims(&["exp"]);
        match aud {
            Some(a) => v.set_audience(&[a]),
            None => v.validate_aud = false,
        }
        Ok(jsonwebtoken::decode::<T>(token, &self.decoding, &v)?.claims)
    }

    pub fn jwks(&self) -> Value {
        json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": self.kid,
                "n": self.jwk_n,
                "e": self.jwk_e,
            }]
        })
    }
}

fn kid_for(key: &RsaPrivateKey) -> String {
    use sha2::{Digest, Sha256};
    let n = key.to_public_key().n().to_bytes_be();
    let digest = Sha256::digest(&n);
    hex16(&digest)
}

fn hex16(bytes: &[u8]) -> String {
    bytes.iter().take(8).map(|b| format!("{b:02x}")).collect()
}
