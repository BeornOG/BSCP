//! TOTP (RFC 6238) compatible with `pyotp` defaults: SHA-1, 6 digits, 30s step.

use base32::Alphabet;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha1::Sha1;

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const DIGITS: u32 = 6;
const PERIOD: u64 = 30;

/// 32-char base32 secret, matching `pyotp.random_base32()`.
pub fn random_base32() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect()
}

fn hotp(secret: &[u8], counter: u64) -> Option<u32> {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).ok()?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let bin = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | (digest[offset + 3] as u32);
    Some(bin % 10u32.pow(DIGITS))
}

fn decode_secret(secret_b32: &str) -> Option<Vec<u8>> {
    base32::decode(Alphabet::Rfc4648 { padding: false }, &secret_b32.to_uppercase())
}

/// Verify a code for the current time, allowing `window` steps of clock drift
/// on each side (`window = 0` matches pyotp's default).
pub fn verify(secret_b32: &str, code: &str, window: i64) -> bool {
    let code = code.trim();
    let Some(secret) = decode_secret(secret_b32) else { return false };
    let target: u32 = match code.parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let now = crate::now_ts() as u64;
    let step = now / PERIOD;
    for w in -window..=window {
        let counter = match step.checked_add_signed(w) {
            Some(c) => c,
            None => continue,
        };
        if hotp(&secret, counter) == Some(target) {
            return true;
        }
    }
    false
}

/// Current 6-digit code for a secret (useful for tests and tooling).
pub fn current_code(secret_b32: &str) -> Option<String> {
    let secret = decode_secret(secret_b32)?;
    let step = (crate::now_ts() as u64) / PERIOD;
    hotp(&secret, step).map(|v| format!("{v:06}"))
}

/// `otpauth://` provisioning URI, matching `pyotp`'s output for default params.
pub fn provisioning_uri(secret_b32: &str, account_name: &str, issuer: &str) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    format!(
        "otpauth://totp/{issuer_l}:{acct}?secret={secret}&issuer={issuer_q}",
        issuer_l = enc(issuer),
        acct = enc(account_name),
        secret = secret_b32,
        issuer_q = enc(issuer),
    )
}

/// Render `data` as a QR code PNG and return it base64-encoded (no data URI prefix).
pub fn qr_png_base64(data: &str) -> anyhow::Result<String> {
    use image::{ImageBuffer, Luma};
    let code = qrcode::QrCode::new(data.as_bytes())?;
    let modules = code.to_colors();
    let width = code.width();
    let scale = 8u32;
    let quiet = 4u32;
    let img_size = (width as u32 + quiet * 2) * scale;
    let mut img = ImageBuffer::from_pixel(img_size, img_size, Luma([255u8]));
    for (i, color) in modules.iter().enumerate() {
        if *color == qrcode::Color::Dark {
            let mx = (i % width) as u32 + quiet;
            let my = (i / width) as u32 + quiet;
            for dx in 0..scale {
                for dy in 0..scale {
                    img.put_pixel(mx * scale + dx, my * scale + dy, Luma([0u8]));
                }
            }
        }
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(img).write_to(&mut buf, image::ImageFormat::Png)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_generated_code() {
        // Known vector: secret "JBSWY3DPEHPK3PXP" is base32 for "Hello!\xDE\xAD\xBE\xEF"
        let secret = "JBSWY3DPEHPK3PXP";
        let now = crate::now_ts() as u64;
        let counter = now / PERIOD;
        let bytes = decode_secret(secret).unwrap();
        let code = format!("{:06}", hotp(&bytes, counter).unwrap());
        assert!(verify(secret, &code, 0));
        assert!(!verify(secret, "000000", 0) || code == "000000");
    }
}
