//! The CD ripper: drives, reading the disc, MusicBrainz and the CUETools database, the drive offset, the rip.
//! Everything that touches the drive or the network runs off the window's thread.

use crate::jobs::Kind;
use crate::run_cmd::ProgressGate;
use crate::state::AppState;
use base64::Engine;
use psyaudio_core::cdrip::{self, DiscMeta, Toc};
use psyaudio_core::convert;
use psyaudio_core::ctdb::{self, Entry};
use psyaudio_core::encode::{self, Env};
use psyaudio_core::ffmpeg::FfmpegSet;
use psyaudio_core::i18n::t;
use psyaudio_core::rip::{self, CtdbResult, RipContext, RipMsg, RipPhase, RipSummary, TrackStatus};
use psyaudio_core::settings::{DriveProfile, OffsetMethod, OutputMode, Settings};
use psyaudio_core::tags;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Instant;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

#[cfg(test)]
mod tests {
    use super::*;
    use psyaudio_core::cdrip::TocTrack;

    #[test]
    fn meta_survives_the_window() {
        let meta = DiscMeta {
            release_id: "release-1".into(),
            artist: "Band".into(),
            album: "Album".into(),
            year: "2024".into(),
            genre: "Rock".into(),
            disc: 1,
            disc_total: 2,
            label: "Label".into(),
            tracks: vec![("Lied".into(), "Band".into(), "ISRC1".into())],
        };
        let back = DiscMetaView::from(&meta).into_meta();
        assert_eq!(back.release_id, "release-1");
        assert_eq!(back.tracks, meta.tracks);
        assert_eq!((back.disc, back.disc_total), (1, 2));
    }

    #[test]
    fn disc_info_has_track_lengths() {
        let toc = Toc { tracks: vec![TocTrack { number: 1, lba: 0, audio: true }, TocTrack { number: 2, lba: 7500, audio: true }], leadout: 15000 };
        let info = disc_info(&toc);
        let secs: Vec<f64> = info.tracks.iter().map(|t| t.seconds).collect();
        assert_eq!(secs, vec![100.0, 100.0]);
        assert_eq!(info.total_seconds, 200.0);
        assert_eq!(info.freedb_id.len(), 8);
    }

    #[test]
    fn rip_preview_uses_the_disc() {
        let dir = std::env::temp_dir().join("psyaudio-rip-preview");
        let mut settings = Settings { rip_folder: dir.to_string_lossy().into_owned(), ..Settings::default() };
        settings.rip.name_scheme = ["album", "/", "track", " - ", "title"].iter().map(|s| s.to_string()).collect();
        let meta = DiscMetaView {
            album: "Album".into(),
            artist: "Band".into(),
            tracks: vec![TrackMeta { title: "Lied".into(), ..TrackMeta::default() }],
            ..DiscMetaView::default()
        };
        let got = PathBuf::from(rip_preview(&settings, meta, 0, 3, 1).unwrap());
        assert_eq!(got.parent(), Some(dir.join("Album").as_path()));
        assert_eq!(got.file_stem().map(|s| s.to_string_lossy().into_owned()), Some("03 - Lied".into()));
    }

    #[test]
    fn old_lookup_does_not_touch_new_disc() {
        let mut cd = CdState::default();
        let first = cd.generation;
        cd.new_disc(Some('G'), None);
        cd.set_cover(first, vec![1, 2, 3]);
        cd.set_ctdb(first, Ok(Vec::new()));
        assert!(cd.cover.is_none() && cd.ctdb.is_none(), "a lookup of the disc before must not land here");
        let now = cd.generation;
        cd.set_cover(now, vec![4]);
        assert_eq!(cd.cover.as_deref(), Some(&[4u8][..]));
    }

    #[test]
    fn rip_events_carry_codes() {
        let e = rip_event(RipMsg::TrackDone(2, TrackStatus::Insecure { sectors: 4 }, None));
        let v = serde_json::to_value(e).unwrap();
        assert_eq!(v, serde_json::json!({ "kind": "TrackDone", "row": 2, "status": { "kind": "Insecure", "sectors": 4 }, "ctdb": null }));
    }

    #[test]
    fn ctdb_answer_becomes_count_and_link() {
        let toc = Toc {
            tracks: vec![
                psyaudio_core::cdrip::TocTrack { number: 1, lba: 0, audio: true },
                psyaudio_core::cdrip::TocTrack { number: 2, lba: 11413, audio: true },
            ],
            leadout: 25024,
        };
        let entries = vec![psyaudio_core::ctdb::Entry { confidence: 7, track_crcs: vec![1, 2] }];
        let v = serde_json::to_value(ctdb_event(&toc, &Ok(entries))).unwrap();
        assert_eq!(v["kind"], "Ctdb");
        assert_eq!(v["total"], 7);
        assert!(v["url"].as_str().unwrap().starts_with("https://db.cue.tools/ui/?tocid="));
        let v = serde_json::to_value(ctdb_event(&toc, &Err("weg".into()))).unwrap();
        assert_eq!(v, serde_json::json!({ "kind": "Failed", "source": "ctdb", "reason": "weg" }));
    }

    #[test]
    fn a_measurement_becomes_a_drive_profile() {
        let p = profile_from("Band Drive X".into(), 667, Some(4096), "2026-11-14");
        assert_eq!(
            (p.model.as_str(), p.offset, p.method, p.measured.as_str(), p.buffer_kb),
            ("Band Drive X", 667, OffsetMethod::Ctdb, "2026-11-14", Some(4096))
        );
        let v = serde_json::to_value(MeasureEvent::Done { profile: p, agreeing_tracks: 2 }).unwrap();
        assert_eq!(v["kind"], "Done");
        assert_eq!(v["profile"]["offset"], 667);
        assert_eq!(v["agreeing_tracks"], 2);
    }
}

/// The disc read last, with what the lookups found (kept between commands).
#[derive(Default)]
pub struct CdState {
    /// Counts the discs read (and ejects): a lookup still running for the disc before must not
    /// write its cover or database answer onto the next one.
    generation: u64,
    drive: Option<char>,
    toc: Option<Toc>,
    /// The CUETools database's entries, or why there are none; `None` while not asked yet.
    ctdb: Option<Result<Vec<Entry>, String>>,
    ctdb_pending: bool,
    cover: Option<Vec<u8>>,
}

impl CdState {
    /// A disc was read (or the tray opened): forget the old one and its lookups.
    fn new_disc(&mut self, drive: Option<char>, toc: Option<Toc>) {
        *self = CdState { generation: self.generation + 1, drive, toc, ..CdState::default() };
    }

    /// The cover found by a lookup started for disc `generation`.
    fn set_cover(&mut self, generation: u64, cover: Vec<u8>) {
        if generation == self.generation {
            self.cover = Some(cover);
        }
    }

    /// The database's answer for disc `generation`.
    fn set_ctdb(&mut self, generation: u64, r: Result<Vec<Entry>, String>) {
        if generation == self.generation {
            self.ctdb = Some(r);
            self.ctdb_pending = false;
        }
    }
}

fn drive_char(id: &str) -> Result<char, String> {
    id.chars().next().ok_or_else(|| t("No CD drive found").to_string())
}

#[derive(Serialize)]
pub struct DriveInfo {
    id: String,
    name: String,
    /// Vendor and product, when the system tells them.
    model: Option<String>,
}

/// The CD drives of this computer (letters on Windows, /dev/srN on Linux).
#[tauri::command]
pub fn list_drives() -> Vec<DriveInfo> {
    cdrip::drives().into_iter().map(|d| DriveInfo { id: d.to_string(), name: cdrip::drive_name(d), model: cdrip::drive_model(d) }).collect()
}

#[derive(Serialize)]
pub struct TrackInfo {
    number: u8,
    seconds: f64,
}

#[derive(Serialize)]
pub struct DiscInfo {
    tracks: Vec<TrackInfo>,
    total_seconds: f64,
    musicbrainz_id: String,
    freedb_id: String,
}

fn disc_info(toc: &Toc) -> DiscInfo {
    let tracks: Vec<TrackInfo> = toc
        .audio_tracks()
        .iter()
        .map(|tr| TrackInfo { number: tr.number, seconds: toc.span(tr.number).map(|(a, b)| (b - a) as f64 / 75.0).unwrap_or(0.0) })
        .collect();
    DiscInfo {
        total_seconds: tracks.iter().map(|tr| tr.seconds).sum(),
        tracks,
        musicbrainz_id: cdrip::musicbrainz_id(toc),
        freedb_id: format!("{:08x}", cdrip::freedb_id(toc)),
    }
}

/// Reads the disc's table of contents; the lookups follow with `lookup_disc`.
#[tauri::command]
pub async fn read_disc(state: State<'_, AppState>, drive: String) -> Result<DiscInfo, String> {
    let d = drive_char(&drive)?;
    let toc = tauri::async_runtime::spawn_blocking(move || cdrip::Drive::open(d).and_then(|dr| dr.toc())).await.map_err(|e| e.to_string())??;
    cdrip::require_audio(&toc)?;
    let info = disc_info(&toc);
    state.cd.lock().unwrap_or_else(|e| e.into_inner()).new_disc(Some(d), Some(toc));
    Ok(info)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrackMeta {
    title: String,
    artist: String,
    isrc: String,
}

/// Album data as the window shows and edits it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiscMetaView {
    album: String,
    artist: String,
    year: String,
    genre: String,
    disc: u32,
    disc_total: u32,
    label: String,
    release_id: Option<String>,
    tracks: Vec<TrackMeta>,
}

impl From<&DiscMeta> for DiscMetaView {
    fn from(m: &DiscMeta) -> Self {
        DiscMetaView {
            album: m.album.clone(),
            artist: m.artist.clone(),
            year: m.year.clone(),
            genre: m.genre.clone(),
            disc: m.disc,
            disc_total: m.disc_total,
            label: m.label.clone(),
            release_id: (!m.release_id.is_empty()).then(|| m.release_id.clone()),
            tracks: m.tracks.iter().map(|(title, artist, isrc)| TrackMeta { title: title.clone(), artist: artist.clone(), isrc: isrc.clone() }).collect(),
        }
    }
}

impl DiscMetaView {
    fn into_meta(self) -> DiscMeta {
        DiscMeta {
            release_id: self.release_id.unwrap_or_default(),
            artist: self.artist,
            album: self.album,
            year: self.year,
            genre: self.genre,
            disc: self.disc.max(1),
            disc_total: self.disc_total.max(1),
            label: self.label,
            tracks: self.tracks.into_iter().map(|tr| (tr.title, tr.artist, tr.isrc)).collect(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind")]
pub enum LookupEvent {
    Candidates {
        list: Vec<DiscMetaView>,
    },
    Cover {
        mime: String,
        data_base64: String,
    },
    /// How many rips of the disc the database knows, and its public page.
    Ctdb {
        total: u32,
        url: String,
    },
    Failed {
        source: &'static str,
        reason: String,
    },
}

fn image_mime(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else {
        "image/jpeg"
    }
}

/// The database's answer as the window shows it: how many rips it knows and the disc's page.
fn ctdb_event(toc: &Toc, r: &Result<Vec<Entry>, String>) -> LookupEvent {
    match r {
        Ok(entries) => LookupEvent::Ctdb { total: ctdb::disc_confidence(entries, toc.audio_tracks().len()), url: ctdb::disc_url(toc) },
        Err(reason) => LookupEvent::Failed { source: "ctdb", reason: reason.clone() },
    }
}

/// Asks MusicBrainz (titles, cover) and the CUETools database (whether the disc is known) about
/// the disc read last; each answer comes as its own event.
#[tauri::command]
pub fn lookup_disc(app: AppHandle, musicbrainz: bool, ctdb: bool, on: Channel<LookupEvent>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (toc, generation) = {
        let mut cd = state.cd.lock().unwrap_or_else(|e| e.into_inner());
        if ctdb {
            cd.ctdb_pending = true;
        }
        (cd.toc.clone().ok_or_else(|| t("Read a CD first").to_string())?, cd.generation)
    };
    if musicbrainz {
        let (toc, on, app) = (toc.clone(), on.clone(), app.clone());
        std::thread::spawn(move || match cdrip::musicbrainz_lookup(&toc) {
            Ok(list) => {
                let _ = on.send(LookupEvent::Candidates { list: list.iter().map(DiscMetaView::from).collect() });
                if let Some(cover) = list.first().and_then(|m| cdrip::cover_art(&m.release_id)) {
                    let data_base64 = base64::engine::general_purpose::STANDARD.encode(&cover);
                    let _ = on.send(LookupEvent::Cover { mime: image_mime(&cover).into(), data_base64 });
                    app.state::<AppState>().cd.lock().unwrap_or_else(|e| e.into_inner()).set_cover(generation, cover);
                }
            }
            Err(reason) => {
                let _ = on.send(LookupEvent::Failed { source: "musicbrainz", reason });
            }
        });
    }
    if ctdb {
        std::thread::spawn(move || {
            let r = ctdb::lookup(&toc);
            let event = ctdb_event(&toc, &r);
            app.state::<AppState>().cd.lock().unwrap_or_else(|e| e.into_inner()).set_ctdb(generation, r);
            let _ = on.send(event);
        });
    }
    Ok(())
}

/// Opens the drive tray; the disc read last is forgotten.
#[tauri::command]
pub async fn eject(state: State<'_, AppState>, drive: String) -> Result<(), String> {
    let d = drive_char(&drive)?;
    if state.jobs.busy_with(Kind::Rip) {
        return Err(t("Wait for the rip to finish or cancel it first").into());
    }
    tauri::async_runtime::spawn_blocking(move || cdrip::Drive::open(d).and_then(|dr| dr.eject())).await.map_err(|e| e.to_string())??;
    state.cd.lock().unwrap_or_else(|e| e.into_inner()).new_disc(None, None);
    Ok(())
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind")]
pub enum MeasureEvent {
    Progress {
        fraction: f32,
    },
    /// The drive's profile for the window to store, and how many tracks agreed on the offset.
    Done {
        profile: DriveProfile,
        agreeing_tracks: usize,
    },
    Failed {
        reason: String,
    },
}

/// A measured drive as the settings keep it.
fn profile_from(model: String, offset: i32, buffer_kb: Option<u32>, today: &str) -> DriveProfile {
    DriveProfile { model, offset, method: OffsetMethod::Ctdb, measured: today.to_string(), buffer_kb }
}

/// Measures the drive's read offset (with the disc in it and the CUETools database) and its
/// buffer; the window stores the profile. Uses the drive like a rip, so it counts as one.
#[tauri::command]
pub fn measure_drive(app: AppHandle, drive: String, on: Channel<MeasureEvent>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let d = drive_char(&drive)?;
    let handle = state.jobs.begin(Kind::Rip)?;
    std::thread::spawn(move || {
        let progress = {
            let on = on.clone();
            let last = std::sync::Mutex::new(Instant::now());
            move |f: f32| {
                let mut l = last.lock().unwrap_or_else(|e| e.into_inner());
                if l.elapsed().as_millis() >= 100 {
                    *l = Instant::now();
                    let _ = on.send(MeasureEvent::Progress { fraction: f });
                }
            }
        };
        let r = (|| {
            let dr = cdrip::Drive::open(d)?;
            let toc = dr.toc()?;
            let entries = ctdb::lookup(&toc).map_err(|why| ctdb::unavailable_text(&why))?;
            let found = ctdb::find_offset(&dr, &toc, &entries, &handle.cancel, &progress);
            // our own handle would keep the disc busy (macOS unmounts it for the buffer query)
            drop(dr);
            // the drive would keep spinning at full speed for a while: let it rest
            let _ = psyaudio_core::scsi::stop_disc(d);
            let (offset, agreeing) = found?;
            let profile = profile_from(cdrip::drive_key(d), offset, psyaudio_core::scsi::buffer_kb(d), &psyaudio_core::settings::today());
            Ok::<_, String>((profile, agreeing))
        })();
        handle.finish();
        let _ = on.send(match r {
            Ok((profile, agreeing_tracks)) => MeasureEvent::Done { profile, agreeing_tracks },
            Err(reason) => MeasureEvent::Failed { reason },
        });
    });
    Ok(())
}

/// Opens the CUETools database's page of the disc read last in the browser, so anyone can check
/// the count; only that page, never an address from the window.
#[tauri::command]
pub fn open_ctdb_page(state: State<'_, AppState>) -> Result<(), String> {
    let toc = state.cd.lock().unwrap_or_else(|e| e.into_inner()).toc.clone().ok_or_else(|| t("Read a CD first").to_string())?;
    let url = ctdb::disc_url(&toc);
    psyaudio_core::audio::open_external(url.as_ref())
}

/// One row of the track list as the rip needs it.
#[derive(Clone, Debug, Deserialize)]
pub struct RipTrack {
    number: u8,
    title: String,
    artist: String,
    #[serde(default)]
    isrc: String,
    selected: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind")]
pub enum RipEvent {
    Progress { row: usize, fraction: f32, phase: RipPhase },
    TrackDone { row: usize, status: TrackStatus, ctdb: Option<CtdbResult> },
    Log { line: String },
    Finished { summary: RipSummary },
}

fn rip_event(m: RipMsg) -> RipEvent {
    match m {
        RipMsg::Progress(row, fraction, phase) => RipEvent::Progress { row, fraction, phase },
        RipMsg::TrackDone(row, status, ctdb) => RipEvent::TrackDone { row, status, ctdb },
        RipMsg::Log(line) => RipEvent::Log { line },
        RipMsg::Finished(summary) => RipEvent::Finished { summary },
    }
}

/// Rips the ticked tracks into `rip_folder` with the CD ripper's format and naming; may run while
/// a conversion runs. Events per row; the window words the statuses.
#[tauri::command]
pub fn start_rip(app: AppHandle, settings: Settings, tracks: Vec<RipTrack>, meta: DiscMetaView, on: Channel<RipEvent>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (letter, toc, ctdb, cover) = {
        let cd = state.cd.lock().unwrap_or_else(|e| e.into_inner());
        let (Some(letter), Some(toc)) = (cd.drive, cd.toc.clone()) else {
            return Err(t("Read a CD first").into());
        };
        // no answer from the database: say why in every row instead of "not in the database"
        let ctdb = match (&cd.ctdb, cd.ctdb_pending) {
            (Some(r), _) => r.clone(),
            (None, true) => Err(t("lookup was still running at the start").to_string()),
            (None, false) => Err(t("not looked up").to_string()),
        };
        (letter, toc, ctdb, cd.cover.clone())
    };
    let codec = settings.rip_codec;
    let tools = crate::tools_cmd::ffmpeg(&state, &settings.tools.ffmpeg, false);
    let env = Env { ffmpeg: &tools.set, tools: &settings.tools, temp: Path::new("") };
    if let Some(m) = encode::plan_requirement(codec, &settings.enc, &env) {
        return Err(m);
    }
    let mut plan = convert::plan_from_settings(&settings, codec, tools.set.clone());
    plan.output_mode = OutputMode::SingleFolder;
    plan.output_folder = PathBuf::from(&settings.rip_folder);
    if let Some(m) = convert::output_location_problem(&plan.output_folder) {
        return Err(m);
    }
    plan.scheme = settings.rip.name_scheme.clone();
    plan.overwrite = true;
    plan.copy_art = settings.rip.embed_art;
    plan.copy_tags = true;
    plan.threads = 1;

    let meta = meta.into_meta();
    let audio = toc.audio_tracks();
    let total = audio.len();
    let mut jobs = Vec::new();
    for (row, tr) in tracks.iter().enumerate() {
        if !tr.selected {
            continue;
        }
        let artist = if tr.artist.trim().is_empty() { meta.artist.clone() } else { tr.artist.clone() };
        let mut tg = cdrip::tags_for_track(&meta, tr.number, total, &tr.title, &artist);
        tg.set("isrc", tr.isrc.clone());
        if let (Some(c), true) = (&cover, settings.rip.embed_art) {
            tg.set_front_cover(tags::Art::new(c.clone(), tags::FRONT));
        }
        jobs.push((row, tr.number, tg));
    }
    if jobs.is_empty() {
        return Err(t("No tracks selected").into());
    }
    let handle = state.jobs.begin(Kind::Rip)?;
    let first_audio = audio.first().map(|tr| tr.number).unwrap_or(1);
    let last_audio = audio.last().map(|tr| tr.number).unwrap_or(1);
    let ctx = RipContext { letter, toc: toc.clone(), plan, rs: settings.rip.clone(), ctdb, cover, first_audio, last_audio, meta };
    let (tx, rx) = channel();
    let cancel = handle.cancel.clone();
    std::thread::spawn(move || rip::rip_thread(ctx, jobs, cancel, tx));
    let rows = tracks.len();
    std::thread::spawn(move || {
        // Windows ties "stay awake" to the thread that asked for it: this one lasts as long as the rip
        psyaudio_core::integration::keep_awake(true);
        let mut gate = ProgressGate::new(rows);
        for m in rx {
            let send = match &m {
                RipMsg::Progress(row, ..) => gate.due(*row, Instant::now()),
                _ => true,
            };
            let finished = matches!(m, RipMsg::Finished(_));
            if finished {
                handle.finish();
            }
            if send {
                let _ = on.send(rip_event(m));
            }
            if finished {
                break;
            }
        }
        handle.finish();
        psyaudio_core::integration::keep_awake(false);
    });
    Ok(())
}

/// Where track `number` (row `row`, of `total`) would be saved with the CD ripper's folder, naming and format.
fn rip_preview(settings: &Settings, meta: DiscMetaView, row: usize, number: u8, total: usize) -> Option<String> {
    let mut plan = convert::plan_from_settings(settings, settings.rip_codec, FfmpegSet::default());
    plan.output_mode = OutputMode::SingleFolder;
    plan.output_folder = PathBuf::from(&settings.rip_folder);
    plan.scheme = settings.rip.name_scheme.clone();
    let codec = *plan.outputs().first()?;
    let meta = meta.into_meta();
    let (title, artist, _) = meta.tracks.get(row).cloned().unwrap_or_default();
    let artist = if artist.trim().is_empty() { meta.artist.clone() } else { artist };
    let tg = cdrip::tags_for_track(&meta, number, total, &title, &artist);
    let src = PathBuf::from(format!("Track{number:02}.wav"));
    Some(plan.destination(&src, None, &tg, &tags::Info::default(), codec, None).to_string_lossy().into_owned())
}

/// The preview line of the CD ripper's naming: the first ticked track of this disc (or sample data).
#[tauri::command]
pub fn preview_rip_name(settings: Settings, meta: DiscMetaView, row: usize, number: u8, total: usize) -> Option<String> {
    rip_preview(&settings, meta, row, number, total)
}

/// Asks the running rip (or offset search) to stop.
#[tauri::command]
pub fn cancel_rip(state: State<'_, AppState>) {
    state.jobs.cancel(Kind::Rip);
}
