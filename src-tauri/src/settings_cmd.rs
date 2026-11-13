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

/// The saved settings (in the sample mode the defaults, in German); the file is not written here.
#[tauri::command]
pub fn load_settings(state: State<'_, AppState>) -> LoadedSettings {
    let (settings, warning) = match &state.demo {
        Some(demo) => {
            let mut s = Settings::default();
            s.lang = "de".into();
            s.dark = !demo.light;
            (s, None)
        }
        None => Settings::load(),
    };
    psyaudio_core::i18n::set_lang(&settings.lang);
    LoadedSettings { settings, warning }
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
}
