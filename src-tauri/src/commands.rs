//! Commands the frontend calls.

use crate::state::AppState;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, State};

/// Shows the window and returns the files PsyAudio was started with; later files arrive as
/// `files-added` events.
#[tauri::command]
pub fn frontend_ready(app: AppHandle, state: State<'_, AppState>) -> Vec<String> {
    state.frontend_ready.store(true, Ordering::Relaxed);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
    let files = std::mem::take(&mut *state.startup_files.lock().unwrap_or_else(|e| e.into_inner()));
    files.iter().map(|p| p.to_string_lossy().into_owned()).collect()
}

/// Adds or removes "Convert with PsyAudio" in Explorer or Dolphin; returns the message.
/// Async, so the window keeps drawing while one key per extension is written.
#[tauri::command]
pub async fn file_manager_menu(add: bool) -> String {
    psyaudio_core::integration::file_manager_menu(add)
}
