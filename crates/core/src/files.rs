//! The file list: folders expanded to audio files, and outputs that already exist.

use crate::audio;
use crate::convert::{Job, Plan};
use crate::tags::{self, Info, Tags};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Files and folders as dropped or picked, each with its root folder: a folder becomes its
/// audio files (sorted per folder, each folder walked once even through a junction that loops
/// back) with the folder as root unless `root` is given; other files keep `root`.
pub fn expand_paths(paths: &[PathBuf], root: Option<&Path>) -> Vec<(PathBuf, Option<PathBuf>)> {
    let mut found = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut seen = HashSet::new();
            let mut stack = vec![p.clone()];
            while let Some(d) = stack.pop() {
                if !seen.insert(std::fs::canonicalize(&d).unwrap_or_else(|_| d.clone())) {
                    continue;
                }
                if let Ok(rd) = std::fs::read_dir(&d) {
                    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
                    entries.sort();
                    for e in entries {
                        if e.is_dir() {
                            stack.push(e);
                        } else if audio::is_audio_file(&e) {
                            found.push((e, Some(root.map(Path::to_path_buf).unwrap_or_else(|| p.clone()))));
                        }
                    }
                }
            }
        } else if audio::is_audio_file(p) {
            found.push((p.clone(), root.map(Path::to_path_buf)));
        }
    }
    found
}

/// The outputs of a run that already exist, for the question "overwrite?". Tags are only read
/// when the names are built from them (not for a batch with its folder structure, nor for "keep
/// path"); tags already loaded for the list (`known`) are reused.
pub fn existing_outputs(plan: &Plan, jobs: &[Job], known: &HashMap<PathBuf, (Tags, Info)>) -> Vec<PathBuf> {
    let need_tags = plan.names_need_tags();
    let mut existing = Vec::new();
    let positions = crate::convert::folder_positions(jobs.iter().map(|j| j.src.as_path()));
    for j in jobs {
        let (t, i) = match known.get(&j.src) {
            Some(x) => x.clone(),
            None if need_tags => tags::read(&j.src),
            None => (Tags::default(), Info::default()),
        };
        for c in plan.outputs() {
            let d = plan.destination(&j.src, j.root.as_deref(), &t, &i, c, crate::convert::position_of(&positions, &j.src));
            if d.exists() {
                existing.push(d);
            }
        }
    }
    existing
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn folders_expand_sorted_and_only_audio() {
        let dir = std::env::temp_dir().join(format!("psyaudio-expand-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("c")).unwrap();
        for f in ["b.flac", "a.mp3", "x.txt", "c/d.wav"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        let got = expand_paths(std::slice::from_ref(&dir), None);
        let want: Vec<(PathBuf, Option<PathBuf>)> = ["a.mp3", "b.flac", "c/d.wav"].iter().map(|f| (dir.join(f), Some(dir.clone()))).collect();
        assert_eq!(got, want);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
