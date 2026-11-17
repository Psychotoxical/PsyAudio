//! Folder and file names from chips: the chips on offer, presets, a preview of the target path,
//! and the preview for "Tags aus Dateiname".

use crate::state::AppState;
use psyaudio_core::convert;
use psyaudio_core::ffmpeg::FfmpegSet;
use psyaudio_core::naming;
use psyaudio_core::settings::Settings;
use psyaudio_core::tags;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use tauri::State;

#[cfg(test)]
mod tests {
    use super::*;
    use psyaudio_core::settings::OutputMode;

    #[test]
    fn labels_cover_all_chips() {
        let o = naming_options();
        for token in o.data.iter().chain(&o.separators).chain(std::iter::once(&o.folder)) {
            assert!(o.labels.contains_key(*token), "{token:?}");
        }
        assert_eq!(o.presets.first().map(|p| p.scheme.clone()), Some(naming::CD_SCHEME.to_vec()));
    }

    #[test]
    fn preview_follows_the_scheme() {
        let dir = std::env::temp_dir().join("psyaudio-preview-test");
        let s = Settings {
            output_mode: OutputMode::SingleFolder,
            output_folder: dir.join("out").to_string_lossy().into_owned(),
            name_scheme: vec!["filename".into()],
            ..Settings::default()
        };
        let src = dir.join("release-1").join("01 Lied.flac");
        let got = preview(&s, &src, None, FfmpegSet::default());
        assert_eq!(got.map(std::path::PathBuf::from), Some(dir.join("out").join("01 Lied.mp3")));
    }

    #[test]
    fn tags_are_read_from_the_file_name() {
        // built by the OS: on Linux and macOS a backslash is part of the file name, not a separator
        let path = std::env::temp_dir().join("Musik").join("03 Band - Lied.flac");
        let got = tags_from_name(path.to_string_lossy().into_owned(), naming::TAG_SCHEME.iter().map(|s| s.to_string()).collect());
        assert_eq!(got.first().map(|kv| kv.0.as_str()), Some("track"));
        assert_eq!(got.iter().find(|kv| kv.0 == "title").map(|kv| kv.1.as_str()), Some("Lied"));
    }
}

#[derive(Serialize)]
pub struct Preset {
    name: &'static str,
    scheme: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct NamingOptions {
    presets: Vec<Preset>,
    data: Vec<&'static str>,
    separators: Vec<&'static str>,
    folder: &'static str,
    /// Chip → German label (the frontend translates it).
    labels: BTreeMap<&'static str, &'static str>,
}

/// The chips on offer and the presets (names in German, translated in the frontend).
#[tauri::command]
pub fn naming_options() -> NamingOptions {
    let all = naming::DATA.iter().chain(naming::SEPARATORS).chain(std::iter::once(&naming::FOLDER));
    NamingOptions {
        presets: naming::PRESETS.iter().map(|(name, scheme)| Preset { name, scheme: scheme.to_vec() }).collect(),
        data: naming::DATA.to_vec(),
        separators: naming::SEPARATORS.to_vec(),
        folder: naming::FOLDER,
        labels: all.map(|t| (*t, naming::label(t))).collect(),
    }
}

fn preview(s: &Settings, src: &Path, root: Option<&Path>, ffmpeg: FfmpegSet) -> Option<String> {
    let plan = convert::plan_from_settings(s, s.codec, ffmpeg);
    let codec = *plan.outputs().first()?;
    let (t, info) = tags::read(src);
    Some(plan.destination(src, root, &t, &info, codec).to_string_lossy().into_owned())
}

/// Where the converter would write `path` with these settings (none for tool formats).
#[tauri::command]
pub async fn preview_name(state: State<'_, AppState>, settings: Settings, path: String, root: Option<String>) -> Result<Option<String>, String> {
    let cache = crate::tools_cmd::ffmpeg(&state, &settings.tools.ffmpeg, false);
    tauri::async_runtime::spawn_blocking(move || preview(&settings, Path::new(&path), root.as_deref().map(Path::new), cache.set))
        .await
        .map_err(|e| e.to_string())
}

/// The tags "Tags aus Dateiname" would read from `path` with `scheme`.
#[tauri::command]
pub fn tags_from_name(path: String, scheme: Vec<String>) -> Vec<(String, String)> {
    let stem = Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    convert::tags_from_scheme(&stem, &scheme)
}
