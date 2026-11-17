//! The conversion run: a pool of workers that decode, process, encode and tag each file.

use crate::audio::{self, Audio};
use crate::dsp::{self, Effect};
use crate::encode::{self, Codec, EncSettings, Env, Tools};
use crate::ffmpeg::FfmpegSet;
use crate::loudness;
use crate::naming;
use crate::settings::OutputMode;
use crate::tags::{self, Tags};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct Job {
    pub id: usize,
    pub src: PathBuf,
    pub root: Option<PathBuf>,
    /// Tags to use instead of the file's own (CD ripper).
    pub tags: Option<Tags>,
    /// Audio already in memory (CD ripper).
    pub audio: Option<Arc<Audio>>,
}

#[derive(Clone)]
pub struct Plan {
    pub codec: Codec,
    pub enc: EncSettings,
    pub effects: Vec<Effect>,
    pub output_mode: OutputMode,
    pub output_folder: PathBuf,
    /// Chips that build folder and file names (see `naming`).
    pub scheme: Vec<String>,
    /// Batch run: keep the folders below the batch folder and the source file names.
    pub keep_structure: bool,
    pub overwrite: bool,
    /// Existing outputs the person ticked in the overwrite list.
    pub overwrite_list: Vec<PathBuf>,
    pub copy_tags: bool,
    pub copy_art: bool,
    pub threads: usize,
    pub low_priority: bool,
    /// The FFmpeg for each job.
    pub ffmpeg: FfmpegSet,
    pub tools: Tools,
    pub temp: PathBuf,
    /// Chips for "tags from file name".
    pub tag_scheme: Vec<String>,
}

pub enum Msg {
    Started(usize),
    Progress(usize, f32, &'static str),
    Done { id: usize, outputs: Vec<PathBuf>, audio_secs: f64, wall_secs: f64, note: String },
    Skipped(usize, String),
    Failed(usize, String),
    Report(String),
    Finished,
}

/// Checks an output location on a drive letter before converting.
/// The drive must exist (GetLogicalDrives), must not be a CD drive (GetDriveTypeW = 5), needs
/// at least `OUTPUT_MIN_FREE` bytes free (GetDiskFreeSpaceExW) and must accept a test file
/// "_tmp_<n>" (created and deleted again). Returns the message to show, or None when it's fine.
pub fn output_location_problem(folder: &Path) -> Option<String> {
    let s = folder.to_string_lossy().to_string();
    let b = s.as_bytes();
    if b.len() < 3 || b[1] != b':' || !b[0].is_ascii_alphabetic() {
        return None;
    }
    // drive letter, CD drive and free space can only be asked on Windows
    #[cfg(windows)]
    {
        let letter = b[0].to_ascii_uppercase();
        let root = format!("{}:\\", letter as char);
        use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        let mask = unsafe { GetLogicalDrives() };
        if mask & (1 << (letter - b'A')) == 0 {
            return Some(crate::i18n::tf(crate::i18n::t("Ziellaufwerk {0} gibt es nicht"), &[("0", root.clone())]));
        }
        if unsafe { GetDriveTypeW(wide.as_ptr()) } == 5 {
            return Some(crate::i18n::tf(crate::i18n::t("Ziellaufwerk {0} ist ein CD-Laufwerk"), &[("0", root.clone())]));
        }
        let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
        let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) } != 0;
        if ok && free < OUTPUT_MIN_FREE {
            return Some(crate::i18n::tf(crate::i18n::t("Ziellaufwerk {0} ist voll"), &[("0", root.clone())]));
        }
    }
    // the folder may not exist yet: test the nearest folder that does
    let mut dir = folder.to_path_buf();
    while !dir.is_dir() {
        if !dir.pop() {
            return None;
        }
    }
    let probe = dir.join(format!("_tmp_{}", std::process::id()));
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(f) => {
            drop(f);
            let _ = std::fs::remove_file(&probe);
            None
        }
        Err(_) => Some(crate::i18n::t("In den Zielordner kann nicht geschrieben werden – ist er schreibgeschützt oder fehlen Schreibrechte?").to_string()),
    }
}

/// Free space an output drive needs at least (256000 bytes).
pub const OUTPUT_MIN_FREE: u64 = 256_000;

impl Plan {
    fn env(&self) -> Env<'_> {
        Env { ffmpeg: &self.ffmpeg, tools: &self.tools, temp: &self.temp }
    }

    /// Codecs that produce files in this run.
    pub fn outputs(&self) -> Vec<Codec> {
        match self.codec {
            Codec::Multi => self.enc.multi.clone(),
            Codec::ChannelSplit | Codec::LengthSplit => vec![self.enc.split_codec],
            c if c.is_tool() => Vec::new(),
            c => vec![c],
        }
    }

    /// Destination path (with extension) for one source and codec.
    /// Whether destinations depend on tags (else they need not be read to know them).
    pub fn names_need_tags(&self) -> bool {
        !self.keep_structure
            && self.output_mode != OutputMode::PreservePath
            && self.scheme.iter().any(|t| naming::DATA.contains(&t.as_str()) && t != "filename")
    }

    /// Error text when two files of a run get the same destination `name`. The chips are only
    /// worth adjusting when they name the file; a structure run (batch) or a run keeping the
    /// source path uses the source's own name, so the clash comes from the source itself.
    pub fn name_clash(&self, name: &str, root: Option<&Path>) -> String {
        let source_names = (self.keep_structure && root.is_some()) || self.output_mode == OutputMode::PreservePath;
        let text = if source_names {
            "Eine andere Datei dieses Laufs bekommt schon den Namen {0} – die Quelle enthält diesen Namen zweimal, nur mit anderer Endung"
        } else {
            "Eine andere Datei dieses Laufs bekommt schon den Namen {0} – Bausteine für die Namen anpassen"
        };
        crate::i18n::tf(crate::i18n::t(text), &[("0", name.to_string())])
    }

    /// `t` are the source's own tags: names follow them after the run's tag processing, also
    /// when the output gets no tags, so preview, overwrite question and worker agree.
    /// `position` is the source's number and count in its folder (see [`folder_positions`]).
    pub fn destination(&self, src: &Path, root: Option<&Path>, t: &Tags, info: &tags::Info, codec: Codec, position: Option<(usize, usize)>) -> PathBuf {
        let _ = info;
        let mut processed = t.clone();
        apply_tag_processing(&self.effects, &mut processed, position);
        let t = &processed;
        let stem = || {
            let s = naming::clean_value(&src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
            if s.trim().is_empty() {
                "Track".to_string()
            } else {
                s.trim().to_string()
            }
        };
        // source folders as plain names: below the batch folder, or the whole path without the drive
        let folders = |from: Option<&Path>| -> Vec<String> {
            let parent = src.parent().unwrap_or(Path::new(""));
            let rel = match from {
                Some(r) => parent.strip_prefix(r).map(|p| p.to_path_buf()).unwrap_or_default(),
                None => parent.to_path_buf(),
            };
            rel.components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(n) => Some(n.to_string_lossy().to_string()),
                    _ => None,
                })
                .collect()
        };
        let (mut rel, file) = if self.keep_structure && root.is_some() {
            (folders(root), String::new())
        } else if self.output_mode == OutputMode::PreservePath {
            (folders(None), String::new())
        } else {
            naming::names(&self.scheme, t, src)
        };
        // no file name from the chips (or a structure run): the source's own name
        rel.push(if file.is_empty() { stem() } else { file });
        let base = match self.output_mode {
            OutputMode::SourceFolder if !self.keep_structure => src.parent().map(|p| p.to_path_buf()).unwrap_or_default(),
            _ => self.output_folder.clone(),
        };
        let mut p = base;
        for part in rel {
            p.push(part);
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        // ALAC and AAC share .m4a: in a multi run keep both apart
        let suffix = if self.codec == Codec::Multi && codec == Codec::Alac && self.enc.multi.contains(&Codec::AacM4a) { " (ALAC)" } else { "" };
        p.set_file_name(format!("{name}{suffix}.{}", codec.ext()));
        p
    }
}

/// A path as Windows compares it: case-insensitive, either slash.
fn path_key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// Whether two paths name the same file (also when they differ in case or slashes,
/// or reach the same file another way).
pub fn same_path(a: &Path, b: &Path) -> bool {
    if path_key(a) == path_key(b) {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => path_key(&x) == path_key(&y),
        _ => false,
    }
}

fn tmp_name(dest: &Path) -> PathBuf {
    let ext = dest.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
    let stem = dest.file_stem().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
    dest.with_file_name(format!("{stem}.psyaudio-tmp.{ext}"))
}

/// Per album: its loudness blocks, its peak and the files written, for album gain at the end.
type AlbumBlocks = BTreeMap<String, (Vec<f64>, f32, Vec<PathBuf>)>;

/// (album, artist) of each source in one folder, filled once by whichever thread comes first.
type FolderAlbums = std::sync::OnceLock<Vec<(String, String)>>;

struct Shared {
    queue: Mutex<VecDeque<Job>>,
    /// Number and count of each source in its folder, in list order (see [`folder_positions`]).
    positions: HashMap<String, (usize, usize)>,
    /// The sources of each folder with the tags the ripper handed over, for the album artist.
    by_folder: HashMap<PathBuf, Vec<(PathBuf, Option<Tags>)>>,
    /// (album, artist) of every source per folder after the tag processing, read once per folder.
    folder_albums: Mutex<HashMap<PathBuf, Arc<FolderAlbums>>>,
    album_blocks: Mutex<AlbumBlocks>,
    playlist: Mutex<Vec<(usize, PathBuf)>>,
    /// Destinations already taken in this run (`path_key`), so two sources never share one.
    reserved: Mutex<std::collections::HashSet<String>>,
}

impl Shared {
    fn new(jobs: Vec<Job>) -> Shared {
        let mut by_folder: HashMap<PathBuf, Vec<(PathBuf, Option<Tags>)>> = HashMap::new();
        for j in &jobs {
            by_folder.entry(j.src.parent().map(Path::to_path_buf).unwrap_or_default()).or_default().push((j.src.clone(), j.tags.clone()));
        }
        Shared {
            positions: folder_positions(jobs.iter().map(|j| j.src.as_path())),
            by_folder,
            folder_albums: Mutex::new(HashMap::new()),
            queue: Mutex::new(jobs.into_iter().collect()),
            album_blocks: Mutex::new(BTreeMap::new()),
            playlist: Mutex::new(Vec::new()),
            reserved: Mutex::new(std::collections::HashSet::new()),
        }
    }
}

impl Shared {
    fn position(&self, src: &Path) -> Option<(usize, usize)> {
        position_of(&self.positions, src)
    }

    /// The album artist for a source without one: the artist when every track of its album in
    /// the folder has the same one, otherwise "Various Artists"; without an album its own artist.
    /// All tags are compared as the run writes them, after the tag processing in `effects`.
    fn album_artist(&self, src: &Path, t: &Tags, effects: &[Effect]) -> String {
        let album = t.get("album").trim().to_string();
        let own = t.get("artist").trim().to_string();
        if album.is_empty() {
            return own;
        }
        let dir = src.parent().map(Path::to_path_buf).unwrap_or_default();
        let cell = self.folder_albums.lock().unwrap().entry(dir.clone()).or_default().clone();
        let entries = cell.get_or_init(|| {
            self.by_folder
                .get(&dir)
                .map(|files| {
                    files
                        .iter()
                        .map(|(p, given)| {
                            let mut other = given.clone().unwrap_or_else(|| tags::read(p).0);
                            apply_tag_processing(effects, &mut other, self.position(p));
                            (other.get("album").trim().to_string(), other.get("artist").trim().to_string())
                        })
                        .collect()
                })
                .unwrap_or_default()
        });
        let mut artists: std::collections::BTreeSet<&str> =
            entries.iter().filter(|(al, ar)| *al == album && !ar.is_empty()).map(|(_, ar)| ar.as_str()).collect();
        if !own.is_empty() {
            artists.insert(&own);
        }
        match artists.len() {
            0 => String::new(),
            1 => artists.into_iter().next().unwrap_or_default().to_string(),
            _ => VARIOUS_ARTISTS.to_string(),
        }
    }
}

/// Whether the tag processing removes the album artist (then it is not filled in again).
fn removes_album_artist(effects: &[Effect]) -> bool {
    effects.iter().any(|e| matches!(e, Effect::TagProcessing { field, mode: 1, .. } if matches!(field.trim(), "album artist" | "*" | "alle")))
}

/// Number and count of each source within its folder, in list order (key: `path_key`).
pub fn folder_positions<'a>(sources: impl IntoIterator<Item = &'a Path>) -> HashMap<String, (usize, usize)> {
    let mut by_dir: HashMap<PathBuf, Vec<&Path>> = HashMap::new();
    for p in sources {
        by_dir.entry(p.parent().map(Path::to_path_buf).unwrap_or_default()).or_default().push(p);
    }
    let mut out = HashMap::new();
    for files in by_dir.values() {
        for (i, p) in files.iter().enumerate() {
            out.insert(path_key(p), (i + 1, files.len()));
        }
    }
    out
}

/// The position of `src` in a map from [`folder_positions`].
pub fn position_of(positions: &HashMap<String, (usize, usize)>, src: &Path) -> Option<(usize, usize)> {
    positions.get(&path_key(src)).copied()
}

/// What the album artist of a compilation is set to (the name MusicBrainz uses).
pub const VARIOUS_ARTISTS: &str = "Various Artists";

/// One file at a time from the caller's thread (used by the CD ripper).
pub struct Batch {
    shared: Shared,
}

impl Default for Batch {
    fn default() -> Self {
        Self::new()
    }
}

impl Batch {
    pub fn new() -> Batch {
        Batch { shared: Shared::new(Vec::new()) }
    }
    /// A batch that knows all files of the run (the album artist compares them).
    pub fn with_jobs(jobs: Vec<Job>) -> Batch {
        Batch { shared: Shared::new(jobs) }
    }
    pub fn run(&self, plan: &Plan, job: &Job, cancel: &AtomicBool, tx: &Sender<Msg>) -> Result<Vec<PathBuf>, String> {
        match run_job(plan, job, cancel, tx, &self.shared)? {
            JobResult::Done { outputs, .. } => Ok(outputs),
            JobResult::Skipped(r) => Err(r),
        }
    }
    pub fn finish(&self, plan: &Plan, tx: &Sender<Msg>) {
        finish(plan, &self.shared, tx);
    }
}

/// Start the run in background threads; messages go to `tx`.
pub fn start(plan: Plan, jobs: Vec<Job>, cancel: Arc<AtomicBool>, tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let shared = Arc::new(Shared::new(jobs));
        let plan = Arc::new(plan);
        let n = plan.threads.max(1);
        let mut handles = Vec::new();
        for _ in 0..n {
            let (plan, shared, cancel, tx) = (plan.clone(), shared.clone(), cancel.clone(), tx.clone());
            handles.push(std::thread::spawn(move || {
                if plan.low_priority {
                    lower_priority();
                }
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let job = shared.queue.lock().unwrap().pop_front();
                    let Some(job) = job else { break };
                    let id = job.id;
                    let _ = tx.send(Msg::Started(id));
                    let t0 = Instant::now();
                    match run_job(&plan, &job, &cancel, &tx, &shared) {
                        Ok(JobResult::Done { outputs, audio_secs, note }) => {
                            let _ = tx.send(Msg::Done { id, outputs, audio_secs, wall_secs: t0.elapsed().as_secs_f64(), note });
                        }
                        Ok(JobResult::Skipped(r)) => {
                            let _ = tx.send(Msg::Skipped(id, r));
                        }
                        Err(e) => {
                            let _ = tx.send(Msg::Failed(id, e));
                        }
                    }
                }
            }));
        }
        for h in handles {
            let _ = h.join();
        }
        if !cancel.load(Ordering::Relaxed) {
            finish(&plan, &shared, &tx);
        }
        let _ = tx.send(Msg::Finished);
    });
}

fn lower_priority() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

/// The "done" sound of the system: Windows' message beep, the Glass sound on macOS, the
/// freedesktop "complete" sound on Linux (through whichever player the desktop has).
pub fn beep() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(0x40);
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("afplay");
        cmd.arg("/System/Library/Sounds/Glass.aiff");
        if let Ok(mut child) = audio::hidden(&mut cmd).spawn() {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let sound = "/usr/share/sounds/freedesktop/stereo/complete.oga";
        let tries: [(&str, &[&str]); 3] = [("canberra-gtk-play", &["-i", "complete"]), ("paplay", &[sound]), ("pw-play", &[sound])];
        for (prog, args) in tries {
            let mut cmd = std::process::Command::new(prog);
            cmd.args(args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            if let Ok(mut child) = audio::hidden(&mut cmd).spawn() {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                break;
            }
        }
    }
}

/// After all files: album ReplayGain, playlists, sound.
fn finish(plan: &Plan, shared: &Shared, tx: &Sender<Msg>) {
    // album gain: from the ReplayGain effect, or the tool's fixed values (−18 LUFS)
    let album_rg = if plan.codec == Codec::ReplayGainOnly {
        Some((-18.0_f32, 25.0_f32, true))
    } else {
        plan.effects.iter().find_map(|e| match e {
            Effect::ReplayGain { target_lufs, max_gain, album: true, clip_prevention, .. } => Some((*target_lufs, *max_gain, *clip_prevention)),
            _ => None,
        })
    };
    if let Some((target_lufs, max_gain, clip_prevention)) = album_rg {
        {
            let albums = shared.album_blocks.lock().unwrap();
            for (name, (blocks, peak, files)) in albums.iter() {
                let lufs = loudness::integrated(blocks);
                let gain = loudness::replaygain(lufs, *peak, target_lufs as f64, max_gain as f64, clip_prevention);
                for f in files {
                    let (mut t, _) = tags::read(f);
                    t.set("replaygain_album_gain", format!("{gain:+.2} dB"));
                    t.set("replaygain_album_peak", format!("{peak:.6}"));
                    let _ = tags::write(f, &t, false);
                }
                let _ = tx.send(Msg::Report(crate::i18n::tf(
                    crate::i18n::t(crate::i18n::plural(
                        files.len() as u64,
                        "Album-ReplayGain „{name}“: {lufs} LUFS » {gain} dB ({0} Datei)",
                        "Album-ReplayGain „{name}“: {lufs} LUFS » {gain} dB ({0} Dateien)",
                    )),
                    &[("name", name.to_string()), ("lufs", format!("{lufs:.2}")), ("gain", format!("{gain:+.2}")), ("0", format!("{}", files.len()))],
                )));
            }
        }
    }
    for e in &plan.effects {
        if let Effect::PlaylistWriter { kind, same_folder } = e {
            let mut list = shared.playlist.lock().unwrap().clone();
            list.sort_by_key(|x| x.0);
            let mut by_folder: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
            for (_, p) in list {
                let key = if *same_folder { p.parent().map(|x| x.to_path_buf()).unwrap_or_default() } else { plan.output_folder.clone() };
                by_folder.entry(key).or_default().push(p);
            }
            for (folder, files) in by_folder {
                let name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| crate::i18n::t("Wiedergabeliste").into());
                let ext = ["m3u8", "m3u", "pls"][*kind as usize % 3];
                let path = folder.join(format!("{name}.{ext}"));
                let rel = |f: &PathBuf| f.strip_prefix(&folder).map(|r| r.display().to_string()).unwrap_or_else(|_| f.display().to_string());
                let text = if *kind == 2 {
                    let mut s = String::from("[playlist]\r\n");
                    for (i, f) in files.iter().enumerate() {
                        s += &format!("File{}={}\r\n", i + 1, rel(f));
                    }
                    s + &format!("NumberOfEntries={}\r\nVersion=2\r\n", files.len())
                } else {
                    let mut s = String::from("#EXTM3U\r\n");
                    for f in &files {
                        s += &format!("{}\r\n", rel(f));
                    }
                    s
                };
                if std::fs::write(&path, text).is_ok() {
                    let _ = tx.send(Msg::Report(crate::i18n::tf(crate::i18n::t("Wiedergabeliste geschrieben: {0}"), &[("0", format!("{}", path.display()))])));
                }
            }
        }
    }
    if plan.effects.contains(&Effect::PlaySound) {
        beep();
    }
}

enum JobResult {
    Done { outputs: Vec<PathBuf>, audio_secs: f64, note: String },
    Skipped(String),
}

/// The plan for a run with the current settings (a batch sets `keep_structure` afterwards).
pub fn plan_from_settings(s: &crate::settings::Settings, codec: Codec, ffmpeg: FfmpegSet) -> Plan {
    Plan {
        codec,
        enc: s.enc.clone(),
        effects: effects_for(codec, &s.effects),
        output_mode: s.output_mode,
        output_folder: PathBuf::from(&s.output_folder),
        scheme: s.name_scheme.clone(),
        keep_structure: false,
        overwrite: s.overwrite == crate::settings::Overwrite::Always,
        overwrite_list: Vec::new(),
        copy_tags: s.copy_tags,
        copy_art: s.copy_art,
        threads: s.threads.max(1),
        low_priority: s.low_priority,
        ffmpeg,
        tools: s.tools.clone(),
        temp: s.temp_dir(),
        tag_scheme: s.tag_scheme.clone(),
    }
}

/// The effects a run applies. Tools that only read or tag the source files ignore the
/// converter's effect chain (a ReplayGain value must be measured on the file as it is);
/// "Tags aktualisieren" keeps the tag processing it exists for.
pub fn effects_for(codec: Codec, effects: &[Effect]) -> Vec<Effect> {
    match codec {
        Codec::ReplayGainOnly | Codec::AudioInfo | Codec::AudioCrc | Codec::TestConversion | Codec::TagFromFilename => Vec::new(),
        Codec::TagUpdate => effects.iter().filter(|e| matches!(e, Effect::TagProcessing { .. })).cloned().collect(),
        _ => effects.to_vec(),
    }
}

#[cfg(test)]
mod effects_for_tests {
    use super::*;
    #[test]
    fn tools_on_source_files_ignore_the_effect_chain() {
        let chain = vec![
            Effect::Fade { in_ms: 100, out_ms: 100 },
            Effect::TagProcessing { field: "genre".into(), value: "Rock".into(), mode: 0 },
            Effect::DeleteSource,
        ];
        for c in [Codec::ReplayGainOnly, Codec::AudioInfo, Codec::AudioCrc, Codec::TestConversion, Codec::TagFromFilename] {
            assert!(effects_for(c, &chain).is_empty(), "{c:?}");
        }
        assert_eq!(effects_for(Codec::TagUpdate, &chain), vec![chain[1].clone()]);
        assert_eq!(effects_for(Codec::Flac, &chain), chain);
    }
}

/// Audio files below `root` with one of `exts` (any case), sorted; with `subfolders` the whole
/// tree, each folder once (a junction that loops back is not followed again).
pub fn scan_folder(root: &Path, subfolders: bool, exts: &[String]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if !seen.insert(std::fs::canonicalize(&dir).map(|c| path_key(&c)).unwrap_or_else(|_| path_key(&dir))) {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                if subfolders {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|x| exts.iter().any(|e| x.to_string_lossy().eq_ignore_ascii_case(e))) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod scan_tests {
    use super::*;
    #[test]
    fn scan_finds_matching_files_in_subfolders() {
        let d = std::env::temp_dir().join(format!("psyaudio-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("CD1")).unwrap();
        for f in ["a.FLAC", "b.txt", "CD1/c.flac", "CD1/d.mp3"] {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        let exts = vec!["flac".to_string()];
        assert_eq!(scan_folder(&d, true, &exts), vec![d.join("CD1").join("c.flac"), d.join("a.FLAC")]);
        assert_eq!(scan_folder(&d, false, &exts), vec![d.join("a.FLAC")]);
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// Which album a file belongs to for album ReplayGain (album artist, else artist, plus album).
fn album_key(t: &Tags) -> String {
    let artist = if t.get("album artist").is_empty() { t.get("artist") } else { t.get("album artist") };
    format!("{artist} – {}", t.get("album"))
}

/// Fields "Tag-Bearbeitung" fills in by itself: the file's number and the count in its folder.
pub const AUTO_FIELDS: &[&str] = &["track", "track_total"];

/// `<artist>`, `<album>` … in a value become the file's tags (names as in file naming).
fn expand_placeholders(value: &str, source: &Tags) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('>') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = after[..close].trim().to_ascii_lowercase();
        match crate::naming::tag_key(&name).or_else(|| tags::FIELDS.iter().find(|f| f.0 == name).map(|f| f.0)) {
            Some(key) => out.push_str(source.get(key)),
            None => {
                // not a tag: keep the "<" and look again right after it ("a <3 <artist>")
                out.push('<');
                rest = after;
                continue;
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// "Tag-Bearbeitung": set, fill in or remove a field. Titelnummer and Titel gesamt count by
/// themselves (`position`: number and count in the folder); other values may name other tags.
fn apply_tag_processing(effects: &[Effect], t: &mut Tags, position: Option<(usize, usize)>) {
    for e in effects {
        if let Effect::TagProcessing { field, value, mode } = e {
            let f = field.trim().to_ascii_lowercase();
            if *mode == 1 {
                if f == "*" || f == "alle" {
                    t.fields.clear();
                    t.art.clear();
                } else if f == "cover" || f == "bild" {
                    t.art.clear();
                } else {
                    t.fields.remove(&f);
                }
                continue;
            }
            if f == "*" || f == "alle" || f == "cover" || f == "bild" || f.is_empty() {
                continue; // only removing makes sense for these
            }
            // the automatic numbers always number every file (some only would give gaps and doubles)
            if *mode == 2 && !AUTO_FIELDS.contains(&f.as_str()) && !t.get(&f).is_empty() {
                continue;
            }
            let new = match (f.as_str(), position) {
                ("track", Some((n, _))) => n.to_string(),
                ("track_total", Some((_, total))) => total.to_string(),
                ("track" | "track_total", None) => continue,
                _ => expand_placeholders(value, t),
            };
            t.set(&f, new);
        }
    }
}

/// Read tags from a file name with a chip scheme: data chips are fields, separators split them.
pub fn tags_from_scheme(name: &str, scheme: &[String]) -> Vec<(String, String)> {
    // separators in a row form one literal, so "Große Platte (1992)" splits at " (" only
    let mut parts: Vec<(bool, String)> = Vec::new();
    for token in scheme {
        match crate::naming::tag_key(token) {
            Some(key) => parts.push((true, key.to_string())),
            None if crate::naming::SEPARATORS.contains(&token.as_str()) => match parts.last_mut() {
                Some((false, lit)) => lit.push_str(token),
                _ => parts.push((false, token.clone())),
            },
            None => {}
        }
    }
    parse_parts(name, &parts)
}

#[cfg(test)]
mod safety_tests {
    use super::*;

    fn wav_plan(dir: &Path, mode: OutputMode, scheme: &[&str], delete_source: bool) -> Plan {
        Plan {
            codec: Codec::Wave,
            enc: EncSettings::default(),
            effects: if delete_source { vec![Effect::DeleteSource] } else { Vec::new() },
            output_mode: mode,
            output_folder: dir.join("out"),
            scheme: scheme.iter().map(|s| s.to_string()).collect(),
            keep_structure: false,
            overwrite: false,
            overwrite_list: Vec::new(),
            copy_tags: true,
            copy_art: true,
            threads: 1,
            low_priority: false,
            ffmpeg: FfmpegSet::default(),
            tools: Tools::default(),
            temp: std::env::temp_dir(),
            tag_scheme: Vec::new(),
        }
    }
    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("psyaudio-safety-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn write_wav(p: &Path) {
        let a = Audio { rate: 8000, channels: 1, bits: 16, float: false, lossless: true, data: vec![0.1; 800] };
        audio::write_wav(p, &a, audio::PcmFormat::S16).unwrap();
    }
    fn run(plan: &Plan, batch: &Batch, src: &Path, id: usize) -> Result<Vec<PathBuf>, String> {
        let (tx, _rx) = std::sync::mpsc::channel();
        batch.run(plan, &Job { id, src: src.to_path_buf(), root: None, tags: None, audio: None }, &AtomicBool::new(false), &tx)
    }

    #[test]
    #[cfg_attr(not(windows), ignore = "Windows paths: drive letters, backslashes, case-insensitive names")]
    fn paths_differing_in_case_are_the_same_file() {
        assert!(same_path(Path::new("C:\\Musik\\Song.MP3"), Path::new("c:/musik/song.mp3")));
        assert!(!same_path(Path::new("C:\\Musik\\a.mp3"), Path::new("C:\\Musik\\b.mp3")));
    }

    #[test]
    #[cfg_attr(not(windows), ignore = "Windows paths: drive letters, backslashes, case-insensitive names")]
    fn source_with_upper_case_extension_is_never_overwritten() {
        let d = dir("case");
        let src = d.join("Lied.WAV");
        write_wav(&src);
        let before = std::fs::read(&src).unwrap();
        let mut plan = wav_plan(&d, OutputMode::SourceFolder, &["filename"], true);
        plan.overwrite = true;
        assert!(run(&plan, &Batch::new(), &src, 0).is_err());
        assert_eq!(std::fs::read(&src).unwrap(), before);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn kept_existing_output_does_not_delete_the_source() {
        let d = dir("kept");
        let src = d.join("a.wav");
        write_wav(&src);
        std::fs::create_dir_all(d.join("out")).unwrap();
        std::fs::write(d.join("out").join("a.wav"), b"older file").unwrap();
        let plan = wav_plan(&d, OutputMode::SingleFolder, &["filename"], true);
        assert!(run(&plan, &Batch::new(), &src, 0).is_err(), "nothing written must not count as done");
        assert!(src.exists(), "source deleted although nothing was converted");
        assert_eq!(std::fs::read(d.join("out").join("a.wav")).unwrap(), b"older file");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn names_come_from_the_source_tags_even_when_tags_are_not_copied() {
        let d = dir("names");
        let src = d.join("a.wav");
        write_wav(&src);
        let mut t = Tags::default();
        t.set("artist", "Band");
        t.set("title", "Lied");
        tags::write(&src, &t, false).unwrap();
        let mut plan = wav_plan(&d, OutputMode::SingleFolder, &["artist", " - ", "title"], false);
        plan.copy_tags = false;
        let outs = run(&plan, &Batch::new(), &src, 0).unwrap();
        assert_eq!(outs, vec![d.join("out").join("Band - Lied.wav")]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    #[cfg_attr(not(windows), ignore = "Windows paths: drive letters, backslashes, case-insensitive names")]
    fn preview_and_worker_apply_tag_processing_alike() {
        let mut plan = wav_plan(Path::new("D:\\x"), OutputMode::SingleFolder, &["artist", " - ", "title"], false);
        plan.effects = vec![Effect::TagProcessing { field: "artist".into(), value: "Neu".into(), mode: 0 }];
        let mut t = Tags::default();
        t.set("artist", "Band");
        t.set("title", "Lied");
        let dest = plan.destination(Path::new("C:\\M\\a.wav"), None, &t, &tags::Info::default(), Codec::Wave, None);
        assert_eq!(dest, PathBuf::from("D:\\x\\out\\Neu - Lied.wav"));
    }

    #[test]
    fn replaygain_tool_writes_album_gain() {
        let d = dir("rg");
        let mut plan = wav_plan(&d, OutputMode::SingleFolder, &["filename"], false);
        plan.codec = Codec::ReplayGainOnly;
        let batch = Batch::new();
        let files = [d.join("01.wav"), d.join("02.wav")];
        for (i, f) in files.iter().enumerate() {
            write_wav(f);
            let mut t = Tags::default();
            t.set("album artist", "Verschiedene");
            t.set("artist", format!("Band {i}"));
            t.set("album", "Sampler");
            tags::write(f, &t, false).unwrap();
            run(&plan, &batch, f, i).unwrap();
        }
        let (tx, _rx) = std::sync::mpsc::channel();
        batch.finish(&plan, &tx);
        for f in &files {
            assert!(!tags::read(f).0.get("replaygain_album_gain").is_empty(), "{}", f.display());
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_failed_encode_leaves_the_existing_file_alone() {
        let d = dir("onerror");
        let src = d.join("a.wav");
        write_wav(&src);
        std::fs::create_dir_all(d.join("out")).unwrap();
        let old = d.join("out").join("a.mp3");
        std::fs::write(&old, b"good old file").unwrap();
        // with "move on error" and without any effect (the half-written file is then deleted)
        for effects in [vec![Effect::MoveDestOnError { folder: d.join("errors").display().to_string() }], Vec::new()] {
            let mut plan = wav_plan(&d, OutputMode::SingleFolder, &["filename"], false);
            plan.codec = Codec::Mp3; // no FFmpeg in this plan: encoding fails
            plan.overwrite = true;
            plan.effects = effects;
            assert!(run(&plan, &Batch::new(), &src, 0).is_err());
            assert_eq!(std::fs::read(&old).unwrap(), b"good old file");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn copy_instead_of_convert_respects_existing_files() {
        let d = dir("copy");
        let src = d.join("a.wav");
        write_wav(&src);
        std::fs::create_dir_all(d.join("out")).unwrap();
        std::fs::write(d.join("out").join("a.wav"), b"keep me").unwrap();
        let mut plan = wav_plan(&d, OutputMode::SingleFolder, &["filename"], false);
        plan.codec = Codec::Flac;
        plan.effects = vec![Effect::ConditionalEncoding { skip_ext: "wav".into(), copy_instead: true }];
        assert!(run(&plan, &Batch::new(), &src, 0).is_err(), "kept file must count as skipped");
        assert_eq!(std::fs::read(d.join("out").join("a.wav")).unwrap(), b"keep me");
        // next to the source the copy would be the source itself
        plan.output_mode = OutputMode::SourceFolder;
        plan.overwrite = true;
        let before = std::fs::read(&src).unwrap();
        assert!(run(&plan, &Batch::new(), &src, 0).is_err());
        assert_eq!(std::fs::read(&src).unwrap(), before);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn old_replaygain_values_go_when_the_level_changes() {
        let d = dir("rgold");
        let src = d.join("a.wav");
        write_wav(&src);
        let mut t = Tags::default();
        t.set("title", "Lied");
        t.set("replaygain_track_gain", "-7.00 dB");
        tags::write(&src, &t, false).unwrap();
        let mut plan = wav_plan(&d, OutputMode::SingleFolder, &["filename"], false);
        plan.effects = vec![Effect::VolumeNormalize { mode: 5, target_db: 0.0, max_amp: 8.0, window_ms: 6000, fixed_db: 6.0 }];
        let outs = run(&plan, &Batch::new(), &src, 0).unwrap();
        let (back, _) = tags::read(&outs[0]);
        assert_eq!(back.get("title"), "Lied");
        assert!(back.get("replaygain_track_gain").is_empty(), "stale gain {}", back.get("replaygain_track_gain"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(windows)]
    #[test]
    fn run_external_gets_quoted_paths_with_spaces() {
        let d = dir("run ext");
        let src = d.join("a b.wav");
        write_wav(&src);
        let copy = d.join("copied file.wav");
        let mut plan = wav_plan(&d, OutputMode::SingleFolder, &["filename"], false);
        plan.effects = vec![Effect::RunExternal { command: format!("copy [outfile] \"{}\"", copy.display()) }];
        run(&plan, &Batch::new(), &src, 0).unwrap();
        assert!(copy.exists(), "the command did not run with the right paths");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn two_sources_with_one_destination_do_not_overwrite_each_other() {
        let d = dir("clash");
        std::fs::create_dir_all(d.join("cd1")).unwrap();
        std::fs::create_dir_all(d.join("cd2")).unwrap();
        let (a, b) = (d.join("cd1").join("Intro.wav"), d.join("cd2").join("Intro.wav"));
        write_wav(&a);
        write_wav(&b);
        let mut plan = wav_plan(&d, OutputMode::SingleFolder, &["filename"], true);
        plan.overwrite = true;
        let batch = Batch::new();
        assert!(run(&plan, &batch, &a, 0).is_ok());
        assert!(run(&plan, &batch, &b, 1).is_err(), "second file must not replace the first output");
        assert!(b.exists(), "second source deleted although its output was refused");
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod destination_tests {
    use super::*;
    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    fn plan(mode: OutputMode, scheme: &[&str]) -> Plan {
        Plan {
            codec: Codec::Flac,
            enc: EncSettings::default(),
            effects: Vec::new(),
            output_mode: mode,
            output_folder: PathBuf::from("D:\\Out"),
            scheme: s(scheme),
            keep_structure: false,
            overwrite: false,
            overwrite_list: Vec::new(),
            copy_tags: true,
            copy_art: true,
            threads: 1,
            low_priority: false,
            ffmpeg: FfmpegSet::default(),
            tools: Tools::default(),
            temp: std::env::temp_dir(),
            tag_scheme: s(crate::naming::TAG_SCHEME),
        }
    }
    #[test]
    #[cfg_attr(not(windows), ignore = "Windows paths: drive letters, backslashes, case-insensitive names")]
    fn destination_by_mode() {
        let mut t = Tags::default();
        t.set("artist", "Band");
        t.set("album", "Platte");
        t.set("title", "Lied");
        let i = tags::Info::default();
        let src = Path::new("C:\\Musik\\Alt\\a.wav");
        let folders = plan(OutputMode::SingleFolder, &["artist", "/", "album", "/", "title"]);
        assert_eq!(folders.destination(src, None, &t, &i, Codec::Flac, None), PathBuf::from("D:\\Out\\Band\\Platte\\Lied.flac"));
        let beside = plan(OutputMode::SourceFolder, &["artist", " - ", "title"]);
        assert_eq!(beside.destination(src, None, &t, &i, Codec::Mp3, None), PathBuf::from("C:\\Musik\\Alt\\Band - Lied.mp3"));
        let keep = plan(OutputMode::PreservePath, &["title"]);
        assert_eq!(keep.destination(src, None, &t, &i, Codec::Flac, None), PathBuf::from("D:\\Out\\Musik\\Alt\\a.flac"));
        let mut batch = plan(OutputMode::SingleFolder, &["title"]);
        batch.keep_structure = true;
        assert_eq!(batch.destination(src, Some(Path::new("C:\\Musik")), &t, &i, Codec::Flac, None), PathBuf::from("D:\\Out\\Alt\\a.flac"));
        let empty = plan(OutputMode::SingleFolder, &["year"]);
        assert_eq!(empty.destination(src, None, &t, &i, Codec::Flac, None), PathBuf::from("D:\\Out\\a.flac"));
        assert!(!plan(OutputMode::SingleFolder, &["filename", " - ", "("]).names_need_tags());
        assert!(plan(OutputMode::SingleFolder, &["filename", "artist"]).names_need_tags());
        assert!(!plan(OutputMode::PreservePath, &["artist"]).names_need_tags());
        let no_title = plan(OutputMode::SingleFolder, &["artist", "/", "album", "/", "year"]);
        assert_eq!(no_title.destination(src, None, &t, &i, Codec::Flac, None), PathBuf::from("D:\\Out\\Band\\Platte\\a.flac"));
    }

    #[test]
    fn name_clash_hint_points_to_the_chips_only_when_they_name_the_file() {
        let root = Path::new("/music");
        let chips = plan(OutputMode::SingleFolder, &["title"]);
        assert!(chips.name_clash("a.opus", None).contains("Bausteine"));
        let mut batch = plan(OutputMode::SingleFolder, &["title"]);
        batch.keep_structure = true;
        let msg = batch.name_clash("a.opus", Some(root));
        assert!(!msg.contains("Bausteine") && msg.contains("a.opus"), "{msg}");
        let keep = plan(OutputMode::PreservePath, &["title"]);
        assert!(!keep.name_clash("a.opus", None).contains("Bausteine"));
    }
}

#[cfg(test)]
mod scheme_parse_tests {
    use super::*;
    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    fn kv(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }
    #[test]
    fn tags_from_filename_by_scheme() {
        assert_eq!(tags_from_scheme("03 Band - Lied", &s(crate::naming::TAG_SCHEME)), kv(&[("track", "03"), ("artist", "Band"), ("title", "Lied")]));
        assert_eq!(tags_from_scheme("Band - Lied - Live", &s(&["artist", " - ", "title"])), kv(&[("artist", "Band"), ("title", "Lied - Live")]));
        assert_eq!(
            tags_from_scheme("Band - Platte (1992)", &s(&["albumartist", " - ", "album", " ", "(", "year", ")"])),
            kv(&[("album artist", "Band"), ("album", "Platte"), ("year", "1992")])
        );
    }
    #[test]
    fn names_that_do_not_fit_change_nothing() {
        let tag = s(crate::naming::TAG_SCHEME);
        assert_eq!(tags_from_scheme("Lied", &tag), kv(&[]));
        assert_eq!(tags_from_scheme("03 Band", &tag), kv(&[]));
        assert_eq!(tags_from_scheme("Band - Lied", &tag), kv(&[]));
    }
}

/// Fields (true, tag key) and literals (false, text) matched against a file name.
fn parse_parts(name: &str, parts: &[(bool, String)]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut s = name;
    let mut i = 0;
    while i < parts.len() {
        let (is_field, text) = &parts[i];
        if !is_field {
            // a separator the name does not have: the scheme does not fit, change nothing
            match s.strip_prefix(text.as_str()) {
                Some(stripped) => s = stripped,
                None => return Vec::new(),
            }
            i += 1;
            continue;
        }
        // field: up to the next literal, or the end
        let next_lit = parts.get(i + 1).filter(|p| !p.0).map(|p| p.1.clone());
        let value = if text == "track" || text == "disc" {
            // numbers end at the first non-digit; a name without one does not fit
            let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                return Vec::new();
            }
            digits
        } else {
            match &next_lit {
                Some(l) => match s.find(l.as_str()) {
                    Some(p) => s[..p].to_string(),
                    None => return Vec::new(),
                },
                None => s.to_string(),
            }
        };
        s = &s[value.len().min(s.len())..];
        // an empty value would delete the tag; leave it as it is
        if !value.trim().is_empty() {
            out.push((text.to_ascii_lowercase(), value.trim().to_string()));
        }
        i += 1;
    }
    out
}

/// 1:1 copy in 64 KiB blocks into "<dest>.tmp", one file read at a time (so parallel jobs
/// don't make the source disk seek back and forth), progress = copied * 100 / size.
/// Then the existing destination is deleted and the temporary file renamed onto it.
/// Cancelled or failed copies delete the temporary file.
pub const COPY_BLOCK: usize = 0x10000;
static READ_ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn copy_file(src: &Path, dest: &Path, cancel: &AtomicBool, progress: impl Fn(f32)) -> Result<(), String> {
    use std::io::{Read, Write};
    let t = crate::i18n::t;
    let size = std::fs::metadata(src).map_err(|_| t("Quelldatei ist nicht zugänglich (in einem anderen Programm geöffnet?)").to_string())?.len();
    let mut input = std::fs::File::open(src).map_err(|_| t("Quelldatei lässt sich nicht öffnen (in einem anderen Programm geöffnet?)").to_string())?;
    let mut tmp_name = dest.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    let mut out = std::fs::File::create(&tmp).map_err(|_| t("Zieldatei kann nicht angelegt werden (Schreibrechte für den Ordner prüfen)").to_string())?;
    let fail = |msg: &str| {
        let _ = std::fs::remove_file(&tmp);
        Err(t(msg).to_string())
    };
    {
        let _one = READ_ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let mut buf = vec![0u8; COPY_BLOCK];
        let mut done = 0u64;
        while done < size {
            if cancel.load(Ordering::Relaxed) {
                drop(out);
                return fail("abgebrochen");
            }
            let want = COPY_BLOCK.min((size - done) as usize);
            if input.read_exact(&mut buf[..want]).is_err() {
                drop(out);
                return fail("Fehler beim Lesen der Quelldatei");
            }
            if out.write_all(&buf[..want]).is_err() {
                drop(out);
                return fail("Fehler beim Schreiben der Zieldatei");
            }
            done += want as u64;
            progress((done * 100 / size.max(1)) as f32 / 100.0);
        }
    }
    drop(out);
    if dest.exists() && std::fs::remove_file(dest).is_err() {
        return fail("Vorhandene Zieldatei kann nicht gelöscht werden (in einem anderen Programm geöffnet?)");
    }
    if std::fs::rename(&tmp, dest).is_err() {
        return fail("Temporäre Datei kann nicht zur Zieldatei umbenannt werden");
    }
    Ok(())
}

fn run_job(plan: &Plan, job: &Job, cancel: &AtomicBool, tx: &Sender<Msg>, shared: &Shared) -> Result<JobResult, String> {
    let env = plan.env();
    let id = job.id;
    let (mut t, info) = match &job.tags {
        Some(t) => (t.clone(), tags::Info::default()),
        None => tags::read(&job.src),
    };
    let src_ext = job.src.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();

    // conditional encoding
    for e in &plan.effects {
        if let Effect::ConditionalEncoding { skip_ext, copy_instead } = e {
            let want = skip_ext.trim().trim_start_matches('.').to_ascii_lowercase();
            if !want.is_empty() && want == src_ext {
                if *copy_instead {
                    if let Some(c) = plan.outputs().first() {
                        let mut dest = plan.destination(&job.src, job.root.as_deref(), &t, &info, *c, shared.position(&job.src));
                        dest.set_extension(&src_ext);
                        // the same rules as for converted files
                        if same_path(&dest, &job.src) {
                            return Err(crate::i18n::t("Ziel ist dieselbe Datei wie die Quelle – anderen Ordner oder Namen wählen").into());
                        }
                        if !shared.reserved.lock().unwrap().insert(path_key(&dest)) {
                            let name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            return Err(plan.name_clash(&name, job.root.as_deref()));
                        }
                        if dest.exists() && !plan.overwrite && !plan.overwrite_list.iter().any(|o| same_path(o, &dest)) {
                            return Ok(JobResult::Skipped(crate::i18n::t("Zieldatei gibt es schon – übersprungen").into()));
                        }
                        if let Some(d) = dest.parent() {
                            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
                        }
                        copy_file(&job.src, &dest, cancel, |f| {
                            let _ = tx.send(Msg::Progress(id, f, crate::i18n::t("Kopieren")));
                        })?;
                        return Ok(JobResult::Done { outputs: vec![dest], audio_secs: info.seconds, note: crate::i18n::t("1:1 kopiert").into() });
                    }
                }
                return Ok(JobResult::Skipped(crate::i18n::tf(
                    crate::i18n::t("bedingte Umwandlung: .{want} wird übersprungen"),
                    &[("want", want.to_string())],
                )));
            }
        }
    }

    // tools that only touch tags
    match plan.codec {
        Codec::TagUpdate => {
            apply_tag_processing(&plan.effects, &mut t, shared.position(&job.src));
            tags::write(&job.src, &t, true)?;
            return Ok(JobResult::Done { outputs: vec![job.src.clone()], audio_secs: info.seconds, note: crate::i18n::t("Tags aktualisiert").into() });
        }
        Codec::TagFromFilename => {
            let stem = job.src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let found = tags_from_scheme(&stem, &plan.tag_scheme);
            if found.is_empty() {
                return Ok(JobResult::Skipped(crate::i18n::t("Muster passt nicht").into()));
            }
            let mut note = Vec::new();
            for (k, v) in found {
                note.push(format!("{k}={v}"));
                t.set(&k, v);
            }
            tags::write(&job.src, &t, false)?;
            return Ok(JobResult::Done { outputs: vec![job.src.clone()], audio_secs: info.seconds, note: note.join(", ") });
        }
        _ => {}
    }

    // decode
    let progress_decode = |f: f32| {
        let _ = tx.send(Msg::Progress(id, f * 0.35, crate::i18n::t("Dekodieren")));
    };
    let mut a = match &job.audio {
        Some(a) => (**a).clone(),
        None => audio::decode(&job.src, env.ffmpeg.for_decode().map(|f| f.path.as_path()), cancel, &progress_decode)?,
    };
    let original_secs = a.seconds();
    let _ = tx.send(Msg::Progress(id, 0.35, crate::i18n::t("Effekte")));

    match plan.codec {
        Codec::TestConversion => {
            return Ok(JobResult::Done {
                outputs: vec![],
                audio_secs: original_secs,
                note: crate::i18n::tf(
                    crate::i18n::t(crate::i18n::plural(
                        a.channels as u64,
                        "fehlerfrei dekodiert ({0} Hz, {1} Kanal)",
                        "fehlerfrei dekodiert ({0} Hz, {1} Kanäle)",
                    )),
                    &[("0", format!("{}", a.rate)), ("1", format!("{}", a.channels))],
                ),
            });
        }
        Codec::AudioInfo => {
            let blocks = loudness::block_energies(&a);
            let lufs = loudness::integrated(&blocks);
            let peak = a.peak();
            let text = crate::i18n::tf(
                crate::i18n::t(crate::i18n::plural(
                    a.channels as u64,
                    "{0}\n  Länge {1} · {2} Hz · {3} Bit · {4} Kanal · {5} · Spitze {6} dBFS · Lautheit {7} LUFS",
                    "{0}\n  Länge {1} · {2} Hz · {3} Bit · {4} Kanäle · {5} · Spitze {6} dBFS · Lautheit {7} LUFS",
                )),
                &[
                    ("0", format!("{}", job.src.display())),
                    ("1", audio::fmt_len(original_secs).to_string()),
                    ("2", format!("{}", a.rate)),
                    ("3", format!("{}", a.bits)),
                    ("4", format!("{}", a.channels)),
                    ("5", info.bitrate.map(|b| format!("{b} kbit/s")).unwrap_or_else(|| "–".into()).to_string()),
                    ("6", format!("{:.1}", 20.0 * peak.max(1e-9).log10())),
                    ("7", format!("{:.1}", lufs)),
                ],
            );
            let _ = tx.send(Msg::Report(text));
            return Ok(JobResult::Done { outputs: vec![], audio_secs: original_secs, note: format!("{lufs:.1} LUFS") });
        }
        Codec::AudioCrc => {
            let mut bytes = Vec::new();
            audio::pcm_bytes_le(&a.data, audio::PcmFormat::S16, &mut bytes);
            let crc = audio::crc32(&bytes, 0);
            let _ = tx.send(Msg::Report(crate::i18n::tf(
                crate::i18n::t("{0}  CRC32 (16 Bit PCM): {crc}"),
                &[("0", format!("{}", job.src.display())), ("crc", format!("{crc:08X}"))],
            )));
            return Ok(JobResult::Done { outputs: vec![], audio_secs: original_secs, note: format!("CRC {crc:08X}") });
        }
        _ => {}
    }

    // audio effects in order; ReplayGain measures at its position
    // the source's ReplayGain values no longer fit once an effect changes the level
    // (a ReplayGain effect in the chain measures and writes new ones)
    if plan.effects.iter().any(|e| e.changes_level()) {
        for k in tags::RG_FIELDS {
            t.set(k, "");
        }
    }
    let mut rg_written = false;
    for e in &plan.effects {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::i18n::t("abgebrochen").into());
        }
        if let Effect::ReplayGain { target_lufs, max_gain, album, clip_prevention, true_peak_ratio } = e {
            let blocks = loudness::block_energies(&a);
            let lufs = loudness::integrated(&blocks);
            let peak = loudness::true_peak(&a, *true_peak_ratio);
            if lufs <= -69.9 {
                let _ = tx.send(Msg::Report(crate::i18n::tf(
                    crate::i18n::t("{0}: Stille erkannt, kein ReplayGain-Titelwert"),
                    &[("0", format!("{}", job.src.display()))],
                )));
            } else {
                let gain = loudness::replaygain(lufs, peak, *target_lufs as f64, *max_gain as f64, *clip_prevention);
                t.set("replaygain_track_gain", format!("{gain:+.2} dB"));
                t.set("replaygain_track_peak", format!("{peak:.6}"));
                rg_written = true;
            }
            if *album {
                let key = album_key(&t);
                let mut m = shared.album_blocks.lock().unwrap();
                let entry = m.entry(key).or_insert((Vec::new(), 0.0, Vec::new()));
                entry.0.extend(blocks);
                entry.1 = entry.1.max(peak);
            }
            continue;
        }
        if !e.is_audio() {
            continue;
        }
        if let Effect::Hdcd { amp_6db } = e {
            // report when HDCD was found and decoded
            if dsp::hdcd_decode(&mut a, plan.ffmpeg.for_hdcd().map(|f| f.path.as_path()), *amp_6db)? {
                let _ = tx.send(Msg::Report(format!("{}: {}", job.src.display(), crate::i18n::t("HDCD-Titel erkannt, auf 24 Bit dekodiert"))));
            }
            continue;
        }
        dsp::apply(e, &mut a, &t);
    }
    // names and the album key come from the source's tags (see `Plan::destination`)
    let source_tags = t.clone();
    apply_tag_processing(&plan.effects, &mut t, shared.position(&job.src));
    if !plan.copy_tags && job.tags.is_none() {
        let keep_rg: Vec<(String, String)> = tags::RG_FIELDS.iter().map(|k| (k.to_string(), t.get(k).to_string())).filter(|x| !x.1.is_empty()).collect();
        t = Tags::default();
        for (k, v) in keep_rg {
            t.set(&k, v);
        }
    }
    if !plan.copy_art {
        t.art.clear();
    }

    if plan.codec == Codec::ReplayGainOnly {
        // only measure and tag the source file
        let blocks = loudness::block_energies(&a);
        let lufs = loudness::integrated(&blocks);
        let peak = a.peak();
        let gain = loudness::replaygain(lufs, peak, -18.0, 25.0, true);
        let (mut st, _) = tags::read(&job.src);
        st.set("replaygain_track_gain", format!("{gain:+.2} dB"));
        st.set("replaygain_track_peak", format!("{peak:.6}"));
        tags::write(&job.src, &st, false)?;
        let key = album_key(&st);
        let mut m = shared.album_blocks.lock().unwrap();
        let entry = m.entry(key).or_insert((Vec::new(), 0.0, Vec::new()));
        entry.0.extend(blocks);
        entry.1 = entry.1.max(peak);
        entry.2.push(job.src.clone());
        return Ok(JobResult::Done { outputs: vec![job.src.clone()], audio_secs: original_secs, note: format!("{gain:+.2} dB") });
    }
    // every converted file names its album artist (players sort albums by it)
    if !t.fields.is_empty() && t.get("album artist").trim().is_empty() && !removes_album_artist(&plan.effects) {
        let aa = shared.album_artist(&job.src, &t, &plan.effects);
        if !aa.is_empty() {
            t.set("album artist", aa);
        }
    }

    // pieces for the split tools
    let pieces: Vec<(String, Audio)> = match plan.codec {
        Codec::ChannelSplit => (0..a.channels)
            .map(|c| {
                let data: Vec<f32> = a.data.iter().skip(c).step_by(a.channels).copied().collect();
                let label = match (a.channels, c) {
                    (2, 0) => "L".to_string(),
                    (2, 1) => "R".to_string(),
                    _ => crate::i18n::tf(crate::i18n::t("Kanal {0}"), &[("0", format!("{}", c + 1))]),
                };
                (format!(" - {label}"), Audio { channels: 1, ..a.with_data(data) })
            })
            .collect(),
        Codec::LengthSplit => {
            let per = (plan.enc.split_seconds.max(1) as usize) * a.rate as usize * a.channels;
            a.data
                .chunks(per.max(1))
                .enumerate()
                .map(|(i, d)| (crate::i18n::tf(crate::i18n::t(" - Teil {0}"), &[("0", format!("{:02}", i + 1))]), a.with_data(d.to_vec())))
                .collect()
        }
        _ => vec![(String::new(), a)],
    };

    let codecs = plan.outputs();
    let total_steps = (codecs.len() * pieces.len()).max(1);
    // files written by this job, and existing ones it left alone
    let mut outputs = Vec::new();
    let mut kept = Vec::new();
    let mut step = 0;
    for codec in &codecs {
        for (suffix, piece) in &pieces {
            if cancel.load(Ordering::Relaxed) {
                return Err(crate::i18n::t("abgebrochen").into());
            }
            let mut dest = plan.destination(&job.src, job.root.as_deref(), &source_tags, &info, *codec, shared.position(&job.src));
            if !suffix.is_empty() {
                let stem = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                dest.set_file_name(format!("{stem}{suffix}.{}", codec.ext()));
            }
            if same_path(&dest, &job.src) {
                return Err(crate::i18n::t("Ziel ist dieselbe Datei wie die Quelle – anderen Ordner oder Namen wählen").into());
            }
            if !shared.reserved.lock().unwrap().insert(path_key(&dest)) {
                let name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                return Err(plan.name_clash(&name, job.root.as_deref()));
            }
            if dest.exists() && !plan.overwrite && !plan.overwrite_list.iter().any(|o| same_path(o, &dest)) {
                kept.push(dest);
                step += 1;
                continue;
            }
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d).map_err(|e| crate::i18n::tf(crate::i18n::t("Ordner kann nicht angelegt werden: {e}"), &[("e", format!("{e}"))]))?;
            }
            let tmp = tmp_name(&dest);
            let meta = if plan.copy_tags || job.tags.is_some() { tags::ffmpeg_metadata(&t) } else { Vec::new() };
            let base = 0.4 + 0.6 * step as f32 / total_steps as f32;
            let span = 0.6 / total_steps as f32;
            let prog = |f: f32| {
                let _ = tx.send(Msg::Progress(id, base + span * f, crate::i18n::t("Kodieren")));
            };
            let result = encode::encode(*codec, &plan.enc, piece, &tmp, &meta, &env, cancel, &prog);
            if let Err(e) = result {
                on_error(plan, &tmp, &dest);
                return Err(e);
            }
            // tags
            if (plan.copy_tags || job.tags.is_some() || rg_written) && !matches!(codec, Codec::Wma | Codec::Dsd | Codec::Speex) {
                if let Err(e) = tags::write(&tmp, &t, plan.copy_art) {
                    let _ = tx.send(Msg::Report(format!("{}: {e}", dest.display())));
                }
            }
            if dest.exists() && std::fs::remove_file(&dest).is_err() {
                let _ = std::fs::remove_file(&tmp);
                return Err(crate::i18n::t("Vorhandene Zieldatei kann nicht gelöscht werden (in einem anderen Programm geöffnet?)").into());
            }
            std::fs::rename(&tmp, &dest)
                .map_err(|e| crate::i18n::tf(crate::i18n::t("Temporäre Datei kann nicht verschoben werden: {e}"), &[("e", format!("{e}"))]))?;
            outputs.push(dest);
            step += 1;
        }
    }
    if outputs.is_empty() && !kept.is_empty() {
        return Ok(JobResult::Skipped(crate::i18n::t("Zieldatei gibt es schon – übersprungen").into()));
    }

    // actions after a successful file (only on files this job wrote)
    for e in &plan.effects {
        match e {
            Effect::PreserveAttributes => {
                if let Ok(m) = std::fs::metadata(&job.src).and_then(|m| m.modified()) {
                    for o in &outputs {
                        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(o) {
                            let _ = f.set_modified(m);
                        }
                    }
                }
            }
            Effect::FolderJpgPreserve => {
                if let Some(dir) = job.src.parent() {
                    for name in ["folder.jpg", "Folder.jpg", "cover.jpg", "Cover.jpg"] {
                        let f = dir.join(name);
                        if f.exists() {
                            for o in &outputs {
                                if let Some(od) = o.parent() {
                                    let target = od.join(name);
                                    if !target.exists() && od != dir {
                                        let _ = std::fs::copy(&f, target);
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
            Effect::RunExternal { command } if !command.trim().is_empty() => {
                for o in &outputs {
                    let c = command.replace("[outfile]", &shell_quote(o)).replace("[infile]", &shell_quote(&job.src));
                    let mut cmd = shell_command(&c);
                    audio::hidden(&mut cmd);
                    match cmd.status() {
                        Ok(s) if s.success() => {}
                        Ok(s) => {
                            let _ = tx.send(Msg::Report(crate::i18n::tf(
                                crate::i18n::t("Programm nach {0} endete mit Code {1}"),
                                &[("0", o.display().to_string()), ("1", s.code().map(|c| c.to_string()).unwrap_or_else(|| "?".into()))],
                            )));
                        }
                        Err(e) => {
                            let _ =
                                tx.send(Msg::Report(crate::i18n::tf(crate::i18n::t("Programm konnte nicht gestartet werden: {e}"), &[("e", e.to_string())])));
                        }
                    }
                }
            }
            Effect::ReplayGain { album: true, .. } => {
                let key = album_key(&source_tags);
                if let Some(entry) = shared.album_blocks.lock().unwrap().get_mut(&key) {
                    entry.2.extend(outputs.iter().cloned());
                }
            }
            Effect::PlaylistWriter { .. } => {
                let mut pl = shared.playlist.lock().unwrap();
                for o in &outputs {
                    pl.push((id, o.clone()));
                }
            }
            _ => {}
        }
    }
    // the source goes only when every output was written now and none of them is the source
    if plan.effects.contains(&Effect::DeleteSource)
        && job.audio.is_none()
        && !outputs.is_empty()
        && kept.is_empty()
        && !outputs.iter().any(|o| same_path(o, &job.src))
    {
        if let Err(e) = discard_source(&job.src) {
            let _ = tx.send(Msg::Report(crate::i18n::tf(crate::i18n::t("Quelle konnte nicht in den Papierkorb gelegt werden: {e}"), &[("e", e.to_string())])));
        }
    }
    let _ = tx.send(Msg::Progress(id, 1.0, crate::i18n::t("Fertig")));
    Ok(JobResult::Done { outputs, audio_secs: original_secs, note: String::new() })
}

/// "Quelldatei löschen": into the recycle bin, so a mistake can be undone. Tests delete
/// directly, so a test run leaves nothing in the user's recycle bin.
fn discard_source(p: &Path) -> Result<(), String> {
    #[cfg(test)]
    return std::fs::remove_file(p).map_err(|e| e.to_string());
    #[cfg(not(test))]
    trash::delete(p).map_err(|e| e.to_string())
}

/// A path as one argument for the system shell.
fn shell_quote(p: &Path) -> String {
    let s = p.display().to_string();
    if cfg!(windows) {
        format!("\"{s}\"")
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// The command line as written, run by cmd.exe on Windows and by sh on Linux and macOS.
fn shell_command(line: &str) -> std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("cmd");
        // cmd.exe does not understand the C-style quoting of normal arguments:
        // hand it the line as written, wrapped once (/S keeps inner quotes intact)
        cmd.raw_arg(format!("/S /C \"{line}\""));
        cmd
    }
    #[cfg(not(windows))]
    {
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", line]);
        cmd
    }
}

/// A failed encode only ever leaves its temporary file behind (an existing destination is
/// untouched): "move on error" keeps that broken output for a look under the destination's
/// name, otherwise it is deleted.
fn on_error(plan: &Plan, tmp: &Path, dest: &Path) {
    let keep = plan.effects.iter().find_map(|e| match e {
        Effect::MoveDestOnError { folder } if !folder.trim().is_empty() => Some(PathBuf::from(folder.trim())),
        _ => None,
    });
    match keep {
        Some(folder) if tmp.exists() => {
            let _ = std::fs::create_dir_all(&folder);
            if std::fs::rename(tmp, folder.join(dest.file_name().unwrap_or_default())).is_err() {
                let _ = std::fs::remove_file(tmp);
            }
        }
        _ => {
            let _ = std::fs::remove_file(tmp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end: a generated WAV converted to every format FFmpeg on this computer can write.
    #[test]
    fn convert_all_formats() {
        // PSYAUDIO_TEST_FFMPEG: exactly this FFmpeg (the CI's shipped one), which must write all nine formats
        let forced = std::env::var_os("PSYAUDIO_TEST_FFMPEG").map(PathBuf::from);
        let Some(ff) = forced.clone().or_else(|| audio::find_tool("", "ffmpeg")) else { return };
        let dir = std::env::temp_dir().join(format!("psyaudio-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rate = 44100;
        let data: Vec<f32> = (0..rate * 3)
            .flat_map(|i| {
                let s = (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5;
                [s, s * 0.8]
            })
            .collect();
        let a = Audio { rate: rate as u32, channels: 2, bits: 16, float: false, lossless: true, data };
        let src = dir.join("01 Testband - Sinus.wav");
        audio::write_wav(&src, &a, audio::PcmFormat::S16).unwrap();
        let mut t = Tags::default();
        t.set("title", "Sinus");
        t.set("artist", "Testband");
        t.set("album", "Testalbum");
        t.set("track", "1");
        tags::write(&src, &t, false).unwrap();

        let set = FfmpegSet::only(&ff);
        assert!(set.main.is_some(), "FFmpeg does not start: {}", ff.display());
        let mut multi =
            vec![Codec::Mp3, Codec::Flac, Codec::AacM4a, Codec::Alac, Codec::Opus, Codec::Vorbis, Codec::Wave, Codec::Aiff, Codec::WavPack, Codec::Wma];
        if forced.is_some() {
            multi.push(Codec::Speex);
        }
        let expected = multi.len();
        let enc = EncSettings { multi, flac_verify: true, ..EncSettings::default() };
        let plan = Plan {
            codec: Codec::Multi,
            enc,
            effects: vec![
                Effect::Resample { rate: 48000 },
                Effect::Fade { in_ms: 100, out_ms: 500 },
                Effect::ReplayGain { target_lufs: -18.0, max_gain: 25.0, album: true, clip_prevention: true, true_peak_ratio: 1 },
                Effect::PlaylistWriter { kind: 0, same_folder: true },
            ],
            output_mode: OutputMode::SingleFolder,
            output_folder: dir.join("out"),
            scheme: ["artist", "/", "album", "/", "track", " ", "title"].iter().map(|s| s.to_string()).collect(),
            keep_structure: false,
            overwrite: true,
            overwrite_list: Vec::new(),
            copy_tags: true,
            copy_art: true,
            threads: 2,
            low_priority: false,
            ffmpeg: set,
            tools: Tools::default(),
            temp: std::env::temp_dir(),
            tag_scheme: Vec::new(),
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let batch = Batch::new();
        let cancel = AtomicBool::new(false);
        let job = Job { id: 0, src: src.clone(), root: None, tags: None, audio: None };
        let outs = batch.run(&plan, &job, &cancel, &tx).expect("conversion");
        batch.finish(&plan, &tx);
        drop(tx);
        for m in rx.iter() {
            if let Msg::Report(r) = m {
                println!("report: {r}");
            }
        }
        assert_eq!(outs.len(), expected, "{outs:?}");
        for o in &outs {
            assert!(o.exists(), "{}", o.display());
            assert!(o.parent().is_some_and(|d| d.ends_with(Path::new("Testband").join("Testalbum"))), "{}", o.display());
            assert!(o.file_name().and_then(|s| s.to_str()).is_some_and(|n| n.starts_with("01 Sinus")), "{}", o.display());
            let back = audio::decode(o, Some(&ff), &cancel, &|_| {}).expect("decode back");
            println!("{} -> {} Hz, {:.2} s", o.display(), back.rate, back.seconds());
            assert!((back.seconds() - 3.0).abs() < 0.2);
        }
        let flac = outs.iter().find(|o| o.extension().unwrap() == "flac").unwrap();
        let (ft, fi) = tags::read(flac);
        assert_eq!(ft.get("title"), "Sinus");
        assert_eq!(fi.rate, Some(48000));
        assert!(!ft.get("replaygain_track_gain").is_empty());
        assert!(!ft.get("replaygain_album_gain").is_empty(), "album gain");
        let mp3 = outs.iter().find(|o| o.extension().unwrap() == "mp3").unwrap();
        assert_eq!(tags::read(mp3).0.get("artist"), "Testband");
        assert!(dir.join("out").join("Testband").join("Testalbum").join("Testalbum.m3u8").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod output_check_tests {
    use super::*;
    #[test]
    fn output_location_checks() {
        let tmp = std::env::temp_dir().join("psyaudio-outcheck").join("new sub folder");
        assert_eq!(output_location_problem(&tmp), None);
        assert_eq!(output_location_problem(Path::new("relative/path")), None);
        #[cfg(windows)]
        if let Some(l) = (b'D'..=b'Z').rev().find(|l| !Path::new(&format!("{}:\\", *l as char)).exists()) {
            let missing = PathBuf::from(format!("{}:/Music", l as char));
            assert!(output_location_problem(&missing).is_some());
        }
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;
    #[test]
    fn copies_through_tmp_and_replaces() {
        let dir = std::env::temp_dir().join("psyaudio-copytest");
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("a.bin");
        let dest = dir.join("b.bin");
        let data: Vec<u8> = (0..(COPY_BLOCK * 3 + 17)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&src, &data).unwrap();
        std::fs::write(&dest, b"old").unwrap();
        let steps = std::cell::Cell::new(0);
        copy_file(&src, &dest, &AtomicBool::new(false), |_| steps.set(steps.get() + 1)).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert_eq!(steps.get(), 4);
        assert!(!dir.join("b.bin.tmp").exists());
        assert!(copy_file(&src, &dir.join("c.bin"), &AtomicBool::new(true), |_| {}).is_err());
        assert!(!dir.join("c.bin.tmp").exists() && !dir.join("c.bin").exists());
    }
}

#[cfg(test)]
mod year_tests {
    use super::*;
    use lofty::prelude::*;

    /// FFmpeg writes the year from `-metadata date=`, the tag library writes the tags again
    /// afterwards: the file must still hold the year once (other programs show two keys as "2001\2001").
    #[test]
    fn the_year_is_stored_once() {
        let Some(ff) = audio::find_tool("", "ffmpeg") else { return };
        let dir = std::env::temp_dir().join(format!("psyaudio-year-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = Audio { rate: 44100, channels: 2, bits: 16, float: false, lossless: true, data: vec![0.0; 44100 * 2] };
        let mut t = Tags::default();
        t.set("title", "Lied");
        t.set("year", "2001");
        let enc = EncSettings { multi: vec![Codec::Opus, Codec::Flac, Codec::Vorbis, Codec::Mp3, Codec::AacM4a, Codec::WavPack], ..Default::default() };
        let plan = Plan {
            codec: Codec::Multi,
            enc,
            effects: Vec::new(),
            output_mode: OutputMode::SingleFolder,
            output_folder: dir.clone(),
            scheme: vec!["title".to_string()],
            keep_structure: false,
            overwrite: true,
            overwrite_list: Vec::new(),
            copy_tags: true,
            copy_art: false,
            threads: 1,
            low_priority: false,
            ffmpeg: FfmpegSet::only(&ff),
            tools: Tools::default(),
            temp: std::env::temp_dir(),
            tag_scheme: Vec::new(),
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let job = Job { id: 0, src: PathBuf::from("Track01.wav"), root: None, tags: Some(t), audio: Some(Arc::new(a)) };
        let outs = Batch::new().run(&plan, &job, &AtomicBool::new(false), &tx).expect("conversion");
        assert_eq!(outs.len(), 6, "{outs:?}");
        for o in &outs {
            let file = lofty::read_from_path(o).unwrap();
            let tag = file.primary_tag().unwrap();
            let years: Vec<String> =
                tag.items().filter(|i| matches!(i.key(), ItemKey::Year | ItemKey::RecordingDate)).filter_map(|i| i.value().text().map(String::from)).collect();
            assert_eq!(years, vec!["2001".to_string()], "{}", o.display());
            assert_eq!(tags::read(o).0.get("year"), "2001", "{}", o.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod effect_behaviour_tests {
    use super::*;

    fn tags(kv: &[(&str, &str)]) -> Tags {
        let mut t = Tags::default();
        for (k, v) in kv {
            t.set(k, v.to_string());
        }
        t
    }

    #[test]
    fn track_numbers_count_within_each_folder_in_list_order() {
        let p = |s: &str| PathBuf::from(s);
        let files = [p("/m/a/2.wav"), p("/m/a/1.wav"), p("/m/b/x.wav"), p("/m/a/3.wav")];
        let pos = folder_positions(files.iter().map(|f| f.as_path()));
        assert_eq!(position_of(&pos, &files[0]), Some((1, 3)));
        assert_eq!(position_of(&pos, &files[1]), Some((2, 3)));
        assert_eq!(position_of(&pos, &files[3]), Some((3, 3)));
        assert_eq!(position_of(&pos, &files[2]), Some((1, 1)));

        let fx = vec![
            Effect::TagProcessing { field: "track".into(), value: String::new(), mode: 0 },
            Effect::TagProcessing { field: "track_total".into(), value: "egal".into(), mode: 0 },
        ];
        let mut t = tags(&[("track", "9")]);
        apply_tag_processing(&fx, &mut t, Some((2, 3)));
        assert_eq!((t.get("track"), t.get("track_total")), ("2", "3"));
        // without a position (single file preview) the number stays as it was
        let mut t = tags(&[("track", "9")]);
        apply_tag_processing(&fx, &mut t, None);
        assert_eq!(t.get("track"), "9");
    }

    #[test]
    fn values_take_other_tags_and_nonsense_is_ignored() {
        let mut t = tags(&[("artist", "Band"), ("album", "Platte")]);
        let fx = vec![
            Effect::TagProcessing { field: "album artist".into(), value: "<artist>".into(), mode: 2 },
            Effect::TagProcessing { field: "comment".into(), value: "<albumartist> – <album> <unbekannt>".into(), mode: 0 },
            Effect::TagProcessing { field: "*".into(), value: "x".into(), mode: 0 },
            Effect::TagProcessing { field: "cover".into(), value: "x".into(), mode: 0 },
        ];
        apply_tag_processing(&fx, &mut t, None);
        assert_eq!(t.get("album artist"), "Band");
        assert_eq!(t.get("comment"), "Band – Platte <unbekannt>");
        let mut t = tags(&[("artist", "Band")]);
        apply_tag_processing(&[Effect::TagProcessing { field: "comment".into(), value: "a <3 <artist>".into(), mode: 0 }], &mut t, None);
        assert_eq!(t.get("comment"), "a <3 Band");
        // the number counts every file, also with "set if empty"
        let mut t = tags(&[("track", "9")]);
        apply_tag_processing(&[Effect::TagProcessing { field: "track".into(), value: String::new(), mode: 2 }], &mut t, Some((4, 5)));
        assert_eq!(t.get("track"), "4");
        assert!(t.get("*").is_empty() && t.get("cover").is_empty());
        // "set if empty" leaves a filled field alone
        let mut t = tags(&[("artist", "Band"), ("album artist", "Andere")]);
        apply_tag_processing(&fx[..1], &mut t, None);
        assert_eq!(t.get("album artist"), "Andere");
    }

    #[test]
    fn album_artist_is_the_artist_or_various_artists() {
        let job = |p: &str, t: Tags| Job { id: 0, src: PathBuf::from(p), root: None, tags: Some(t), audio: None };
        let one = tags(&[("artist", "Band"), ("album", "Platte")]);
        let other = tags(&[("artist", "Gast"), ("album", "Platte")]);
        let shared = Shared::new(vec![job("/m/a/1.wav", one.clone()), job("/m/a/2.wav", one.clone())]);
        assert_eq!(shared.album_artist(Path::new("/m/a/1.wav"), &one, &[]), "Band");
        let shared = Shared::new(vec![job("/m/a/1.wav", one.clone()), job("/m/a/2.wav", other.clone())]);
        assert_eq!(shared.album_artist(Path::new("/m/a/1.wav"), &one, &[]), VARIOUS_ARTISTS);
        // the same album name in another folder is another album
        let shared = Shared::new(vec![job("/m/a/1.wav", one.clone()), job("/m/b/2.wav", tags(&[("artist", "Gast"), ("album", "Platte")]))]);
        assert_eq!(shared.album_artist(Path::new("/m/a/1.wav"), &one, &[]), "Band");
        // the neighbours count as the run writes them: "artist = X" for all makes one artist
        let set = vec![Effect::TagProcessing { field: "artist".into(), value: "X".into(), mode: 0 }];
        let shared2 = Shared::new(vec![job("/m/c/1.wav", tags(&[("artist", "X"), ("album", "Platte")])), job("/m/c/2.wav", other)]);
        assert_eq!(shared2.album_artist(Path::new("/m/c/1.wav"), &tags(&[("artist", "X"), ("album", "Platte")]), &set), "X");
        assert!(removes_album_artist(&[Effect::TagProcessing { field: "album artist".into(), value: String::new(), mode: 1 }]));
        // no album: the file's own artist
        assert_eq!(Shared::new(Vec::new()).album_artist(Path::new("/x.wav"), &tags(&[("artist", "Solo")]), &[]), "Solo");
    }

    #[test]
    #[cfg(not(windows))]
    fn external_programs_run_through_the_shell_with_quoted_paths() {
        let d = std::env::temp_dir().join(format!("psyaudio-shell-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let src = d.join("it's a \"song\" $HOME.wav");
        std::fs::write(&src, b"x").unwrap();
        let line = format!("cp {} {}", shell_quote(&src), shell_quote(&d.join("copy.wav")));
        assert!(shell_command(&line).status().unwrap().success());
        assert!(d.join("copy.wav").exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
