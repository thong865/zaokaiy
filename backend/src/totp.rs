//! Time-based one-time passwords (RFC 6238: HMAC-SHA1, 30-second steps, 6 digits) — the codes
//! shown by Google Authenticator, Microsoft Authenticator, 1Password, Authy, etc.

use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;

pub const STEP_SECS: i64 = 30;
/// Accept the previous and next step too, for clock drift and slow typing.
const WINDOW: i64 = 1;

/// 160-bit secret, the size RFC 4226 recommends.
pub fn generate_secret() -> Vec<u8> {
    let mut b = vec![0u8; 20];
    rand::thread_rng().fill_bytes(&mut b);
    b
}

/// RFC 4648 base32 without padding (the format authenticator apps expect).
pub fn base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

fn hotp(secret: &[u8], counter: u64) -> u32 {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(&counter.to_be_bytes());
    let h = mac.finalize().into_bytes();
    let off = (h[19] & 0x0f) as usize;
    let bin = u32::from_be_bytes([h[off] & 0x7f, h[off + 1], h[off + 2], h[off + 3]]);
    bin % 1_000_000
}

/// The step a valid 6-digit `code` belongs to, if it matches around `now` and is newer than
/// `last_step` (a code can be used once).
pub fn verify(secret: &[u8], code: &str, now_unix: i64, last_step: Option<i64>) -> Option<i64> {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let want: u32 = code.parse().ok()?;
    let current = now_unix.div_euclid(STEP_SECS);
    let mut found = None;
    // Check every step in the window (no early exit) so timing doesn't reveal which matched.
    for step in (current - WINDOW)..=(current + WINDOW) {
        if step >= 0 && hotp(secret, step as u64) == want && last_step.is_none_or(|l| step > l) {
            found = Some(step);
        }
    }
    found
}

fn pct(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'@' | b'+' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `otpauth://` URI for the QR code.
pub fn otpauth_uri(issuer: &str, account: &str, secret_b32: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={secret_b32}&issuer={}&algorithm=SHA1&digits=6&period={STEP_SECS}",
        pct(issuer),
        pct(account),
        pct(issuer)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const RFC_SECRET: &[u8] = b"12345678901234567890";

    #[test]
    fn rfc6238_vectors() {
        // RFC 6238 appendix B (SHA1), last 6 of the 8-digit values.
        assert_eq!(hotp(RFC_SECRET, 59 / 30), 287082);
        assert_eq!(hotp(RFC_SECRET, 1111111109 / 30), 81804);
        assert_eq!(hotp(RFC_SECRET, 1234567890 / 30), 5924);
        assert_eq!(hotp(RFC_SECRET, 2000000000 / 30), 279037);
    }

    #[test]
    fn verify_window_and_replay() {
        let t = 1111111109;
        assert_eq!(verify(RFC_SECRET, "081804", t, None), Some(t / 30));
        assert_eq!(verify(RFC_SECRET, "081804", t + 30, None), Some(t / 30), "previous step accepted");
        assert_eq!(verify(RFC_SECRET, "081804", t + 90, None), None, "too old");
        assert_eq!(verify(RFC_SECRET, "081804", t, Some(t / 30)), None, "replay rejected");
        assert_eq!(verify(RFC_SECRET, "81804", t, None), None);
        assert_eq!(verify(RFC_SECRET, "08180a", t, None), None);
    }

    #[test]
    fn base32_encoding() {
        assert_eq!(base32(b""), "");
        assert_eq!(base32(b"f"), "MY");
        assert_eq!(base32(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32(RFC_SECRET), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
    }

    #[test]
    fn uri() {
        assert_eq!(
            otpauth_uri("zaokaiy", "a b@x.com", "ABC"),
            "otpauth://totp/zaokaiy:a%20b@x.com?secret=ABC&issuer=zaokaiy&algorithm=SHA1&digits=6&period=30"
        );
    }
}
