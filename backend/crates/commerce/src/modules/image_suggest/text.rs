//! Matching a typed product name against photo titles/keywords in any language.
//!
//! Lao and Thai are written without spaces between words, so full-text search (which splits on
//! spaces) doesn't work for them. Instead everything is compared in a *compact* form — lower case,
//! no spaces or punctuation — using substring tests, plus synonym groups ("beer lao" = "beerlao" =
//! "ເບຍລາວ") and a character-bigram similarity for near misses.

use std::collections::HashSet;

fn is_sep(c: char) -> bool {
    c.is_whitespace() || c.is_ascii_punctuation() || matches!(c, '–' | '—' | '•' | '·' | '“' | '”' | '‘' | '’' | '…' | '«' | '»' | '（' | '）' | '、' | '。' | '，')
}

/// Compact form: "Beer Lao 640 ml" → "beerlao640ml". Must agree with SQL `image_suggest_norm`.
pub fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| !is_sep(*c)).collect()
}

/// Words of the query (compact, at least 2 characters), in order, without duplicates.
pub fn tokens(s: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    s.to_lowercase()
        .split(is_sep)
        .map(norm)
        .filter(|t| t.chars().count() >= 2 && seen.insert(t.clone()))
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct Query {
    pub compact: String,
    pub tokens: Vec<String>,
    /// Terms from synonym groups the query belongs to (other spellings / languages).
    pub synonyms: Vec<String>,
}

/// A synonym group applies when one of its terms appears in the query, or the query is the start
/// of one of its terms (so "beerl" already finds Beer Lao while typing).
pub fn expand(q: &str, groups: &[Vec<String>]) -> Query {
    let compact = norm(q);
    let tokens = tokens(q);
    let mut synonyms: Vec<String> = Vec::new();
    if compact.chars().count() >= 2 {
        for g in groups {
            let hit = g.iter().any(|t| !t.is_empty() && (compact.contains(t.as_str()) || (compact.chars().count() >= 4 && t.starts_with(&compact))));
            if hit {
                for t in g {
                    if !synonyms.contains(t) && *t != compact {
                        synonyms.push(t.clone());
                    }
                }
            }
        }
    }
    Query { compact, tokens, synonyms }
}

/// LIKE patterns for a first, cheap candidate filter in SQL (at most 24).
pub fn patterns(q: &Query) -> Vec<String> {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: &str| {
        let p = format!("%{}%", esc(s));
        if s.chars().count() >= 2 && !out.contains(&p) {
            out.push(p);
        }
    };
    push(&q.compact);
    q.tokens.iter().for_each(|t| push(t));
    q.synonyms.iter().for_each(|t| push(t));
    out.truncate(24);
    out
}

fn bigrams(s: &str) -> Vec<(char, char)> {
    let c: Vec<char> = s.chars().collect();
    c.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Sørensen–Dice similarity of character bigrams (0..1).
pub fn dice(a: &str, b: &str) -> f64 {
    let (x, y) = (bigrams(a), bigrams(b));
    if x.is_empty() || y.is_empty() {
        return if !a.is_empty() && a == b { 1.0 } else { 0.0 };
    }
    let mut pool = y.clone();
    let mut hits = 0usize;
    for g in &x {
        if let Some(i) = pool.iter().position(|h| h == g) {
            pool.swap_remove(i);
            hits += 1;
        }
    }
    2.0 * hits as f64 / (x.len() + y.len()) as f64
}

/// Relevance of one photo. `text` = compact title+keywords (+ product name); `title` = compact title.
/// ≥ 3 is a useful match; the whole phrase or a synonym scores ≥ 8.
pub fn score(q: &Query, text: &str, title: &str) -> f64 {
    if q.compact.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    if text.contains(q.compact.as_str()) {
        s += 10.0;
    }
    let total: usize = q.tokens.iter().map(|t| t.chars().count()).sum();
    if total > 0 && q.tokens.len() > 1 {
        let matched: usize = q.tokens.iter().filter(|t| text.contains(t.as_str())).map(|t| t.chars().count()).sum();
        let r = matched as f64 / total as f64;
        s += 6.0 * r * r; // one shared word out of several counts little
    }
    if q.synonyms.iter().any(|t| text.contains(t.as_str())) {
        s += 8.0;
    }
    s + 4.0 * dice(&q.compact, title)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups() -> Vec<Vec<String>> {
        vec![vec!["beerlao".into(), "ເບຍລາວ".into(), "เบียร์ลาว".into()], vec!["pepsi".into(), "ເປັບຊີ".into()]]
    }

    #[test]
    fn normalises_every_script() {
        assert_eq!(norm("Beer Lao 640 ml."), "beerlao640ml");
        assert_eq!(norm("ເບຍລາວ ກະປ໋ອງ"), "ເບຍລາວກະປ໋ອງ");
        assert_eq!(tokens("Beer  Lao, beer!"), vec!["beer", "lao"]);
        assert_eq!(tokens("a b"), Vec::<String>::new());
    }

    #[test]
    fn synonyms_bridge_languages_and_spellings() {
        let g = groups();
        for q in ["beer lao", "Beerlao", "ເບຍລາວ", "เบียร์ลาว 330ml", "beerl"] {
            let x = expand(q, &g);
            assert!(!x.synonyms.is_empty(), "{q}");
        }
        assert!(expand("pepsi can", &g).synonyms.contains(&"ເປັບຊີ".to_string()));
        assert!(expand("lao coffee", &g).synonyms.is_empty(), "'lao' alone is not Beer Lao");
    }

    #[test]
    fn ranking() {
        let g = groups();
        let q = expand("beer lao", &g);
        let beerlao = score(&q, &norm("Beerlao Original 640ml bottle, ເບຍລາວ"), &norm("Beerlao Original 640ml bottle"));
        let lao_only = score(&q, &norm("ເບຍລາວ ກະປ໋ອງ"), &norm("ເບຍລາວ ກະປ໋ອງ"));
        let coffee = score(&q, &norm("Lao coffee beans"), &norm("Lao coffee beans"));
        assert!(beerlao >= 8.0, "{beerlao}");
        assert!(lao_only >= 8.0, "Lao-only title found through the synonym: {lao_only}");
        assert!(coffee < 3.0, "unrelated product sharing one word: {coffee}");
        assert!(beerlao > coffee && lao_only > coffee);
        let typo = expand("heineken", &g);
        assert!(score(&typo, &norm("Heiniken can 330ml"), &norm("Heiniken can")) > score(&typo, &norm("Tiger can 330ml"), &norm("Tiger can")));
    }

    #[test]
    fn like_patterns_are_escaped() {
        let q = expand("100% cotton_shirt", &[]);
        assert_eq!(q.compact, "100cottonshirt");
        assert_eq!(patterns(&q), vec!["%100cottonshirt%", "%100%", "%cotton%", "%shirt%"]);
        let q = Query { compact: "a_b".into(), tokens: vec![], synonyms: vec!["x%y".into()] };
        assert_eq!(patterns(&q), vec!["%a\\_b%", "%x\\%y%"]);
    }
}
