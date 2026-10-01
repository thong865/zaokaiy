//! Customer identity: phone number → E.164 → keyed hash (the only identifier on the ledger).

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// E.164 from what sellers/customers type. Accepts international numbers (`+856 20 5555 1234`,
/// `00856…`), Lao local numbers (`020 5555 1234`, `030 555 1234`), `856 20…` without the plus,
/// and the common `+856 020…` (national 0 kept after the country code).
pub fn normalize(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || " +-().".contains(c)) {
        return None;
    }
    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    let e164 = if t.starts_with('+') || t.starts_with("00") {
        let d = digits.strip_prefix("00").filter(|_| t.starts_with("00")).unwrap_or(&digits).to_string();
        match d.strip_prefix("8560") {
            Some(rest) => format!("+856{rest}"),
            None => format!("+{d}"),
        }
    } else if let Some(rest) = digits.strip_prefix('0') {
        format!("+856{rest}")
    } else if digits.starts_with("856") && (11..=13).contains(&digits.len()) {
        format!("+{}", digits.replacen("8560", "856", 1))
    } else if (digits.starts_with("20") && digits.len() == 10) || (digits.starts_with("30") && digits.len() == 9) {
        format!("+856{digits}")
    } else {
        return None;
    };
    let n = e164.len() - 1;
    (!e164.starts_with("+0") && (8..=15).contains(&n)).then_some(e164)
}

/// `HMAC-SHA256(pepper, e164)` as 64 hex chars.
pub fn customer_key_with(pepper: &[u8], e164: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(pepper).expect("hmac accepts any key length");
    mac.update(b"cod-risk:v1:");
    mac.update(e164.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn customer_key(e164: &str) -> String {
    customer_key_with(&super::cfg().pepper, e164)
}

/// "•••• 1234" for screens that shouldn't show the whole number.
pub fn hint(e164: &str) -> String {
    let tail: String = e164.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("•••• {tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lao_numbers() {
        let want = Some("+8562055551234".to_string());
        for raw in ["+856 20 5555 1234", "+856 020 5555 1234", "00856 20 5555 1234", "020 5555 1234", "020-5555-1234", "8562055551234", "2055551234", " (020) 5555 1234 "] {
            assert_eq!(normalize(raw), want, "{raw}");
        }
        assert_eq!(normalize("030 555 1234").as_deref(), Some("+856305551234"));
        assert_eq!(normalize("+66 81 234 5678").as_deref(), Some("+66812345678"));
        for bad in ["", "abc", "12", "+0123456789", "020 5555 1234 ext 2", "5555"] {
            assert_eq!(normalize(bad), None, "{bad}");
        }
    }

    #[test]
    fn keys_are_stable_and_peppered() {
        let a = customer_key_with(b"pepper-1", "+8562055551234");
        assert_eq!(a.len(), 64);
        assert_eq!(a, customer_key_with(b"pepper-1", "+8562055551234"));
        assert_ne!(a, customer_key_with(b"pepper-2", "+8562055551234"));
        assert_ne!(a, customer_key_with(b"pepper-1", "+8562055551235"));
        assert_eq!(hint("+8562055551234"), "•••• 1234");
    }
}
