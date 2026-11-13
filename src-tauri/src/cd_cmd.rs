//! The CD ripper: drives, reading the disc, MusicBrainz and AccurateRip, the drive offset, the rip.
//! Everything that touches the drive or the network runs off the window's thread.

use crate::jobs::Kind;
use crate::run_cmd::ProgressGate;
use crate::state::AppState;
use base64::Engine;
use psyaudio_core::cdrip::{self, DiscMeta, Pressing, Toc};
use psyaudio_core::convert;
use psyaudio_core::encode::{self, Env};
use psyaudio_core::i18n::t;
use psyaudio_core::rip::{self, ArResult, RipContext, RipMsg, RipPhase, RipSummary, TrackStatus};
use psyaudio_core::settings::{OutputMode, Settings};
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
        let toc = Toc {
            tracks: vec![TocTrack { number: 1, lba: 0, audio: true }, TocTrack { number: 2, lba: 7500, audio: true }],
            leadout: 15000,
        };
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
    fn rip_events_carry_codes() {
        let e = rip_event(RipMsg::TrackDone(2, TrackStatus::Insecure { sectors: 4 }, None));
        let v = serde_json::to_value(e).unwrap();
        assert_eq!(v, serde_json::json!({ "kind": "TrackDone", "row": 2, "status": { "kind": "Insecure", "sectors": 4 }, "ar": null }));
    }
}

/// The disc read last, with what the lookups found (kept between commands).
#[derive(Default)]
pub struct CdState {
    drive: Option<char>,
    toc: Option<Toc>,
    /// AccurateRip's data, or why there is none; `None` while not asked yet.
    ar: Option<Result<Vec<Pressing>, String>>,
    ar_pending: bool,
    cover: Option<Vec<u8>>,
}

fn drive_char(id: &str) -> Result<char, String> {
    id.chars().next().ok_or_else(|| t("Kein CD-Laufwerk gefunden").to_string())
}

#[derive(Serialize)]
pub struct DriveInfo {
    id: String,
    name: String,
}

/// The CD drives of this computer (letters on Windows, /dev/srN on Linux).
#[tauri::command]
pub fn list_drives() -> Vec<DriveInfo> {
    cdrip::drives().into_iter().map(|d| DriveInfo { id: d.to_string(), name: cdrip::drive_name(d) }).collect()
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
    let info = disc_info(&toc);
    let mut cd = state.cd.lock().unwrap_or_else(|e| e.into_inner());
    *cd = CdState { drive: Some(d), toc: Some(toc), ..CdState::default() };
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
    Candidates { list: Vec<DiscMetaView> },
    Cover { mime: String, data_base64: String },
    AccurateRip { pressings: usize },
    Failed { source: &'static str, reason: String },
}

fn image_mime(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else {
        "image/jpeg"
    }
}

/// Asks MusicBrainz (titles, cover) and AccurateRip (whether the disc is known) about the disc read
/// last; each answer comes as its own event.
#[tauri::command]
pub fn lookup_disc(app: AppHandle, musicbrainz: bool, accuraterip: bool, on: Channel<LookupEvent>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let toc = {
        let mut cd = state.cd.lock().unwrap_or_else(|e| e.into_inner());
        if accuraterip {
            cd.ar_pending = true;
        }
        cd.toc.clone().ok_or_else(|| t("Zuerst eine CD lesen").to_string())?
    };
    if musicbrainz {
        let (toc, on, app) = (toc.clone(), on.clone(), app.clone());
        std::thread::spawn(move || match cdrip::musicbrainz_lookup(&toc) {
            Ok(list) => {
                let _ = on.send(LookupEvent::Candidates { list: list.iter().map(DiscMetaView::from).collect() });
                if let Some(cover) = list.first().and_then(|m| cdrip::cover_art(&m.release_id)) {
                    let data_base64 = base64::engine::general_purpose::STANDARD.encode(&cover);
                    let _ = on.send(LookupEvent::Cover { mime: image_mime(&cover).into(), data_base64 });
                    app.state::<AppState>().cd.lock().unwrap_or_else(|e| e.into_inner()).cover = Some(cover);
                }
            }
            Err(reason) => {
                let _ = on.send(LookupEvent::Failed { source: "musicbrainz", reason });
            }
        });
    }
    if accuraterip {
        std::thread::spawn(move || {
            let r = cdrip::accuraterip_lookup(&toc);
            let event = match &r {
                Ok(db) => LookupEvent::AccurateRip { pressings: db.len() },
                Err(reason) => LookupEvent::Failed { source: "accuraterip", reason: reason.clone() },
            };
            {
                let state = app.state::<AppState>();
                let mut cd = state.cd.lock().unwrap_or_else(|e| e.into_inner());
                cd.ar = Some(r);
                cd.ar_pending = false;
            }
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
        return Err(t("Erst das Rippen abwarten oder abbrechen").into());
    }
    tauri::async_runtime::spawn_blocking(move || cdrip::Drive::open(d).and_then(|dr| dr.eject())).await.map_err(|e| e.to_string())??;
    *state.cd.lock().unwrap_or_else(|e| e.into_inner()) = CdState::default();
    Ok(())
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind")]
pub enum OffsetEvent {
    Progress { fraction: f32 },
    Found { offset: i32, matching_tracks: usize },
    Failed { reason: String },
}

/// Finds the drive's read offset with this disc and AccurateRip's data; the window enters it.
/// Uses the drive like a rip, so it counts as one.
#[tauri::command]
pub fn find_offset(app: AppHandle, on: Channel<OffsetEvent>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (d, toc) = {
        let cd = state.cd.lock().unwrap_or_else(|e| e.into_inner());
        match (cd.drive, cd.toc.clone()) {
            (Some(d), Some(toc)) => (d, toc),
            _ => return Err(t("Zuerst eine CD lesen, die in der AccurateRip-Datenbank ist.").into()),
        }
    };
    let handle = state.jobs.begin(Kind::Rip)?;
    std::thread::spawn(move || {
        let progress = {
            let on = on.clone();
            let last = std::sync::Mutex::new(Instant::now());
            move |f: f32| {
                let mut l = last.lock().unwrap_or_else(|e| e.into_inner());
                if l.elapsed().as_millis() >= 100 {
                    *l = Instant::now();
                    let _ = on.send(OffsetEvent::Progress { fraction: f });
                }
            }
        };
        let r = (|| {
            let db = cdrip::accuraterip_lookup_full(&toc)?;
            let drive = cdrip::Drive::open(d)?;
            psyaudio_core::cdrip_read::find_offset(&drive, &toc, &db, &handle.cancel, &progress)
        })();
        handle.finish();
        let _ = on.send(match r {
            Ok((offset, matching_tracks)) => OffsetEvent::Found { offset, matching_tracks },
            Err(reason) => OffsetEvent::Failed { reason },
        });
    });
    Ok(())
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
    TrackDone { row: usize, status: TrackStatus, ar: Option<ArResult> },
    Log { line: String },
    Finished { summary: RipSummary },
}

fn rip_event(m: RipMsg) -> RipEvent {
    match m {
        RipMsg::Progress(row, fraction, phase) => RipEvent::Progress { row, fraction, phase },
        RipMsg::TrackDone(row, status, ar) => RipEvent::TrackDone { row, status, ar },
        RipMsg::Log(line) => RipEvent::Log { line },
        RipMsg::Finished(summary) => RipEvent::Finished { summary },
    }
}

/// Rips the ticked tracks into `rip_folder` with the CD ripper's format and naming; may run while
/// a conversion runs. Events per row; the window words the statuses.
#[tauri::command]
pub fn start_rip(app: AppHandle, settings: Settings, tracks: Vec<RipTrack>, meta: DiscMetaView, on: Channel<RipEvent>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (letter, toc, ar_db, cover) = {
        let cd = state.cd.lock().unwrap_or_else(|e| e.into_inner());
        let (Some(letter), Some(toc)) = (cd.drive, cd.toc.clone()) else {
            return Err(t("Zuerst eine CD lesen").into());
        };
        // no AccurateRip data: say why in every row instead of "not in the database"
        let ar_db = match (&cd.ar, cd.ar_pending) {
            (Some(r), _) => r.clone(),
            (None, true) => Err(t("Abfrage lief beim Start noch").to_string()),
            (None, false) => Err(t("nicht abgefragt").to_string()),
        };
        (letter, toc, ar_db, cd.cover.clone())
    };
    let codec = settings.rip_codec;
    let tools = crate::tools_cmd::ffmpeg(&state, &settings.tools.ffmpeg, false);
    let env = Env { ffmpeg: tools.ffmpeg.as_deref(), ffmpeg_encoders: &tools.encoders, tools: &settings.tools, temp: Path::new("") };
    if let Some(m) = encode::plan_requirement(codec, &settings.enc, &env) {
        return Err(m);
    }
    let mut plan = convert::plan_from_settings(&settings, codec, tools.ffmpeg.clone(), tools.encoders.clone());
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
        return Err(t("Keine Titel ausgewählt").into());
    }
    let handle = state.jobs.begin(Kind::Rip)?;
    let first_audio = audio.first().map(|tr| tr.number).unwrap_or(1);
    let last_audio = audio.last().map(|tr| tr.number).unwrap_or(1);
    let ctx = RipContext { letter, toc: toc.clone(), plan, rs: settings.rip.clone(), ar_db, cover, first_audio, last_audio, meta };
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
    let mut plan = convert::plan_from_settings(settings, settings.rip_codec, None, Vec::new());
    plan.output_mode = OutputMode::SingleFolder;
    plan.output_folder = PathBuf::from(&settings.rip_folder);
    plan.scheme = settings.rip.name_scheme.clone();
    let codec = *plan.outputs().first()?;
    let meta = meta.into_meta();
    let (title, artist, _) = meta.tracks.get(row).cloned().unwrap_or_default();
    let artist = if artist.trim().is_empty() { meta.artist.clone() } else { artist };
    let tg = cdrip::tags_for_track(&meta, number, total, &title, &artist);
    let src = PathBuf::from(format!("Track{number:02}.wav"));
    Some(plan.destination(&src, None, &tg, &tags::Info::default(), codec).to_string_lossy().into_owned())
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
