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
pub fn t<'a>(s: &'a str) -> &'a str {
    if LANG.load(Ordering::Relaxed) == 0 {
        return s;
    }
    english().get(s).copied().unwrap_or(s)
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
