//! Converting: the checks before a start, the run in the background, its events for the window,
//! the window title with the overall progress and keeping the computer awake.

use crate::jobs::{Kind, RunHandle};
use crate::state::AppState;
use psyaudio_core::convert::{self, Job, Msg, Plan};
use psyaudio_core::encode::{self, Codec, Env};
use psyaudio_core::i18n::{t, tf};
use psyaudio_core::settings::{OutputMode, Overwrite, Settings};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_events_carry_their_kind() {
        let v = serde_json::to_value(RunEvent::Failed { id: 3, reason: "kaputt".into() }).unwrap();
        assert_eq!(v, serde_json::json!({ "kind": "Failed", "id": 3, "reason": "kaputt" }));
    }

    #[test]
    fn progress_is_sent_at_most_every_interval_per_file() {
        let mut gate = ProgressGate::new(2);
        let t0 = Instant::now();
        assert!(gate.due(0, t0));
        assert!(!gate.due(0, t0 + Duration::from_millis(20)));
        assert!(gate.due(1, t0 + Duration::from_millis(20)), "each file has its own clock");
        assert!(gate.due(0, t0 + PROGRESS_EVERY));
    }

    #[test]
    fn overall_never_drops() {
        let mut o = Overall::new(2);
        o.set(0, 0.8);
        assert_eq!(o.percent(), 40);
        o.set(0, 0.1); // a job restarts a stage at 0
        assert_eq!(o.percent(), 40);
        o.set(1, 1.0);
        o.set(0, 1.0);
        assert_eq!(o.percent(), 100);
    }
}

/// What the window learns about a run (mirrors `convert::Msg`).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "kind")]
pub enum RunEvent {
    Started { id: usize },
    Progress { id: usize, progress: f32, stage: String },
    Done { id: usize, outputs: Vec<String>, audio_secs: f64, wall_secs: f64, note: String },
    Skipped { id: usize, reason: String },
    Failed { id: usize, reason: String },
    Report { text: String },
    Finished { elapsed: f64 },
}

/// Progress of one file goes out at most this often; the stage and the end always go out.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

pub(crate) struct ProgressGate {
    last: Vec<Option<Instant>>,
}

impl ProgressGate {
    pub(crate) fn new(n: usize) -> Self {
        ProgressGate { last: vec![None; n] }
    }
    pub(crate) fn due(&mut self, id: usize, now: Instant) -> bool {
        let Some(slot) = self.last.get_mut(id) else { return false };
        if slot.is_some_and(|t| now.duration_since(t) < PROGRESS_EVERY) {
            return false;
        }
        *slot = Some(now);
        true
    }
}

/// The run's overall progress; it never goes back (a file starting its next stage at 0).
struct Overall {
    each: Vec<f32>,
    best: f32,
}

impl Overall {
    fn new(n: usize) -> Self {
        Overall { each: vec![0.0; n], best: 0.0 }
    }
    fn set(&mut self, id: usize, f: f32) {
        if let Some(p) = self.each.get_mut(id) {
            *p = f.clamp(0.0, 1.0);
        }
        let now = self.each.iter().sum::<f32>() / self.each.len().max(1) as f32;
        self.best = self.best.max(now);
    }
    fn percent(&self) -> u32 {
        (self.best * 100.0).round() as u32
    }
}

type Items = Vec<(String, Option<String>)>;

fn jobs(items: Items) -> Vec<Job> {
    items.into_iter().enumerate().map(|(id, (src, root))| Job { id, src: PathBuf::from(src), root: root.map(PathBuf::from), tags: None, audio: None }).collect()
}

/// The plan for a run, or why it cannot start (the texts the egui version showed).
fn prepared(state: &AppState, settings: &Settings, codec: Codec, keep_structure: bool) -> Result<Plan, String> {
    let cache = crate::tools_cmd::ffmpeg(state, &settings.tools.ffmpeg, false);
    let env = Env { ffmpeg: &cache.set, tools: &settings.tools, temp: Path::new("") };
    if let Some(m) = encode::plan_requirement(codec, &settings.enc, &env) {
        return Err(m);
    }
    let mut plan = convert::plan_from_settings(settings, codec, cache.set.clone());
    plan.keep_structure = keep_structure;
    if (keep_structure || plan.output_mode != OutputMode::SourceFolder) && !plan.outputs().is_empty() {
        if let Some(m) = convert::output_location_problem(&plan.output_folder) {
            return Err(m);
        }
    }
    Ok(plan)
}

/// Target files that exist already (only asked for with "Wenn vorhanden: Fragen").
#[tauri::command]
pub async fn check_existing(state: State<'_, AppState>, settings: Settings, items: Items, codec: Codec, keep_structure: bool) -> Result<Vec<String>, String> {
    if items.is_empty() {
        return Err(t("Keine Dateien in der Liste").into());
    }
    let plan = prepared(&state, &settings, codec, keep_structure)?;
    if settings.overwrite != Overwrite::Ask || plan.outputs().is_empty() {
        return Ok(Vec::new());
    }
    let jobs = jobs(items);
    tauri::async_runtime::spawn_blocking(move || {
        psyaudio_core::files::existing_outputs(&plan, &jobs, &HashMap::new()).into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Heading and target of the run view.
#[derive(Serialize)]
pub struct RunInfo {
    title: String,
    target: String,
    threads: usize,
}

fn run_info(plan: &Plan, files: usize) -> RunInfo {
    let title = if plan.codec == Codec::Multi {
        tf(t("Umwandeln in {0} Formate"), &[("0", plan.enc.multi.len().to_string())])
    } else if plan.codec.is_tool() && plan.outputs().is_empty() {
        plan.codec.name().to_string()
    } else if plan.codec.is_tool() {
        format!("{} ({})", plan.codec.name(), plan.enc.split_codec.name())
    } else {
        tf(t("Umwandeln nach {0}"), &[("0", plan.codec.name().to_string())])
    };
    let target =
        if plan.codec == Codec::Multi { plan.enc.multi.iter().map(|c| c.name()).collect::<Vec<_>>().join(", ") } else { plan.codec.name().to_string() };
    RunInfo { title, target, threads: plan.threads.min(files).max(1) }
}

/// Starts converting `items`; `overwrite_list` are the existing targets ticked in the question.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn start_conversion(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
    items: Items,
    codec: Codec,
    keep_structure: bool,
    overwrite_list: Vec<String>,
    on: Channel<RunEvent>,
) -> Result<RunInfo, String> {
    if items.is_empty() {
        return Err(t("Keine Dateien in der Liste").into());
    }
    let mut plan = prepared(&state, &settings, codec, keep_structure)?;
    if settings.overwrite == Overwrite::Ask {
        plan.overwrite = false;
        plan.overwrite_list = overwrite_list.into_iter().map(PathBuf::from).collect();
    }
    let handle = state.jobs.begin(Kind::Conversion)?;
    let jobs = jobs(items);
    let info = run_info(&plan, jobs.len());
    let n = jobs.len();
    let (tx, rx) = channel();
    convert::start(plan, jobs, handle.cancel.clone(), tx);
    std::thread::spawn(move || forward(app, handle, rx, n, on));
    Ok(info)
}

fn set_title(app: &AppHandle, text: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_title(text);
    }
}

/// Passes the run's messages to the window and keeps title and standby in step. Runs on its own
/// thread for the whole run: Windows ties "stay awake" to the thread that asked for it.
fn forward(app: AppHandle, handle: RunHandle, rx: Receiver<Msg>, n: usize, on: Channel<RunEvent>) {
    psyaudio_core::integration::keep_awake(true);
    let started = Instant::now();
    let mut gate = ProgressGate::new(n);
    let mut overall = Overall::new(n);
    let mut shown = None;
    let mut window_open = true;
    for msg in rx {
        let finished = matches!(msg, Msg::Finished);
        let event = match msg {
            Msg::Started(id) => Some(RunEvent::Started { id }),
            Msg::Progress(id, f, stage) => {
                overall.set(id, f);
                gate.due(id, Instant::now()).then(|| RunEvent::Progress { id, progress: f, stage: stage.to_string() })
            }
            Msg::Done { id, outputs, audio_secs, wall_secs, note } => {
                overall.set(id, 1.0);
                let outputs = outputs.iter().map(|o| o.to_string_lossy().into_owned()).collect();
                Some(RunEvent::Done { id, outputs, audio_secs, wall_secs, note })
            }
            Msg::Skipped(id, reason) => {
                overall.set(id, 1.0);
                Some(RunEvent::Skipped { id, reason })
            }
            Msg::Failed(id, reason) => {
                overall.set(id, 1.0);
                Some(RunEvent::Failed { id, reason })
            }
            Msg::Report(text) => Some(RunEvent::Report { text }),
            Msg::Finished => None,
        };
        if finished {
            break;
        }
        if shown != Some(overall.percent()) {
            shown = Some(overall.percent());
            set_title(&app, &tf(t("[{0}%] PsyAudio – Umwandeln"), &[("0", overall.percent().to_string())]));
        }
        if let Some(e) = event {
            // a closed window no longer listens; the run still ends normally
            window_open = window_open && on.send(e).is_ok();
        }
    }
    // free the slot before the window hears of the end, so a new start right away is accepted
    handle.finish();
    psyaudio_core::integration::keep_awake(false);
    set_title(&app, t("[Fertig] PsyAudio"));
    if window_open {
        let _ = on.send(RunEvent::Finished { elapsed: started.elapsed().as_secs_f64() });
    }
}

/// Asks the running conversion to stop.
#[tauri::command]
pub fn cancel_conversion(state: State<'_, AppState>) {
    state.jobs.cancel(Kind::Conversion);
}

/// The summary was closed: the title loses "[Fertig]".
#[tauri::command]
pub fn clear_run_title(app: AppHandle) {
    set_title(&app, "PsyAudio");
}
