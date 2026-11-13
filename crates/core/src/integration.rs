//! The parts of the desktop PsyAudio plugs into: the file manager's context menu and standby.

#[cfg(windows)]
use crate::audio;

/// Add or remove the Explorer right-click entry for audio files (current user only).
pub fn file_manager_menu(add: bool) -> String {
    #[cfg(windows)]
    {
        let Ok(exe) = std::env::current_exe() else { return crate::i18n::t("Programmpfad unbekannt").into() };
        let key = r"HKCU\Software\Classes\SystemFileAssociations\audio\shell\PsyAudio";
        let run = |args: &[&str]| {
            let mut c = std::process::Command::new("reg");
            c.args(args);
            audio::hidden(&mut c);
            c.status().map(|s| s.success()).unwrap_or(false)
        };
        if add {
            let cmd = format!("\"{}\" \"%1\"", exe.display());
            let icon = exe.display().to_string();
            let ok = run(&["add", key, "/ve", "/d", crate::i18n::t("Mit PsyAudio konvertieren"), "/f"])
                && run(&["add", key, "/v", "Icon", "/d", &icon, "/f"])
                && run(&["add", &format!(r"{key}\command"), "/ve", "/d", &cmd, "/f"]);
            if ok {
                crate::i18n::t("Kontextmenü eingerichtet").into()
            } else {
                crate::i18n::t("Kontextmenü konnte nicht eingerichtet werden").into()
            }
        } else if run(&["delete", key, "/f"]) {
            crate::i18n::t("Kontextmenü entfernt").into()
        } else {
            crate::i18n::t("Kein Kontextmenü vorhanden").into()
        }
    }
    #[cfg(target_os = "linux")]
    {
        let Some(data) = dirs::data_dir() else { return crate::i18n::t("Kontextmenü konnte nicht eingerichtet werden").into() };
        let menu = data.join("kio/servicemenus/psyaudio.desktop");
        let icon = data.join("PsyAudio/PsyAudio.png");
        if !add {
            let _ = std::fs::remove_file(&icon);
            return if std::fs::remove_file(&menu).is_ok() {
                crate::i18n::t("Kontextmenü entfernt").into()
            } else {
                crate::i18n::t("Kein Kontextmenü vorhanden").into()
            };
        }
        let Ok(exe) = std::env::current_exe() else { return crate::i18n::t("Programmpfad unbekannt").into() };
        match write_service_menu(&menu, &icon, &exe) {
            Ok(()) => crate::i18n::t("Kontextmenü eingerichtet").into(),
            Err(_) => crate::i18n::t("Kontextmenü konnte nicht eingerichtet werden").into(),
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = add;
        crate::i18n::t("Nur unter Windows verfügbar").into()
    }
}

/// Dolphin service menu for audio files; Dolphin only offers local service menus that are executable.
#[cfg(target_os = "linux")]
fn write_service_menu(menu: &std::path::Path, icon: &std::path::Path, exe: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for dir in [menu.parent(), icon.parent()].into_iter().flatten() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(icon, crate::APP_ICON_PNG)?;
    std::fs::write(menu, service_menu_text(exe, icon, crate::i18n::t("Mit PsyAudio konvertieren")))?;
    std::fs::set_permissions(menu, std::fs::Permissions::from_mode(0o755))
}

/// The `.desktop` text: one entry for audio files that hands the whole selection to PsyAudio.
#[cfg(target_os = "linux")]
pub fn service_menu_text(exe: &std::path::Path, icon: &std::path::Path, label: &str) -> String {
    // Exec quoting: inside double quotes, `"`, `` ` ``, `$` and `\` need a backslash; then the
    // string escape doubles every backslash once more, and `%` is written `%%`
    let quoted: String = exe.to_string_lossy().chars().flat_map(|c| if matches!(c, '"' | '`' | '$' | '\\') { vec!['\\', c] } else { vec![c] }).collect();
    let quoted = quoted.replace('\\', "\\\\").replace('%', "%%");
    format!(
        "[Desktop Entry]\nType=Service\nMimeType=audio/*;\nActions=psyaudio;\nX-KDE-Priority=TopLevel\n\n[Desktop Action psyaudio]\nName={label}\nIcon={}\nExec=\"{quoted}\" %F\n",
        icon.display()
    )
}

/// SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) = 0x80000001 while converting
/// or ripping, so the PC doesn't go to standby; ES_CONTINUOUS alone (0x80000000) lets it sleep again.
pub const KEEP_AWAKE_FLAGS: u32 = 0x8000_0001;

pub fn keep_awake(on: bool) {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Power::SetThreadExecutionState(if on { KEEP_AWAKE_FLAGS } else { 0x8000_0000 });
    }
    #[cfg(not(windows))]
    let _ = on;
}

#[cfg(all(test, target_os = "linux"))]
mod service_menu_tests {
    use super::service_menu_text;
    use std::path::Path;

    #[test]
    fn hands_the_selection_to_psyaudio() {
        let t = service_menu_text(Path::new("/opt/Psy Audio/PsyAudio"), Path::new("/tmp/i.png"), "Convert");
        assert!(t.contains("MimeType=audio/*;"));
        assert!(t.contains("Name=Convert\n"));
        assert!(t.contains("Exec=\"/opt/Psy Audio/PsyAudio\" %F\n"));
    }

    #[test]
    fn special_characters_in_the_path_are_escaped() {
        let t = service_menu_text(Path::new("/a$b/50%\"x"), Path::new("/i.png"), "C");
        assert!(t.contains(r#"Exec="/a\\$b/50%%\\"x" %F"#), "{t}");
    }
}
