//! Loading and saving settings.json (same file and format as the egui version).

use crate::state::{AppState, Demo};
use psyaudio_core::settings::Settings;
use serde::Serialize;
use std::path::Path;
use tauri::State;

#[derive(Serialize)]
pub struct LoadedSettings {
    settings: Settings,
    /// Set when parts of the file could not be read; the old file is kept as settings.json.bak.
    warning: Option<String>,
}

/// The saved settings (in the sample mode the defaults); the file is not written here.
#[tauri::command]
pub fn load_settings(state: State<'_, AppState>) -> LoadedSettings {
    let (settings, warning) = match &state.demo {
        Some(demo) => (demo_settings(demo), None),
        None => {
            let (mut s, warning) = Settings::load();
            // the single offset of earlier versions belongs to the drive attached now
            if s.rip.drive_offset != 0 {
                let key = psyaudio_core::cdrip::drives().first().map(|d| psyaudio_core::cdrip::drive_key(*d));
                if adopt_on_start(&mut s, key, &psyaudio_core::settings::today()) {
                    let _ = s.save();
                }
            }
            (s, warning)
        }
    };
    psyaudio_core::i18n::set_lang(&settings.lang);
    LoadedSettings { settings, warning }
}

/// The sample mode's settings: the defaults, in German unless `--en`, Mocha unless `--light`.
fn demo_settings(demo: &Demo) -> Settings {
    Settings { lang: if demo.en { "en" } else { "de" }.into(), dark: !demo.light, ..Settings::default() }
}

/// Hand the old single offset to the drive `key` (the first one attached); true when it moved.
fn adopt_on_start(st: &mut Settings, key: Option<String>, today: &str) -> bool {
    key.is_some_and(|k| st.rip.adopt_legacy_offset(&k, today))
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    psyaudio_core::i18n::set_lang(&settings.lang);
    save_unless_demo(&settings, state.demo.is_some(), &psyaudio_core::settings::path())
}

/// The sample mode never touches the real settings file.
pub fn save_unless_demo(settings: &Settings, demo: bool, path: &Path) -> Result<(), String> {
    if demo {
        Ok(())
    } else {
        settings.save_to(path)
    }
}

/// The sample mode's page, or nothing on a normal start.
#[tauri::command]
pub fn demo_state(state: State<'_, AppState>) -> Option<Demo> {
    state.demo.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_demo_speaks_english() {
        let demo = |en| Demo { page: "cd".into(), light: false, en };
        assert_eq!(demo_settings(&demo(true)).lang, "en");
        assert_eq!(demo_settings(&demo(false)).lang, "de");
    }

    #[test]
    fn demo_never_writes_settings() {
        let dir = std::env::temp_dir().join(format!("psyaudio-demo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, "{}").unwrap();
        save_unless_demo(&Settings::default(), true, &p).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "{}", "demo leaves the file alone");
        save_unless_demo(&Settings::default(), false, &p).unwrap();
        assert_ne!(std::fs::read_to_string(&p).unwrap(), "{}", "a normal start saves");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn start_hands_the_old_offset_to_the_drive_present() {
        let mut st = Settings::default();
        st.rip.drive_offset = 667;
        assert!(!adopt_on_start(&mut st, None, "2026-11-14"), "no drive attached: keep the old value");
        assert_eq!(st.rip.drive_offset, 667);
        assert!(adopt_on_start(&mut st, Some("Band Drive X".into()), "2026-11-14"));
        assert_eq!(st.rip.drive("Band Drive X").map(|p| p.offset), Some(667));
    }
}

/// "Alle Einstellungen auf Standard": the defaults, with the profiles kept (the window saves them).
#[tauri::command]
pub fn reset_settings(mut settings: Settings) -> Settings {
    settings.reset_keep_profiles();
    settings
}

/// What the settings page shows about this computer.
#[derive(serde::Serialize)]
pub struct SystemInfo {
    cores: usize,
    /// The temporary folder used when none is set.
    temp_default: String,
    /// "windows", "linux" or "macos": which file manager the menu goes into.
    os: &'static str,
}

#[tauri::command]
pub fn system_info() -> SystemInfo {
    SystemInfo {
        cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8),
        temp_default: std::env::temp_dir().to_string_lossy().into_owned(),
        os: std::env::consts::OS,
    }
}
