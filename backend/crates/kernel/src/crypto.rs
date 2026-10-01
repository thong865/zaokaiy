//! Encryption at rest for KYC data (AES-256-GCM).
//!
//! Format: `b"k1" || nonce(12) || ciphertext+tag` for bytes, and `"k1:" + base64(nonce || ct)` for
//! text fields. The key comes from `KYC_ENCRYPTION_KEY` (32 bytes, base64 or hex). The version
//! prefix leaves room for key rotation.

use std::sync::Arc;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::error::AppError;

const TAG: &[u8] = b"k1";

#[derive(Clone)]
pub struct Sealer {
    cipher: Arc<Aes256Gcm>,
}

impl Sealer {
    pub fn new(key: [u8; 32]) -> Self {
        Self { cipher: Arc::new(Aes256Gcm::new(&key.into())) }
    }

    /// Key from `KYC_ENCRYPTION_KEY`; without it (development) a key is derived from the JWT secret.
    pub fn from_config(raw: &str, jwt_secret: &str) -> Self {
        match parse_key(raw) {
            Some(k) => Self::new(k),
            None => {
                if !raw.trim().is_empty() {
                    panic!("KYC_ENCRYPTION_KEY must be 32 bytes, base64 or hex (openssl rand -base64 32)");
                }
                tracing::warn!("KYC_ENCRYPTION_KEY not set — deriving a development key from JWT_SECRET. Set it in production.");
                let k: [u8; 32] = Sha256::digest(format!("zaokaiy-kyc:{jwt_secret}")).into();
                Self::new(k)
            }
        }
    }

    pub fn seal(&self, plain: &[u8]) -> Vec<u8> {
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let ct = self.cipher.encrypt(Nonce::from_slice(&nonce), plain).expect("aes-gcm encrypt");
        let mut out = Vec::with_capacity(TAG.len() + 12 + ct.len());
        out.extend_from_slice(TAG);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        out
    }

    pub fn open(&self, data: &[u8]) -> Result<Vec<u8>, AppError> {
        if data.len() < TAG.len() + 12 + 16 || &data[..TAG.len()] != TAG {
            return Err(AppError::Internal("encrypted data has an unknown format".into()));
        }
        let (nonce, ct) = data[TAG.len()..].split_at(12);
        self.cipher
            .decrypt(Nonce::from_slice(nonce), ct)
            .map_err(|_| AppError::Internal("could not decrypt (wrong KYC_ENCRYPTION_KEY?)".into()))
    }

    /// Encrypt a text field ("" stays "").
    pub fn seal_str(&self, s: &str) -> String {
        if s.is_empty() {
            return String::new();
        }
        let sealed = self.seal(s.as_bytes());
        format!("k1:{}", B64.encode(&sealed[TAG.len()..]))
    }

    pub fn open_str(&self, s: &str) -> Result<String, AppError> {
        if s.is_empty() {
            return Ok(String::new());
        }
        let body = s.strip_prefix("k1:").ok_or_else(|| AppError::Internal("encrypted field has an unknown format".into()))?;
        let mut raw = TAG.to_vec();
        raw.extend(B64.decode(body).map_err(|_| AppError::Internal("encrypted field is corrupt".into()))?);
        String::from_utf8(self.open(&raw)?).map_err(|_| AppError::Internal("encrypted field is corrupt".into()))
    }
}

fn parse_key(raw: &str) -> Option<[u8; 32]> {
    let raw = raw.trim();
    let bytes = if raw.len() == 64 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
        hex::decode(raw).ok()?
    } else {
        B64.decode(raw).ok()?
    };
    bytes.try_into().ok()
}

/// Last 4 characters of an identifier, for display ("•••• 1234").
pub fn last4(s: &str) -> String {
    let c: Vec<char> = s.chars().filter(|c| c.is_alphanumeric()).collect();
    c[c.len().saturating_sub(4)..].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tamper() {
        let s = Sealer::new([7u8; 32]);
        let blob = s.seal(b"passport scan");
        assert_ne!(&blob[2..], b"passport scan");
        assert_eq!(s.open(&blob).unwrap(), b"passport scan");
        let mut bad = blob.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(s.open(&bad).is_err());
        assert!(Sealer::new([8u8; 32]).open(&blob).is_err());
        let f = s.seal_str("010-12-00-12345678");
        assert!(f.starts_with("k1:") && !f.contains("12345678"));
        assert_eq!(s.open_str(&f).unwrap(), "010-12-00-12345678");
        assert_ne!(s.seal_str("x"), s.seal_str("x"), "random nonce");
        assert_eq!(s.seal_str(""), "");
        assert_eq!(last4("P 12-34 5678"), "5678");
        assert_eq!(last4("12"), "12");
    }

    #[test]
    fn keys() {
        assert!(parse_key(&B64.encode([1u8; 32])).is_some());
        assert!(parse_key(&hex::encode([1u8; 32])).is_some());
        assert!(parse_key("short").is_none());
        assert!(parse_key("").is_none());
    }
}
