//! The PsyAudio window: starts Tauri, hands files over between instances and from drops.

mod commands;
mod effects_cmd;
mod files_cmd;
mod jobs;
mod naming_cmd;
mod run_cmd;
mod settings_cmd;
mod state;
mod tools_cmd;

use serde::Serialize;
use state::AppState;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, DragDropEvent, Emitter, Manager, WindowEvent};

/// The files on a command line: the program itself and `--…` switches are skipped, relative
/// paths are taken from `cwd` (the Explorer menu and a terminal both start with a working folder).
pub fn file_args(args: impl IntoIterator<Item = OsString>, cwd: &Path) -> Vec<PathBuf> {
    args.into_iter()
        .skip(1)
        .filter(|a| !a.to_string_lossy().starts_with("--"))
        .map(|a| {
            let p = PathBuf::from(a);
            if p.is_absolute() {
                p
            } else {
                cwd.join(p)
            }
        })
        .collect()
}

#[derive(Clone, Serialize)]
struct FilesAdded {
    paths: Vec<String>,
    source: &'static str,
}

#[derive(Clone, Serialize)]
struct DragState {
    active: bool,
}

/// Hands files to the frontend, or keeps them until it listens.
fn send_files(app: &AppHandle, files: Vec<PathBuf>, source: &'static str) {
    if files.is_empty() {
        return;
    }
    let state = app.state::<AppState>();
    if state.frontend_ready.load(Ordering::Relaxed) {
        let paths = files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        let _ = app.emit_to("main", "files-added", FilesAdded { paths, source });
    } else if let Ok(mut f) = state.startup_files.lock() {
        f.extend(files);
    }
}

pub fn run() {
    let args: Vec<OsString> = std::env::args_os().collect();
    let demo = state::Demo::from_args(&args);
    let state = AppState { demo: demo.clone(), ..AppState::default() };
    if let Ok(mut f) = state.startup_files.lock() {
        *f = file_args(args, &std::env::current_dir().unwrap_or_default());
    }

    let mut builder = tauri::Builder::default();
    // first, so a second start hands its files over before anything else runs; the sample mode
    // runs on its own, next to a PsyAudio that is already open
    if demo.is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            send_files(app, file_args(args.into_iter().map(OsString::from), Path::new(&cwd)), "instance");
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::frontend_ready,
            commands::file_manager_menu,
            settings_cmd::load_settings,
            settings_cmd::save_settings,
            settings_cmd::demo_state,
            files_cmd::add_paths,
            files_cmd::read_file_info,
            files_cmd::reveal_folder,
            files_cmd::open_folder,
            files_cmd::input_extensions,
            tools_cmd::encode_options,
            tools_cmd::tool_status,
            tools_cmd::codec_problems,
            tools_cmd::plan_requirement,
            tools_cmd::estimate_kbps,
            tools_cmd::set_language,
            naming_cmd::naming_options,
            naming_cmd::preview_name,
            naming_cmd::tags_from_name,
            effects_cmd::effect_catalogue,
            effects_cmd::effect_lines,
            run_cmd::check_existing,
            run_cmd::start_conversion,
            run_cmd::cancel_conversion,
            run_cmd::clear_run_title,
        ])
        .setup(|app| {
            // show the window even if the frontend never reports ready
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(4));
                if !handle.state::<AppState>().frontend_ready.load(Ordering::Relaxed) {
                    if let Some(w) = handle.get_webview_window("main") {
                        let _ = w.show();
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::DragDrop(drag) = event {
                let app = window.app_handle();
                match drag {
                    DragDropEvent::Enter { .. } => {
                        let _ = window.emit_to("main", "drag-state", DragState { active: true });
                    }
                    DragDropEvent::Drop { paths, .. } => {
                        let _ = window.emit_to("main", "drag-state", DragState { active: false });
                        send_files(app, paths.clone(), "drop");
                    }
                    DragDropEvent::Leave => {
                        let _ = window.emit_to("main", "drag-state", DragState { active: false });
                    }
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to start PsyAudio");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::Path;

    fn os(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn file_args_skips_program_and_flags() {
        let cwd = Path::new(r"C:\x");
        assert_eq!(file_args(os(&["PsyAudio.exe", "--demo", "a.flac"]), cwd), vec![cwd.join("a.flac")]);
    }

    #[test]
    fn file_args_keeps_special_characters() {
        let cwd = std::env::temp_dir();
        let name = "Ä [live] b.mp3";
        let abs = cwd.join(name);
        let got = file_args(vec![OsString::from("PsyAudio"), abs.clone().into_os_string(), OsString::from(name)], &cwd);
        assert_eq!(got, vec![abs.clone(), abs]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn file_args_survives_non_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let bad = OsString::from_vec(vec![0x66, 0xff]);
        let got = file_args(vec![OsString::from("PsyAudio"), bad], Path::new("/tmp"));
        assert_eq!(got.len(), 1);
    }
}
