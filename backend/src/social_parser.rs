//! Parses social comments / chat messages into product claims.
//!
//! Understands the way people actually order in Thai/Lao/SEA live commerce:
//!   "CF A01", "cf a01 x2", "A01 2", "A01=3", "CFA01", "F a01", "เอา A01 2 ชิ้น",
//!   "A01 x2, B03", "a01×2", full-width "ＣＦ Ａ０１", Thai digits "A01 ๒"
//! Questions ("A01 ราคาเท่าไหร่?") are ignored unless a trigger word is present.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub code: String, // upper-case product code as configured
    pub qty: i32,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub claims: Vec<Claim>,
    pub has_trigger: bool,
    pub question: bool,
}

const QUESTION_MARKERS: [&str; 12] = [
    "?", "？", "เท่าไหร่", "เท่าไร", "ราคา", "ไหม", "มั้ย", "how much", "price",
    "ເທົ່າໃດ", "ທໍ່ໃດ", "ລາຄາ", // Lao: how much (formal/colloquial) / price
];
const UNIT_WORDS: [&str; 10] = ["ชิ้น", "ตัว", "อัน", "ชุด", "ຊິ້ນ", "ໂຕ", "ອັນ", "ຊຸດ", "pcs", "pc"];

fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c), // full-width ASCII
            '๐'..='๙' => char::from_u32('0' as u32 + (c as u32 - '๐' as u32)).unwrap_or(c), // Thai digits
            '໐'..='໙' => char::from_u32('0' as u32 + (c as u32 - '໐' as u32)).unwrap_or(c), // Lao digits
            '×' | '✖' | '✕' | '╳' => 'x',
            _ => c,
        })
        .collect::<String>()
        .to_lowercase()
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// Does `text` (normalized) contain `trigger` as a word? ASCII triggers need a left boundary
/// and must not be followed by more letters unless a product code starts right there ("cfa01").
fn trigger_at(text: &[char], i: usize, trig: &[char], codes: &[Vec<char>]) -> bool {
    if i + trig.len() > text.len() || text[i..i + trig.len()] != *trig {
        return false;
    }
    if !trig.iter().all(|c| c.is_ascii()) {
        return true; // Thai/Lao triggers: substring match
    }
    if i > 0 && is_word(text[i - 1]) {
        return false;
    }
    let j = i + trig.len();
    match text.get(j) {
        None => true,
        Some(c) if !c.is_ascii_alphabetic() => true,
        Some(_) => codes.iter().any(|code| text[j..].starts_with(code)),
    }
}

pub fn parse(message: &str, codes: &[String], triggers: &[String], require_trigger: bool, max_qty: i32) -> Parsed {
    let norm = normalize(message);
    let text: Vec<char> = norm.chars().collect();
    let mut codes_lc: Vec<(Vec<char>, String)> = codes
        .iter()
        .filter(|c| !c.trim().is_empty())
        .map(|c| (c.trim().to_lowercase().chars().collect(), c.trim().to_uppercase()))
        .collect();
    codes_lc.sort_by(|a, b| b.0.len().cmp(&a.0.len())); // longest first
    let code_chars: Vec<Vec<char>> = codes_lc.iter().map(|c| c.0.clone()).collect();
    let trig_chars: Vec<Vec<char>> = triggers
        .iter()
        .map(|t| normalize(t.trim()).chars().collect::<Vec<char>>())
        .filter(|t| !t.is_empty())
        .collect();

    // Where do triggers end? (used to allow "cfa01" with no space)
    let mut trigger_ends = vec![false; text.len() + 1];
    let mut has_trigger = false;
    for i in 0..text.len() {
        for t in &trig_chars {
            if trigger_at(&text, i, t, &code_chars) {
                has_trigger = true;
                trigger_ends[i + t.len()] = true;
            }
        }
    }
    let question = QUESTION_MARKERS.iter().any(|q| norm.contains(q));

    let mut claims: Vec<Claim> = Vec::new();
    let mut i = 0;
    'scan: while i < text.len() {
        for (code, upper) in &codes_lc {
            if !text[i..].starts_with(code) {
                continue;
            }
            // Left boundary: start, non-word char, or right after a trigger ("cfa01").
            if i > 0 && is_word(text[i - 1]) && !trigger_ends[i] {
                continue;
            }
            let mut j = i + code.len();
            let mut qty: Option<i32> = None;
            // Right side: optional separator + quantity.
            let mut k = j;
            if matches!(text.get(k), Some(c) if is_word(*c)) {
                // Only "a01x2" style is allowed directly after a code.
                if text.get(k) == Some(&'x') && matches!(text.get(k + 1), Some(c) if c.is_ascii_digit()) {
                    k += 1;
                } else {
                    continue;
                }
            } else {
                while matches!(text.get(k), Some(' ') | Some('\t')) {
                    k += 1;
                }
                if matches!(text.get(k), Some('x') | Some('*') | Some('=') | Some(':') | Some('-') | Some('+')) {
                    // "x" must be followed by a digit to count as a separator
                    let sep_ok = text.get(k) != Some(&'x')
                        || matches!(text.get(k + 1), Some(c) if c.is_ascii_digit() || *c == ' ');
                    if sep_ok {
                        k += 1;
                        while matches!(text.get(k), Some(' ')) {
                            k += 1;
                        }
                    }
                }
            }
            let start = k;
            while matches!(text.get(k), Some(c) if c.is_ascii_digit()) && k - start < 4 {
                k += 1;
            }
            if k > start && !matches!(text.get(k), Some(c) if c.is_ascii_digit()) {
                // Avoid reading the next code's digits or a price like "A01 100 บาท" as qty? keep ≤ 3 digits
                let n: i32 = text[start..k].iter().collect::<String>().parse().unwrap_or(1);
                if (1..=999).contains(&n) {
                    qty = Some(n);
                    j = k;
                    // swallow a unit word ("2 ชิ้น")
                    let rest: String = text[j..].iter().collect();
                    let trimmed = rest.trim_start();
                    for u in UNIT_WORDS {
                        if trimmed.starts_with(u) {
                            j += (rest.len() - trimmed.len()) + u.chars().count(); // leading whitespace is ASCII
                            break;
                        }
                    }
                }
            }
            let qty = qty.unwrap_or(1).clamp(1, max_qty.max(1));
            if let Some(existing) = claims.iter_mut().find(|c| &c.code == upper) {
                existing.qty = (existing.qty + qty).min(max_qty.max(1));
            } else {
                claims.push(Claim { code: upper.clone(), qty });
            }
            i = j.max(i + 1);
            continue 'scan;
        }
        i += 1;
    }

    if (require_trigger && !has_trigger) || (question && !has_trigger) {
        claims.clear();
    }
    Parsed { claims, has_trigger, question }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(msg: &str) -> Vec<(String, i32)> {
        let codes = vec!["A01".into(), "A02".into(), "B03".into(), "A010".into(), "M1".into()];
        let triggers: Vec<String> = ["cf", "f", "เอา", "รับ", "สั่ง", "order", "ເອົາ", "ຮັບ", "ສັ່ງ", "ຈອງ"].iter().map(|s| s.to_string()).collect();
        parse(msg, &codes, &triggers, false, 10).claims.into_iter().map(|c| (c.code, c.qty)).collect()
    }
    fn v(items: &[(&str, i32)]) -> Vec<(String, i32)> {
        items.iter().map(|(c, q)| (c.to_string(), *q)).collect()
    }

    #[test]
    fn basic_forms() {
        assert_eq!(p("CF A01"), v(&[("A01", 1)]));
        assert_eq!(p("cf a01 x2"), v(&[("A01", 2)]));
        assert_eq!(p("A01 2"), v(&[("A01", 2)]));
        assert_eq!(p("a01=3"), v(&[("A01", 3)]));
        assert_eq!(p("a01x2"), v(&[("A01", 2)]));
        assert_eq!(p("a01 × 4"), v(&[("A01", 4)]));
        assert_eq!(p("CFA01"), v(&[("A01", 1)]));
        assert_eq!(p("F a02"), v(&[("A02", 1)]));
    }

    #[test]
    fn thai_and_fullwidth() {
        assert_eq!(p("เอา A01 2 ชิ้น"), v(&[("A01", 2)]));
        assert_eq!(p("รับA02ค่ะ"), v(&[("A02", 1)]));
        assert_eq!(p("ＣＦ Ａ０１ ２"), v(&[("A01", 2)]));
        assert_eq!(p("cf a01 ๓"), v(&[("A01", 3)]));
    }

    #[test]
    fn lao() {
        assert_eq!(p("ເອົາ A01 2"), v(&[("A01", 2)]));
        assert_eq!(p("ຈອງ CFA01"), v(&[("A01", 1)]));
        assert_eq!(p("ຮັບA02ເດີ"), v(&[("A02", 1)]));
        assert_eq!(p("ສັ່ງ B03 ໓ ຊິ້ນ"), v(&[("B03", 3)])); // Lao digit + unit word
        assert!(p("A01 ລາຄາເທົ່າໃດ").is_empty()); // question without trigger
        assert_eq!(p("ຈອງ a01 ລາຄາເທົ່າໃດ"), v(&[("A01", 1)])); // trigger overrides question
        let codes = vec!["A01".to_string()];
        let t = vec!["ຈອງ".to_string()];
        assert!(parse("A01 2", &codes, &t, true, 10).claims.is_empty());
        assert_eq!(parse("ຈອງ A01 2", &codes, &t, true, 10).claims, vec![Claim { code: "A01".into(), qty: 2 }]);
    }

    #[test]
    fn multiple_and_merge() {
        assert_eq!(p("CF A01 x2, B03"), v(&[("A01", 2), ("B03", 1)]));
        assert_eq!(p("a01 a01"), v(&[("A01", 2)]));
        assert_eq!(p("cf a01 2 b03 3 m1"), v(&[("A01", 2), ("B03", 3), ("M1", 1)]));
    }

    #[test]
    fn longest_code_wins_and_boundaries() {
        assert_eq!(p("cf a010"), v(&[("A010", 1)]));
        assert!(p("ba01").is_empty()); // part of another word
        assert!(p("a012").is_empty()); // ambiguous digits glued to code
        assert!(p("hello there").is_empty());
    }

    #[test]
    fn questions_and_limits() {
        assert!(p("A01 ราคาเท่าไหร่คะ").is_empty());
        assert!(p("a01 how much?").is_empty());
        assert_eq!(p("cf a01 ยังมีไหม"), v(&[("A01", 1)])); // trigger overrides question
        assert_eq!(p("cf a01 50"), v(&[("A01", 10)])); // capped at max qty
    }

    #[test]
    fn require_trigger() {
        let codes = vec!["A01".to_string()];
        let t = vec!["cf".to_string()];
        assert!(parse("A01 2", &codes, &t, true, 10).claims.is_empty());
        assert_eq!(parse("cf A01 2", &codes, &t, true, 10).claims.len(), 1);
        assert!(parse("cfx", &codes, &t, true, 10).claims.is_empty());
    }
}
