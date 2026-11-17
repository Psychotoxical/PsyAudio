//! The converter list: adding files and folders, reading their tags in the background.

use psyaudio_core::{audio, files, tags};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::ipc::Channel;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize)]
pub struct FileEntry {
    id: u64,
    path: String,
    root: Option<String>,
    size: u64,
}

#[derive(Clone, Serialize)]
pub struct FileInfo {
    id: u64,
    tags: BTreeMap<String, String>,
    info: tags::Info,
    cover: bool,
}

/// Files and folders → audio files with their root folder (folders walked as in egui).
#[tauri::command]
pub async fn add_paths(paths: Vec<String>, root: Option<String>) -> Vec<FileEntry> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let root = root.map(PathBuf::from);
    files::expand_paths(&paths, root.as_deref())
        .into_iter()
        .map(|(p, r)| FileEntry {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            size: std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
            path: p.to_string_lossy().into_owned(),
            root: r.map(|r| r.to_string_lossy().into_owned()),
        })
        .collect()
}

/// Reads tags and length of each file on a worker thread; each file reports on its own.
#[tauri::command]
pub fn read_file_info(items: Vec<(u64, String)>, on: Channel<FileInfo>) {
    std::thread::spawn(move || {
        for (id, path) in items {
            let (t, info) = tags::read(Path::new(&path));
            let cover = !t.art.is_empty();
            if on.send(FileInfo { id, tags: t.fields, info, cover }).is_err() {
                break;
            }
        }
    });
}

fn open_in_file_manager(dir: &Path) {
    let _ = audio::open_external(dir.as_os_str());
}

/// Opens the folder a file lies in (Explorer, the desktop's file manager, Finder).
#[tauri::command]
pub fn reveal_folder(path: String) {
    if let Some(dir) = Path::new(&path).parent() {
        open_in_file_manager(dir);
    }
}

/// Opens a folder, creating it first (the converter's target folder may not exist yet).
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    let dir = Path::new(&path);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    open_in_file_manager(dir);
    Ok(())
}

/// The extensions PsyAudio reads, for the file dialog.
#[tauri::command]
pub fn input_extensions() -> Vec<&'static str> {
    audio::INPUT_EXTENSIONS.to_vec()
}
