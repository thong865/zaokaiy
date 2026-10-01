//! Every English error message in every core has a Lao translation
//! (kernel dictionary + the dictionaries cores register).

use regex::Regex;

const CTORS: &str = r"AppError::(?:bad|BadRequest|Conflict|Upstream)\s*\(\s*";
const STR: &str = r#""((?:[^"\\]|\\.)*)""#;

fn register_all() {
    for m in [platform::module(), commerce::module(), vehicle::module(), restaurant::module(), insurance::module(), finance::module()] {
        kernel::i18n::register_lao(m.lao, m.lao_patterns);
        if let Some(f) = m.lao_fn {
            kernel::i18n::register_lao_fn(f);
        }
    }
}

fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("i18n.rs") && !p.ends_with("lao.rs") {
            out.push(p);
        }
    }
}

fn sources() -> Vec<(String, String)> {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for c in std::fs::read_dir(&crates).unwrap() {
        let src = c.unwrap().path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    files.into_iter().map(|p| (p.display().to_string(), std::fs::read_to_string(&p).unwrap())).collect()
}

fn unescape(s: &str) -> String {
    s.replace("\\\"", "\"").replace("\\n", "\n").replace("\\\\", "\\")
}

#[test]
fn every_literal_error_message_is_translated() {
    register_all();
    let re = Regex::new(&format!("{CTORS}{STR}")).unwrap();
    let mut missing = Vec::new();
    let mut n = 0;
    for (file, src) in sources() {
        for c in re.captures_iter(&src) {
            n += 1;
            let msg = unescape(&c[1]);
            if kernel::i18n::translate_lo(&msg).is_none() {
                missing.push(format!("{file}: {msg}"));
            }
        }
    }
    assert!(n > 150, "extractor found only {n} messages");
    assert!(missing.is_empty(), "untranslated messages:\n{}", missing.join("\n"));
}

#[test]
fn every_format_error_message_is_translated() {
    register_all();
    let re = Regex::new(&format!(r"{CTORS}format!\s*\(\s*{STR}")).unwrap();
    let ph = Regex::new(r"\{[^}]*\}").unwrap();
    let mut missing = Vec::new();
    let mut n = 0;
    for (file, src) in sources() {
        for c in re.captures_iter(&src) {
            n += 1;
            let sample = ph.replace_all(&unescape(&c[1]), "7").into_owned();
            if kernel::i18n::translate_lo(&sample).is_none() {
                missing.push(format!("{file}: {sample}"));
            }
        }
    }
    assert!(n > 25, "extractor found only {n} format messages");
    assert!(missing.is_empty(), "untranslated format messages:\n{}", missing.join("\n"));
}
