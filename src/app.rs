//! The PsyAudio window: navigation, converter page and the progress view.
//! The other pages live in pages.rs.

use crate::audio::{self, fmt_len, fmt_size};
use crate::cdrip::{self, DiscMeta, Pressing, Toc};
use crate::convert::{self, Job, Msg, Plan};
use crate::dsp;
use crate::encode::{self, Codec, Env};
use crate::naming;
use crate::settings::{OutputMode, Overwrite, Profile, Settings};
use crate::tags::{self, Info, Tags};
use crate::theme::{self, Palette};
use crate::widgets::*;
use eframe::egui::{self, RichText};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Convert,
    Batch,
    Cd,
    Tags,
    Settings,
}

#[derive(Clone)]
pub(crate) struct FileItem {
    pub path: PathBuf,
    pub root: Option<PathBuf>,
    pub size: u64,
    pub info: Option<(Tags, Info)>,
}

#[derive(Clone, PartialEq)]
pub(crate) enum JobState {
    Waiting,
    Running,
    Done,
    Skipped(String),
    Failed(String),
}

pub(crate) struct JobView {
    pub src: PathBuf,
    pub root: Option<PathBuf>,
    pub state: JobState,
    pub progress: f32,
    pub stage: &'static str,
    pub outputs: Vec<PathBuf>,
    pub speed: f64,
    pub note: String,
}

pub(crate) struct RunState {
    pub title: String,
    pub jobs: Vec<JobView>,
    pub rx: Receiver<Msg>,
    pub cancel: Arc<AtomicBool>,
    pub started: Instant,
    pub finished: Option<f64>,
    pub reports: Vec<String>,
    pub threads: usize,
    /// Highest overall progress shown so far (it never goes back).
    pub max_overall: f32,
    /// Total audio length of the run's files (for the encoding speed).
    pub audio_secs: f64,
    /// Output format name for "N files converted to …".
    pub target: String,
}

/// An action on the tag page that would drop unsaved changes in the editor.
pub(crate) enum TagPending {
    Select(usize),
    Replace(Vec<PathBuf>),
}

/// A run that is still working blocks a second one (its workers could not be cancelled any more).
pub(crate) fn run_active(run: &Option<RunState>) -> bool {
    run.as_ref().is_some_and(|r| r.finished.is_none())
}

#[cfg(test)]
mod run_tests {
    use super::*;
    fn state(finished: Option<f64>) -> RunState {
        let (_tx, rx) = channel();
        RunState {
            title: String::new(),
            jobs: Vec::new(),
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            started: Instant::now(),
            finished,
            reports: Vec::new(),
            threads: 1,
            max_overall: 0.0,
            audio_secs: 0.0,
            target: String::new(),
        }
    }
    #[test]
    fn a_working_run_blocks_a_second_one() {
        assert!(run_active(&Some(state(None))));
        assert!(!run_active(&Some(state(Some(3.0)))));
        assert!(!run_active(&None));
    }
}

pub(crate) enum RipMsg {
    Progress(usize, f32, String),
    TrackDone(usize, String, String),
    Log(String),
    Finished(String),
}

pub(crate) struct RipRun {
    pub rx: Receiver<RipMsg>,
    pub cancel: Arc<AtomicBool>,
    pub rows: Vec<(f32, String, String)>,
    pub log: Vec<String>,
    pub finished: Option<String>,
}

pub(crate) type LookupResult = Result<(Vec<DiscMeta>, Option<Vec<u8>>), String>;

#[derive(Default)]
pub(crate) struct CdState {
    pub drives: Vec<char>,
    pub drive: Option<char>,
    pub toc: Option<Toc>,
    pub error: String,
    pub meta: DiscMeta,
    pub candidates: Vec<DiscMeta>,
    pub selected: Vec<bool>,
    pub lookup: Option<Receiver<LookupResult>>,
    pub cover: Option<Vec<u8>>,
    pub cover_tex: Option<egui::TextureHandle>,
    pub ar: Option<Result<Vec<Pressing>, String>>,
    pub ar_rx: Option<Receiver<Result<Vec<Pressing>, String>>>,
    pub rip: Option<RipRun>,
    /// Running drive-offset search: result (offset, matching tracks) or a message.
    pub offset_rx: Option<Receiver<Result<(i32, usize), String>>>,
}

pub struct PsyApp {
    pub(crate) s: Settings,
    pub(crate) saved: Settings,
    pub(crate) pal: Palette,
    pub(crate) page: Page,
    pub(crate) files: Vec<FileItem>,
    pub(crate) selected: BTreeSet<usize>,
    pub(crate) info_rx: Receiver<(PathBuf, Tags, Info)>,
    pub(crate) info_tx: Sender<(PathBuf, Tags, Info)>,
    pub(crate) run: Option<RunState>,
    pub(crate) ffmpeg: Option<PathBuf>,
    pub(crate) ffmpeg_encoders: Vec<String>,
    pub(crate) screenshot: Option<PathBuf>,
    pub(crate) frame: u64,
    pub(crate) toast: Option<(String, Instant)>,
    /// Existing outputs with their tick (ticked = overwrite), the jobs and the plan.
    pub(crate) overwrite_prompt: Option<(Vec<(PathBuf, bool)>, Vec<Job>, Plan)>,
    pub(crate) edit_effect: Option<usize>,
    pub(crate) profile_name: String,
    pub(crate) batch_folder: String,
    pub(crate) batch_found: Vec<PathBuf>,
    /// The folder `batch_found` came from (the text field may have changed since).
    pub(crate) batch_root: PathBuf,
    /// A folder scan running in the background: (folder, files).
    pub(crate) batch_rx: Option<Receiver<(PathBuf, Vec<PathBuf>)>>,
    pub(crate) tag_files: Vec<PathBuf>,
    pub(crate) tag_sel: usize,
    pub(crate) tag_edit: Tags,
    pub(crate) tag_info: Info,
    pub(crate) tag_loaded: Option<PathBuf>,
    /// The tags as they are in the file (to see unsaved changes in `tag_edit`).
    pub(crate) tag_saved: Tags,
    /// What waits for the answer "save the changed tags?".
    pub(crate) tag_pending: Option<TagPending>,
    pub(crate) tag_cover_tex: Option<egui::TextureHandle>,
    pub(crate) cd: CdState,
    pub(crate) applied_theme: Option<bool>,
    pub(crate) window_title: String,
    /// The white PsyAudio lettering (assets/PsyAudio-wordmark.png), drawn in the accent colour.
    pub(crate) logo: Option<egui::TextureHandle>,
    /// The lettering at full size; `logo` is made from it at the exact pixel size it is shown at
    /// (a texture scaled down on the GPU looks soft).
    pub(crate) logo_src: Option<image::RgbaImage>,
    /// Whether standby is currently held off (see `keep_awake`).
    pub(crate) awake: bool,
    /// When the settings were last written (they are written at most twice a second).
    pub(crate) last_save: Instant,
    /// For threads that must wake the window up.
    pub(crate) egui_ctx: egui::Context,
    /// When the hand-over folder was last looked at.
    pub(crate) last_inbox: Instant,
    /// The window was asked to close while work was running: show the question.
    pub(crate) close_asked: bool,
    /// Work was cancelled for closing: close as soon as it has stopped.
    pub(crate) closing: bool,
}

pub(crate) fn ffmpeg_state(s: &Settings) -> (Option<PathBuf>, Vec<String>) {
    let ff = audio::find_tool(&s.tools.ffmpeg, "ffmpeg");
    let enc = ff.as_deref().map(audio::ffmpeg_encoders).unwrap_or_default();
    (ff, enc)
}

impl PsyApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, files: Vec<PathBuf>) -> Self {
        let (mut s, settings_note) = Settings::load();
        if screenshot.is_some() {
            if let Ok(l) = std::env::var("PSYAUDIO_LANG") {
                s.lang = l;
            }
        }
        crate::i18n::set_lang(&s.lang);
        theme::install_fonts(&cc.egui_ctx);
        let pal = theme::palette(s.dark);
        theme::apply(&cc.egui_ctx, &pal, s.dark);
        let (ffmpeg, ffmpeg_encoders) = ffmpeg_state(&s);
        let (info_tx, info_rx) = channel();
        let mut cd = CdState::default();
        cd.drives = cdrip::drives();
        cd.drive = cd.drives.first().copied();
        let mut app = PsyApp {
            saved: s.clone(),
            s,
            pal,
            page: Page::Convert,
            files: Vec::new(),
            selected: BTreeSet::new(),
            info_rx,
            info_tx,
            run: None,
            ffmpeg,
            ffmpeg_encoders,
            screenshot,
            frame: 0,
            toast: None,
            overwrite_prompt: None,
            edit_effect: None,
            profile_name: String::new(),
            batch_folder: String::new(),
            batch_found: Vec::new(),
            batch_root: PathBuf::new(),
            batch_rx: None,
            tag_files: Vec::new(),
            tag_sel: 0,
            tag_edit: Tags::default(),
            tag_info: Info::default(),
            tag_loaded: None,
            tag_saved: Tags::default(),
            tag_pending: None,
            tag_cover_tex: None,
            cd,
            applied_theme: None,
            window_title: String::new(),
            awake: false,
            last_save: Instant::now(),
            egui_ctx: cc.egui_ctx.clone(),
            last_inbox: Instant::now(),
            close_asked: false,
            closing: false,
            logo: None,
            logo_src: image::load_from_memory(crate::WORDMARK_PNG).ok().map(|i| i.to_rgba8()),
        };
        app.add_paths(files, None);
        if let Some(n) = settings_note {
            app.notify(n);
        }
        // test aid for --screenshot: PSYAUDIO_PAGE=cd shows the CD ripper with a sample disc
        if app.screenshot.is_some() {
            if std::env::var("PSYAUDIO_LIGHT").is_ok() {
                app.s.dark = false;
            }
            if std::env::var("PSYAUDIO_SIDE").as_deref() == Ok("closed") {
                app.s.side_open = false;
            }
            match std::env::var("PSYAUDIO_PAGE").as_deref() {
                Ok("cd") => {
                    app.page = Page::Cd;
                    let tracks: Vec<cdrip::TocTrack> =
                        (0..15).map(|i| cdrip::TocTrack { number: i + 1, lba: i as u32 * 13000, audio: true }).collect();
                    app.cd.toc = Some(Toc { tracks, leadout: 15 * 13000 });
                    app.cd.selected = vec![true; 15];
                    app.cd.meta = DiscMeta {
                        artist: crate::i18n::t("Beispiel-Band").into(),
                        album: crate::i18n::t("Beispiel-Album").into(),
                        year: "1992".into(),
                        disc: 1,
                        disc_total: 1,
                        tracks: (1..=15)
                            .map(|i| (crate::i18n::tf(crate::i18n::t("Ein ziemlich langer Beispiel-Titel Nummer {i}"), &[("i", format!("{i}"))]), crate::i18n::t("Beispiel-Band mit Gästen").into(), String::new()))
                            .collect(),
                        ..Default::default()
                    };
                    let (_tx, rx) = channel();
                    let mut rows = vec![(0.0f32, String::new(), String::new()); 15];
                    rows[0] = (1.0, crate::i18n::t("Sicher (1 Durchgang, von AccurateRip bestätigt)").into(), crate::i18n::t("AccurateRip: Genau (Vertrauen 70, v2)").into());
                    rows[1] = (1.0, crate::i18n::t("Sicher (2 Durchgänge identisch)").into(), crate::i18n::t("AccurateRip: Ungenau (Vertrauen 69) – oder andere Pressung").into());
                    rows[2] = (0.5, crate::i18n::t("Lesen 63 %").into(), String::new());
                    app.cd.rip = Some(RipRun { rx, cancel: Arc::new(AtomicBool::new(false)), rows, log: Vec::new(), finished: None });
                }
                Ok("settings") => app.page = Page::Settings,
                Ok("tags") => app.page = Page::Tags,
                Ok("batch") => app.page = Page::Batch,
                _ => {}
            }
        }
        app
    }

    pub(crate) fn env(&self) -> Env<'_> {
        Env { ffmpeg: self.ffmpeg.as_deref(), ffmpeg_encoders: &self.ffmpeg_encoders, tools: &self.s.tools, temp: Path::new("") }
    }

    pub(crate) fn notify(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    // ---------------- files ----------------

    pub(crate) fn add_paths(&mut self, paths: Vec<PathBuf>, root: Option<PathBuf>) {
        let mut new = Vec::new();
        for p in paths {
            if p.is_dir() {
                let mut seen = std::collections::HashSet::new();
                let mut stack = vec![p.clone()];
                while let Some(d) = stack.pop() {
                    // a junction that loops back is walked once
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
                                new.push((e, Some(root.clone().unwrap_or_else(|| p.clone()))));
                            }
                        }
                    }
                }
            } else if audio::is_audio_file(&p) {
                new.push((p, root.clone()));
            }
        }
        let mut known: std::collections::HashSet<PathBuf> = self.files.iter().map(|f| f.path.clone()).collect();
        let mut to_load = Vec::new();
        for (p, r) in new {
            if !known.insert(p.clone()) {
                continue;
            }
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            to_load.push(p.clone());
            self.files.push(FileItem { path: p, root: r, size, info: None });
        }
        if !to_load.is_empty() {
            let tx = self.info_tx.clone();
            let ctx = self.egui_ctx.clone();
            std::thread::spawn(move || {
                for p in to_load {
                    let (t, i) = tags::read(&p);
                    if tx.send((p, t, i)).is_err() {
                        break;
                    }
                    // the list shows the new length and size at once, not at the next mouse move
                    ctx.request_repaint();
                }
            });
        }
    }

    pub(crate) fn plan(&self, codec: Codec) -> Plan {
        Plan {
            codec,
            enc: self.s.enc.clone(),
            effects: crate::convert::effects_for(codec, &self.s.effects),
            output_mode: self.s.output_mode,
            output_folder: PathBuf::from(&self.s.output_folder),
            scheme: self.s.name_scheme.clone(),
            keep_structure: false,
            overwrite: self.s.overwrite == Overwrite::Always,
            overwrite_list: Vec::new(),
            copy_tags: self.s.copy_tags,
            copy_art: self.s.copy_art,
            threads: self.s.threads.max(1),
            low_priority: self.s.low_priority,
            ffmpeg: self.ffmpeg.clone(),
            ffmpeg_encoders: self.ffmpeg_encoders.clone(),
            tools: self.s.tools.clone(),
            temp: self.s.temp_dir(),
            tag_scheme: self.s.tag_scheme.clone(),
        }
    }

    pub(crate) fn start_conversion(&mut self, items: Vec<(PathBuf, Option<PathBuf>)>, codec: Codec) {
        self.start_conversion_with(items, codec, false);
    }

    /// `keep_structure`: batch run that keeps the folders below the batch folder.
    pub(crate) fn start_conversion_with(&mut self, items: Vec<(PathBuf, Option<PathBuf>)>, codec: Codec, keep_structure: bool) {
        if run_active(&self.run) {
            self.notify(crate::i18n::t("Es läuft schon eine Umwandlung – erst abwarten oder abbrechen"));
            return;
        }
        if items.is_empty() {
            self.notify(crate::i18n::t("Keine Dateien in der Liste"));
            return;
        }
        if let Some(m) = plan_requirement(codec, &self.s.enc, &self.env()) {
            self.notify(m);
            return;
        }
        let mut plan = self.plan(codec);
        plan.keep_structure = keep_structure;
        if (keep_structure || plan.output_mode != OutputMode::SourceFolder) && !plan.outputs().is_empty() {
            if let Some(m) = crate::convert::output_location_problem(&plan.output_folder) {
                self.notify(m);
                return;
            }
        }
        let jobs: Vec<Job> =
            items.into_iter().enumerate().map(|(i, (src, root))| Job { id: i, src, root, tags: None, audio: None }).collect();
        if self.s.overwrite == Overwrite::Ask && !plan.outputs().is_empty() {
            let mut existing = Vec::new();
            // tags are only read when the names are built from them (not for a batch with its
            // folder structure, nor for "keep path"), and a loaded file's tags are reused
            let known: std::collections::HashMap<&PathBuf, &(Tags, tags::Info)> =
                self.files.iter().filter_map(|f| f.info.as_ref().map(|i| (&f.path, i))).collect();
            let need_tags = plan.names_need_tags();
            for j in &jobs {
                let (t, i) = match known.get(&j.src) {
                    Some(x) => (*x).clone(),
                    None if need_tags => tags::read(&j.src),
                    None => (Tags::default(), tags::Info::default()),
                };
                for c in plan.outputs() {
                    let d = plan.destination(&j.src, j.root.as_deref(), &t, &i, c);
                    if d.exists() {
                        existing.push((d, true));
                    }
                }
            }
            if !existing.is_empty() {
                self.overwrite_prompt = Some((existing, jobs, plan));
                return;
            }
        }
        self.launch(jobs, plan);
    }

    pub(crate) fn launch(&mut self, jobs: Vec<Job>, plan: Plan) {
        // an overwrite question still open belongs to an older start; it must not start a second run
        self.overwrite_prompt = None;
        if run_active(&self.run) {
            self.notify(crate::i18n::t("Es läuft schon eine Umwandlung – erst abwarten oder abbrechen"));
            return;
        }
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let views = jobs
            .iter()
            .map(|j| JobView {
                src: j.src.clone(),
                root: j.root.clone(),
                state: JobState::Waiting,
                progress: 0.0,
                stage: "",
                outputs: Vec::new(),
                speed: 0.0,
                note: String::new(),
            })
            .collect();
        let threads = plan.threads.min(jobs.len()).max(1);
        let title = if plan.codec == Codec::Multi {
            crate::i18n::tf(crate::i18n::t("Umwandeln in {0} Formate"), &[("0", format!("{}", plan.enc.multi.len()))])
        } else if plan.codec.is_tool() && plan.outputs().is_empty() {
            plan.codec.name().to_string()
        } else if plan.codec.is_tool() {
            format!("{} ({})", plan.codec.name(), plan.enc.split_codec.name())
        } else {
            crate::i18n::tf(crate::i18n::t("Umwandeln nach {0}"), &[("0", format!("{}", plan.codec.name()))])
        };
        let audio_secs: f64 = jobs
            .iter()
            .filter_map(|j| self.files.iter().find(|f| f.path == j.src).and_then(|f| f.info.as_ref()).map(|i| i.1.seconds))
            .sum();
        let target = if plan.codec == Codec::Multi {
            plan.enc.multi.iter().map(|c| c.name()).collect::<Vec<_>>().join(", ")
        } else {
            plan.codec.name().to_string()
        };
        convert::start(plan, jobs, cancel.clone(), tx);
        self.run = Some(RunState { title, jobs: views, rx, cancel, started: Instant::now(), finished: None, reports: Vec::new(), threads, max_overall: 0.0, audio_secs, target });
        self.page = Page::Convert;
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((p, t, i)) = self.info_rx.try_recv() {
            if let Some(f) = self.files.iter_mut().find(|f| f.path == p) {
                f.info = Some((t, i));
            }
        }
        if let Some(run) = &mut self.run {
            while let Ok(m) = run.rx.try_recv() {
                match m {
                    Msg::Started(id) => run.jobs[id].state = JobState::Running,
                    Msg::Progress(id, f, stage) => {
                        run.jobs[id].progress = f;
                        run.jobs[id].stage = stage;
                    }
                    Msg::Done { id, outputs, audio_secs, wall_secs, note } => {
                        let j = &mut run.jobs[id];
                        j.state = JobState::Done;
                        j.progress = 1.0;
                        j.outputs = outputs;
                        j.speed = if wall_secs > 0.0 { audio_secs / wall_secs } else { 0.0 };
                        j.note = note;
                    }
                    Msg::Skipped(id, r) => {
                        run.jobs[id].state = JobState::Skipped(r);
                        run.jobs[id].progress = 1.0;
                    }
                    Msg::Failed(id, e) => {
                        run.jobs[id].state = JobState::Failed(e);
                        run.jobs[id].progress = 1.0;
                    }
                    Msg::Report(r) => run.reports.push(r),
                    Msg::Finished => {
                        run.finished = Some(run.started.elapsed().as_secs_f64());
                        // files whose tags changed: refresh
                        for f in self.files.iter_mut() {
                            f.info = None;
                        }
                        let paths: Vec<PathBuf> = self.files.iter().map(|f| f.path.clone()).collect();
                        let tx = self.info_tx.clone();
                        std::thread::spawn(move || {
                            for p in paths {
                                let (t, i) = tags::read(&p);
                                if tx.send((p, t, i)).is_err() {
                                    break;
                                }
                            }
                        });
                        if self.tag_edit.same_as(&self.tag_saved) {
                            self.tag_loaded = None;
                        }
                    }
                }
            }
            if run.finished.is_none() {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        // CD background work
        if let Some(rx) = &self.cd.lookup {
            if let Ok(r) = rx.try_recv() {
                self.cd.lookup = None;
                match r {
                    Ok((cands, cover)) => {
                        if let Some(first) = cands.first() {
                            self.cd.meta = first.clone();
                        }
                        self.cd.candidates = cands;
                        self.cd.cover = cover;
                        self.cd.cover_tex = None;
                        self.notify(crate::i18n::t("Titel-Infos gefunden"));
                    }
                    Err(e) => self.notify(e),
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
        if let Some(rx) = &self.cd.offset_rx {
            if let Ok(r) = rx.try_recv() {
                self.cd.offset_rx = None;
                match r {
                    Ok((off, n)) => {
                        self.s.rip.drive_offset = off;
                        self.notify(crate::i18n::tf(crate::i18n::t("Laufwerks-Versatz {off} Samples gefunden und eingetragen ({n} Titel stimmen überein)"), &[("off", format!("{off:+}")), ("n", format!("{n}"))]));
                    }
                    Err(e) => self.notify(e),
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
        if let Some(rx) = &self.cd.ar_rx {
            if let Ok(r) = rx.try_recv() {
                self.cd.ar = Some(r);
                self.cd.ar_rx = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
        }
        if let Some(rip) = &mut self.cd.rip {
            while let Ok(m) = rip.rx.try_recv() {
                match m {
                    RipMsg::Progress(i, f, s) => {
                        if let Some(r) = rip.rows.get_mut(i) {
                            r.0 = f;
                            r.1 = s;
                        }
                    }
                    RipMsg::TrackDone(i, s, ar) => {
                        if let Some(r) = rip.rows.get_mut(i) {
                            r.0 = 1.0;
                            r.1 = s;
                            r.2 = ar;
                        }
                    }
                    RipMsg::Log(l) => rip.log.push(l),
                    RipMsg::Finished(s) => rip.finished = Some(s),
                }
            }
            if rip.finished.is_none() {
                ctx.request_repaint_after(std::time::Duration::from_millis(150));
            }
        }
    }
}

impl eframe::App for PsyApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.s != self.saved && self.screenshot.is_none() {
            let _ = self.s.save();
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.applied_theme != Some(self.s.dark) {
            self.pal = theme::palette(self.s.dark);
            theme::apply(ctx, &self.pal, self.s.dark);
            self.applied_theme = Some(self.s.dark);
        }
        crate::i18n::set_lang(&self.s.lang);
        self.poll(ctx);
        // keep the PC awake while converting or ripping
        let busy = self.run.as_ref().is_some_and(|r| r.finished.is_none())
            || self.cd.rip.as_ref().is_some_and(|r| r.finished.is_none());
        if busy != self.awake {
            keep_awake(busy);
            self.awake = busy;
        }
        // progress only grows, and the window title shows "[NN%]"
        let want = match &mut self.run {
            Some(r) if r.finished.is_none() => {
                let total = r.jobs.len().max(1) as f32;
                let now = r.jobs.iter().map(|j| j.progress).sum::<f32>() / total;
                r.max_overall = r.max_overall.max(now);
                crate::i18n::tf(crate::i18n::t("[{0}%] PsyAudio – Umwandeln"), &[("0", format!("{:.0}", r.max_overall * 100.0))])
            }
            // "[Finished] PsyAudio" once the run is over
            Some(r) if r.finished.is_some() => crate::i18n::t("[Fertig] PsyAudio").to_string(),
            _ => "PsyAudio".to_string(),
        };
        if self.window_title != want {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(want.clone()));
            self.window_title = want;
        }

        // drag & drop
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            match self.page {
                Page::Tags => {
                    for d in dropped {
                        if audio::is_audio_file(&d) && !self.tag_files.contains(&d) {
                            self.tag_files.push(d);
                        }
                    }
                }
                Page::Batch => {
                    if let Some(d) = dropped.into_iter().find(|d| d.is_dir()) {
                        self.batch_folder = d.display().to_string();
                    }
                }
                _ => {
                    self.add_paths(dropped, None);
                    self.page = Page::Convert;
                }
            }
        }

        let p = self.pal;
        egui::SidePanel::left("nav")
            .show_separator_line(false)
            .exact_width(220.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(p.panel).inner_margin(egui::Margin::same(16)))
            .show(ctx, |ui| self.nav(ui));

        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg).inner_margin(egui::Margin::same(22))).show(ctx, |ui| {
            match self.page {
                Page::Convert => {
                    if self.run.is_some() {
                        self.run_view(ui);
                    } else {
                        self.convert_page(ui);
                    }
                }
                Page::Batch => self.batch_page(ui),
                Page::Cd => self.cd_page(ui),
                Page::Tags => self.tags_page(ui),
                Page::Settings => self.settings_page(ui),
            }
        });

        self.overwrite_window(ctx);
        self.tag_pending_window(ctx);
        self.close_guard(ctx);

        // files another PsyAudio handed over (Explorer menu with several files)
        if crate::PRIMARY.load(std::sync::atomic::Ordering::Relaxed) {
            if self.last_inbox.elapsed().as_millis() >= 700 {
                self.last_inbox = Instant::now();
                let files = crate::settings::take_handoffs(&crate::settings::handoff_dir());
                if !files.is_empty() {
                    self.add_paths(files, None);
                    self.page = Page::Convert;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(700));
        }

        if let Some((msg, t)) = &self.toast {
            if t.elapsed().as_secs_f32() < 4.0 {
                let msg = msg.clone();
                egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(110.0, -24.0)).show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(p.card2)
                        .stroke(egui::Stroke::new(1.0_f32, p.accent))
                        .corner_radius(egui::CornerRadius::same(theme::R_CTRL))
                        .inner_margin(egui::Margin::symmetric(16, 10))
                        .show(ui, |ui| ui.label(msg));
                });
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            } else {
                self.toast = None;
            }
        }

        // settings are written when they changed, at most twice a second (not every frame of a
        // slider drag); a failed write is shown once and tried again with the next change
        if self.s != self.saved && self.screenshot.is_none() {
            if self.last_save.elapsed().as_millis() >= 500 {
                self.last_save = Instant::now();
                match self.s.save() {
                    Ok(()) => self.saved = self.s.clone(),
                    Err(e) => {
                        self.saved = self.s.clone();
                        self.notify(crate::i18n::tf(crate::i18n::t("Einstellungen konnten nicht gespeichert werden: {e}"), &[("e", e)]));
                    }
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
        }

        // --screenshot <file.png>: draw ~120 frames, save the window picture, quit
        if let Some(path) = self.screenshot.clone() {
            self.frame += 1;
            if self.frame == 120 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let shot = ctx.input(|i| {
                i.raw.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
            });
            if let Some(img) = shot {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for c in &img.pixels {
                    buf.extend_from_slice(&c.to_array());
                }
                if let Some(d) = path.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                if let Some(rgba) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                    let _ = rgba.save(&path);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if self.frame > 600 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }
}

impl PsyApp {
    fn nav(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        // the PsyAudio lettering in the accent colour, as in the other Psy apps
        match &self.logo_src {
            Some(src) => {
                let ppp = ui.ctx().pixels_per_point();
                let w = (ui.available_width() - 8.0).min(176.0);
                // exact pixels on screen: width and height in whole device pixels
                let (pw, ph) = ((w * ppp).round() as u32, ((w * src.height() as f32 / src.width() as f32) * ppp).round() as u32);
                if self.logo.as_ref().map(|t| t.size() != [pw as usize, ph as usize]).unwrap_or(true) {
                    let small = image::imageops::resize(src, pw.max(1), ph.max(1), image::imageops::FilterType::Lanczos3);
                    let ci = egui::ColorImage::from_rgba_unmultiplied([small.width() as usize, small.height() as usize], small.as_raw());
                    self.logo = Some(ui.ctx().load_texture("psyaudio-logo", ci, egui::TextureOptions::NEAREST));
                }
                let (rect, _) = ui.allocate_exact_size(egui::vec2(pw as f32 / ppp, ph as f32 / ppp), egui::Sense::hover());
                // snapped to the pixel grid, so one texture pixel is one screen pixel
                let min = egui::pos2((rect.min.x * ppp).round() / ppp, (rect.min.y * ppp).round() / ppp);
                let rect = egui::Rect::from_min_size(min, rect.size());
                if let Some(logo) = &self.logo {
                    ui.painter().image(logo.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), p.accent);
                }
            }
            None => {
                ui.label(RichText::new("PsyAudio").size(22.0).family(theme::semibold()).color(p.accent));
            }
        }
        ui.add_space(18.0);
        let items = [
            (Page::Convert, egui_phosphor::regular::MUSIC_NOTES, crate::i18n::t("Konvertieren"), self.files.len()),
            (Page::Batch, egui_phosphor::regular::FOLDERS, crate::i18n::t("Stapel"), 0),
            (Page::Cd, egui_phosphor::regular::DISC, "CD-Ripper", 0),
            (Page::Tags, egui_phosphor::regular::TAG, "Tags", self.tag_files.len()),
            (Page::Settings, egui_phosphor::regular::GEAR_SIX, crate::i18n::t("Einstellungen"), 0),
        ];
        for (page, icon, label, badge) in items {
            let sel = self.page == page;
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click());
            let painter = ui.painter();
            if sel {
                painter.rect_filled(rect, egui::CornerRadius::same(theme::R_CTRL), p.accent.gamma_multiply(0.22));
                painter.rect_filled(
                    egui::Rect::from_min_size(rect.min + egui::vec2(0.0, 9.0), egui::vec2(3.0, 22.0)),
                    egui::CornerRadius::same(2),
                    p.accent,
                );
            } else if resp.hovered() {
                painter.rect_filled(rect, egui::CornerRadius::same(theme::R_CTRL), p.card);
            }
            painter.text(rect.left_center() + egui::vec2(16.0, 0.0), egui::Align2::LEFT_CENTER, icon, egui::FontId::proportional(16.0), if sel { p.accent } else { p.muted });
            painter.text(rect.left_center() + egui::vec2(44.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(14.5), if sel { p.text } else { p.muted });
            if badge > 0 {
                let r = egui::Rect::from_center_size(rect.right_center() - egui::vec2(22.0, 0.0), egui::vec2(32.0, 20.0));
                painter.rect_filled(r, egui::CornerRadius::same(theme::R_CTRL), p.card2);
                painter.text(r.center(), egui::Align2::CENTER_CENTER, badge.to_string(), egui::FontId::proportional(11.5), p.text);
            }
            if resp.clicked() {
                self.page = page;
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.horizontal(|ui| {
                for (code, short) in [("en", "EN"), ("de", "DE")] {
                    let on = self.s.lang == code;
                    let txt = RichText::new(short).size(11.5).color(if on { p.text } else { p.muted });
                    if ui.add(egui::Button::new(if on { txt.strong() } else { txt }).frame(on).small()).on_hover_text(crate::i18n::t("Sprache")).clicked() {
                        self.s.lang = code.to_string();
                        crate::i18n::set_lang(code);
                    }
                }
            });
            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).size(11.0).color(p.muted));
            let (txt, col) = match &self.ffmpeg {
                Some(_) => (icon_text(ph::CHECK_CIRCLE, crate::i18n::t("FFmpeg bereit")), p.good),
                None => (icon_text(ph::WARNING_CIRCLE, crate::i18n::t("FFmpeg fehlt (Einstellungen)")), p.warn),
            };
            ui.label(RichText::new(txt).size(11.5).color(col));
            if let Some(run) = &self.run {
                if run.finished.is_none() {
                    let done = run.jobs.iter().filter(|j| !matches!(j.state, JobState::Waiting | JobState::Running)).count();
                    ui.add(egui::ProgressBar::new(done as f32 / run.jobs.len().max(1) as f32).desired_height(6.0));
                    ui.label(RichText::new(crate::i18n::tf(crate::i18n::t("Umwandlung läuft: {done}/{0}"), &[("done", format!("{done}")), ("0", format!("{}", run.jobs.len()))])).size(11.5).color(p.accent));
                }
            }
        });
    }

    pub(crate) fn header(&self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        ui.label(RichText::new(title).size(26.0).family(theme::semibold()));
        ui.label(RichText::new(subtitle).color(self.pal.muted));
        ui.add_space(10.0);
    }

    /// Closing while files are written would leave half-written files: ask first, then cancel
    /// the work and close once the workers have stopped.
    fn close_guard(&mut self, ctx: &egui::Context) {
        // the screenshot mode shows sample work and closes itself
        if self.screenshot.is_some() {
            return;
        }
        let working = run_active(&self.run) || self.cd.rip.as_ref().is_some_and(|r| r.finished.is_none());
        if self.closing {
            if working {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
        if working && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_asked = true;
        }
        if !self.close_asked {
            return;
        }
        let mut answer = None;
        egui::Window::new(crate::i18n::t("PsyAudio beenden?"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(crate::i18n::t("Es wird noch umgewandelt oder gerippt. Beim Beenden wird die Arbeit abgebrochen; fertige Dateien bleiben."));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add(theme::primary(&self.pal, crate::i18n::t("Weiterarbeiten"))).clicked() {
                        answer = Some(false);
                    }
                    if ui.button(crate::i18n::t("Abbrechen und beenden")).clicked() {
                        answer = Some(true);
                    }
                });
            });
        match answer {
            Some(true) => {
                if let Some(r) = &self.run {
                    r.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                if let Some(r) = &self.cd.rip {
                    r.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                self.close_asked = false;
                self.closing = true;
            }
            Some(false) => self.close_asked = false,
            None => {}
        }
    }

    /// Page header with the button that brings back the hidden settings column (as in Psysonic).
    pub(crate) fn header_with_side(&mut self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        let p = self.pal;
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(26.0).family(theme::semibold()));
            if !self.s.side_open {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let b = egui::Button::new(RichText::new(egui_phosphor::regular::SIDEBAR_SIMPLE).size(18.0)).min_size(egui::vec2(34.0, 30.0));
                    if ui.add(b).on_hover_text(crate::i18n::t("Einstellungen einblenden")).clicked() {
                        self.s.side_open = true;
                    }
                });
            }
        });
        ui.label(RichText::new(subtitle).color(p.muted));
        ui.add_space(10.0);
    }

    /// The round handle on the seam between the page and its settings column: hides the column.
    pub(crate) fn side_handle(&mut self, ui: &mut egui::Ui, panel: egui::Rect) {
        let p = self.pal;
        let center = egui::pos2(panel.left() + 8.0, panel.center().y);
        let rect = egui::Rect::from_center_size(center, egui::vec2(24.0, 24.0));
        let r = ui.interact(rect, ui.id().with("side-handle"), egui::Sense::click()).on_hover_text(crate::i18n::t("Einstellungen ausblenden"));
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, ui.id().with("side-handle-paint")));
        painter.circle(center, 12.0, if r.hovered() { p.hover } else { p.card2 }, egui::Stroke::new(1.0_f32, p.field_border));
        painter.text(center, egui::Align2::CENTER_CENTER, egui_phosphor::regular::CARET_RIGHT, egui::FontId::proportional(13.0), p.text);
        if r.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if r.clicked() {
            self.s.side_open = false;
        }
    }

    fn remove_selected(&mut self) {
        let sel = std::mem::take(&mut self.selected);
        let mut i = 0;
        self.files.retain(|_| {
            let keep = !sel.contains(&i);
            i += 1;
            keep
        });
    }

    // ---------------- converter ----------------

    fn convert_page(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        if self.s.side_open {
            let panel = egui::SidePanel::right("output")
                .show_separator_line(false)
                .exact_width(side_panel_width(ui))
                .resizable(false)
                .frame(egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 0, top: 0, bottom: 0 }))
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        // room for the scroll bar, so nothing is drawn under or past it
                        ui.set_max_width(ui.available_width() - 14.0);
                        self.output_panel(ui)
                    });
                });
            self.side_handle(ui, panel.response.rect);
        }

        self.header_with_side(ui, crate::i18n::t("Konvertieren"), crate::i18n::t("Dateien hineinziehen, Format wählen, umwandeln."));
        ui.horizontal_wrapped(|ui| {
            if ui.add(theme::ghost(&icon_text(ph::PLUS, crate::i18n::t("Dateien")))).clicked() {
                if let Some(f) = rfd::FileDialog::new().add_filter("Audio", audio::INPUT_EXTENSIONS).pick_files() {
                    self.add_paths(f, None);
                }
            }
            if ui.add(theme::ghost(&icon_text(ph::FOLDER, crate::i18n::t("Ordner")))).clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    self.add_paths(vec![d], None);
                }
            }
            if ui.add_enabled(!self.selected.is_empty(), theme::ghost(&icon_text(ph::X, crate::i18n::t("Entfernen")))).clicked() {
                self.remove_selected();
            }
            if ui.add_enabled(!self.files.is_empty(), theme::ghost(crate::i18n::t("Liste leeren"))).clicked() {
                self.files.clear();
                self.selected.clear();
            }
            if ui.add_enabled(!self.files.is_empty(), theme::ghost(&icon_text(ph::TAG, crate::i18n::t("Tags bearbeiten")))).clicked() {
                let list: Vec<PathBuf> = if self.selected.is_empty() {
                    self.files.iter().map(|f| f.path.clone()).collect()
                } else {
                    self.selected.iter().filter_map(|i| self.files.get(*i)).map(|f| f.path.clone()).collect()
                };
                self.tag_files = list;
                self.tag_sel = 0;
                self.tag_loaded = None;
                self.page = Page::Tags;
            }
        });
        ui.add_space(6.0);

        egui::TopBottomPanel::bottom("convert_bar")
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 0, top: 12, bottom: 0 }))
            .show_inside(ui, |ui| {
                let total_secs: f64 = self.files.iter().filter_map(|f| f.info.as_ref()).map(|i| i.1.seconds).sum();
                let plan = self.plan(self.s.codec);
                let codecs = plan.outputs();
                let est: f64 = self
                    .files
                    .iter()
                    .filter_map(|f| f.info.as_ref())
                    .map(|(_, i)| {
                        codecs
                            .iter()
                            .map(|c| {
                                encode::estimate_bps(*c, &self.s.enc, i.rate.unwrap_or(44100), i.channels.unwrap_or(2) as usize, i.bits.unwrap_or(16) as u16)
                                    * i.seconds
                            })
                            .sum::<f64>()
                    })
                    .sum();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(crate::i18n::tf(crate::i18n::t("{0} Dateien · {1}"), &[("0", format!("{}", self.files.len())), ("1", format!("{}", fmt_len(total_secs)))])).strong());
                        if est > 0.0 {
                            ui.label(muted(&p, crate::i18n::tf(crate::i18n::t("Geschätzte Größe: {0}"), &[("0", format!("{}", fmt_size(est as u64)))])));
                        } else {
                            ui.label(muted(&p, self.s.codec.description()));
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if codecs.is_empty() { icon_text(ph::PLAY, crate::i18n::t("Ausführen")) } else { icon_text(ph::PLAY, crate::i18n::t("Konvertieren")) };
                        if ui.add_enabled(!self.files.is_empty(), theme::primary(&p, &label)).clicked() {
                            let items: Vec<(PathBuf, Option<PathBuf>)> = if self.selected.len() > 1 {
                                self.selected.iter().filter_map(|i| self.files.get(*i)).map(|f| (f.path.clone(), f.root.clone())).collect()
                            } else {
                                self.files.iter().map(|f| (f.path.clone(), f.root.clone())).collect()
                            };
                            let codec = self.s.codec;
                            self.start_conversion(items, codec);
                        }
                        if self.selected.len() > 1 {
                            ui.label(muted(&p, crate::i18n::tf(crate::i18n::t("nur {0} ausgewählte"), &[("0", format!("{}", self.selected.len()))])));
                        }
                    });
                });
            });

        theme::card(&p).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            if self.files.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() * 0.30).max(10.0));
                    ui.label(RichText::new(ph::DOWNLOAD_SIMPLE).size(46.0).color(p.accent));
                    ui.label(RichText::new(crate::i18n::t("Musikdateien oder Ordner hierher ziehen")).size(19.0).strong());
                    ui.label(muted(&p, crate::i18n::t("MP3, FLAC, WAV, AAC/M4A, ALAC, Opus, Ogg, WMA, AIFF, APE, WavPack, DSD und mehr")));
                    ui.add_space(12.0);
                    if ui.add(theme::primary(&p, crate::i18n::t("Dateien auswählen"))).clicked() {
                        if let Some(f) = rfd::FileDialog::new().add_filter("Audio", audio::INPUT_EXTENSIONS).pick_files() {
                            self.add_paths(f, None);
                        }
                    }
                });
                return;
            }
            ui.horizontal(|ui| {
                ui.add_space(46.0);
                ui.label(muted(&p, crate::i18n::t("Datei · Interpret – Titel")));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_sized([80.0, 18.0], egui::Label::new(muted(&p, crate::i18n::t("Größe"))));
                    ui.add_sized([64.0, 18.0], egui::Label::new(muted(&p, crate::i18n::t("Länge"))));
                    ui.add_sized([86.0, 18.0], egui::Label::new(muted(&p, "Format")));
                });
            });
            ui.separator();
            let mut clicked: Option<(usize, egui::Modifiers)> = None;
            let mut open_folder: Option<PathBuf> = None;
            egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, 44.0, self.files.len(), |ui, range| {
                for i in range {
                    let f = &self.files[i];
                    let sel = self.selected.contains(&i);
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click());
                    let painter = ui.painter();
                    if sel {
                        painter.rect_filled(rect, egui::CornerRadius::same(theme::R_CTRL), p.accent.gamma_multiply(0.2));
                    } else if resp.hovered() {
                        painter.rect_filled(rect, egui::CornerRadius::same(theme::R_CTRL), p.card2);
                    }
                    let ext = f.path.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default();
                    let badge = egui::Rect::from_center_size(rect.left_center() + egui::vec2(20.0, 0.0), egui::vec2(30.0, 30.0));
                    painter.rect_filled(badge, egui::CornerRadius::same(theme::R_CTRL), p.card2);
                    painter.text(badge.center(), egui::Align2::CENTER_CENTER, ph::MUSIC_NOTE, egui::FontId::proportional(15.0), p.accent);
                    let name = f.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let (sub, len, fmt) = match &f.info {
                        Some((t, i)) => {
                            let at = match (t.get("artist"), t.get("title")) {
                                ("", "") => f.path.parent().map(|d| d.display().to_string()).unwrap_or_default(),
                                (a, t2) => format!("{a} – {t2}"),
                            };
                            let fmt = match (i.rate, i.bits) {
                                (Some(r), Some(b)) => format!("{ext} {:.1}k/{b}", r as f32 / 1000.0),
                                (Some(r), None) => format!("{ext} {:.1}k", r as f32 / 1000.0),
                                _ => ext.clone(),
                            };
                            (at, fmt_len(i.seconds), fmt)
                        }
                        None => ("…".into(), String::new(), ext.clone()),
                    };
                    let right = rect.right();
                    let x_name = rect.left() + 46.0;
                    let name_w = (rect.width() - 46.0 - 240.0).max(80.0);
                    let clip = egui::Rect::from_min_size(egui::pos2(x_name, rect.top()), egui::vec2(name_w, rect.height()));
                    let cp = ui.painter().with_clip_rect(clip);
                    cp.text(egui::pos2(x_name, rect.top() + 12.0), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(14.0), p.text);
                    cp.text(egui::pos2(x_name, rect.top() + 29.0), egui::Align2::LEFT_CENTER, sub, egui::FontId::proportional(12.0), p.muted);
                    let painter = ui.painter();
                    painter.text(egui::pos2(right - 197.0, rect.center().y), egui::Align2::CENTER_CENTER, fmt, egui::FontId::proportional(12.0), p.muted);
                    painter.text(egui::pos2(right - 120.0, rect.center().y), egui::Align2::CENTER_CENTER, len, egui::FontId::proportional(13.0), p.text);
                    painter.text(egui::pos2(right - 44.0, rect.center().y), egui::Align2::CENTER_CENTER, fmt_size(f.size), egui::FontId::proportional(12.0), p.muted);
                    if resp.clicked() {
                        clicked = Some((i, ui.input(|i| i.modifiers)));
                    }
                    let path = f.path.clone();
                    resp.context_menu(|ui| {
                        if ui.button(crate::i18n::t("Ordner öffnen")).clicked() {
                            open_folder = path.parent().map(|d| d.to_path_buf());
                            ui.close_menu();
                        }
                    });
                }
            });
            if let Some(d) = open_folder {
                let _ = std::process::Command::new("explorer").arg(d).spawn();
            }
            if let Some((i, m)) = clicked {
                if m.ctrl || m.command {
                    if !self.selected.remove(&i) {
                        self.selected.insert(i);
                    }
                } else if m.shift {
                    if let Some(&a) = self.selected.iter().next() {
                        let (lo, hi) = if a < i { (a, i) } else { (i, a) };
                        self.selected = (lo..=hi).collect();
                    }
                } else {
                    self.selected = [i].into_iter().collect();
                }
            }
            let (select_all, delete) = ui.input(|i| (i.key_pressed(egui::Key::A) && i.modifiers.command, i.key_pressed(egui::Key::Delete)));
            if select_all && !ui.ctx().wants_keyboard_input() {
                self.selected = (0..self.files.len()).collect();
            }
            if delete && !self.selected.is_empty() && !ui.ctx().wants_keyboard_input() {
                self.remove_selected();
            }
        });
    }

    fn output_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        theme::card(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, &p, "Format");
            ui.add_space(4.0);
            // the formats in two short rows of chips, lossless first (instead of 15 big tiles)
            for (title, lossless) in [(crate::i18n::t("Verlustfrei"), true), (crate::i18n::t("Komprimiert"), false)] {
                ui.label(RichText::new(title).size(12.0).color(p.muted));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for c in encode::AUDIO_CODECS.iter().filter(|c| c.lossless() == lossless) {
                        let sel = self.s.codec == *c;
                        let missing = encode::missing_requirement(*c, &self.env()).is_some();
                        let text = RichText::new(c.short()).color(if sel { p.on_accent } else if missing { p.disabled } else { p.text });
                        let text = if sel { text.family(theme::semibold()) } else { text };
                        let r = ui.add(
                            egui::Button::new(text)
                                .fill(if sel { p.accent } else { p.card2 })
                                .stroke(egui::Stroke::new(1.0_f32, if sel { p.accent } else { p.field_border }))
                                .corner_radius(theme::R_CTRL)
                                .min_size(egui::vec2(64.0, 30.0)),
                        );
                        if r.clicked() {
                            self.s.codec = *c;
                        }
                        r.on_hover_text(format!("{} – {}", c.name(), c.description()));
                    }
                });
                ui.add_space(2.0);
            }
            ui.add_space(4.0);
            egui::CollapsingHeader::new(RichText::new(crate::i18n::t("Werkzeuge")).color(p.muted)).default_open(self.s.codec.is_tool()).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for c in encode::TOOL_CODECS {
                        if ui.selectable_label(self.s.codec == *c, c.name()).on_hover_text(c.description()).clicked() {
                            self.s.codec = *c;
                        }
                    }
                });
            });
            ui.add_space(6.0);
            ui.separator();
            ui.label(RichText::new(self.s.codec.name()).size(16.0).strong());
            if let Some(m) = plan_requirement(self.s.codec, &self.s.enc, &self.env()) {
                ui.label(RichText::new(format!("{}  {m}", ph::WARNING)).color(p.warn));
            }
            let codec = self.s.codec;
            codec_settings(ui, &p, codec, &mut self.s.enc);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let mut load = None;
                egui::ComboBox::from_id_salt("profiles").height(dropdown_height(ui)).selected_text(crate::i18n::t("Profile")).width(100.0).show_ui(ui, |ui| {
                    for (i, pr) in self.s.profiles.iter().enumerate() {
                        if ui.selectable_label(false, format!("{} ({})", pr.name, pr.codec.short())).clicked() {
                            load = Some(i);
                        }
                    }
                    if self.s.profiles.is_empty() {
                        ui.label(muted(&p, crate::i18n::t("Noch keine Profile")));
                    }
                });
                if let Some(i) = load {
                    let pr = self.s.profiles[i].clone();
                    self.profile_name = pr.name.clone();
                    self.s.codec = pr.codec;
                    self.s.enc = pr.enc;
                    self.s.effects = pr.effects;
                }
                ui.add(egui::TextEdit::singleline(&mut self.profile_name).hint_text(crate::i18n::t("Profilname")).desired_width(110.0));
                if ui.add_enabled(!self.profile_name.trim().is_empty(), egui::Button::new(crate::i18n::t("Speichern"))).clicked() {
                    let name = self.profile_name.trim().to_string();
                    self.s.profiles.retain(|x| x.name != name);
                    self.s.profiles.push(Profile { name: name.clone(), codec: self.s.codec, enc: self.s.enc.clone(), effects: self.s.effects.clone() });
                    self.notify(crate::i18n::tf(crate::i18n::t("Profil „{name}“ gespeichert"), &[("name", format!("{name}"))]));
                }
                if ui.add_enabled(self.s.profiles.iter().any(|x| x.name == self.profile_name.trim()), egui::Button::new(ph::TRASH)).on_hover_text(crate::i18n::t("Dieses Profil löschen")).clicked() {
                    let name = self.profile_name.trim().to_string();
                    self.s.profiles.retain(|x| x.name != name);
                    self.profile_name.clear();
                    self.notify(crate::i18n::tf(crate::i18n::t("Profil „{name}“ gelöscht"), &[("name", format!("{name}"))]));
                }
            });
        });
        ui.add_space(10.0);

        theme::card(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, &p, crate::i18n::t("Speichern in"));
            segmented(ui, &p, &mut self.s.output_mode, &[(OutputMode::SingleFolder, crate::i18n::t("Ein Ordner")), (OutputMode::SourceFolder, crate::i18n::t("Quellordner")), (OutputMode::PreservePath, crate::i18n::t("Pfad beibehalten"))]);
            if self.s.output_mode != OutputMode::SourceFolder {
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 44.0;
                    folder_field(ui, &p, &mut self.s.output_folder, crate::i18n::t("Ordner wählen …"), w);
                    if ui.button(ph::FOLDER_OPEN).on_hover_text(crate::i18n::t("Ordner im Explorer öffnen")).clicked() {
                        let _ = std::fs::create_dir_all(&self.s.output_folder);
                        let _ = std::process::Command::new("explorer").arg(&self.s.output_folder).spawn();
                    }
                });
            }
            if self.s.output_mode == OutputMode::PreservePath {
                ui.label(muted(&p, crate::i18n::t("Ordner und Dateinamen wie bei der Quelle.")));
            } else {
                scheme_editor(ui, &p, &mut self.s.name_scheme, naming::DATA, true, naming::CONVERT_SCHEME);
            }
            let first = self.selected.iter().next().and_then(|i| self.files.get(*i)).or_else(|| self.files.first());
            if let Some(f) = first {
                if let Some((t, i)) = &f.info {
                    let plan = self.plan(self.s.codec);
                    if let Some(c) = plan.outputs().first() {
                        let d = plan.destination(&f.path, f.root.as_deref(), t, i, *c);
                        ui.label(RichText::new(format!("» {}", d.display())).size(11.5).color(p.accent2));
                    }
                }
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(crate::i18n::t("Wenn vorhanden:"));
                segmented(ui, &p, &mut self.s.overwrite, &[(Overwrite::Ask, crate::i18n::t("Fragen")), (Overwrite::Always, crate::i18n::t("Überschreiben")), (Overwrite::Skip, crate::i18n::t("Überspringen"))]);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.s.copy_tags, crate::i18n::t("Tags übernehmen"));
                ui.checkbox(&mut self.s.copy_art, crate::i18n::t("Cover übernehmen"));
            });
        });
        ui.add_space(10.0);

        theme::card(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                section(ui, &p, crate::i18n::t("Effekte & Aktionen"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button(icon_text(ph::PLUS, crate::i18n::t("Hinzufügen")), |ui| {
                        let cat = dsp::catalogue();
                        let mut groups: Vec<&str> = cat.iter().map(|c| c.0).collect();
                        groups.dedup();
                        for g in groups {
                            ui.menu_button(g, |ui| {
                                for (_, n, e) in cat.iter().filter(|c| c.0 == g) {
                                    if ui.button(*n).clicked() {
                                        self.s.effects.push(e.clone());
                                        self.edit_effect = Some(self.s.effects.len() - 1);
                                        ui.close_menu();
                                    }
                                }
                            });
                        }
                    });
                });
            });
            if self.s.effects.is_empty() {
                ui.label(muted(&p, crate::i18n::t("Keine – das Audio wird unverändert umgewandelt.")));
            }
            let mut remove = None;
            let mut swap = None;
            let n = self.s.effects.len();
            for i in 0..n {
                let open = self.edit_effect == Some(i);
                egui::Frame::new().fill(p.card2).corner_radius(egui::CornerRadius::same(theme::R_CTRL)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let e = &self.s.effects[i];
                        ui.label(RichText::new(format!("{}.", i + 1)).color(p.muted));
                        ui.vertical(|ui| {
                            ui.label(RichText::new(e.name()).strong());
                            let s = e.summary();
                            if !s.is_empty() {
                                ui.label(RichText::new(s).size(11.5).color(p.muted));
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(ph::X).clicked() {
                                remove = Some(i);
                            }
                            if i + 1 < n && ui.small_button(ph::ARROW_DOWN).clicked() {
                                swap = Some((i, i + 1));
                            }
                            if i > 0 && ui.small_button(ph::ARROW_UP).clicked() {
                                swap = Some((i - 1, i));
                            }
                            if ui.small_button(if open { ph::CHECK } else { ph::GEAR_SIX }).clicked() {
                                self.edit_effect = if open { None } else { Some(i) };
                            }
                        });
                    });
                    if open {
                        ui.separator();
                        effect_editor(ui, &p, &mut self.s.effects[i]);
                    }
                });
            }
            if let Some(i) = remove {
                self.s.effects.remove(i);
                self.edit_effect = None;
            }
            if let Some((a, b)) = swap {
                self.s.effects.swap(a, b);
                self.edit_effect = None;
            }
        });
    }

    fn overwrite_window(&mut self, ctx: &egui::Context) {
        let p = self.pal;
        let Some((existing, _, _)) = &mut self.overwrite_prompt else { return };
        let mut action = None;
        let total = existing.len();
        // count the ticked files that exist; the main button reads "Overwrite (N)", or "Convert"
        // when none is ticked, and "Skip existing" only works when N > 0
        let ticked = existing.iter().filter(|(d, on)| *on && d.exists()).count();
        egui::Window::new(crate::i18n::t("Dateien sind schon vorhanden"))
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_width(620.0)
            .show(ctx, |ui| {
                ui.label(crate::i18n::tf(crate::i18n::t("{count} Zieldatei(en) gibt es bereits. Überschreiben?"), &[("count", format!("{total}"))]));
                ui.label(muted(&p, crate::i18n::t("Häkchen entfernen, um eine Datei zu überspringen.")));
                ui.horizontal(|ui| {
                    if ui.small_button(crate::i18n::t("Alle")).clicked() {
                        existing.iter_mut().for_each(|e| e.1 = true);
                    }
                    if ui.small_button(crate::i18n::t("Keine")).clicked() {
                        existing.iter_mut().for_each(|e| e.1 = false);
                    }
                });
                egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                    for (d, on) in existing.iter_mut() {
                        ui.checkbox(on, RichText::new(d.display().to_string()).size(12.0));
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let main = if ticked > 0 {
                        crate::i18n::tf(crate::i18n::t("Überschreiben ({count})"), &[("count", format!("{ticked}"))])
                    } else {
                        crate::i18n::t("Konvertieren").to_string()
                    };
                    if ui.add(theme::primary(&p, &main)).clicked() {
                        action = Some(1);
                    }
                    if ui.add_enabled(ticked > 0, egui::Button::new(crate::i18n::t("Vorhandene überspringen"))).clicked() {
                        action = Some(2);
                    }
                    if ui.button(crate::i18n::t("Abbrechen")).clicked() {
                        action = Some(0);
                    }
                });
            });
        if let Some(a) = action {
            let (existing, jobs, mut plan) = self.overwrite_prompt.take().unwrap();
            if a > 0 {
                plan.overwrite = false;
                if a == 1 {
                    plan.overwrite_list = existing.into_iter().filter(|(_, on)| *on).map(|(d, _)| d).collect();
                }
                self.launch(jobs, plan);
            }
        }
    }

    fn run_view(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        let mut action = 0;
        {
            let Some(run) = &mut self.run else { return };
            let total = run.jobs.len();
            let done = run.jobs.iter().filter(|j| matches!(j.state, JobState::Done)).count();
            let failed = run.jobs.iter().filter(|j| matches!(j.state, JobState::Failed(_))).count();
            let skipped = run.jobs.iter().filter(|j| matches!(j.state, JobState::Skipped(_))).count();
            // overall progress never goes backwards
            let now: f32 = run.jobs.iter().map(|j| j.progress).sum::<f32>() / total.max(1) as f32;
            run.max_overall = run.max_overall.max(now);
            let overall = run.max_overall;
            let elapsed = run.finished.unwrap_or_else(|| run.started.elapsed().as_secs_f64());
            if run.finished.is_some() {
                // "N files converted to <format>" (singular for exactly one file)
                let key = if done == 1 { "{0} Datei umgewandelt nach {1}" } else { "{0} Dateien umgewandelt nach {1}" };
                ui.label(RichText::new(crate::i18n::tf(crate::i18n::t(key), &[("0", done.to_string()), ("1", run.target.clone())])).size(26.0).strong());
            } else {
                ui.label(RichText::new(&run.title).size(26.0).strong());
            }
            let sub = if run.finished.is_some() {
                let mut s = crate::i18n::tf(crate::i18n::t("Fertig in {0} · {done} erledigt · {skipped} übersprungen · {failed} Fehler"), &[("0", long_duration(elapsed as u64, true)), ("done", format!("{done}")), ("skipped", format!("{skipped}")), ("failed", format!("{failed}"))]);
                if run.audio_secs > 0.0 && done > 0 {
                    s += " · ";
                    s += &encoding_speed(run.audio_secs, elapsed);
                }
                s
            } else {
                // "File X of Y", cores only with 2+ files, elapsed, remaining after 4 s
                let current = (run.jobs.iter().filter(|j| !matches!(j.state, JobState::Waiting)).count()).clamp(1, total.max(1));
                let mut s = crate::i18n::tf(crate::i18n::t("Datei {current} von {total}"), &[("current", format!("{current}")), ("total", format!("{total}"))]);
                if total >= 2 {
                    s += &crate::i18n::tf(crate::i18n::t(" · {0} Kern{1}"), &[("0", format!("{}", run.threads)), ("1", format!("{}", if run.threads == 1 { "" } else { "e" }))]);
                }
                s += &crate::i18n::tf(crate::i18n::t(" · {0} vergangen"), &[("0", format!("{}", fmt_len(elapsed)))]);
                if elapsed > 4.0 && overall > 0.0 {
                    let remaining = (elapsed / overall as f64 - elapsed).max(0.0);
                    s += &crate::i18n::tf(crate::i18n::t(" · noch etwa {0}"), &[("0", format!("{}", fmt_len(remaining)))]);
                }
                s
            };
            ui.label(muted(&p, sub));
            ui.add_space(8.0);
            ui.add(egui::ProgressBar::new(if run.finished.is_some() { 1.0 } else { overall }).show_percentage().desired_height(14.0));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if run.finished.is_none() {
                    if ui.add(theme::ghost(&icon_text(ph::STOP, crate::i18n::t("Abbrechen")))).clicked() {
                        run.cancel.store(true, Ordering::Relaxed);
                    }
                } else {
                    if ui.add(theme::primary(&p, crate::i18n::t("Fertig"))).clicked() {
                        action = 1;
                    }
                    if failed > 0 && ui.add(theme::ghost(crate::i18n::t("Fehlgeschlagene erneut umwandeln"))).clicked() {
                        action = 2;
                    }
                    if ui.add(theme::ghost(&icon_text(ph::FOLDER_OPEN, crate::i18n::t("Zielordner öffnen")))).clicked() {
                        if let Some(o) = run.jobs.iter().flat_map(|j| j.outputs.iter()).next() {
                            if let Some(d) = o.parent() {
                                let _ = std::process::Command::new("explorer").arg(d).spawn();
                            }
                        }
                    }
                }
            });
            ui.add_space(8.0);
            let reports = run.reports.clone();
            theme::card(&p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let h = if reports.is_empty() { ui.available_height() } else { ui.available_height() * 0.6 };
                egui::ScrollArea::vertical().max_height(h).auto_shrink([false, false]).show(ui, |ui| {
                    for j in &run.jobs {
                        ui.horizontal(|ui| {
                            let (icon, col) = match &j.state {
                                JobState::Waiting => (ph::CIRCLE, p.muted),
                                JobState::Running => (ph::PLAY, p.accent),
                                JobState::Done => (ph::CHECK, p.good),
                                JobState::Skipped(_) => (ph::WARNING, p.warn),
                                JobState::Failed(_) => (ph::X, p.bad),
                            };
                            ui.label(RichText::new(icon).color(col).size(16.0));
                            ui.vertical(|ui| {
                                let name = j.src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                ui.label(RichText::new(name).strong());
                                let detail = match &j.state {
                                    JobState::Waiting => crate::i18n::t("wartet").to_string(),
                                    JobState::Running => j.stage.to_string(),
                                    JobState::Done => {
                                        let mut s = if j.speed > 0.0 { crate::i18n::tf(crate::i18n::t("{0}× Echtzeit"), &[("0", format!("{:.1}", j.speed))]) } else { String::new() };
                                        if let Some(o) = j.outputs.first() {
                                            s = format!("{s}  » {}", o.display());
                                        }
                                        if !j.note.is_empty() {
                                            s = format!("{s}  {}", j.note);
                                        }
                                        s
                                    }
                                    JobState::Skipped(r) => crate::i18n::tf(crate::i18n::t("übersprungen: {r}"), &[("r", format!("{r}"))]),
                                    JobState::Failed(e) => crate::i18n::tf(crate::i18n::t("Fehler: {e}"), &[("e", format!("{e}"))]),
                                };
                                let col = if matches!(j.state, JobState::Failed(_)) { p.bad } else { p.muted };
                                ui.label(RichText::new(detail).size(11.5).color(col));
                            });
                            if matches!(j.state, JobState::Running) {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add(egui::ProgressBar::new(j.progress).desired_width(180.0).desired_height(8.0));
                                });
                            }
                        });
                        ui.add_space(2.0);
                    }
                });
                if !reports.is_empty() {
                    ui.separator();
                    section(ui, &p, crate::i18n::t("Ergebnisse"));
                    egui::ScrollArea::vertical().id_salt("reports").auto_shrink([false, false]).show(ui, |ui| {
                        for r in &reports {
                            ui.label(RichText::new(r).size(12.5).family(egui::FontFamily::Monospace));
                        }
                    });
                }
            });
        }
        if action == 2 {
            let run = self.run.take().unwrap();
            let failed: Vec<(PathBuf, Option<PathBuf>)> =
                run.jobs.iter().filter(|j| matches!(j.state, JobState::Failed(_))).map(|j| (j.src.clone(), j.root.clone())).collect();
            self.files.retain(|f| failed.iter().any(|x| x.0 == f.path));
            // files of a batch keep their batch folder, so a retry keeps the folder structure
            let missing: Vec<(PathBuf, Option<PathBuf>)> = failed.iter().filter(|x| !self.files.iter().any(|f| f.path == x.0)).cloned().collect();
            let mut roots: Vec<Option<PathBuf>> = missing.iter().map(|x| x.1.clone()).collect();
            roots.dedup();
            for root in roots {
                let paths: Vec<PathBuf> = missing.iter().filter(|x| x.1 == root).map(|x| x.0.clone()).collect();
                self.add_paths(paths, root);
            }
            self.selected.clear();
            self.notify(crate::i18n::t("Fehlgeschlagene Dateien sind wieder in der Liste – Einstellungen anpassen und erneut umwandeln"));
        } else if action == 1 {
            self.run = None;
        }
    }
}

/// SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) = 0x80000001 while converting
/// or ripping, so the PC doesn't go to standby; ES_CONTINUOUS alone (0x80000000) lets it sleep again.
pub(crate) const KEEP_AWAKE_FLAGS: u32 = 0x8000_0001;

fn keep_awake(on: bool) {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Power::SetThreadExecutionState(if on { KEEP_AWAKE_FLAGS } else { 0x8000_0000 });
    }
    #[cfg(not(windows))]
    let _ = on;
}

/// Seconds as words. Splits into days, hours, minutes and seconds
/// (/3600, %24, /60), shows the largest non-zero unit and, when `verbose` or that unit is below 4,
/// the next unit after a space. Singular for exactly 1.
pub(crate) fn long_duration(total: u64, verbose: bool) -> String {
    let hours_all = total / 3600;
    let rest = total % 3600;
    let (d, h, m, s) = (hours_all / 24, hours_all % 24, rest / 60, rest % 60);
    let unit = |n: u64, one: &str, many: &str| {
        crate::i18n::tf(crate::i18n::t(if n == 1 { one } else { many }), &[("0", n.to_string())])
    };
    let parts = [
        (d, unit(d, "{0} Tag", "{0} Tage")),
        (h, unit(h, "{0} Stunde", "{0} Stunden")),
        (m, unit(m, "{0} Minute", "{0} Minuten")),
        (s, unit(s, "{0} Sekunde", "{0} Sekunden")),
    ];
    match parts.iter().position(|(n, _)| *n > 0) {
        None => unit(0, "{0} Sekunde", "{0} Sekunden"),
        Some(3) => parts[3].1.clone(),
        Some(i) => {
            let mut out = parts[i].1.clone();
            if verbose || parts[i].0 < 4 {
                out.push(' ');
                out.push_str(&parts[i + 1].1);
            }
            out
        }
    }
}

/// Audio length / elapsed time (elapsed 0 counts as 1);
/// up to 4× shown with one decimal, above that rounded (+0.5) to a whole number.
pub(crate) fn encoding_speed(audio_secs: f64, elapsed_secs: f64) -> String {
    let e = if elapsed_secs == 0.0 { 1.0 } else { elapsed_secs };
    let x = audio_secs / e;
    let n = if x <= 4.0 { format!("{x:.1}") } else { format!("{}", (x + 0.5) as i64) };
    crate::i18n::tf(crate::i18n::t("{0}× Kodiergeschwindigkeit"), &[("0", n)])
}

#[cfg(test)]
mod finish_tests {
    use super::*;
    #[test]
    fn duration_and_speed_wording() {
        assert_eq!(long_duration(0, true), "0 Sekunden");
        assert_eq!(long_duration(1, true), "1 Sekunde");
        assert_eq!(long_duration(75, false), "1 Minute 15 Sekunden");
        assert_eq!(long_duration(5 * 60 + 9, false), "5 Minuten");
        assert_eq!(long_duration(5 * 60 + 9, true), "5 Minuten 9 Sekunden");
        assert_eq!(long_duration(90_000, true), "1 Tag 1 Stunde");
        assert_eq!(encoding_speed(300.0, 100.0), "3.0× Kodiergeschwindigkeit");
        assert_eq!(encoding_speed(300.0, 40.0), "8× Kodiergeschwindigkeit");
    }
}
