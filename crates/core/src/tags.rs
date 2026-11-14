//! Reading and writing tags (ID3, Vorbis comments, MP4, APE...).

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::Tag;
use std::collections::BTreeMap;
use std::path::Path;

/// The tag fields the program knows, with their names in the editor.
pub const FIELDS: &[(&str, &str)] = &[
    ("title", "Titel"),
    ("artist", "Interpret"),
    ("album", "Album"),
    ("album artist", "Albuminterpret"),
    ("track", "Titelnummer"),
    ("track_total", "Titel gesamt"),
    ("disc", "Disc"),
    ("disc_total", "Discs gesamt"),
    ("year", "Jahr"),
    ("genre", "Genre"),
    ("composer", "Komponist"),
    ("conductor", "Dirigent"),
    ("comment", "Kommentar"),
    ("label", "Label"),
    ("isrc", "ISRC"),
    ("compilation", "Sampler (1 = ja)"),
    ("lyrics", "Liedtext"),
];

pub const RG_FIELDS: &[&str] = &["replaygain_track_gain", "replaygain_track_peak", "replaygain_album_gain", "replaygain_album_peak"];

fn key_of(name: &str) -> Option<ItemKey> {
    Some(match name {
        "title" => ItemKey::TrackTitle,
        "artist" => ItemKey::TrackArtist,
        "album" => ItemKey::AlbumTitle,
        "album artist" => ItemKey::AlbumArtist,
        "track" => ItemKey::TrackNumber,
        "track_total" => ItemKey::TrackTotal,
        "disc" => ItemKey::DiscNumber,
        "disc_total" => ItemKey::DiscTotal,
        "year" => ItemKey::Year,
        "genre" => ItemKey::Genre,
        "composer" => ItemKey::Composer,
        "conductor" => ItemKey::Conductor,
        "comment" => ItemKey::Comment,
        "label" => ItemKey::Label,
        "isrc" => ItemKey::Isrc,
        "compilation" => ItemKey::FlagCompilation,
        "lyrics" => ItemKey::Lyrics,
        "replaygain_track_gain" => ItemKey::ReplayGainTrackGain,
        "replaygain_track_peak" => ItemKey::ReplayGainTrackPeak,
        "replaygain_album_gain" => ItemKey::ReplayGainAlbumGain,
        "replaygain_album_peak" => ItemKey::ReplayGainAlbumPeak,
        "title sort" => ItemKey::TrackTitleSortOrder,
        "artist sort" => ItemKey::TrackArtistSortOrder,
        "album sort" => ItemKey::AlbumTitleSortOrder,
        "album artist sort" => ItemKey::AlbumArtistSortOrder,
        "composersort" => ItemKey::ComposerSortOrder,
        "encoder" => ItemKey::EncoderSoftware,
        _ => return None,
    })
}

/// Separator for fields with several values ("Band A; Band B").
pub const MULTI_SEP: &str = "; ";
/// Fields saved as several values when they contain `MULTI_SEP`.
const MULTI_FIELDS: &[&str] = &["artist", "album artist", "genre", "composer", "conductor"];

const ALL_KEYS: &[&str] = &[
    "title",
    "artist",
    "album",
    "album artist",
    "track",
    "track_total",
    "disc",
    "disc_total",
    "year",
    "genre",
    "composer",
    "conductor",
    "comment",
    "label",
    "isrc",
    "compilation",
    "lyrics",
    "replaygain_track_gain",
    "replaygain_track_peak",
    "replaygain_album_gain",
    "replaygain_album_peak",
    "title sort",
    "artist sort",
    "album sort",
    "album artist sort",
    "composersort",
];

/// Picture types as ID3 numbers them (the ones the program handles by name).
pub const FRONT: u8 = 3;
#[cfg(test)]
pub const BACK: u8 = 4;

#[derive(Clone, Debug, Default)]
pub struct Art {
    pub mime: String,
    pub data: Vec<u8>,
    /// Picture type (`FRONT`, `BACK`, …), kept when tags are saved again.
    pub kind: u8,
}

impl Art {
    pub fn new(data: Vec<u8>, kind: u8) -> Art {
        Art { mime: image_mime(&data).into(), data, kind }
    }
}

/// MIME type from the image's first bytes.
fn image_mime(data: &[u8]) -> &'static str {
    if data.starts_with(b"\x89PNG") {
        "image/png"
    } else if data.starts_with(b"GIF8") {
        "image/gif"
    } else if data.starts_with(b"BM") {
        "image/bmp"
    } else {
        "image/jpeg"
    }
}

/// Album-wide fields copied by "Album-Felder für alle übernehmen".
pub const ALBUM_FIELDS: &[&str] = &["album", "album artist", "year", "genre", "disc", "disc_total", "composer", "compilation", "label"];

/// Copy the album fields that `from` has, and its front cover if it has one; nothing is removed.
pub fn take_album_fields(target: &mut Tags, from: &Tags) {
    for k in ALBUM_FIELDS {
        let v = from.get(k);
        if !v.is_empty() {
            target.set(k, v.to_string());
        }
    }
    if let Some(front) = from.front_cover() {
        target.set_front_cover(front.clone());
    }
}

#[derive(Clone, Debug, Default)]
pub struct Tags {
    pub fields: BTreeMap<String, String>,
    pub art: Vec<Art>,
}

impl Tags {
    pub fn get(&self, k: &str) -> &str {
        self.fields.get(k).map(|s| s.as_str()).unwrap_or("")
    }
    pub fn set(&mut self, k: &str, v: impl Into<String>) {
        let v = v.into();
        if v.trim().is_empty() {
            self.fields.remove(k);
        } else {
            self.fields.insert(k.to_string(), v);
        }
    }
    /// Same fields and pictures (to tell whether the editor holds unsaved changes).
    pub fn same_as(&self, other: &Tags) -> bool {
        self.fields == other.fields && self.art.len() == other.art.len() && self.art.iter().zip(&other.art).all(|(a, b)| a.kind == b.kind && a.data == b.data)
    }
    /// The front cover, or the first picture when none is marked as front.
    pub fn front_cover(&self) -> Option<&Art> {
        self.art.iter().find(|a| a.kind == FRONT).or_else(|| self.art.first())
    }
    /// Replace the front cover; other pictures (back, booklet…) stay.
    pub fn set_front_cover(&mut self, mut art: Art) {
        art.kind = FRONT;
        self.art.retain(|a| a.kind != FRONT);
        self.art.insert(0, art);
    }
}

#[derive(Clone, Debug, Default, serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Info {
    pub seconds: f64,
    pub bitrate: Option<u32>,
    pub rate: Option<u32>,
    pub bits: Option<u8>,
    pub channels: Option<u8>,
}

pub fn read(path: &Path) -> (Tags, Info) {
    let mut tags = Tags::default();
    let mut info = Info::default();
    let Ok(file) = lofty::read_from_path(path) else { return (tags, info) };
    let p = file.properties();
    info.seconds = p.duration().as_secs_f64();
    info.bitrate = p.audio_bitrate();
    info.rate = p.sample_rate();
    info.bits = p.bit_depth();
    info.channels = p.channels();
    let tag = file.primary_tag().or_else(|| file.first_tag());
    if let Some(tag) = tag {
        for name in ALL_KEYS {
            if let Some(k) = key_of(name) {
                // several values (two artists, two genres) are shown joined with "; "
                let vals: Vec<String> = tag.get_strings(k).map(|v| v.trim_matches(char::from(0)).trim().to_string()).filter(|v| !v.is_empty()).collect();
                if !vals.is_empty() {
                    tags.fields.insert(name.to_string(), vals.join(MULTI_SEP));
                }
            }
        }
        // "3/12" style track numbers
        for (n, t) in [("track", "track_total"), ("disc", "disc_total")] {
            if let Some(v) = tags.fields.get(n).cloned() {
                if let Some((a, b)) = v.split_once('/') {
                    tags.fields.insert(n.into(), a.trim().into());
                    if !b.trim().is_empty() {
                        tags.fields.entry(t.into()).or_insert(b.trim().into());
                    }
                }
            }
        }
        if tags.get("year").is_empty() {
            if let Some(d) = tag.get_string(ItemKey::RecordingDate) {
                tags.set("year", d.chars().take(4).collect::<String>());
            }
        }
        for pic in tag.pictures() {
            tags.art.push(Art {
                mime: pic.mime_type().map(|m| m.as_str().to_string()).unwrap_or_else(|| image_mime(pic.data()).into()),
                data: pic.data().to_vec(),
                kind: pic.pic_type().as_u8(),
            });
        }
    }
    (tags, info)
}

/// Write `tags` into `path`, replacing the known fields. Returns an error text when the format has no tags.
pub fn write(path: &Path, tags: &Tags, replace_art: bool) -> Result<(), String> {
    let mut file = lofty::read_from_path(path).map_err(|e| format!("Tags: {e}"))?;
    if file.primary_tag_mut().is_none() {
        let tt = file.primary_tag_type();
        file.insert_tag(Tag::new(tt));
    }
    let tag = file.primary_tag_mut().ok_or(crate::i18n::t("Format unterstützt keine Tags"))?;
    for name in ALL_KEYS {
        if *name == "year" {
            write_year(tag, tags.get("year"));
            continue;
        }
        if let Some(k) = key_of(name) {
            tag.remove_key(k);
            if let Some(v) = tags.fields.get(*name) {
                if MULTI_FIELDS.contains(name) && v.contains(MULTI_SEP) {
                    for part in v.split(MULTI_SEP).map(str::trim).filter(|p| !p.is_empty()) {
                        tag.push(lofty::tag::TagItem::new(k, lofty::tag::ItemValue::Text(part.to_string())));
                    }
                } else if !v.trim().is_empty() {
                    tag.insert_text(k, v.clone());
                }
            }
        }
    }
    if replace_art {
        // the file gets exactly the pictures in `tags`, each with its own type
        let old: Vec<PictureType> = tag.pictures().iter().map(|p| p.pic_type()).collect();
        for t in old {
            tag.remove_picture_type(t);
        }
        for a in &tags.art {
            let mime = match image_mime(&a.data) {
                "image/png" => MimeType::Png,
                "image/gif" => MimeType::Gif,
                "image/bmp" => MimeType::Bmp,
                _ => MimeType::Jpeg,
            };
            tag.push_picture(Picture::unchecked(a.data.clone()).pic_type(PictureType::from_u8(a.kind)).mime_type(mime).build());
        }
    }
    tag.save_to_path(path, WriteOptions::default()).map_err(|e| crate::i18n::tf(crate::i18n::t("Tags speichern: {e}"), &[("e", format!("{e}"))]))
}

/// The year goes into the recording date, the key every format and other programs read
/// (Vorbis DATE, ID3v2 TDRC, MP4 ©day); FFmpeg has already written it there from
/// `-metadata date=`. A separate year key next to it shows up as a doubled year elsewhere.
/// A full date that starts with the year stays as it is.
fn write_year(tag: &mut Tag, year: &str) {
    tag.remove_key(ItemKey::Year);
    let year = year.trim();
    if year.is_empty() {
        tag.remove_key(ItemKey::RecordingDate);
    } else if !tag.get_string(ItemKey::RecordingDate).is_some_and(|d| d.trim().starts_with(year)) {
        tag.insert_text(ItemKey::RecordingDate, year.to_string());
    }
}

/// FFmpeg -metadata arguments (for formats the tag library can't write, e.g. WMA).
pub fn ffmpeg_metadata(tags: &Tags) -> Vec<String> {
    let mut out = Vec::new();
    let map = [
        ("title", "title"),
        ("artist", "artist"),
        ("album", "album"),
        ("album artist", "album_artist"),
        ("year", "date"),
        ("genre", "genre"),
        ("composer", "composer"),
        ("comment", "comment"),
    ];
    for (k, f) in map {
        let v = tags.get(k);
        if !v.is_empty() {
            out.push("-metadata".into());
            out.push(format!("{f}={v}"));
        }
    }
    let t = tags.get("track");
    if !t.is_empty() {
        out.push("-metadata".into());
        let tt = tags.get("track_total");
        out.push(if tt.is_empty() { format!("track={t}") } else { format!("track={t}/{tt}") });
    }
    out
}

#[cfg(test)]
mod art_tests {
    use super::*;
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfront";
    const JPG: &[u8] = b"\xff\xd8\xff\xe0back";

    fn wav(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("psyaudio-art-{name}-{}.wav", std::process::id()));
        let a = crate::audio::Audio { rate: 8000, channels: 1, bits: 16, float: false, lossless: true, data: vec![0.0; 80] };
        crate::audio::write_wav(&p, &a, crate::audio::PcmFormat::S16).unwrap();
        p
    }
    fn kinds(t: &Tags) -> Vec<u8> {
        let mut k: Vec<u8> = t.art.iter().map(|a| a.kind).collect();
        k.sort();
        k
    }

    #[test]
    fn saving_keeps_picture_types_without_duplicates() {
        let p = wav("types");
        let mut t = Tags::default();
        t.set("title", "Lied");
        t.art = vec![Art::new(PNG.to_vec(), FRONT), Art::new(JPG.to_vec(), BACK)];
        write(&p, &t, true).unwrap();
        let (again, _) = read(&p);
        write(&p, &again, true).unwrap();
        let (back, _) = read(&p);
        assert_eq!(kinds(&back), vec![FRONT, BACK]);
        assert!(back.art.iter().any(|a| a.kind == BACK && a.data == JPG));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn several_artists_survive_a_save() {
        let p = wav("multi");
        let mut t = Tags::default();
        t.set("title", "Lied");
        t.set("artist", "Band A; Band B");
        write(&p, &t, false).unwrap();
        let (back, _) = read(&p);
        assert_eq!(back.get("artist"), "Band A; Band B");
        write(&p, &back, false).unwrap();
        let file = lofty::read_from_path(&p).unwrap();
        let tag = file.primary_tag().unwrap();
        assert_eq!(tag.get_strings(ItemKey::TrackArtist).count(), 2, "stored as two values");
        assert_eq!(read(&p).0.get("artist"), "Band A; Band B");
        let _ = std::fs::remove_file(&p);
    }

    fn dates(p: &std::path::Path) -> Vec<String> {
        let file = lofty::read_from_path(p).unwrap();
        let tag = file.primary_tag().unwrap();
        tag.items().filter(|i| matches!(i.key(), ItemKey::Year | ItemKey::RecordingDate)).filter_map(|i| i.value().text().map(String::from)).collect()
    }

    #[test]
    fn the_recording_date_follows_the_year() {
        let p = wav("date");
        let mut t = Tags::default();
        t.set("title", "Lied");
        write(&p, &t, false).unwrap();
        let mut file = lofty::read_from_path(&p).unwrap();
        file.primary_tag_mut().unwrap().insert_text(ItemKey::RecordingDate, "2001-05-12".into());
        file.save_to_path(&p, WriteOptions::default()).unwrap();
        let (mut back, _) = read(&p);
        assert_eq!(back.get("year"), "2001");
        write(&p, &back, false).unwrap();
        assert_eq!(dates(&p), vec!["2001-05-12".to_string()], "a full date with the same year stays");
        back.set("year", "1999");
        write(&p, &back, false).unwrap();
        assert_eq!(dates(&p), vec!["1999".to_string()]);
        back.set("year", "");
        write(&p, &back, false).unwrap();
        assert!(dates(&p).is_empty(), "{:?}", dates(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn edits_are_noticed() {
        let mut a = Tags::default();
        a.set("title", "Lied");
        let mut b = a.clone();
        assert!(a.same_as(&b));
        b.set("title", "Lied 2");
        assert!(!a.same_as(&b));
        let mut c = a.clone();
        c.set_front_cover(Art::new(PNG.to_vec(), FRONT));
        assert!(!a.same_as(&c));
    }

    #[test]
    fn album_fields_never_remove_covers() {
        let mut target = Tags { art: vec![Art::new(PNG.to_vec(), FRONT), Art::new(JPG.to_vec(), BACK)], ..Tags::default() };
        let mut from = Tags::default();
        from.set("album", "Platte");
        take_album_fields(&mut target, &from);
        assert_eq!(target.get("album"), "Platte");
        assert_eq!(kinds(&target), vec![FRONT, BACK]);
        from.art = vec![Art::new(b"\x89PNGnew".to_vec(), FRONT)];
        take_album_fields(&mut target, &from);
        assert_eq!(kinds(&target), vec![FRONT, BACK]);
        assert_eq!(target.front_cover().unwrap().data, b"\x89PNGnew");
    }
}
