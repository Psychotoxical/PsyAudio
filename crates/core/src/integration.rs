//! The parts of the desktop PsyAudio plugs into: the file manager's context menu and standby.

use crate::audio;

/// The Explorer menu entry: one key per extension PsyAudio reads, below HKEY_CURRENT_USER.
/// Explorer skips the "audio" perceived-type group for some types (.opus on Windows 11), so
/// every extension gets its own entry.
pub fn explorer_menu_keys() -> Vec<String> {
    audio::INPUT_EXTENSIONS.iter().map(|e| format!(r"Software\Classes\SystemFileAssociations\.{e}\shell\PsyAudio")).collect()
}

/// The single entry for the "audio" group that earlier versions wrote; it goes with every change.
pub const OLD_EXPLORER_MENU_KEY: &str = r"Software\Classes\SystemFileAssociations\audio\shell\PsyAudio";

/// Writing and deleting keys below HKEY_CURRENT_USER.
#[cfg(windows)]
mod reg {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Creates `path` and sets its string values (`None` is the default value).
    pub fn set(path: &str, values: &[(Option<&str>, &str)]) -> bool {
        let p = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        let created = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                p.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if created != 0 {
            return false;
        }
        let mut ok = true;
        for (name, data) in values {
            let n = name.map(wide);
            let d = wide(data);
            let name_ptr = n.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
            let r = unsafe { RegSetValueExW(key, name_ptr, 0, REG_SZ, d.as_ptr().cast(), (d.len() * 2) as u32) };
            ok &= r == 0;
        }
        unsafe { RegCloseKey(key) };
        ok
    }

    /// Deletes `path` with everything below it; true when it was there.
    pub fn delete(path: &str) -> bool {
        let p = wide(path);
        let r = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, p.as_ptr()) };
        if r == 0 {
            // RegDeleteTreeW empties the key; the key itself goes with RegDeleteKeyW
            unsafe { windows_sys::Win32::System::Registry::RegDeleteKeyW(HKEY_CURRENT_USER, p.as_ptr()) };
        }
        r == 0
    }

    #[cfg(test)]
    pub fn exists(path: &str) -> bool {
        use windows_sys::Win32::System::Registry::{RegOpenKeyExW, KEY_READ};
        let p = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        let r = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, p.as_ptr(), 0, KEY_READ, &mut key) };
        if r == 0 {
            unsafe { RegCloseKey(key) };
        }
        r == 0
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn a_key_is_written_and_deleted_with_its_subkeys() {
            let base = format!(r"Software\PsyAudioTest-{}", std::process::id());
            let sub = format!(r"{base}\shell\PsyAudio");
            assert!(super::set(&sub, &[(None, "Mit PsyAudio konvertieren"), (Some("Icon"), r"C:\x\PsyAudio.exe")]));
            assert!(super::set(&format!(r"{sub}\command"), &[(None, r#""C:\x\PsyAudio.exe" "%1""#)]));
            assert!(super::exists(&format!(r"{sub}\command")));
            assert!(super::delete(&sub));
            assert!(!super::exists(&sub), "the key itself is gone");
            assert!(!super::delete(&sub), "a second delete finds nothing");
            super::delete(&base);
        }
    }
}

/// Add or remove the Explorer right-click entry for audio files (current user only).
pub fn file_manager_menu(add: bool) -> String {
    #[cfg(windows)]
    {
        let Ok(exe) = std::env::current_exe() else { return crate::i18n::t("Programmpfad unbekannt").into() };
        // the old single entry would show the item twice where Explorer does use the group
        let had_old = reg::delete(OLD_EXPLORER_MENU_KEY);
        if add {
            let cmd = format!("\"{}\" \"%1\"", exe.display());
            let icon = exe.display().to_string();
            let label = crate::i18n::t("Mit PsyAudio konvertieren");
            let ok = explorer_menu_keys()
                .iter()
                .all(|k| reg::set(k, &[(None, label), (Some("Icon"), &icon)]) && reg::set(&format!(r"{k}\command"), &[(None, &cmd)]));
            if ok {
                crate::i18n::t("Kontextmenü eingerichtet").into()
            } else {
                crate::i18n::t("Kontextmenü konnte nicht eingerichtet werden").into()
            }
        } else {
            let mut removed = had_old;
            for k in explorer_menu_keys() {
                removed |= reg::delete(&k);
            }
            if removed {
                crate::i18n::t("Kontextmenü entfernt").into()
            } else {
                crate::i18n::t("Kein Kontextmenü vorhanden").into()
            }
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
        let Some(exe) = own_launch_path() else { return crate::i18n::t("Programmpfad unbekannt").into() };
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
    let quoted = exec_quoted(exe);
    format!(
        "[Desktop Entry]\nType=Service\nMimeType=audio/*;\nActions=psyaudio;\nX-KDE-Priority=TopLevel\n\n[Desktop Action psyaudio]\nName={label}\nIcon={}\nExec=\"{quoted}\" %F\n",
        icon.display()
    )
}

/// A path for a `.desktop` `Exec` line: inside double quotes, `"`, `` ` ``, `$` and `\\` need a
/// backslash; then the string escape doubles every backslash once more, and `%` is written `%%`.
#[cfg(target_os = "linux")]
fn exec_quoted(exe: &std::path::Path) -> String {
    let quoted: String = exe.to_string_lossy().chars().flat_map(|c| if matches!(c, '"' | '`' | '$' | '\\') { vec!['\\', c] } else { vec![c] }).collect();
    quoted.replace('\\', "\\\\").replace('%', "%%")
}

/// Wayland shows a window's icon only through a `.desktop` file named after its app id
/// (`PsyAudio`); a hidden entry gives the taskbar the real icon without a second menu item.
/// Written only when it changed, so a new build path is picked up on the next start.
/// A deb or rpm package brings its own `PsyAudio.desktop` with the icon; the hidden entry is then
/// not needed, and under the same name in the user's folder it would hide the menu item.
#[cfg(target_os = "linux")]
pub fn register_window_icon() {
    let Some(data) = dirs::data_dir() else { return };
    let icon = data.join("PsyAudio/PsyAudio.png");
    let entry = data.join("applications/PsyAudio.desktop");
    if package_entry_exists(&system_data_dirs(&std::env::var("XDG_DATA_DIRS").unwrap_or_default())) {
        // only the entry PsyAudio wrote itself goes, never one the person made
        if std::fs::read_to_string(&entry).is_ok_and(|t| t.contains("NoDisplay=true\n") && t.contains("StartupWMClass=PsyAudio\n")) {
            let _ = std::fs::remove_file(&entry);
        }
        return;
    }
    let Some(exe) = own_launch_path() else { return };
    let text = window_entry_text(&exe, &icon);
    if std::fs::read_to_string(&entry).is_ok_and(|t| t == text) && icon.exists() {
        return;
    }
    for dir in [entry.parent(), icon.parent()].into_iter().flatten() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&icon, crate::APP_ICON_PNG);
    let _ = std::fs::write(&entry, text);
}

/// The system data folders after the XDG base directory spec (`XDG_DATA_DIRS`, `:`-separated).
#[cfg(any(test, target_os = "linux"))]
fn system_data_dirs(var: &str) -> Vec<std::path::PathBuf> {
    let var = if var.is_empty() { "/usr/local/share:/usr/share" } else { var };
    var.split(':').filter(|d| !d.is_empty()).map(std::path::PathBuf::from).collect()
}

/// Whether a package installed the application entry `PsyAudio.desktop` in one of `dirs`.
#[cfg(any(test, target_os = "linux"))]
fn package_entry_exists(dirs: &[std::path::PathBuf]) -> bool {
    dirs.iter().any(|d| d.join("applications/PsyAudio.desktop").is_file())
}

/// The path other programs start PsyAudio with: an AppImage runs from a new mount point on
/// every start, so its own file (`$APPIMAGE`) is the path that stays.
#[cfg(any(test, target_os = "linux"))]
fn launch_path(appimage: Option<std::ffi::OsString>, exe: std::path::PathBuf) -> std::path::PathBuf {
    appimage.filter(|a| !a.is_empty()).map(std::path::PathBuf::from).unwrap_or(exe)
}

#[cfg(target_os = "linux")]
fn own_launch_path() -> Option<std::path::PathBuf> {
    Some(launch_path(std::env::var_os("APPIMAGE"), std::env::current_exe().ok()?))
}

/// The hidden application entry that ties the app id `PsyAudio` to the icon.
#[cfg(target_os = "linux")]
pub fn window_entry_text(exe: &std::path::Path, icon: &std::path::Path) -> String {
    let quoted = exec_quoted(exe);
    format!("[Desktop Entry]\nType=Application\nName=PsyAudio\nIcon={}\nExec=\"{quoted}\" %F\nStartupWMClass=PsyAudio\nNoDisplay=true\n", icon.display())
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

#[cfg(test)]
mod explorer_menu_tests {
    use super::*;

    #[test]
    fn every_readable_extension_gets_its_own_entry() {
        let keys = explorer_menu_keys();
        for e in ["opus", "mp3", "flac", "ape", "wv", "dsf", "mpc"] {
            let want = format!(r"Software\Classes\SystemFileAssociations\.{e}\shell\PsyAudio");
            assert!(keys.contains(&want), "{e}");
        }
        assert_eq!(keys.len(), audio::INPUT_EXTENSIONS.len());
        let unique: std::collections::HashSet<&String> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len(), "no extension twice");
    }

    #[test]
    fn the_old_group_entry_is_known_for_cleanup() {
        assert_eq!(OLD_EXPLORER_MENU_KEY, r"Software\Classes\SystemFileAssociations\audio\shell\PsyAudio");
    }
}

#[cfg(test)]
mod linux_entry_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn data_dirs_follow_xdg() {
        assert_eq!(system_data_dirs(""), vec![PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]);
        assert_eq!(system_data_dirs("/a::/b"), vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn a_package_entry_is_found() {
        let dir = std::env::temp_dir().join(format!("psyaudio-entry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("applications")).unwrap();
        assert!(!package_entry_exists(std::slice::from_ref(&dir)));
        std::fs::write(dir.join("applications/PsyAudio.desktop"), "[Desktop Entry]\n").unwrap();
        assert!(package_entry_exists(&[PathBuf::from("/nowhere"), dir.clone()]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_appimage_is_started_from_its_own_file() {
        let exe = PathBuf::from("/tmp/.mount_PsyAu1/PsyAudio");
        assert_eq!(launch_path(Some("/home/u/PsyAudio.AppImage".into()), exe.clone()), PathBuf::from("/home/u/PsyAudio.AppImage"));
        assert_eq!(launch_path(None, exe.clone()), exe);
        assert_eq!(launch_path(Some("".into()), exe.clone()), exe);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod service_menu_tests {
    use super::service_menu_text;
    use std::path::Path;

    #[test]
    fn the_window_entry_ties_the_app_id_to_the_icon() {
        let t = super::window_entry_text(Path::new("/opt/Psy Audio/PsyAudio"), Path::new("/tmp/i.png"));
        assert!(t.contains("StartupWMClass=PsyAudio\n"));
        assert!(t.contains("Icon=/tmp/i.png\n"));
        assert!(t.contains("NoDisplay=true\n"));
        assert!(t.contains("Exec=\"/opt/Psy Audio/PsyAudio\" %F\n"));
    }

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
