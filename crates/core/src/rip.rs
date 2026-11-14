//! Ripping a CD: the worker thread, its messages and the per-track verdicts.

use crate::cdrip::{self, DiscMeta, Toc};
use crate::convert::{self, Job, Msg, Plan};
use crate::ctdb::{self, Entry};
use crate::naming;
use crate::settings::{DriveProfile, OffsetMethod, RipMode, RipSettings};
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
    TrackDone(usize, TrackStatus, Option<CtdbResult>),
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
    /// Passes agreed (or the CUETools database confirmed the first one); `rereads` blocks were read again.
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
    /// "Titel als Fehler markieren": insecure, or (`unconfirmed`) not confirmed by the CUETools database.
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
        use crate::i18n::{plural, t, tf};
        match self {
            TrackStatus::Secure { verified_first_pass: true, .. } => t("Sicher (1 Durchgang, von CTDB bestätigt)").to_string(),
            TrackStatus::Secure { passes, rereads, .. } if *rereads > 0 => {
                let text = plural(
                    *rereads as u64,
                    "Sicher (Warnung: {0} Durchgänge, {1} Block neu gelesen)",
                    "Sicher (Warnung: {0} Durchgänge, {1} Blöcke neu gelesen)",
                );
                tf(t(text), &[("0", passes.to_string()), ("1", rereads.to_string())])
            }
            TrackStatus::Secure { passes, .. } => tf(t("Sicher ({0} Durchgänge identisch)"), &[("0", passes.to_string())]),
            TrackStatus::Burst => t("Gerippt (Burst)").to_string(),
            TrackStatus::Insecure { sectors } => {
                tf(t(plural(*sectors as u64, "Unsicher ({0} Sektor)", "Unsicher ({0} Sektoren)")), &[("0", sectors.to_string())])
            }
            TrackStatus::Marked { unconfirmed: false, was } => tf(t("Fehler – {status}"), &[("status", was.text())]),
            TrackStatus::Marked { unconfirmed: true, was } => tf(t("Fehler – nicht von CTDB bestätigt ({status})"), &[("status", was.text())]),
            TrackStatus::Failed { reason, encoding: false } => tf(t("Fehler: {e}"), &[("e", reason.clone())]),
            TrackStatus::Failed { reason, encoding: true } => tf(t("Fehler beim Kodieren: {e}"), &[("e", reason.clone())]),
            TrackStatus::Cancelled => t("Abgebrochen").to_string(),
        }
    }
}

/// What the CUETools database (CTDB) says about one ripped track.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CtdbResult {
    pub confirmed: bool,
    /// Rips in the database with the same checksum for this track.
    pub confidence: u32,
    /// Rips of the disc the database knows.
    pub total: u32,
    /// The database knows the disc.
    pub in_db: bool,
    /// Why there is no answer from the database at all.
    pub unavailable: Option<String>,
}

impl CtdbResult {
    pub fn text(&self) -> String {
        use crate::i18n::{t, tf};
        if let Some(why) = &self.unavailable {
            return format!("CTDB: {why}");
        }
        if self.confirmed {
            tf(t("CTDB: bestätigt ({c} von {total})"), &[("c", self.confidence.to_string()), ("total", self.total.to_string())])
        } else if self.in_db {
            t("CTDB: nicht bestätigt").to_string()
        } else {
            t("CTDB: CD nicht in der Datenbank").to_string()
        }
    }
}

/// The database's answer for track `idx` (among `audio_tracks` audio tracks) with checksum `crc`.
pub fn ctdb_result(db: &Result<Vec<Entry>, String>, idx: usize, crc: u32, audio_tracks: usize) -> CtdbResult {
    match db {
        Err(why) => CtdbResult { confirmed: false, confidence: 0, total: 0, in_db: false, unavailable: Some(why.clone()) },
        Ok(entries) => {
            let total = ctdb::disc_confidence(entries, audio_tracks);
            let confidence = ctdb::track_confidence(entries, idx, crc, audio_tracks);
            CtdbResult { confirmed: confidence > 0, confidence, total, in_db: total > 0, unavailable: None }
        }
    }
}

/// How the drive's offset was found, for the log: method and date, or that it was never measured.
pub fn offset_note(p: Option<&DriveProfile>) -> String {
    use crate::i18n::t;
    match p {
        None => t("nicht gemessen").to_string(),
        Some(p) => {
            let how = match p.method {
                OffsetMethod::Ctdb => "CTDB",
                OffsetMethod::Manual => t("von Hand"),
                OffsetMethod::Adopted => t("übernommen"),
            };
            format!("{how}, {}", p.measured)
        }
    }
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
        use crate::i18n::{plural, t, tf};
        match self {
            RipSummary::Done { files, accurate, .. } => {
                let text = plural(*files as u64, "Fertig: {0} Datei, {accurate} von CTDB bestätigt", "Fertig: {0} Dateien, {accurate} von CTDB bestätigt");
                tf(t(text), &[("0", files.to_string()), ("accurate", accurate.to_string())])
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
    /// The disc's entries in the CUETools database, or why there are none.
    pub ctdb: Result<Vec<Entry>, String>,
    pub cover: Option<Vec<u8>>,
    pub first_audio: u8,
    pub last_audio: u8,
    pub meta: DiscMeta,
}

/// Rip the chosen tracks one after another, check them against the CUETools database and encode each.
pub fn rip_thread(c: RipContext, tracks: Vec<(usize, u8, Tags)>, cancel: Arc<AtomicBool>, tx: Sender<RipMsg>) {
    let RipContext { letter, toc, plan, rs, ctdb: db, cover, first_audio, last_audio, meta } = c;
    // the drive's measured offset; until it has one, the single offset of earlier versions
    let key = cdrip::drive_key(letter);
    let profile = rs.drive(&key);
    let offset = profile.map(|p| p.offset).unwrap_or(rs.drive_offset);
    // a re-read must come from the disc: read more elsewhere than the drive's buffer holds
    let buffer = profile.and_then(|p| p.buffer_kb).or_else(|| crate::scsi::buffer_kb(letter));
    let flush = crate::scsi::flush_kb(rs.cache_kb, buffer);
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
            crate::i18n::t("Laufwerk {letter} ({model})  Versatz {0} Samples ({how})  Modus: {1}"),
            &[
                ("letter", cdrip::drive_name(letter)),
                ("model", key.clone()),
                ("0", format!("{offset:+}")),
                ("how", offset_note(profile)),
                ("1", (if rs.mode == RipMode::Secure { crate::i18n::t("Sicher") } else { "Burst" }).to_string()),
            ],
        ),
        crate::i18n::tf(
            crate::i18n::t("Laufwerkspuffer {0}, vor dem Neulesen werden {1} KB gelesen"),
            &[("0", buffer.map(|kb| format!("{kb} KB")).unwrap_or_else(|| crate::i18n::t("unbekannt").into())), ("1", flush.to_string())],
        ),
        format!("MusicBrainz Disc-ID: {}   freedb: {:08x}", cdrip::musicbrainz_id(&toc), cdrip::freedb_id(&toc)),
        String::new(),
    ];
    let mut outputs: Vec<(u8, PathBuf)> = Vec::new();
    let (ctx, crx) = channel::<Msg>();
    let mut accurate = 0;
    let mut insecure_tracks = 0;
    let audio_numbers: Vec<u8> = toc.audio_tracks().iter().map(|t| t.number).collect();
    let disc_samples = ctdb::disc_samples(&toc);
    let ctdb_crc = |raw: &[u8], number: u8| ctdb::track_crc(raw, number == first_audio, number == last_audio, disc_samples);
    for (row, number, tg) in tracks {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let _ = tx.send(RipMsg::Progress(row, 0.0, RipPhase::Reading { done: 0.0 }));
        let txp = tx.clone();
        let prog = move |f: f32| {
            let _ = txp.send(RipMsg::Progress(row, f * 0.8, RipPhase::Reading { done: f }));
        };
        // a first pass that the database confirms needs no second pass
        let idx = audio_numbers.iter().position(|n| *n == number).unwrap_or(0);
        let confirms = |raw: &[u8]| -> bool {
            match db.as_ref() {
                Ok(entries) if rs.ctdb => ctdb::track_confidence(entries, idx, ctdb_crc(raw, number), audio_numbers.len()) > 0,
                _ => false,
            }
        };
        let rp = crate::cdrip_read::ReadParams {
            secure: rs.mode == RipMode::Secure,
            reread_limit: rs.reread_limit,
            extra_passes: rs.extra_passes,
            extra_min: rs.extra_min,
            extra_max: rs.extra_max,
            extra_clean: rs.extra_clean,
            cache_kb: flush,
            abort_unrecoverable: rs.abort_unrecoverable,
            interpolate: rs.interpolate,
        };
        let r = crate::cdrip_read::rip_track(&drive, &toc, number, offset, rp, &cancel, &prog, &confirms);
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
        let check = rs.ctdb.then(|| ctdb_result(&db, idx, ctdb_crc(&rip.raw, number), audio_numbers.len()));
        let confirmed = check.as_ref().is_some_and(|c| c.confirmed);
        if confirmed {
            accurate += 1;
        }
        let status = rip_status(rs.mark_error, status, rip.insecure_sectors > 0, confirmed);
        log.push(crate::i18n::tf(
            crate::i18n::t("Titel {number}  {0}  CRC32 {1}  {status}  {ctdb}"),
            &[
                ("number", format!("{number:02}")),
                ("0", tg.get("title").to_string()),
                ("1", format!("{:08X}", rip.crc)),
                ("status", status.text()),
                ("ctdb", check.as_ref().map(CtdbResult::text).unwrap_or_default()),
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
                let _ = tx.send(RipMsg::TrackDone(row, status.clone(), check));
            }
            Err(e) => {
                let _ = tx.send(RipMsg::TrackDone(row, TrackStatus::Failed { reason: e.clone(), encoding: true }, check));
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
                crate::i18n::t(crate::i18n::plural(
                    accurate as u64,
                    "1 Titel von CTDB bestätigt, {insecure_tracks} unsicher",
                    "{accurate} Titel von CTDB bestätigt, {insecure_tracks} unsicher",
                )),
                &[("accurate", format!("{accurate}")), ("insecure_tracks", format!("{insecure_tracks}"))],
            ));
            // anyone can check the result on the disc's public page
            if rs.ctdb {
                log.push(format!("CTDB: {}", ctdb::disc_url(&toc)));
            }
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
    } else {
        // the drive would keep spinning at full speed for a while: let it rest
        let _ = crate::scsi::stop_disc(letter);
    }
    let summary = if cancel.load(Ordering::Relaxed) {
        RipSummary::Cancelled
    } else {
        RipSummary::Done { files: outputs.len(), accurate, insecure: insecure_tracks, folder: folder.map(|f| f.to_string_lossy().into_owned()) }
    };
    // the extraction log (CRC32 and CTDB per track) for the window, also when no file is written
    for line in &log {
        let _ = tx.send(RipMsg::Log(line.clone()));
    }
    let _ = tx.send(RipMsg::Finished(summary));
}

#[cfg(test)]
mod rip_status_tests {
    use super::*;

    fn entry(confidence: u32, crcs: &[u32]) -> Entry {
        Entry { confidence, track_crcs: crcs.to_vec() }
    }

    #[test]
    fn ctdb_result_counts_and_explains() {
        let db = vec![entry(29, &[0xA, 0xB]), entry(1039, &[0xA, 0xB]), entry(1, &[0xA, 0xC])];
        let r = ctdb_result(&Ok(db.clone()), 1, 0xB, 2);
        assert_eq!((r.confirmed, r.confidence, r.total, r.in_db), (true, 1068, 1069, true));
        assert_eq!(r.text(), "CTDB: bestätigt (1068 von 1069)");
        let r = ctdb_result(&Ok(db), 1, 0xD, 2);
        assert_eq!((r.confirmed, r.in_db), (false, true));
        assert_eq!(r.text(), "CTDB: nicht bestätigt");
        let r = ctdb_result(&Ok(Vec::new()), 0, 0xA, 2);
        assert_eq!(r.text(), "CTDB: CD nicht in der Datenbank");
        let r = ctdb_result(&Err("nicht abgefragt".into()), 0, 1, 2);
        assert!(!r.confirmed);
        assert_eq!(r.unavailable.as_deref(), Some("nicht abgefragt"));
        assert_eq!(r.text(), "CTDB: nicht abgefragt");
    }

    #[test]
    fn mark_error_two_uses_ctdb_wording() {
        let secure = TrackStatus::Secure { passes: 2, rereads: 0, verified_first_pass: false };
        assert!(rip_status(2, secure, false, false).text().contains("nicht von CTDB bestätigt"));
        let first = TrackStatus::Secure { passes: 1, rereads: 0, verified_first_pass: true };
        assert_eq!(first.text(), "Sicher (1 Durchgang, von CTDB bestätigt)");
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
    fn one_sector_block_or_file_is_named_in_the_singular() {
        assert_eq!(TrackStatus::Insecure { sectors: 1 }.text(), "Unsicher (1 Sektor)");
        assert_eq!(TrackStatus::Insecure { sectors: 3 }.text(), "Unsicher (3 Sektoren)");
        let warned = TrackStatus::Secure { passes: 2, rereads: 1, verified_first_pass: false };
        assert_eq!(warned.text(), "Sicher (Warnung: 2 Durchgänge, 1 Block neu gelesen)");
        let done = RipSummary::Done { files: 1, accurate: 1, insecure: 0, folder: None };
        assert_eq!(done.text(), "Fertig: 1 Datei, 1 von CTDB bestätigt");
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
    fn offset_note_names_method_and_date() {
        let p = crate::settings::DriveProfile {
            model: "Band Drive X".into(),
            offset: 667,
            method: crate::settings::OffsetMethod::Ctdb,
            measured: "2026-11-14".into(),
            buffer_kb: None,
        };
        assert_eq!(offset_note(Some(&p)), "CTDB, 2026-11-14");
        assert_eq!(offset_note(None), "nicht gemessen");
    }
}
