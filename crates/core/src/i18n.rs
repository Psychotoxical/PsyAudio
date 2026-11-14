//! Interface language. German text is the key; English comes from `i18n_dict.rs`.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[path = "i18n_dict.rs"]
mod dict;

/// 0 = German, 1 = English. Starts German so tests see the German source text;
/// the app sets it from the settings (English by default) before drawing.
static LANG: AtomicU8 = AtomicU8::new(0);

pub const LANGUAGES: &[(&str, &str)] = &[("en", "English"), ("de", "Deutsch")];

pub fn set_lang(code: &str) {
    LANG.store(if code == "de" { 0 } else { 1 }, Ordering::Relaxed);
}

fn english() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| dict::EN.iter().copied().collect())
}

/// Translate a German interface text into the chosen language.
pub fn t(s: &str) -> &str {
    if LANG.load(Ordering::Relaxed) == 0 {
        return s;
    }
    english().get(s).copied().unwrap_or(s)
}

/// The German text for a count: `one` for exactly 1, else `many`. German and English share this
/// rule, so the English singular is the dictionary's entry for `one`. Where German has one form
/// for both ("Titel"), `one` spells out the 1 ("1 Titel") to stay a key of its own.
pub fn plural<'a>(n: u64, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// Fill `{name}` placeholders of a (translated) template.
pub fn tf(template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let key = &after[..close];
                match args.iter().find(|(k, _)| *k == key) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(key);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plural_picks_one_only_for_exactly_one() {
        assert_eq!(plural(1, "1 Datei", "{0} Dateien"), "1 Datei");
        assert_eq!(plural(0, "1 Datei", "{0} Dateien"), "{0} Dateien");
        assert_eq!(plural(2, "1 Datei", "{0} Dateien"), "{0} Dateien");
    }

    /// The first two string literals of a Rust source, unescaped.
    fn literals(src: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut chars = src.chars();
        while let Some(c) = chars.next() {
            if c != '"' {
                continue;
            }
            let mut s = String::new();
            while let Some(c) = chars.next() {
                match c {
                    '"' => break,
                    '\\' => match chars.next() {
                        Some('n') => s.push('\n'),
                        Some(o) => s.push(o),
                        None => {}
                    },
                    o => s.push(o),
                }
            }
            out.push(s);
            if out.len() == 2 {
                break;
            }
        }
        out
    }

    /// Every `plural(n, "…", "…")` of the core has English for both texts, and the English
    /// singular really differs from the plural.
    #[test]
    fn plural_texts_have_english() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") || path.ends_with("i18n.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            for (i, _) in src.match_indices("plural(") {
                let pair = literals(&src[i..]);
                let (one, many) = (&pair[0], &pair[1]);
                let (en_one, en_many) = (english().get(one.as_str()), english().get(many.as_str()));
                assert!(en_one.is_some() && en_many.is_some(), "no English for {one:?} / {many:?} in {}", path.display());
                assert_ne!(en_one, en_many, "English singular equals plural: {one:?}");
                found += 1;
            }
        }
        assert!(found > 0, "no plural() calls found");
    }

    #[test]
    fn fills_and_translates() {
        assert_eq!(tf("{a} von {0}", &[("a", "1".into()), ("0", "2".into())]), "1 von 2");
        for (de, en) in dict::EN {
            let ph = |s: &str| {
                let mut v: Vec<String> = s.split('{').skip(1).filter_map(|p| p.split('}').next()).map(String::from).collect();
                v.sort();
                v
            };
            assert_eq!(ph(de), ph(en), "placeholders differ: {de}");
        }
    }
}
