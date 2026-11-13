//! Ripping a CD: the worker thread, its messages and the per-track verdicts.

use crate::naming;
use crate::cdrip::{self, DiscMeta, Pressing, Toc};
use crate::convert::{self, Job, Msg, Plan};
use crate::settings::{RipMode, RipSettings};
use crate::tags::Tags;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

pub enum RipMsg {
    Progress(usize, f32, String),
    TrackDone(usize, String, String),
    Log(String),
    Finished(String),
}

/// What AccurateRip says about one ripped track: the text for its row and log, and whether it
/// confirms the rip (a flag, so no check depends on the wording of a language).
pub fn ar_verdict(db: &Result<Vec<Pressing>, String>, idx: usize, v1: u32, v2: u32) -> (String, bool) {
    let db = match db {
        Ok(db) => db,
        Err(why) => return (format!("AccurateRip: {why}"), false),
    };
    let mut best: Option<(u8, &str)> = None;
    for pressing in db {
        if let Some((conf, crc)) = pressing.get(idx) {
            let ver = if *crc == v2 { Some("v2") } else if *crc == v1 { Some("v1") } else { None };
            if let Some(v) = ver {
                if best.map(|b| *conf > b.0).unwrap_or(true) {
                    best = Some((*conf, v));
                }
            }
        }
    }
    match best {
        Some((c, v)) => (crate::i18n::tf(crate::i18n::t("AccurateRip: Genau (Vertrauen {c}, {v})"), &[("c", format!("{c}")), ("v", v.to_string())]), true),
        // nothing matches: report the confidence of the first entry
        None => match db.first().and_then(|p| p.get(idx)).filter(|(_, crc)| *crc != 0) {
            Some((c, _)) => (crate::i18n::tf(crate::i18n::t("AccurateRip: Ungenau (Vertrauen {c}) – oder andere Pressung"), &[("c", format!("{c}"))]), false),
            None => (crate::i18n::t("AccurateRip: Titel nicht in der Datenbank").to_string(), false),
        },
    }
}

/// The status shown for a track; `mark_error` 1 marks insecure tracks, 2 unconfirmed ones as failed.
pub fn rip_status(mark_error: u8, status: String, insecure: bool, confirmed: bool) -> String {
    match mark_error {
        1 if insecure => crate::i18n::tf(crate::i18n::t("Fehler – {status}"), &[("status", status)]),
        2 if !confirmed => crate::i18n::tf(crate::i18n::t("Fehler – nicht von AccurateRip bestätigt ({status})"), &[("status", status)]),
        _ => status,
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
            let _ = tx.send(RipMsg::Finished(e));
            return;
        }
    };
    let batch = convert::Batch::new();
    let mut log = vec![
        crate::i18n::t("PsyAudio – Protokoll der Audio-Extraktion").to_string(),
        crate::i18n::tf(crate::i18n::t("Album: {0} – {1}"), &[("0", format!("{}", meta.artist)), ("1", format!("{}", meta.album))]),
        crate::i18n::tf(crate::i18n::t("Laufwerk {letter}  Versatz {0} Samples  Modus: {1}"), &[("letter", cdrip::drive_name(letter)), ("0", format!("{:+}", rs.drive_offset)), ("1", format!("{}", if rs.mode == RipMode::Secure { crate::i18n::t("Sicher") } else { "Burst" }))]),
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
        let _ = tx.send(RipMsg::Progress(row, 0.0, crate::i18n::t("Lesen…").into()));
        let txp = tx.clone();
        let prog = move |f: f32| {
            let _ = txp.send(RipMsg::Progress(row, f * 0.8, crate::i18n::tf(crate::i18n::t("Lesen {0} %"), &[("0", format!("{:.0}", f * 100.0))])));
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
                let _ = tx.send(RipMsg::TrackDone(row, crate::i18n::tf(crate::i18n::t("Fehler: {e}"), &[("e", format!("{e}"))]), String::new()));
                log.push(crate::i18n::tf(crate::i18n::t("Titel {number}: Fehler {e}"), &[("number", format!("{number:02}")), ("e", format!("{e}"))]));
                continue;
            }
        };
        let status = if rs.mode == RipMode::Burst {
            crate::i18n::t("Gerippt (Burst)").to_string()
        } else if rip.insecure_sectors > 0 {
            insecure_tracks += 1;
            crate::i18n::tf(crate::i18n::t("Unsicher ({0} Sektoren)"), &[("0", format!("{}", rip.insecure_sectors))])
        } else if rip.verified_first_pass {
            crate::i18n::t("Sicher (1 Durchgang, von AccurateRip bestätigt)").to_string()
        } else if rip.rereads > 0 {
            crate::i18n::tf(crate::i18n::t("Sicher (Warnung: {0} Durchgänge, {1} Blöcke neu gelesen)"), &[("0", format!("{}", rip.passes)), ("1", format!("{}", rip.rereads))])
        } else {
            crate::i18n::tf(crate::i18n::t("Sicher ({0} Durchgänge identisch)"), &[("0", format!("{}", rip.passes))])
        };
        let (v1, v2) = cdrip::accuraterip_crc(&rip.raw, number == first_audio, number == last_audio);
        let idx = audio_numbers.iter().position(|n| *n == number).unwrap_or(0);
        let (ar, confirmed) = if rs.accuraterip { ar_verdict(&ar_db, idx, v1, v2) } else { (String::new(), false) };
        if confirmed {
            accurate += 1;
        }
        let status = rip_status(rs.mark_error, status, rip.insecure_sectors > 0, confirmed);
        log.push(crate::i18n::tf(crate::i18n::t("Titel {number}  {0}  CRC32 {1}  AR v1 {v1} v2 {v2}  {status}  {ar}"), &[("number", format!("{number:02}")), ("0", format!("{}", tg.get("title"))), ("1", format!("{:08X}", rip.crc)), ("v1", format!("{v1:08X}")), ("v2", format!("{v2:08X}")), ("status", format!("{status}")), ("ar", format!("{ar}"))]));
        let _ = tx.send(RipMsg::Progress(row, 0.85, crate::i18n::t("Kodieren…").into()));
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
                let _ = tx.send(RipMsg::TrackDone(row, crate::i18n::tf(crate::i18n::t("Fehler beim Kodieren: {e}"), &[("e", format!("{e}"))]), ar));
                log.push(crate::i18n::tf(crate::i18n::t("   Kodierfehler: {e}"), &[("e", format!("{e}"))]));
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
            log.push(crate::i18n::tf(crate::i18n::t("{accurate} Titel von AccurateRip bestätigt, {insecure_tracks} unsicher"), &[("accurate", format!("{accurate}")), ("insecure_tracks", format!("{insecure_tracks}"))]));
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
        crate::i18n::t("Abgebrochen").to_string()
    } else {
        crate::i18n::tf(crate::i18n::t("Fertig: {0} Dateien, {accurate} von AccurateRip bestätigt"), &[("0", format!("{}", outputs.len())), ("accurate", format!("{accurate}"))])
    };
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
        assert!(rip_status(2, "Sicher".into(), false, false).starts_with("Fehler"));
        assert_eq!(rip_status(2, "Sicher".into(), false, true), "Sicher");
        assert!(rip_status(1, "Unsicher (3 Sektoren)".into(), true, true).starts_with("Fehler"));
        assert!(stop_after_track(true, 3));
        assert!(!stop_after_track(true, 0));
    }
}
