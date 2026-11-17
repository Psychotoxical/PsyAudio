//! Interface language. The English text is the key; every language is one file in `locales/`
//! (`{"English": "translation"}`, counts as `{"one": …, "few": …, "many": …, "other": …}`),
//! read by the core here and by the interface in `src/i18n.ts`. A text a language lacks stays
//! English.
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// Code, name in its own language, and the language file. English first: the key language.
pub const LANGUAGES: &[(&str, &str, &str)] = &[
    ("en", "English", include_str!("../../../locales/en.json")),
    ("de", "Deutsch", include_str!("../../../locales/de.json")),
    ("es", "Español", include_str!("../../../locales/es.json")),
    ("fr", "Français", include_str!("../../../locales/fr.json")),
    ("nb", "Norsk bokmål", include_str!("../../../locales/nb.json")),
    ("nl", "Nederlands", include_str!("../../../locales/nl.json")),
    ("ro", "Română", include_str!("../../../locales/ro.json")),
    ("ru", "Русский", include_str!("../../../locales/ru.json")),
    ("zh", "中文", include_str!("../../../locales/zh.json")),
    ("ja", "日本語", include_str!("../../../locales/ja.json")),
    ("hu", "Magyar", include_str!("../../../locales/hu.json")),
    ("pl", "Polski", include_str!("../../../locales/pl.json")),
    ("bg", "Български", include_str!("../../../locales/bg.json")),
    ("it", "Italiano", include_str!("../../../locales/it.json")),
    ("uk", "Українська", include_str!("../../../locales/uk.json")),
    ("el", "Ελληνικά", include_str!("../../../locales/el.json")),
];

/// Index into `LANGUAGES`; English until the app sets the language from the settings.
static LANG: AtomicUsize = AtomicUsize::new(0);

/// One entry of a language file: a text, or the forms of a count by plural category.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Entry {
    Text(String),
    Counted(HashMap<String, String>),
}

/// Switch the language by code ("de", "pt-BR" …); unknown codes fall back to English.
pub fn set_lang(code: &str) {
    let base = code.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    let i = LANGUAGES.iter().position(|l| l.0 == base).unwrap_or(0);
    LANG.store(i, Ordering::Relaxed);
}

/// The code of the current language.
pub fn lang() -> &'static str {
    LANGUAGES[LANG.load(Ordering::Relaxed)].0
}

fn table(i: usize) -> &'static HashMap<String, Entry> {
    static TABLES: OnceLock<Vec<OnceLock<HashMap<String, Entry>>>> = OnceLock::new();
    let tables = TABLES.get_or_init(|| LANGUAGES.iter().map(|_| OnceLock::new()).collect());
    tables[i].get_or_init(|| serde_json::from_str(LANGUAGES[i].2).unwrap_or_default())
}

/// Translate an English interface text into the current language.
pub fn t(s: &str) -> &str {
    match table(LANG.load(Ordering::Relaxed)).get(s) {
        Some(Entry::Text(x)) if !x.is_empty() => x,
        _ => s,
    }
}

/// The text for a count in the current language. `one` and `other` are the English forms;
/// `other` is the key, and the language file gives the forms its grammar needs.
pub fn plural<'a>(n: u64, one: &'a str, other: &'a str) -> &'a str {
    let i = LANG.load(Ordering::Relaxed);
    if let Some(Entry::Counted(forms)) = table(i).get(other) {
        let form = forms.get(category(LANGUAGES[i].0, n)).or_else(|| forms.get("other")).filter(|x| !x.is_empty());
        if let Some(f) = form {
            return f;
        }
    }
    if n == 1 {
        one
    } else {
        other
    }
}

/// The CLDR plural category of a whole number in a language (the forms a count file may give).
pub fn category(lang: &str, n: u64) -> &'static str {
    let (m10, m100) = (n % 10, n % 100);
    match lang {
        "ja" | "zh" => "other",
        "fr" => {
            if n <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "ro" => {
            if n == 1 {
                "one"
            } else if n == 0 || (2..=19).contains(&m100) {
                "few"
            } else {
                "other"
            }
        }
        "ru" | "uk" => {
            if m10 == 1 && m100 != 11 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        "pl" => {
            if n == 1 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        _ => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

/// The plural categories a language uses (what its count entries must give).
pub fn categories(lang: &str) -> &'static [&'static str] {
    match lang {
        "ja" | "zh" => &["other"],
        "ro" => &["one", "few", "other"],
        "ru" | "uk" | "pl" => &["one", "few", "many"],
        _ => &["one", "other"],
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

    fn placeholders(s: &str) -> Vec<String> {
        let mut v: Vec<String> = s.split('{').skip(1).filter_map(|p| p.split('}').next()).map(String::from).collect();
        v.sort();
        v
    }

    #[test]
    fn plural_categories_follow_cldr() {
        let cases = [
            ("en", [(0, "other"), (1, "one"), (2, "other"), (21, "other")]),
            ("fr", [(0, "one"), (1, "one"), (2, "other"), (21, "other")]),
            ("ru", [(1, "one"), (3, "few"), (11, "many"), (22, "few")]),
            ("pl", [(1, "one"), (4, "few"), (5, "many"), (21, "many")]),
            ("ro", [(0, "few"), (1, "one"), (19, "few"), (20, "other")]),
            ("ja", [(0, "other"), (1, "other"), (2, "other"), (5, "other")]),
        ];
        for (lang, list) in cases {
            for (n, cat) in list {
                assert_eq!(category(lang, n), cat, "{lang} {n}");
            }
        }
    }

    /// Every language file: only keys English has, the same placeholders, and for counts the
    /// categories its language uses.
    #[test]
    fn language_files_match_english() {
        let en = table(0);
        for (i, (code, _, _)) in LANGUAGES.iter().enumerate() {
            assert!(serde_json::from_str::<HashMap<String, Entry>>(LANGUAGES[i].2).is_ok(), "{code}.json does not parse");
            for (key, entry) in table(i) {
                let base = en.get(key).unwrap_or_else(|| panic!("{code}.json: unknown key {key:?}"));
                match (base, entry) {
                    (Entry::Text(_), Entry::Text(x)) => {
                        if !x.is_empty() {
                            assert_eq!(placeholders(key), placeholders(x), "{code}.json: placeholders differ in {key:?}");
                        }
                    }
                    (Entry::Counted(_), Entry::Counted(forms)) => {
                        for cat in categories(code) {
                            assert!(forms.contains_key(*cat), "{code}.json: {key:?} lacks the form {cat:?}");
                        }
                        for (cat, x) in forms {
                            assert!(
                                categories(code).contains(&cat.as_str()) || cat == "other",
                                "{code}.json: {key:?} has a form {cat:?} the language does not use"
                            );
                            let mut want = placeholders(key);
                            let mut got = placeholders(x);
                            // a singular may spell out the number ("1 file")
                            want.retain(|p| got.contains(p) || cat != "one");
                            got.sort();
                            assert_eq!(want, got, "{code}.json: placeholders differ in {key:?} ({cat})");
                        }
                    }
                    _ => panic!("{code}.json: {key:?} is a count in one file and a text in the other"),
                }
            }
        }
    }

    #[test]
    fn falls_back_to_english_and_fills() {
        set_lang("xx");
        assert_eq!(lang(), "en");
        assert_eq!(t("a text no language has"), "a text no language has");
        assert_eq!(tf("{a} of {0}", &[("a", "1".into()), ("0", "2".into())]), "1 of 2");
        set_lang("de-AT");
        assert_eq!(lang(), "de");
        set_lang("en");
    }
}
