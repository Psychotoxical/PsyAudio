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
            assert!(super::set(&sub, &[(None, "Convert with PsyAudio"), (Some("Icon"), r"C:\x\PsyAudio.exe")]));
            assert!(super::set(&format!(r"{sub}\command"), &[(None, r#""C:\x\PsyAudio.exe" "%1""#)]));
            assert!(super::exists(&format!(r"{sub}\command")));
            assert!(super::delete(&sub));
            assert!(!super::exists(&sub), "the key itself is gone");
            assert!(!super::delete(&sub), "a second delete finds nothing");
            super::delete(&base);
        }
    }
}

/// Add or remove the file manager's right-click entry for audio files (current user only): Explorer,
/// Dolphin, or a Finder quick action.
pub fn file_manager_menu(add: bool) -> String {
    #[cfg(windows)]
    {
        let Ok(exe) = std::env::current_exe() else { return crate::i18n::t("Program path unknown").into() };
        // the old single entry would show the item twice where Explorer does use the group
        let had_old = reg::delete(OLD_EXPLORER_MENU_KEY);
        if add {
            let cmd = format!("\"{}\" \"%1\"", exe.display());
            let icon = exe.display().to_string();
            let label = crate::i18n::t("Convert with PsyAudio");
            let ok = explorer_menu_keys()
                .iter()
                .all(|k| reg::set(k, &[(None, label), (Some("Icon"), &icon)]) && reg::set(&format!(r"{k}\command"), &[(None, &cmd)]));
            if ok {
                crate::i18n::t("Context menu set up").into()
            } else {
                crate::i18n::t("Context menu could not be set up").into()
            }
        } else {
            let mut removed = had_old;
            for k in explorer_menu_keys() {
                removed |= reg::delete(&k);
            }
            if removed {
                crate::i18n::t("Context menu removed").into()
            } else {
                crate::i18n::t("No context menu present").into()
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        let Some(data) = dirs::data_dir() else { return crate::i18n::t("Context menu could not be set up").into() };
        let menu = data.join("kio/servicemenus/psyaudio.desktop");
        let icon = data.join("PsyAudio/PsyAudio.png");
        if !add {
            let _ = std::fs::remove_file(&icon);
            return if std::fs::remove_file(&menu).is_ok() {
                crate::i18n::t("Context menu removed").into()
            } else {
                crate::i18n::t("No context menu present").into()
            };
        }
        let Some(exe) = own_launch_path() else { return crate::i18n::t("Program path unknown").into() };
        match write_service_menu(&menu, &icon, &exe) {
            Ok(()) => crate::i18n::t("Context menu set up").into(),
            Err(_) => crate::i18n::t("Context menu could not be set up").into(),
        }
    }
    #[cfg(target_os = "macos")]
    {
        let Some(services) = dirs::home_dir().map(|h| h.join("Library/Services")) else {
            return crate::i18n::t("Context menu could not be set up").into();
        };
        // one entry, in the current language: an earlier one (other language) goes first
        let removed = remove_quick_actions(&services);
        let result = if !add {
            if removed {
                crate::i18n::t("Context menu removed")
            } else {
                crate::i18n::t("No context menu present")
            }
        } else {
            let label = crate::i18n::t("Convert with PsyAudio");
            match write_quick_action(&services.join(format!("{label}.workflow")), label) {
                Ok(()) => crate::i18n::t("Context menu set up"),
                Err(_) => crate::i18n::t("Context menu could not be set up"),
            }
        };
        // the Finder reads the services again
        let _ = std::process::Command::new("/System/Library/CoreServices/pbs").arg("-update").status();
        result.into()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = add;
        crate::i18n::t("Not available on this system").into()
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
    std::fs::write(menu, service_menu_text(exe, icon, crate::i18n::t("Convert with PsyAudio")))?;
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

/// An AppImage that was moved after the menu was set up (Gear Lever, AppImageLauncher) would leave
/// the Dolphin menu pointing at the old file; on every start an existing menu follows the program.
#[cfg(target_os = "linux")]
fn follow_launch_path(menu: &std::path::Path, exe: &std::path::Path) {
    if let Some(text) = std::fs::read_to_string(menu).ok().and_then(|t| menu_with_launch_path(&t, exe)) {
        let _ = std::fs::write(menu, text);
    }
}

/// The menu text with its `Exec` line set to `exe`, or `None` when it already points there.
/// Only that line changes: the label keeps the language it was set up in.
#[cfg(target_os = "linux")]
fn menu_with_launch_path(text: &str, exe: &std::path::Path) -> Option<String> {
    let exec = format!("Exec=\"{}\" %F", exec_quoted(exe));
    if text.lines().any(|l| l == exec) {
        return None;
    }
    let lines: Vec<&str> = text.lines().map(|l| if l.starts_with("Exec=") { exec.as_str() } else { l }).collect();
    Some(lines.join("\n") + "\n")
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
    let Some(exe) = own_launch_path() else { return };
    follow_launch_path(&data.join("kio/servicemenus/psyaudio.desktop"), &exe);
    let dirs = package_data_dirs(&std::env::var("XDG_DATA_DIRS").unwrap_or_default(), std::env::var_os("APPDIR"));
    if package_entry_exists(&dirs) {
        // only the entry PsyAudio wrote itself goes, never one the person made
        if std::fs::read_to_string(&entry).is_ok_and(|t| t.contains("NoDisplay=true\n") && t.contains("StartupWMClass=PsyAudio\n")) {
            let _ = std::fs::remove_file(&entry);
        }
        return;
    }
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

/// The data folders a deb or rpm package could have installed into. An AppImage puts its own
/// mounted folder (`$APPDIR`) in front of `XDG_DATA_DIRS`, and the entry it carries there is no package.
#[cfg(any(test, target_os = "linux"))]
fn package_data_dirs(var: &str, appdir: Option<std::ffi::OsString>) -> Vec<std::path::PathBuf> {
    let appdir = appdir.filter(|a| !a.is_empty()).map(std::path::PathBuf::from);
    system_data_dirs(var).into_iter().filter(|d| appdir.as_ref().is_none_or(|a| !d.starts_with(a))).collect()
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

/// macOS: the Finder entry is a quick action in `~/Library/Services` that opens PsyAudio by its
/// bundle id with the chosen files. Audio types macOS does not know are declared as audio by
/// PsyAudio's own Info.plist, so `public.audio` covers every extension PsyAudio reads.
#[cfg(target_os = "macos")]
fn write_quick_action(dir: &std::path::Path, label: &str) -> std::io::Result<()> {
    let contents = dir.join("Contents");
    std::fs::create_dir_all(&contents)?;
    std::fs::write(contents.join("Info.plist"), QUICK_ACTION_INFO.replace("{label}", &xml_escape(label)))?;
    std::fs::write(contents.join("document.wflow"), QUICK_ACTION_WFLOW)
}

/// Removes every quick action in `services` that opens PsyAudio (in any language); whether one was there.
#[cfg(target_os = "macos")]
fn remove_quick_actions(services: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(services) else { return false };
    let mut removed = false;
    for e in entries.flatten() {
        let p = e.path();
        let ours = p.extension().is_some_and(|x| x == "workflow")
            && std::fs::read_to_string(p.join("Contents/document.wflow")).is_ok_and(|w| w.contains(QUICK_ACTION_COMMAND));
        if ours && std::fs::remove_dir_all(&p).is_ok() {
            removed = true;
        }
    }
    removed
}

#[cfg(target_os = "macos")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(target_os = "macos")]
const QUICK_ACTION_COMMAND: &str = "open -b app.psyaudio \"$@\"";

#[cfg(target_os = "macos")]
const QUICK_ACTION_INFO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{label}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
				<string>public.audio</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#;

/// An Automator "Run Shell Script" action that gets the files as arguments.
#[cfg(target_os = "macos")]
const QUICK_ACTION_WFLOW: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>534</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.path</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>open -b app.psyaudio "$@"</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/zsh</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>5B0E6A4E-2C41-4F7A-9E0B-0E7B5C1A9A01</string>
				<key>OutputUUID</key>
				<string>5B0E6A4E-2C41-4F7A-9E0B-0E7B5C1A9A02</string>
				<key>UUID</key>
				<string>5B0E6A4E-2C41-4F7A-9E0B-0E7B5C1A9A03</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict>
					<key>0</key>
					<dict>
						<key>default value</key>
						<integer>0</integer>
						<key>name</key>
						<string>inputMethod</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>0</string>
					</dict>
					<key>1</key>
					<dict>
						<key>default value</key>
						<false/>
						<key>name</key>
						<string>CheckedForUserDefaultShell</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>1</string>
					</dict>
					<key>2</key>
					<dict>
						<key>default value</key>
						<string></string>
						<key>name</key>
						<string>source</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>2</string>
					</dict>
					<key>3</key>
					<dict>
						<key>default value</key>
						<string></string>
						<key>name</key>
						<string>COMMAND_STRING</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>3</string>
					</dict>
					<key>4</key>
					<dict>
						<key>default value</key>
						<string>/bin/sh</string>
						<key>name</key>
						<string>shell</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>4</string>
					</dict>
				</dict>
				<key>conversionLabel</key>
				<integer>0</integer>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:305.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleID</key>
		<string>com.apple.finder</string>
		<key>applicationBundleIDsByPath</key>
		<dict>
			<key>/System/Library/CoreServices/Finder.app</key>
			<string>com.apple.finder</string>
		</dict>
		<key>applicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>applicationPaths</key>
		<array>
			<string>/System/Library/CoreServices/Finder.app</string>
		</array>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<false/>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<false/>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<false/>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#;

/// SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) = 0x80000001 while converting
/// or ripping, so the PC doesn't go to standby; ES_CONTINUOUS alone (0x80000000) lets it sleep again.
/// macOS: an IOPM assertion against idle sleep (`macos::power_assertion`).
pub const KEEP_AWAKE_FLAGS: u32 = 0x8000_0001;

pub fn keep_awake(on: bool) {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Power::SetThreadExecutionState(if on { KEEP_AWAKE_FLAGS } else { 0x8000_0000 });
    }
    #[cfg(target_os = "macos")]
    crate::macos::power_assertion(on);
    #[cfg(not(any(windows, target_os = "macos")))]
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
    fn an_appimage_does_not_count_its_own_folder_as_a_package() {
        let var = "/tmp/.mount_PsyAu1/usr/share/:/tmp/.mount_PsyAu1/usr/share:/usr/share";
        let appdir = std::ffi::OsString::from("/tmp/.mount_PsyAu1");
        assert_eq!(package_data_dirs(var, Some(appdir)), vec![PathBuf::from("/usr/share")]);
        assert_eq!(package_data_dirs("/a:/b", None), vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(package_data_dirs("/a", Some("".into())), vec![PathBuf::from("/a")]);
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
    fn a_moved_appimage_is_followed_by_the_menu() {
        let old = service_menu_text(Path::new("/home/u/Downloads/PsyAudio.AppImage"), Path::new("/tmp/i.png"), "Convert with PsyAudio");
        let new = super::menu_with_launch_path(&old, Path::new("/home/u/AppImages/psyaudio.appimage")).unwrap();
        assert_eq!(new, service_menu_text(Path::new("/home/u/AppImages/psyaudio.appimage"), Path::new("/tmp/i.png"), "Convert with PsyAudio"));
        assert!(super::menu_with_launch_path(&new, Path::new("/home/u/AppImages/psyaudio.appimage")).is_none());
    }

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

#[cfg(all(test, target_os = "macos"))]
mod quick_action_tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("psyaudio-qa-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// The quick action starts PsyAudio by its bundle id, so it keeps working after the app moved,
    /// and hands over the files (an already running PsyAudio gets them too).
    #[test]
    fn workflow_uses_bundle_id() {
        let dir = temp("write").join("Mit PsyAudio konvertieren.workflow");
        write_quick_action(&dir, "Convert with PsyAudio").unwrap();
        let wflow = std::fs::read_to_string(dir.join("Contents/document.wflow")).unwrap();
        assert!(wflow.contains("open -b app.psyaudio \"$@\""));
        assert!(!wflow.contains(".app/Contents/MacOS"));
        let plist = std::fs::read_to_string(dir.join("Contents/Info.plist")).unwrap();
        assert!(plist.contains("NSServices") && plist.contains("public.audio"));
        assert!(plist.contains("<string>Mit PsyAudio konvertieren</string>"));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn removing_the_quick_action_deletes_the_bundle() {
        let base = temp("remove");
        let dir = base.join("Convert with PsyAudio.workflow");
        write_quick_action(&dir, "Convert with PsyAudio").unwrap();
        assert!(remove_quick_actions(&base));
        assert!(!dir.exists());
        assert!(!remove_quick_actions(&base), "nothing left to remove");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_label_is_escaped_for_the_plist() {
        let dir = temp("escape").join("x.workflow");
        write_quick_action(&dir, "A & <B>").unwrap();
        let plist = std::fs::read_to_string(dir.join("Contents/Info.plist")).unwrap();
        assert!(plist.contains("A &amp; &lt;B&gt;"));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
