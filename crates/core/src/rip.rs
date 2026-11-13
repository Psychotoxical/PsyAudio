//! Ripping a CD: the worker thread, its messages and the per-track verdicts.

use crate::cdrip::{self, DiscMeta, Pressing, Toc};
use crate::convert::{self, Job, Msg, Plan};
use crate::naming;
use crate::settings::{RipMode, RipSettings};
use crate::tags::Tags;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

/// What a rip reports; statuses are codes, worded only by `text()` (log, egui) or the window.
pub enum RipMsg {
    /// Row, share of the row done (0..1), what it is doing.
    Progress(usize, f32, RipPhase),
    TrackDone(usize, TrackStatus, Option<ArResult>),
    Log(String),
    Finished(RipSummary),
}

/// What a track's rip is doing right now.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum RipPhase {
    /// `done`: share of the reading (0..1).
    Reading {
        done: f32,
    },
    Encoding,
}

impl RipPhase {
    pub fn text(self) -> String {
        match self {
            RipPhase::Reading { done } if done <= 0.0 => crate::i18n::t("Lesen…").to_string(),
            RipPhase::Reading { done } => crate::i18n::tf(crate::i18n::t("Lesen {0} %"), &[("0", format!("{:.0}", done * 100.0))]),
            RipPhase::Encoding => crate::i18n::t("Kodieren…").to_string(),
        }
    }
}

/// How a track came off the disc.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum TrackStatus {
    /// Passes agreed (or AccurateRip confirmed the first one); `rereads` blocks were read again.
    Secure {
        passes: u32,
        rereads: u32,
        verified_first_pass: bool,
    },
    /// Burst mode: read once without correction.
    Burst,
    Insecure {
        sectors: u32,
    },
    /// "Titel als Fehler markieren": insecure, or (`unconfirmed`) not confirmed by AccurateRip.
    Marked {
        unconfirmed: bool,
        was: Box<TrackStatus>,
    },
    /// Reading (`encoding` false) or encoding failed.
    Failed {
        reason: String,
        encoding: bool,
    },
    Cancelled,
}

impl TrackStatus {
    /// The wording of the egui version (log file, egui window).
    pub fn text(&self) -> String {
        use crate::i18n::{t, tf};
        match self {
            TrackStatus::Secure { verified_first_pass: true, .. } => t("Sicher (1 Durchgang, von AccurateRip bestätigt)").to_string(),
            TrackStatus::Secure { passes, rereads, .. } if *rereads > 0 => {
                tf(t("Sicher (Warnung: {0} Durchgänge, {1} Blöcke neu gelesen)"), &[("0", passes.to_string()), ("1", rereads.to_string())])
            }
            TrackStatus::Secure { passes, .. } => tf(t("Sicher ({0} Durchgänge identisch)"), &[("0", passes.to_string())]),
            TrackStatus::Burst => t("Gerippt (Burst)").to_string(),
            TrackStatus::Insecure { sectors } => tf(t("Unsicher ({0} Sektoren)"), &[("0", sectors.to_string())]),
            TrackStatus::Marked { unconfirmed: false, was } => tf(t("Fehler – {status}"), &[("status", was.text())]),
            TrackStatus::Marked { unconfirmed: true, was } => tf(t("Fehler – nicht von AccurateRip bestätigt ({status})"), &[("status", was.text())]),
            TrackStatus::Failed { reason, encoding: false } => tf(t("Fehler: {e}"), &[("e", reason.clone())]),
            TrackStatus::Failed { reason, encoding: true } => tf(t("Fehler beim Kodieren: {e}"), &[("e", reason.clone())]),
            TrackStatus::Cancelled => t("Abgebrochen").to_string(),
        }
    }
}

/// What AccurateRip says about one ripped track.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ArResult {
    pub confirmed: bool,
    /// Of the matching entry; without a match, of the disc's first entry for this track.
    pub confidence: u8,
    /// 1 or 2 for the CRC version that matched, else 0.
    pub version: u8,
    /// The track has an entry in the database.
    pub in_db: bool,
    /// Why there is no AccurateRip data for the disc at all.
    pub unavailable: Option<String>,
}

impl ArResult {
    pub fn text(&self) -> String {
        use crate::i18n::{t, tf};
        if let Some(why) = &self.unavailable {
            return format!("AccurateRip: {why}");
        }
        if self.confirmed {
            tf(t("AccurateRip: Genau (Vertrauen {c}, {v})"), &[("c", self.confidence.to_string()), ("v", format!("v{}", self.version))])
        } else if self.in_db {
            tf(t("AccurateRip: Ungenau (Vertrauen {c}) – oder andere Pressung"), &[("c", self.confidence.to_string())])
        } else {
            t("AccurateRip: Titel nicht in der Datenbank").to_string()
        }
    }
}

/// AccurateRip's answer for one track: the best matching pressing (any confidence confirms).
pub fn ar_result(db: &Result<Vec<Pressing>, String>, idx: usize, v1: u32, v2: u32) -> ArResult {
    let db = match db {
        Ok(db) => db,
        Err(why) => return ArResult { confirmed: false, confidence: 0, version: 0, in_db: false, unavailable: Some(why.clone()) },
    };
    let mut best: Option<(u8, u8)> = None;
    for pressing in db {
        if let Some((conf, crc)) = pressing.get(idx) {
            let ver = if *crc == v2 {
                Some(2)
            } else if *crc == v1 {
                Some(1)
            } else {
                None
            };
            if let Some(v) = ver {
                if best.map(|b| *conf > b.0).unwrap_or(true) {
                    best = Some((*conf, v));
                }
            }
        }
    }
    match best {
        Some((confidence, version)) => ArResult { confirmed: true, confidence, version, in_db: true, unavailable: None },
        // nothing matches: report the confidence of the first entry
        None => match db.first().and_then(|p| p.get(idx)).filter(|(_, crc)| *crc != 0) {
            Some((c, _)) => ArResult { confirmed: false, confidence: *c, version: 0, in_db: true, unavailable: None },
            None => ArResult { confirmed: false, confidence: 0, version: 0, in_db: false, unavailable: None },
        },
    }
}

/// Text and verdict in one, as the egui version used them.
pub fn ar_verdict(db: &Result<Vec<Pressing>, String>, idx: usize, v1: u32, v2: u32) -> (String, bool) {
    let r = ar_result(db, idx, v1, v2);
    (r.text(), r.confirmed)
}

/// The status of a track; `mark_error` 1 marks insecure tracks, 2 unconfirmed ones as failed.
pub fn rip_status(mark_error: u8, status: TrackStatus, insecure: bool, confirmed: bool) -> TrackStatus {
    match mark_error {
        1 if insecure => TrackStatus::Marked { unconfirmed: false, was: Box::new(status) },
        2 if !confirmed => TrackStatus::Marked { unconfirmed: true, was: Box::new(status) },
        _ => status,
    }
}

/// How a rip ended.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum RipSummary {
    Done {
        files: usize,
        accurate: usize,
        insecure: usize,
        folder: Option<String>,
    },
    Cancelled,
    /// The drive could not be opened.
    Failed {
        reason: String,
    },
}

impl RipSummary {
    pub fn text(&self) -> String {
        use crate::i18n::{t, tf};
        match self {
            RipSummary::Done { files, accurate, .. } => {
                tf(t("Fertig: {0} Dateien, {accurate} von AccurateRip bestätigt"), &[("0", files.to_string()), ("accurate", accurate.to_string())])
            }
            RipSummary::Cancelled => t("Abgebrochen").to_string(),
            RipSummary::Failed { reason } => reason.clone(),
        }
    }
}

/// "Stop after an insecure track": decided on the rip itself, not on the status text.
pub fn stop_after_track(cancel_on_insecure: bool, insecure_sectors: u32) -> bool {
    cancel_on_insecure && insecure_sectors > 0
}

pub struct RipContext {
    pub letter: char,
    pub toc: Toc,
    pub plan: Plan,
    pub rs: RipSettings,
    /// The disc's AccurateRip data, or why there is none.
    pub ar_db: Result<Vec<Pressing>, String>,
    pub cover: Option<Vec<u8>>,
    pub first_audio: u8,
    pub last_audio: u8,
    pub meta: DiscMeta,
}

/// Rip the chosen tracks one after another, check them against AccurateRip and encode each.
pub fn rip_thread(c: RipContext, tracks: Vec<(usize, u8, Tags)>, cancel: Arc<AtomicBool>, tx: Sender<RipMsg>) {
    let RipContext { letter, toc, plan, rs, ar_db, cover, first_audio, last_audio, meta } = c;
    let drive = match cdrip::Drive::open(letter) {
        Ok(d) => d,
        Err(e) => {
            let _ = tx.send(RipMsg::Finished(RipSummary::Failed { reason: e }));
            return;
        }
    };
    let batch = convert::Batch::new();
    let mut log = vec![
        crate::i18n::t("PsyAudio – Protokoll der Audio-Extraktion").to_string(),
        crate::i18n::tf(crate::i18n::t("Album: {0} – {1}"), &[("0", meta.artist.to_string()), ("1", meta.album.to_string())]),
        crate::i18n::tf(
            crate::i18n::t("Laufwerk {letter}  Versatz {0} Samples  Modus: {1}"),
            &[
                ("letter", cdrip::drive_name(letter)),
                ("0", format!("{:+}", rs.drive_offset)),
                ("1", (if rs.mode == RipMode::Secure { crate::i18n::t("Sicher") } else { "Burst" }).to_string()),
            ],
        ),
        format!("MusicBrainz Disc-ID: {}   freedb: {:08x}", cdrip::musicbrainz_id(&toc), cdrip::freedb_id(&toc)),
        String::new(),
    ];
    let mut outputs: Vec<(u8, PathBuf)> = Vec::new();
    let (ctx, crx) = channel::<Msg>();
    let mut accurate = 0;
    let mut insecure_tracks = 0;
    let audio_numbers: Vec<u8> = toc.audio_tracks().iter().map(|t| t.number).collect();
    for (row, number, tg) in tracks {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let _ = tx.send(RipMsg::Progress(row, 0.0, RipPhase::Reading { done: 0.0 }));
        let txp = tx.clone();
        let prog = move |f: f32| {
            let _ = txp.send(RipMsg::Progress(row, f * 0.8, RipPhase::Reading { done: f }));
        };
        // a first pass that AccurateRip confirms needs no second pass
        let idx = audio_numbers.iter().position(|n| *n == number).unwrap_or(0);
        let ar_check = |raw: &[u8]| -> bool {
            let Ok(db) = ar_db.as_ref().map_err(|_| ()).and_then(|d| if rs.accuraterip { Ok(d) } else { Err(()) }) else { return false };
            // the 2940-sample edge applies to track 1 and the last track
            let (v1, v2) = cdrip::accuraterip_crc(raw, number == first_audio, number == last_audio);
            // any matching CRC counts, whatever its confidence
            db.iter().any(|p| p.get(idx).map(|(_, crc)| *crc != 0 && (*crc == v1 || *crc == v2)).unwrap_or(false))
        };
        let rp = crate::cdrip_read::ReadParams {
            secure: rs.mode == RipMode::Secure,
            reread_limit: rs.reread_limit,
            extra_passes: rs.extra_passes,
            extra_min: rs.extra_min,
            extra_max: rs.extra_max,
            extra_clean: rs.extra_clean,
            cache_kb: rs.cache_kb,
            abort_unrecoverable: rs.abort_unrecoverable,
            interpolate: rs.interpolate,
        };
        let r = crate::cdrip_read::rip_track(&drive, &toc, number, rs.drive_offset, rp, &cancel, &prog, &ar_check);
        let rip = match r {
            Ok(r) => r,
            Err(e) => {
                // a stop in the middle of a track is no read error
                let status = if cancel.load(Ordering::Relaxed) { TrackStatus::Cancelled } else { TrackStatus::Failed { reason: e.clone(), encoding: false } };
                let _ = tx.send(RipMsg::TrackDone(row, status, None));
                log.push(crate::i18n::tf(crate::i18n::t("Titel {number}: Fehler {e}"), &[("number", format!("{number:02}")), ("e", e.to_string())]));
                continue;
            }
        };
        let status = if rs.mode == RipMode::Burst {
            TrackStatus::Burst
        } else if rip.insecure_sectors > 0 {
            insecure_tracks += 1;
            TrackStatus::Insecure { sectors: rip.insecure_sectors }
        } else {
            TrackStatus::Secure { passes: rip.passes, rereads: rip.rereads, verified_first_pass: rip.verified_first_pass }
        };
        let (v1, v2) = cdrip::accuraterip_crc(&rip.raw, number == first_audio, number == last_audio);
        let idx = audio_numbers.iter().position(|n| *n == number).unwrap_or(0);
        let ar = rs.accuraterip.then(|| ar_result(&ar_db, idx, v1, v2));
        let confirmed = ar.as_ref().is_some_and(|a| a.confirmed);
        if confirmed {
            accurate += 1;
        }
        let status = rip_status(rs.mark_error, status, rip.insecure_sectors > 0, confirmed);
        log.push(crate::i18n::tf(
            crate::i18n::t("Titel {number}  {0}  CRC32 {1}  AR v1 {v1} v2 {v2}  {status}  {ar}"),
            &[
                ("number", format!("{number:02}")),
                ("0", tg.get("title").to_string()),
                ("1", format!("{:08X}", rip.crc)),
                ("v1", format!("{v1:08X}")),
                ("v2", format!("{v2:08X}")),
                ("status", status.text()),
                ("ar", ar.as_ref().map(ArResult::text).unwrap_or_default()),
            ],
        ));
        let _ = tx.send(RipMsg::Progress(row, 0.85, RipPhase::Encoding));
        let audio = Arc::new(cdrip::raw_to_audio(&rip.raw));
        let job = Job { id: 0, src: PathBuf::from(format!("Track{number:02}.wav")), root: None, tags: Some(tg), audio: Some(audio) };
        match batch.run(&plan, &job, &cancel, &ctx) {
            Ok(outs) => {
                for o in outs {
                    outputs.push((number, o));
                }
                let _ = tx.send(RipMsg::TrackDone(row, status.clone(), ar));
            }
            Err(e) => {
                let _ = tx.send(RipMsg::TrackDone(row, TrackStatus::Failed { reason: e.clone(), encoding: true }, ar));
                log.push(crate::i18n::tf(crate::i18n::t("   Kodierfehler: {e}"), &[("e", e.to_string())]));
            }
        }
        while crx.try_recv().is_ok() {}
        if stop_after_track(rs.cancel_on_insecure, rip.insecure_sectors) {
            log.push(crate::i18n::t("Abgebrochen nach unsicherem Titel").into());
            break;
        }
    }
    batch.finish(&plan, &ctx);
    while let Ok(m) = crx.try_recv() {
        if let Msg::Report(r) = m {
            let _ = tx.send(RipMsg::Log(r));
        }
    }
    let folder = outputs.first().and_then(|o| o.1.parent().map(|p| p.to_path_buf()));
    if let Some(folder) = &folder {
        let base = if meta.album.trim().is_empty() { "CD".to_string() } else { naming::clean_value(&meta.album) };
        if rs.write_log {
            log.push(String::new());
            log.push(crate::i18n::tf(
                crate::i18n::t("{accurate} Titel von AccurateRip bestätigt, {insecure_tracks} unsicher"),
                &[("accurate", format!("{accurate}")), ("insecure_tracks", format!("{insecure_tracks}"))],
            ));
            let _ = std::fs::write(folder.join(format!("{base}.log")), log.join("\r\n"));
        }
        if rs.write_cue {
            let mut cue = format!("PERFORMER \"{}\"\r\nTITLE \"{}\"\r\n", meta.artist, meta.album);
            for (n, o) in &outputs {
                let name = o.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                let i = audio_numbers.iter().position(|x| x == n).unwrap_or(0);
                let title = meta.tracks.get(i).map(|t| t.0.clone()).unwrap_or_default();
                cue += &format!("FILE \"{name}\" WAVE\r\n  TRACK {n:02} AUDIO\r\n    TITLE \"{title}\"\r\n    INDEX 01 00:00:00\r\n");
            }
            let _ = std::fs::write(folder.join(format!("{base}.cue")), cue);
        }
        if rs.folder_jpg {
            if let Some(c) = &cover {
                let _ = std::fs::write(folder.join("folder.jpg"), c);
            }
        }
    }
    if rs.eject_after && !cancel.load(Ordering::Relaxed) {
        let _ = drive.eject();
    }
    let summary = if cancel.load(Ordering::Relaxed) {
        RipSummary::Cancelled
    } else {
        RipSummary::Done { files: outputs.len(), accurate, insecure: insecure_tracks, folder: folder.map(|f| f.to_string_lossy().into_owned()) }
    };
    // the extraction log (CRC32 and AccurateRip per track) for the window, also when no file is written
    for line in &log {
        let _ = tx.send(RipMsg::Log(line.clone()));
    }
    let _ = tx.send(RipMsg::Finished(summary));
}

#[cfg(test)]
mod rip_status_tests {
    use super::*;

    #[test]
    fn missing_accuraterip_data_says_why() {
        let (text, ok) = ar_verdict(&Err("nicht abgefragt".into()), 0, 1, 2);
        assert_eq!(text, "AccurateRip: nicht abgefragt");
        assert!(!ok);
    }

    #[test]
    fn a_matching_crc_confirms_in_any_language() {
        let db: Vec<Pressing> = vec![vec![(12, 0xAAAA)], vec![(70, 0xBBBB)]];
        let (_, ok) = ar_verdict(&Ok(db.clone()), 0, 0x1111, 0xBBBB);
        assert!(ok);
        let (_, ok) = ar_verdict(&Ok(db), 0, 0x1111, 0x2222);
        assert!(!ok);
    }

    #[test]
    fn marking_as_failed_does_not_hide_an_insecure_track() {
        let secure = TrackStatus::Secure { passes: 2, rereads: 0, verified_first_pass: false };
        assert!(rip_status(2, secure.clone(), false, false).text().starts_with("Fehler"));
        assert_eq!(rip_status(2, secure.clone(), false, true), secure);
        assert!(rip_status(1, TrackStatus::Insecure { sectors: 3 }, true, true).text().starts_with("Fehler"));
        assert!(stop_after_track(true, 3));
        assert!(!stop_after_track(true, 0));
    }

    #[test]
    fn status_is_a_code() {
        let secure = TrackStatus::Secure { passes: 2, rereads: 0, verified_first_pass: false };
        assert_eq!(rip_status(2, secure.clone(), false, false), TrackStatus::Marked { unconfirmed: true, was: Box::new(secure.clone()) });
        assert_eq!(rip_status(2, secure.clone(), false, true), secure);
        let insecure = TrackStatus::Insecure { sectors: 3 };
        assert_eq!(rip_status(1, insecure.clone(), true, true), TrackStatus::Marked { unconfirmed: false, was: Box::new(insecure.clone()) });
        assert_eq!(rip_status(0, insecure.clone(), true, false), insecure);
        // the wording is only made for log and window
        assert_eq!(secure.text(), "Sicher (2 Durchgänge identisch)");
        let v = serde_json::to_value(&insecure).unwrap();
        assert_eq!(v, serde_json::json!({ "kind": "Insecure", "sectors": 3 }));
    }

    #[test]
    fn ar_result_from_verdict() {
        let db: Vec<Pressing> = vec![vec![(12, 0xAAAA)], vec![(70, 0xBBBB)]];
        let r = ar_result(&Ok(db.clone()), 0, 0x1111, 0xBBBB);
        assert!(r.confirmed);
        assert_eq!((r.confidence, r.version), (70, 2));
        let r = ar_result(&Ok(db), 0, 0x1111, 0x2222);
        assert!(!r.confirmed && r.in_db);
        assert_eq!(r.confidence, 12);
        let r = ar_result(&Err("nicht abgefragt".into()), 0, 1, 2);
        assert_eq!(r.unavailable.as_deref(), Some("nicht abgefragt"));
    }
}
