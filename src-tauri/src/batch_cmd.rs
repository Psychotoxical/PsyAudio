//! The batch converter: finding the files of a folder tree.

use psyaudio_core::convert;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_folder_is_found() {
        let dir = std::env::temp_dir().join(format!("psyaudio-batch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("lied.flac");
        std::fs::write(&file, b"").unwrap();
        let got = first_folder(vec![file.to_string_lossy().into_owned(), dir.to_string_lossy().into_owned()]);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got, Some(dir.to_string_lossy().into_owned()));
        assert_eq!(first_folder(vec![file.to_string_lossy().into_owned()]), None);
    }
}

/// The audio files of `folder` (and its subfolders) with one of `extensions`, sorted by path.
/// Big trees take a moment, so the walk runs off the window's thread.
#[tauri::command]
pub async fn scan_folder(folder: String, subfolders: bool, extensions: Vec<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        convert::scan_folder(Path::new(&folder), subfolders, &extensions).into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// The first of `paths` that is a folder (a folder dropped on the batch page).
#[tauri::command]
pub fn first_folder(paths: Vec<String>) -> Option<String> {
    paths.into_iter().find(|p| Path::new(p).is_dir())
}
