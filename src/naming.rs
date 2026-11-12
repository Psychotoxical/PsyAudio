//! File and folder names built from chips: data chips, separators and "new folder" (as in PsyTags).

use crate::tags::Tags;
use std::path::Path;

/// Replacements for characters Windows does not allow in file names (the same as in PsyTags).
const CHAR_REPLACE: &[(char, &str)] =
    &[('\\', "-"), ('/', ""), (':', "-"), ('*', ""), ('?', ""), ('"', "''"), ('<', ""), ('>', ""), ('|', "")];

/// Characters Windows doesn't allow in names are replaced by the table above;
/// control characters are dropped.
pub fn clean_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars().filter(|c| (*c as u32) >= 32) {
        match CHAR_REPLACE.iter().find(|(from, _)| *from == c) {
            Some((_, to)) => out.push_str(to),
            None => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Names built from chips: data chips, separators and "new folder".

/// Token that starts a new folder.
pub const FOLDER: &str = "/";
/// Data chips (the first one only where a source file exists).
pub const DATA: &[&str] = &["filename", "albumartist", "artist", "album", "year", "track", "disc", "title"];
/// Separator chips.
pub const SEPARATORS: &[&str] = &[" - ", "-", "(", ")", "[", "]", " ", "_", "."];
/// Converter default: keep the source file name.
pub const CONVERT_SCHEME: &[&str] = &["filename"];
/// CD ripper default, the same as in PsyTags.
pub const CD_SCHEME: &[&str] =
    &["albumartist", " - ", "(", "year", ")", " - ", "album", FOLDER, "track", "-", "disc", " - ", "artist", " - ", "title"];
/// Default for reading tags from file names.
pub const TAG_SCHEME: &[&str] = &["track", " ", "artist", " - ", "title"];
/// Longest folder or file name part.
pub const MAX_PART: usize = 80;

/// German label of a chip (translated with `t()` where shown).
pub fn label(token: &str) -> &str {
    match token {
        "filename" => "Dateiname",
        "albumartist" => "Albuminterpret",
        "artist" => "Interpret",
        "album" => "Album",
        "year" => "Jahr",
        "track" => "Track",
        "disc" => "Disc",
        "title" => "Titel",
        " - " => "–",
        " " => "Abstand",
        FOLDER => "Neuer Ordner",
        other => other,
    }
}

/// Tag field a data chip reads (`None` for the file name).
pub fn tag_key(token: &str) -> Option<&'static str> {
    Some(match token {
        "albumartist" => "album artist",
        "artist" => "artist",
        "album" => "album",
        "year" => "year",
        "track" => "track",
        "disc" => "disc",
        "title" => "title",
        _ => return None,
    })
}

/// Keep known chips only; an empty result becomes `default`.
pub fn sanitize(scheme: &[String], data: &[&str], folders: bool, default: &[&str]) -> Vec<String> {
    let out: Vec<String> = scheme
        .iter()
        .filter(|t| data.contains(&t.as_str()) || SEPARATORS.contains(&t.as_str()) || (folders && t.as_str() == FOLDER))
        .cloned()
        .collect();
    if out.iter().any(|t| data.contains(&t.as_str())) {
        out
    } else {
        default.iter().map(|s| s.to_string()).collect()
    }
}

/// Digits at the start ("3/12" → "3"), or the whole text when it starts with none.
fn leading_number(v: &str) -> Option<String> {
    let d: String = v.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    (!d.is_empty()).then_some(d)
}

/// The cleaned text of a data chip.
fn chip_value(token: &str, tags: &Tags, source: &Path) -> String {
    let get = |k: &str| tags.get(k).trim().to_string();
    let first = |a: &str, b: &str| if get(a).is_empty() { get(b) } else { get(a) };
    let raw = match token {
        "filename" => source.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        "albumartist" => first("album artist", "artist"),
        "artist" => first("artist", "album artist"),
        "year" => get("year").chars().take(4).collect(),
        "track" => match leading_number(tags.get("track")) {
            Some(n) => format!("{n:0>2}"),
            None => get("track"),
        },
        "disc" => leading_number(tags.get("disc")).map(|n| n.trim_start_matches('0').to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| get("disc")),
        other => tag_key(other).map(get).unwrap_or_default(),
    };
    clean_value(&raw).trim().to_string()
}

const OPENING: &[&str] = &["(", "["];
const CLOSING: &[&str] = &[")", "]"];

fn closing_of(s: &str) -> Option<&'static str> {
    OPENING.iter().position(|o| *o == s).map(|i| CLOSING[i])
}

/// Values and separators of one folder or file name. Separators around an empty value
/// disappear, so a missing year never leaves "()" or a doubled " - " behind.
fn join(parts: &[(bool, String)]) -> String {
    let mut keep = vec![true; parts.len()];
    for (i, (is_value, text)) in parts.iter().enumerate() {
        if *is_value && text.is_empty() {
            keep[i] = false;
            if i > 0 && i + 1 < parts.len() && !parts[i - 1].0 && !parts[i + 1].0 && closing_of(&parts[i - 1].1) == Some(parts[i + 1].1.as_str()) {
                keep[i - 1] = false;
                keep[i + 1] = false;
            }
        }
    }
    let kept: Vec<&(bool, String)> = parts.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| p).collect();
    let bracket = |s: &str| OPENING.contains(&s) || CLOSING.contains(&s);
    // of several separators in a row the longest stays
    let mut collapsed: Vec<&(bool, String)> = Vec::new();
    for part in kept {
        match collapsed.last() {
            Some(prev) if !part.0 && !prev.0 && !bracket(&part.1) && !bracket(&prev.1) => {
                if part.1.len() > prev.1.len() {
                    *collapsed.last_mut().unwrap() = part;
                }
            }
            _ => collapsed.push(part),
        }
    }
    while collapsed.first().is_some_and(|p| !p.0 && !OPENING.contains(&p.1.as_str())) {
        collapsed.remove(0);
    }
    while collapsed.last().is_some_and(|p| !p.0 && !CLOSING.contains(&p.1.as_str())) {
        collapsed.pop();
    }
    if !collapsed.iter().any(|p| p.0) {
        return String::new();
    }
    collapsed.iter().map(|p| p.1.as_str()).collect()
}

/// At most `MAX_PART` characters (unless `cut` is off), no spaces or dots at the end (Windows drops them).
fn fit_part(s: &str, cut: bool) -> String {
    let s: String = if cut { s.trim().chars().take(MAX_PART).collect() } else { s.trim().to_string() };
    s.trim_end_matches([' ', '.']).trim().to_string()
}

/// Folder names (parts without any value are left out) and the file name without extension,
/// which is empty when its chips have no value. A part with the source file name is not cut,
/// so two long source names never end up the same.
pub fn names(scheme: &[String], tags: &Tags, source: &Path) -> (Vec<String>, String) {
    let mut parts: Vec<String> = Vec::new();
    let mut current: Vec<(bool, String)> = Vec::new();
    let mut has_filename = false;
    for token in scheme.iter().map(|s| s.as_str()).chain(std::iter::once(FOLDER)) {
        if token == FOLDER {
            parts.push(fit_part(&join(&current), !has_filename));
            current.clear();
            has_filename = false;
        } else if DATA.contains(&token) {
            has_filename |= token == "filename";
            current.push((true, chip_value(token, tags, source)));
        } else {
            current.push((false, token.to_string()));
        }
    }
    let file = parts.pop().unwrap_or_default();
    (parts.into_iter().filter(|p| !p.is_empty()).collect(), file)
}

/// Folders and file name in one list (tests).
#[cfg(test)]
fn segments(scheme: &[String], tags: &Tags, source: &Path) -> Vec<String> {
    let (mut out, file) = names(scheme, tags, source);
    if !file.is_empty() {
        out.push(file);
    }
    out
}

#[cfg(test)]
mod scheme_tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    fn tags(pairs: &[(&str, &str)]) -> Tags {
        let mut t = Tags::default();
        for (k, v) in pairs {
            t.set(k, *v);
        }
        t
    }
    fn src() -> std::path::PathBuf {
        std::path::PathBuf::from("C:\\Musik\\a.wav")
    }

    #[test]
    fn empty_year_drops_brackets() {
        let t = tags(&[("artist", "Band"), ("album", "Platte"), ("title", "Lied"), ("track", "3"), ("disc", "1")]);
        assert_eq!(segments(&s(CD_SCHEME), &t, &src()), s(&["Band - Platte", "03-1 - Band - Lied"]));
    }

    #[test]
    fn albumartist_falls_back_to_artist() {
        let t = tags(&[("artist", "Band"), ("title", "Lied")]);
        assert_eq!(segments(&s(&["albumartist", " - ", "title"]), &t, &src()), s(&["Band - Lied"]));
        let t = tags(&[("artist", "Gast"), ("album artist", "Band"), ("title", "Lied")]);
        assert_eq!(segments(&s(&["albumartist", "/", "artist"]), &t, &src()), s(&["Band", "Gast"]));
    }

    #[test]
    fn numbers_before_slash() {
        let t = tags(&[("track", "3/12"), ("disc", "2/2"), ("year", "1992-05-01")]);
        assert_eq!(segments(&s(&["track", "-", "disc", " ", "year"]), &t, &src()), s(&["03-2 1992"]));
    }

    #[test]
    fn invalid_chars_replaced() {
        let t = tags(&[("title", "What? A: \"B\"*<>|\\/")]);
        assert_eq!(segments(&s(&["title"]), &t, &src()), s(&["What A- ''B''-"]));
    }

    #[test]
    fn part_cut_to_80_and_trailing_dots_removed() {
        let long = "x".repeat(100);
        let t = tags(&[("album", long.as_str()), ("title", "Ende...")]);
        let out = segments(&s(&["album", "/", "title"]), &t, &src());
        assert_eq!(out[0].chars().count(), MAX_PART);
        assert_eq!(out[1], "Ende");
    }

    #[test]
    fn folder_parts_without_value_dropped() {
        let t = tags(&[("title", "Lied")]);
        assert_eq!(segments(&s(&["album", " - ", "/", "title"]), &t, &src()), s(&["Lied"]));
    }

    #[test]
    fn filename_chip_uses_source_stem() {
        let t = Tags::default();
        let p = std::path::PathBuf::from("C:\\Musik\\Band - 03 - Lied.flac");
        assert_eq!(segments(&s(CONVERT_SCHEME), &t, &p), s(&["Band - 03 - Lied"]));
    }

    #[test]
    fn empty_file_part_keeps_folders_apart() {
        let t = tags(&[("artist", "Band"), ("album", "Platte")]);
        assert_eq!(names(&s(&["artist", "/", "album", "/", "title"]), &t, &src()), (s(&["Band", "Platte"]), String::new()));
    }

    #[test]
    fn filename_chip_is_not_cut() {
        let long = "x".repeat(100);
        let p = std::path::PathBuf::from(format!("C:\\Musik\\{long}.flac"));
        assert_eq!(names(&s(CONVERT_SCHEME), &Tags::default(), &p).1, long);
    }

    #[test]
    fn sanitize_drops_unknown_and_defaults() {
        assert_eq!(sanitize(&s(&["artist", "unbekannt", " - ", "/", "title"]), DATA, false, CD_SCHEME), s(&["artist", " - ", "title"]));
        assert_eq!(sanitize(&s(&["bogus"]), DATA, true, TAG_SCHEME), s(TAG_SCHEME));
        assert_eq!(sanitize(&s(&["filename", "title"]), &DATA[1..], true, CD_SCHEME), s(&["title"]));
    }

    #[test]
    fn labels_and_keys() {
        assert_eq!(label("albumartist"), "Albuminterpret");
        assert_eq!(label(" - "), "–");
        assert_eq!(label(FOLDER), "Neuer Ordner");
        assert_eq!(tag_key("albumartist"), Some("album artist"));
        assert_eq!(tag_key("filename"), None);
    }
}

