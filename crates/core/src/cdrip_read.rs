//! Secure reading of one CD track: pass 1, CUETools database check, pass 2, optional extra passes,
//! then re-reads of every block that differed.

use crate::cdrip::{read_block, read_block_lenient, read_range, SectorRead, Toc, TrackRip, READ_SECTORS, SAMPLES_PER_SECTOR, SECTOR};
use std::sync::atomic::{AtomicBool, Ordering};

/// How a track is read.
#[derive(Clone, Copy)]
pub struct ReadParams {
    pub secure: bool,
    pub reread_limit: u32,
    pub extra_passes: bool,
    pub extra_min: u32,
    pub extra_max: u32,
    pub extra_clean: u32,
    pub cache_kb: u32,
    pub abort_unrecoverable: u32,
    pub interpolate: bool,
}

/// Read the drive's cache size worth of other sectors (in order, close by), so the next read
/// of the block comes from the disc again instead of the drive's memory.
/// Read errors here do not matter (the sectors are only read to empty the cache); Cancel does.
fn flush_cache(drive: &dyn SectorRead, lba: i64, block_end: i64, max_lba: i64, cache_kb: u32, cancel: &AtomicBool) -> Result<(), String> {
    let sectors = (cache_kb as i64 * 1024 + SECTOR as i64 - 1) / SECTOR as i64 + 1;
    let start = if lba - sectors > 0 { lba - sectors - 1 } else { (block_end + 1).min((max_lba - sectors).max(0)) };
    let mut l = start;
    while l < start + sectors {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::i18n::t("abgebrochen").into());
        }
        let n = READ_SECTORS.min(start + sectors - l);
        let _ = read_block(drive, l, n, max_lba);
        l += n;
    }
    Ok(())
}

type Versions = Vec<Vec<(Vec<u8>, u32)>>;

/// Add one full pass; returns how many blocks newly disagree.
fn add_pass(pass: &[u8], block: usize, versions: &mut Versions, bad: &mut [bool]) -> usize {
    let mut new_bad = 0;
    for (b, vs) in versions.iter_mut().enumerate() {
        let r = b * block..((b + 1) * block).min(pass.len());
        let v = &pass[r];
        match vs.iter_mut().find(|e| e.0 == v) {
            Some(e) => e.1 += 1,
            None => {
                vs.push((v.to_vec(), 1));
                if !bad[b] {
                    bad[b] = true;
                    new_bad += 1;
                }
            }
        }
    }
    new_bad
}

/// Read one track with the drive offset applied. Burst: one pass. Secure: pass 1; if
/// the CUETools database confirms it, done; otherwise pass 2 (and with extra passes more until
/// enough clean passes); every block that differed is re-read after flushing the drive cache,
/// up to `reread_limit` times, until a read matches an earlier one. Each pass streams the
/// track in order, so the drive does not jump back and forth.
#[allow(clippy::too_many_arguments)]
pub fn rip_track(
    drive: &dyn SectorRead,
    toc: &Toc,
    number: u8,
    offset: i32,
    rp: ReadParams,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
    accurate: &dyn Fn(&[u8]) -> bool,
) -> Result<TrackRip, String> {
    let (start, end) = toc.span(number).ok_or(crate::i18n::t("Titel nicht gefunden"))?;
    let sps = SAMPLES_PER_SECTOR as i64;
    let first_sample = start as i64 * sps + offset as i64;
    let total_samples = (end - start) as i64 * sps;
    let s_lo = first_sample.div_euclid(sps);
    let s_hi = (first_sample + total_samples + sps - 1).div_euclid(sps);
    let max_lba = toc.readable_end() as i64;
    let skip = (first_sample - s_lo * sps) as usize * 4;
    let len = total_samples as usize * 4;
    let cut = |raw: &[u8]| raw[skip..(skip + len).min(raw.len())].to_vec();
    let finish = |raw: Vec<u8>, rereads, insecure, passes, verified| {
        let raw = cut(&raw);
        let crc = crate::audio::crc32(&raw, 0);
        TrackRip { raw, rereads, insecure_sectors: insecure, crc, passes, verified_first_pass: verified }
    };

    let p1_share = if rp.secure { 0.4 } else { 1.0 };
    let (mut best, unreadable1) = read_range(drive, s_lo, s_hi, max_lba, cancel, &|f| progress(f * p1_share))?;
    if !rp.secure {
        return Ok(finish(best, 0, unreadable1.len() as u32, 1, false));
    }
    if unreadable1.is_empty() && accurate(&cut(&best)) {
        progress(1.0);
        return Ok(finish(best, 0, 0, 1, true));
    }
    let block = READ_SECTORS as usize * SECTOR;
    let blocks = best.len().div_ceil(block);
    let range = |b: usize, total: usize| b * block..((b + 1) * block).min(total);
    let mut versions: Versions = (0..blocks).map(|b| vec![(best[range(b, best.len())].to_vec(), 1)]).collect();
    let mut bad = vec![false; blocks];
    // a block with a sector the drive refused is never taken from a pass, only from a clean re-read
    let mark = |bad: &mut Vec<bool>, lbas: &[i64]| {
        for l in lbas {
            bad[((l - s_lo) / READ_SECTORS) as usize] = true;
        }
    };
    mark(&mut bad, &unreadable1);
    // every pass after the first starts with an emptied drive cache, so it really reads the disc
    flush_cache(drive, s_lo, s_hi, max_lba, rp.cache_kb, cancel)?;
    let (pass2, unreadable2) = read_range(drive, s_lo, s_hi, max_lba, cancel, &|f| progress(0.4 + f * 0.4))?;
    add_pass(&pass2, block, &mut versions, &mut bad);
    mark(&mut bad, &unreadable2);
    let mut passes = 2;
    if rp.extra_passes {
        let min = rp.extra_min.max(1);
        let max = rp.extra_max.max(min);
        let mut clean = 0;
        for u in 1..=max {
            flush_cache(drive, s_lo, s_hi, max_lba, rp.cache_kb, cancel)?;
            let (pass, unreadable) = read_range(drive, s_lo, s_hi, max_lba, cancel, &|f| progress(0.8 + 0.1 * (u as f32 - 1.0 + f) / max as f32))?;
            passes += 1;
            mark(&mut bad, &unreadable);
            if add_pass(&pass, block, &mut versions, &mut bad) == 0 && unreadable.is_empty() {
                clean += 1;
            } else {
                clean = 0;
            }
            if u >= min && clean >= rp.extra_clean {
                break;
            }
        }
    }
    let mut rereads = 0;
    let mut insecure = 0u32;
    let mut insecure_blocks = Vec::new();
    let bad_list: Vec<usize> = (0..blocks).filter(|b| bad[*b]).collect();
    for (k, &b) in bad_list.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::i18n::t("abgebrochen").into());
        }
        let r = range(b, best.len());
        let lba = s_lo + (b as i64) * READ_SECTORS;
        let n = (r.len() / SECTOR) as i64;
        let mut accepted = None;
        for _ in 0..rp.reread_limit.max(1) {
            if cancel.load(Ordering::Relaxed) {
                return Err(crate::i18n::t("abgebrochen").into());
            }
            flush_cache(drive, lba, lba + n, max_lba, rp.cache_kb, cancel)?;
            let (again, refused) = read_block_lenient(drive, lba, n, max_lba, cancel)?;
            rereads += 1;
            if !refused.is_empty() {
                continue; // a read with refused sectors proves nothing
            }
            match versions[b].iter_mut().find(|e| e.0 == again) {
                Some(e) => {
                    e.1 += 1;
                    accepted = Some(e.0.clone());
                    break;
                }
                None => versions[b].push((again, 1)),
            }
        }
        let data = match accepted {
            Some(d) => d,
            None => {
                insecure += n as u32;
                insecure_blocks.push(b);
                versions[b].sort_by_key(|e| std::cmp::Reverse(e.1));
                versions[b][0].0.clone()
            }
        };
        best[r].copy_from_slice(&data);
        progress(0.9 + 0.1 * (k + 1) as f32 / bad_list.len() as f32);
        if rp.abort_unrecoverable > 0 && insecure > rp.abort_unrecoverable {
            return Err(crate::i18n::tf(crate::i18n::t("abgebrochen nach {insecure} nicht lesbaren Sektoren"), &[("insecure", format!("{insecure}"))]));
        }
    }
    if rp.interpolate {
        for &b in &insecure_blocks {
            let r = range(b, best.len());
            interpolate_block(&mut best, r);
        }
    }
    progress(1.0);
    Ok(finish(best, rereads, insecure, passes, false))
}

/// Replace unreadable audio with a straight line between the samples around it.
fn interpolate_block(raw: &mut [u8], r: std::ops::Range<usize>) {
    let sample = |raw: &[u8], i: usize, c: usize| i16::from_le_bytes([raw[i * 4 + c * 2], raw[i * 4 + c * 2 + 1]]) as f32;
    let (a, z) = (r.start / 4, r.end / 4);
    let total = raw.len() / 4;
    for c in 0..2 {
        let before = if a > 0 { sample(raw, a - 1, c) } else { 0.0 };
        let after = if z < total { sample(raw, z, c) } else { 0.0 };
        let span = (z - a + 1) as f32;
        for i in a..z {
            let t = (i - a + 1) as f32 / span;
            let v = (before + (after - before) * t).round() as i16;
            raw[i * 4 + c * 2..i * 4 + c * 2 + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod reader_tests {
    use super::*;
    use crate::cdrip::{SectorRead, TocTrack};
    use std::cell::RefCell;
    use std::collections::HashSet;

    /// A disc in memory: sector `l` holds the byte `l as u8`; `bad` sectors never read.
    struct FakeDrive {
        sectors: u32,
        bad: HashSet<u32>,
        log: RefCell<Vec<(u32, u32)>>,
        cancel_after: Option<(usize, std::sync::Arc<AtomicBool>)>,
    }
    impl SectorRead for FakeDrive {
        fn read(&self, lba: u32, count: u32) -> Result<Vec<u8>, String> {
            self.log.borrow_mut().push((lba, count));
            if let Some((n, c)) = &self.cancel_after {
                if self.log.borrow().len() >= *n {
                    c.store(true, Ordering::Relaxed);
                }
            }
            if (lba..lba + count).any(|l| self.bad.contains(&l) || l >= self.sectors) {
                return Err("MEDIUM ERROR".into());
            }
            Ok((lba..lba + count).flat_map(|l| vec![l as u8; SECTOR]).collect())
        }
    }
    fn drive(bad: &[u32]) -> FakeDrive {
        FakeDrive { sectors: 400, bad: bad.iter().copied().collect(), log: RefCell::new(Vec::new()), cancel_after: None }
    }
    fn toc() -> Toc {
        Toc { tracks: vec![TocTrack { number: 1, lba: 0, audio: true }, TocTrack { number: 2, lba: 100, audio: true }], leadout: 400 }
    }
    fn params(secure: bool) -> ReadParams {
        ReadParams {
            secure,
            reread_limit: 3,
            extra_passes: false,
            extra_min: 3,
            extra_max: 6,
            extra_clean: 2,
            cache_kb: 64,
            abort_unrecoverable: 0,
            interpolate: false,
        }
    }
    fn rip(d: &FakeDrive, secure: bool, cancel: &AtomicBool) -> Result<TrackRip, String> {
        rip_track(d, &toc(), 1, 0, params(secure), cancel, &|_| {}, &|_| false)
    }

    #[test]
    fn an_unreadable_sector_is_reported_not_fatal() {
        let r = rip(&drive(&[50]), true, &AtomicBool::new(false)).expect("track must still be ripped");
        assert!(r.insecure_sectors >= 1);
        assert_eq!(r.raw.len(), 100 * SECTOR);
    }

    #[test]
    fn burst_mode_counts_unreadable_sectors() {
        let r = rip(&drive(&[50]), false, &AtomicBool::new(false)).unwrap();
        assert!(r.insecure_sectors >= 1);
    }

    #[test]
    fn the_second_pass_starts_after_a_cache_flush() {
        let d = drive(&[]);
        let _ = rip(&d, true, &AtomicBool::new(false)).unwrap();
        let log = d.log.borrow();
        // pass 1 ends at sector 100; before pass 2 starts again at 0, other sectors are read
        let end_of_pass1 = log.iter().position(|(l, n)| l + n >= 100).unwrap();
        let restart = log.iter().skip(end_of_pass1 + 1).position(|(l, _)| *l == 0).unwrap() + end_of_pass1 + 1;
        assert!(log[end_of_pass1 + 1..restart].iter().any(|(l, _)| *l >= 100), "no flush before pass 2: {:?}", &log[..restart + 1]);
    }

    #[test]
    fn cancel_stops_rereads_quickly() {
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let mut d = drive(&[10, 30, 50, 70, 90]);
        d.cancel_after = Some((40, cancel.clone()));
        assert!(rip(&d, true, &cancel).is_err());
        assert!(d.log.borrow().len() < 80, "{} reads after cancel", d.log.borrow().len());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn interpolation_is_a_line() {
        let mut raw = vec![0u8; 4 * 6];
        raw[0..2].copy_from_slice(&0i16.to_le_bytes());
        raw[20..22].copy_from_slice(&500i16.to_le_bytes());
        super::interpolate_block(&mut raw, 4..20);
        let s = |i: usize| i16::from_le_bytes([raw[i * 4], raw[i * 4 + 1]]);
        assert_eq!((s(1), s(2), s(3), s(4)), (100, 200, 300, 400));
    }
}
