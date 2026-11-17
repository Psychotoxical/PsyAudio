//! Settings and profiles, saved as JSON in the user's application data folder.

use crate::dsp::Effect;
use crate::encode::{Codec, EncSettings, Tools};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum OutputMode {
    /// Under one base folder, folders and file names from the chip scheme.
    #[serde(alias = "Dynamic")]
    SingleFolder,
    /// Next to each source file.
    SourceFolder,
    /// Under a base folder, keeping the source path and file name.
    PreservePath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum Overwrite {
    Ask,
    Always,
    Skip,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Profile {
    pub name: String,
    pub codec: Codec,
    pub enc: EncSettings,
    #[serde(deserialize_with = "lenient_effects")]
    pub effects: Vec<Effect>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { name: String::new(), codec: Codec::Mp3, enc: EncSettings::default(), effects: Vec::new() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum RipMode {
    Burst,
    Secure,
}

/// How a drive's read offset was found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum OffsetMethod {
    /// Measured with a disc the CUETools database knows.
    #[default]
    Ctdb,
    /// Entered by hand.
    Manual,
    /// Taken over from the single offset of earlier versions.
    Adopted,
}

/// What PsyAudio knows about one drive model.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct DriveProfile {
    /// Vendor and product ("VENDOR CD-RW X1"), or the drive's name when the system tells no model.
    pub model: String,
    /// Read offset in samples.
    pub offset: i32,
    pub method: OffsetMethod,
    /// When the offset was found, YYYY-MM-DD.
    pub measured: String,
    /// The drive's buffer as it reports it, in KB.
    pub buffer_kb: Option<u32>,
}

/// CD ripper settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct RipSettings {
    pub mode: RipMode,
    /// The single offset of earlier versions: moves to the first drive seen (`adopt_legacy_offset`), 0 afterwards.
    pub drive_offset: i32,
    /// Every drive measured or entered so far, one per model.
    pub drives: Vec<DriveProfile>,
    /// How often a block that differs between passes is read again (default 34).
    pub reread_limit: u32,
    /// Extra check passes after the second pass (default off).
    pub extra_passes: bool,
    /// Extra passes: at least `extra_min`, at most `extra_max`, done after `extra_clean` clean ones.
    pub extra_min: u32,
    pub extra_max: u32,
    pub extra_clean: u32,
    /// Drive cache in KB to flush before a re-read; 0 (default) follows the measured buffer.
    pub cache_kb: u32,
    /// Abort a track after this many unrecoverable blocks (0 = never).
    pub abort_unrecoverable: u32,
    /// Interpolate blocks that could not be recovered (default off).
    pub interpolate: bool,
    /// Mark a track as failed: 0 never, 1 when insecure, 2 when not confirmed by the CUETools database.
    pub mark_error: u8,
    /// Check rips against the CUETools database (CTDB).
    #[serde(alias = "accuraterip")]
    pub ctdb: bool,
    /// Write the rip log into the album folder (default off).
    pub write_log: bool,
    pub write_cue: bool,
    pub embed_art: bool,
    pub folder_jpg: bool,
    pub eject_after: bool,
    /// Stop ripping the disc after the first insecure track (default off).
    pub cancel_on_insecure: bool,
    /// Chips for album folders and track names.
    pub name_scheme: Vec<String>,
}

impl Default for RipSettings {
    fn default() -> Self {
        RipSettings {
            mode: RipMode::Secure,
            drive_offset: 0,
            drives: Vec::new(),
            reread_limit: 34,
            extra_passes: false,
            extra_min: 3,
            extra_max: 6,
            extra_clean: 2,
            cache_kb: 0,
            abort_unrecoverable: 0,
            interpolate: false,
            mark_error: 0,
            ctdb: true,
            write_log: false,
            write_cue: false,
            embed_art: true,
            folder_jpg: true,
            eject_after: false,
            cancel_on_insecure: false,
            name_scheme: scheme(crate::naming::CD_SCHEME),
        }
    }
}

impl RipSettings {
    /// The saved profile of a drive model.
    pub fn drive(&self, model: &str) -> Option<&DriveProfile> {
        self.drives.iter().find(|p| p.model == model)
    }

    /// Store a drive's profile, replacing the one of the same model.
    pub fn set_drive(&mut self, p: DriveProfile) {
        match self.drives.iter_mut().find(|x| x.model == p.model) {
            Some(x) => *x = p,
            None => self.drives.push(p),
        }
    }

    /// Give the single offset of earlier versions to `model`, the first drive seen; true when that happened.
    pub fn adopt_legacy_offset(&mut self, model: &str, today: &str) -> bool {
        if self.drive_offset == 0 || !self.drives.is_empty() {
            return false;
        }
        self.set_drive(DriveProfile {
            model: model.to_string(),
            offset: self.drive_offset,
            method: OffsetMethod::Adopted,
            measured: today.to_string(),
            buffer_kb: None,
        });
        self.drive_offset = 0;
        true
    }
}

/// A day count since 1970-01-01 as YYYY-MM-DD (proleptic Gregorian, H. Hinnant's civil_from_days).
pub(crate) fn date_from_days(days: i64) -> String {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Today's date (UTC), YYYY-MM-DD.
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    date_from_days((secs / 86_400) as i64)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Settings {
    /// The setup assistant was finished; files from before it count as set up.
    pub setup_done: bool,
    pub codec: Codec,
    pub enc: EncSettings,
    #[serde(deserialize_with = "lenient_effects")]
    pub effects: Vec<Effect>,
    pub output_mode: OutputMode,
    pub output_folder: String,
    /// Chips for converted folder and file names.
    pub name_scheme: Vec<String>,
    pub overwrite: Overwrite,
    pub threads: usize,
    pub copy_tags: bool,
    pub copy_art: bool,
    pub tools: Tools,
    pub temp_folder: String,
    pub low_priority: bool,
    pub dark: bool,
    /// Interface language: "en" (default) or "de".
    pub lang: String,
    pub profiles: Vec<Profile>,
    pub rip: RipSettings,
    pub rip_codec: Codec,
    pub rip_folder: String,
    pub batch_extensions: Vec<String>,
    pub batch_subfolders: bool,
    /// Chips for reading tags from file names.
    pub tag_scheme: Vec<String>,
    /// What the window looked like (open columns, tiles, boxes); pages are "convert", "batch",
    /// "cd" and "settings".
    pub ui: UiState,
}

/// The window's layout, kept between starts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct UiState {
    /// Whether a page's right column is open.
    pub panel_open: std::collections::BTreeMap<String, bool>,
    /// The step tile a page showed last.
    pub tile: std::collections::BTreeMap<String, String>,
    pub sidebar_collapsed: bool,
    /// Whether a collapsible box (settings page) is open.
    pub boxes_open: std::collections::BTreeMap<String, bool>,
}

fn scheme(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

impl Default for Settings {
    fn default() -> Self {
        let music = dirs::audio_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Music"));
        Settings {
            setup_done: false,
            codec: Codec::Mp3,
            enc: EncSettings::default(),
            effects: Vec::new(),
            output_mode: OutputMode::SingleFolder,
            output_folder: music.join("PsyAudio").display().to_string(),
            name_scheme: scheme(crate::naming::CONVERT_SCHEME),
            overwrite: Overwrite::Ask,
            threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
            copy_tags: true,
            copy_art: true,
            tools: Tools::default(),
            temp_folder: String::new(),
            low_priority: false,
            dark: true,
            lang: "en".into(),
            profiles: Vec::new(),
            rip: RipSettings::default(),
            rip_codec: Codec::Flac,
            rip_folder: music.display().to_string(),
            batch_extensions: vec![
                "flac".into(),
                "wav".into(),
                "mp3".into(),
                "m4a".into(),
                "ogg".into(),
                "opus".into(),
                "aiff".into(),
                "wma".into(),
                "ape".into(),
                "wv".into(),
            ],
            batch_subfolders: true,
            tag_scheme: scheme(crate::naming::TAG_SCHEME),
            ui: UiState::default(),
        }
    }
}

/// Where settings.json lives (%APPDATA%\PsyAudio on Windows).
pub fn path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("PsyAudio").join("settings.json")
}

/// An effect list from a saved file: effects PsyAudio no longer has (or not yet) are left out
/// quietly, so a profile keeps its name, format and other effects.
fn lenient_effects<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Effect>, D::Error> {
    let items = Vec::<serde_json::Value>::deserialize(d)?;
    Ok(items.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect())
}

/// Keep every top-level value of a saved settings file that still reads, and every entry of a
/// list (profiles, effects) that still reads; the rest falls back to its default.
fn salvage(text: &str) -> Settings {
    let Ok(serde_json::Value::Object(file)) = serde_json::from_str::<serde_json::Value>(text) else {
        return Settings::default();
    };
    let Ok(mut base) = serde_json::to_value(Settings::default()) else { return Settings::default() };
    let fits = |v: &serde_json::Value| serde_json::from_value::<Settings>(v.clone()).is_ok();
    for (key, value) in file {
        let mut trial = base.clone();
        trial[key.as_str()] = value.clone();
        if fits(&trial) {
            base = trial;
        } else if let serde_json::Value::Array(items) = value {
            let kept: Vec<serde_json::Value> = items
                .into_iter()
                .filter(|item| {
                    let mut one = base.clone();
                    one[key.as_str()] = serde_json::Value::Array(vec![item.clone()]);
                    fits(&one)
                })
                .collect();
            base[key.as_str()] = serde_json::Value::Array(kept);
        }
    }
    serde_json::from_value(base).unwrap_or_default()
}

impl Settings {
    /// "Alle Einstellungen auf Standard": everything back to the defaults except the profiles
    /// (as in the egui version) and the window layout, which is no setting.
    pub fn reset_keep_profiles(&mut self) {
        let profiles = std::mem::take(&mut self.profiles);
        let ui = std::mem::take(&mut self.ui);
        *self = Settings { profiles, ui, ..Settings::default() };
    }

    /// Settings from disk, and a note when parts of the file could not be read (the file is then
    /// kept as settings.json.bak, so nothing is lost by the next save).
    pub fn load() -> (Settings, Option<String>) {
        Settings::load_from(&path())
    }

    /// `load` from a given file; the file itself is never changed here.
    pub fn load_from(p: &std::path::Path) -> (Settings, Option<String>) {
        let Ok(text) = std::fs::read_to_string(p) else { return (Settings::default(), None) };
        let (st, complete) = Settings::parse(&text);
        if complete {
            return (st, None);
        }
        let backup = p.with_extension("json.bak");
        let _ = std::fs::copy(p, &backup);
        (st, Some(crate::i18n::tf(crate::i18n::t("Einstellungen teilweise unlesbar – die alte Datei liegt als {0}"), &[("0", backup.display().to_string())])))
    }

    /// Parse saved settings; chip schemes keep only known chips.
    #[cfg(test)]
    pub fn from_json(s: &str) -> Settings {
        Settings::parse(s).0
    }

    /// Parsed settings and whether every part of the file could be read.
    pub fn parse(s: &str) -> (Settings, bool) {
        use crate::naming::{sanitize, CD_SCHEME, CONVERT_SCHEME, DATA, TAG_SCHEME};
        let (mut st, complete) = match serde_json::from_str::<Settings>(s) {
            Ok(st) => (st, true),
            Err(_) => (salvage(s), false),
        };
        // the former effect "CPU schonen" is the setting "low priority" now
        if s.contains("\"CpuThrottle\"") {
            st.low_priority = true;
        }
        st.name_scheme = sanitize(&st.name_scheme, DATA, true, CONVERT_SCHEME);
        st.rip.name_scheme = sanitize(&st.rip.name_scheme, &DATA[1..], true, CD_SCHEME);
        st.tag_scheme = sanitize(&st.tag_scheme, &DATA[1..], false, TAG_SCHEME);
        // a file from before the setup assistant: that installation is set up, and its cache
        // value 1024 was the old default, not a choice
        let had_setup = serde_json::from_str::<serde_json::Value>(s).ok().is_some_and(|v| v.get("setup_done").is_some());
        if !had_setup {
            st.setup_done = true;
            if st.rip.cache_kb == 1024 {
                st.rip.cache_kb = 0;
            }
        }
        (st, complete)
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&path())
    }

    /// Written to a temporary file first and then moved over the old one, so a crash or a full
    /// disk never leaves a half-written settings file.
    pub fn save_to(&self, p: &std::path::Path) -> Result<(), String> {
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, p).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            e.to_string()
        })
    }
    pub fn temp_dir(&self) -> PathBuf {
        if self.temp_folder.trim().is_empty() {
            std::env::temp_dir()
        } else {
            PathBuf::from(self.temp_folder.trim())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::{CD_SCHEME, CONVERT_SCHEME, TAG_SCHEME};
    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn egui_settings_load_with_default_ui() {
        // "side_open" of the egui version is no setting any more; serde skips it
        let json = r#"{"codec":"Opus","side_open":false,"lang":"de","dark":false,"threads":5}"#;
        let (st, complete) = Settings::parse(json);
        assert!(complete);
        assert_eq!(st.ui, UiState::default());
        assert_eq!((st.codec, st.lang.as_str(), st.dark, st.threads), (Codec::Opus, "de", false, 5));
    }

    #[test]
    fn broken_settings_warn_and_keep_file() {
        let dir = std::env::temp_dir().join(format!("psyaudio-broken-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        let text = r#"{"threads": "viele", "lang": "de""#;
        std::fs::write(&p, text).unwrap();
        let (st, warning) = Settings::load_from(&p);
        assert!(warning.is_some());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), text, "the file stays as it was");
        assert_eq!(st.threads, Settings::default().threads);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_settings_keep_other_values() {
        let json = r#"{"codec":"Flac","threads":3,"output_mode":"Dynamic","naming":"[artist]\\[title]",
            "tag_from_name":"[track] [title]","enc":{"filename_pattern":"[track]","flac_level":8},
            "rip":{"naming":"Album/Titel","reread_limit":10}}"#;
        let st = Settings::from_json(json);
        assert_eq!(st.threads, 3);
        assert_eq!(st.codec, Codec::Flac);
        assert_eq!(st.enc.flac_level, 8);
        assert_eq!(st.output_mode, OutputMode::SingleFolder);
        assert_eq!(st.rip.reread_limit, 10);
        assert_eq!(st.name_scheme, s(CONVERT_SCHEME));
        assert_eq!(st.rip.name_scheme, s(CD_SCHEME));
        assert_eq!(st.tag_scheme, s(TAG_SCHEME));
    }
    #[test]
    fn one_unreadable_entry_does_not_wipe_the_rest() {
        let json = r#"{"threads":3,"output_folder":"D:\\Out",
            "profiles":[{"name":"gut","codec":"Flac","enc":{},"effects":[]},{"name":"neu","codec":"FormatAusDerZukunft","enc":{},"effects":[]}],
            "effects":[{"Fade":{"in_ms":10,"out_ms":20}},{"EffektAusDerZukunft":{}}],
            "tools":{"ffmpeg":"C:\\ff\\ffmpeg.exe"}}"#;
        let (st, complete) = Settings::parse(json);
        assert!(!complete);
        assert_eq!(st.threads, 3);
        assert_eq!(st.output_folder, "D:\\Out");
        assert_eq!(st.profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["gut"]);
        assert_eq!(st.effects.len(), 1);
        assert_eq!(st.tools.ffmpeg, "C:\\ff\\ffmpeg.exe");
    }

    #[test]
    fn profiles_with_removed_effects_stay() {
        let json = r#"{"profiles":[{"name":"alt","codec":"Flac","enc":{},"effects":["Reverse",{"Fade":{"in_ms":10,"out_ms":20}},"CpuThrottle"]}],
            "effects":[{"Loop":{"times":2}},"DeEmphasis"]}"#;
        let (st, complete) = Settings::parse(json);
        assert!(complete, "removed effects are no damage to report");
        assert_eq!(st.profiles.len(), 1);
        assert_eq!(st.profiles[0].name, "alt");
        assert_eq!(st.profiles[0].effects, vec![Effect::Fade { in_ms: 10, out_ms: 20 }]);
        assert_eq!(st.effects, vec![Effect::DeEmphasis]);
        assert!(st.low_priority, "CPU schonen became the low priority setting");
    }

    #[test]
    fn saving_replaces_the_file_in_one_step() {
        let dir = std::env::temp_dir().join(format!("psyaudio-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        let mut st = Settings { threads: 7, ..Settings::default() };
        st.save_to(&path).unwrap();
        st.threads = 8;
        st.save_to(&path).unwrap();
        assert_eq!(Settings::parse(&std::fs::read_to_string(&path).unwrap()).0.threads, 8);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temporary file left");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_file_counts_as_set_up() {
        assert!(Settings::parse(r#"{"threads":3}"#).0.setup_done, "a file from before the setup assistant");
        assert!(!Settings::parse(r#"{"threads":3,"setup_done":false}"#).0.setup_done);
        assert!(!Settings::default().setup_done, "a fresh installation runs the setup");
    }

    #[test]
    fn old_cache_default_becomes_automatic() {
        assert_eq!(Settings::parse(r#"{"rip":{"cache_kb":1024}}"#).0.rip.cache_kb, 0);
        assert_eq!(Settings::parse(r#"{"rip":{"cache_kb":2048}}"#).0.rip.cache_kb, 2048);
        assert_eq!(Settings::parse(r#"{"setup_done":true,"rip":{"cache_kb":1024}}"#).0.rip.cache_kb, 1024, "set on purpose");
        assert_eq!(RipSettings::default().cache_kb, 0);
    }

    #[test]
    fn accuraterip_switch_reads_as_ctdb() {
        assert!(!Settings::parse(r#"{"rip":{"accuraterip":false}}"#).0.rip.ctdb);
        assert!(RipSettings::default().ctdb);
    }

    #[test]
    fn legacy_offset_goes_to_the_first_drive() {
        let mut st = Settings::parse(r#"{"rip":{"drive_offset":667}}"#).0;
        assert!(st.rip.adopt_legacy_offset("Band Drive X", "2026-11-14"));
        let p = st.rip.drive("Band Drive X").expect("profile");
        assert_eq!((p.offset, p.method, p.measured.as_str()), (667, OffsetMethod::Adopted, "2026-11-14"));
        assert_eq!(st.rip.drive_offset, 0);
        assert!(!st.rip.adopt_legacy_offset("Band Drive Y", "2026-11-14"), "only once");
        assert!(st.rip.drive("Band Drive Y").is_none());
        let mut fresh = RipSettings::default();
        assert!(!fresh.adopt_legacy_offset("Band Drive X", "2026-11-14"), "nothing to adopt");
    }

    #[test]
    fn set_drive_replaces_the_same_model() {
        let mut r = RipSettings::default();
        let p = |o| DriveProfile { model: "Band Drive X".into(), offset: o, method: OffsetMethod::Ctdb, measured: "2026-11-14".into(), buffer_kb: None };
        r.set_drive(p(6));
        r.set_drive(p(667));
        assert_eq!(r.drives.len(), 1);
        assert_eq!(r.drive("Band Drive X").unwrap().offset, 667);
    }

    #[test]
    fn dates_from_days() {
        assert_eq!(date_from_days(0), "1970-01-01");
        assert_eq!(date_from_days(11017), "2000-03-01");
        assert_eq!(date_from_days(19723), "2024-01-01");
    }

    #[test]
    fn reset_keeps_profiles() {
        let mut s = Settings { threads: 3, low_priority: true, dark: false, ..Settings::default() };
        s.profiles.push(Profile { name: "Unterwegs".into(), ..Profile::default() });
        s.ui.sidebar_collapsed = true;
        s.reset_keep_profiles();
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.threads, Settings::default().threads);
        assert!(!s.low_priority && s.dark);
        assert!(s.ui.sidebar_collapsed, "the window layout is not a setting and stays");
    }

    #[test]
    fn saved_schemes_are_cleaned() {
        let json = r#"{"name_scheme":["artist","/","bogus","title"],"tag_scheme":["artist","/","title"],
            "rip":{"name_scheme":["filename","title"]}}"#;
        let st = Settings::from_json(json);
        assert_eq!(st.name_scheme, s(&["artist", "/", "title"]));
        assert_eq!(st.tag_scheme, s(&["artist", "title"]));
        assert_eq!(st.rip.name_scheme, s(&["title"]));
    }
}
