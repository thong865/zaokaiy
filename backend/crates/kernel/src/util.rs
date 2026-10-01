//! Small helpers shared by several cores.

/// E.164 from what the user typed. The web app sends "+<country><number>"; "00" prefix accepted.
pub fn normalize_phone(raw: &str) -> Option<String> {
    let t = raw.trim();
    let intl = t.starts_with('+') || t.starts_with("00");
    let mut d: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if t.starts_with("00") {
        d = d[2..].to_string();
    }
    if !intl || d.starts_with('0') || !(8..=15).contains(&d.len()) {
        return None;
    }
    Some(format!("+{d}"))
}
