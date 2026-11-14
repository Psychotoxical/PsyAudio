//! The CUETools database (CTDB): disc lookup, track checksums and the disc's public page.
//! Its contents are public domain (cue.tools/wiki/CTDB); no permission is needed to ask it.

use crate::cdrip::{read_range, SectorRead, Toc, SAMPLES_PER_SECTOR};
use base64::Engine;
use std::sync::atomic::AtomicBool;

/// Samples at the start (and, plus what does not fill a whole stride, at the end) of the disc
/// that CTDB leaves out of its checksums.
pub const EDGE: u64 = 5880;

/// The table of contents as CTDB asks for it: audio track starts, data tracks with a minus,
/// then the lead-out; sectors from LBA 0.
pub fn toc_string(toc: &Toc) -> String {
    let audio = toc.audio_tracks();
    let first = audio.first().map(|t| t.number);
    let mut parts: Vec<String> = Vec::new();
    for t in toc.tracks.iter().filter(|t| !t.audio && first.is_some_and(|n| t.number < n)) {
        parts.push(format!("-{}", t.lba));
    }
    parts.extend(audio.iter().map(|t| t.lba.to_string()));
    for t in toc.tracks.iter().filter(|t| !t.audio && first.is_none_or(|n| t.number > n)) {
        parts.push(format!("-{}", t.lba));
    }
    parts.push(toc.leadout.to_string());
    parts.join(":")
}

/// The disc's CTDB id: SHA-1 over 100 eight-digit hex fields (track starts 2… and the audio
/// lead-out, relative to track 1, the rest "00000000"), in base64 with `.`, `_` and `-`.
pub fn tocid(toc: &Toc) -> String {
    use sha1::Digest;
    let audio = toc.audio_tracks();
    let Some(first) = audio.first() else { return String::new() };
    let mut buf = [b'0'; 800];
    let values = audio.iter().skip(1).map(|t| t.lba).chain(std::iter::once(toc.audio_leadout()));
    for (k, v) in values.enumerate().take(100) {
        buf[k * 8..k * 8 + 8].copy_from_slice(format!("{:08X}", v.saturating_sub(first.lba)).as_bytes());
    }
    let digest = sha1::Sha1::digest(buf);
    base64::engine::general_purpose::STANDARD.encode(digest).replace('+', ".").replace('/', "_").replace('=', "-")
}

/// The disc's public page in the database, for anyone to check a result.
pub fn disc_url(toc: &Toc) -> String {
    format!("https://db.cue.tools/ui/?tocid={}", tocid(toc))
}

/// One rip of the disc in the database: how many people submitted it, and a CRC32 per audio track.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub confidence: u32,
    pub track_crcs: Vec<u32>,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

/// The entries of a lookup answer; unreadable ones are left out.
pub fn parse(xml: &str) -> Vec<Entry> {
    xml.split("<entry")
        .skip(1)
        .filter_map(|rest| {
            let tag = &rest[..rest.find('>')?];
            let confidence = attr(tag, "confidence")?.parse().ok()?;
            let track_crcs = attr(tag, "trackcrcs")?.split_whitespace().map(|h| u32::from_str_radix(h, 16).ok()).collect::<Option<Vec<_>>>()?;
            Some(Entry { confidence, track_crcs })
        })
        .collect()
}

/// Ask the database about a disc; an empty list means it does not know the disc. The server
/// has no HTTPS.
pub fn lookup(toc: &Toc) -> Result<Vec<Entry>, String> {
    let url = format!("http://db.cuetools.net/lookup2.php?version=3&ctdb=1&fuzzy=1&toc={}", toc_string(toc));
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(10))).build().into();
    let unreachable = |e: String| crate::i18n::tf(crate::i18n::t("CTDB nicht erreichbar: {e}"), &[("e", e)]);
    let mut resp = agent.get(&url).header("User-Agent", "PsyAudio/1.0").call().map_err(|e| unreachable(e.to_string()))?;
    let text = resp.body_mut().read_to_string().map_err(|e| unreachable(e.to_string()))?;
    Ok(parse(&text))
}

/// The disc's audio in samples, from the first audio track to the end of the audio session.
pub fn disc_samples(toc: &Toc) -> u64 {
    let first = toc.audio_tracks().first().map(|t| t.lba).unwrap_or(0);
    toc.audio_leadout().saturating_sub(first) as u64 * SAMPLES_PER_SECTOR as u64
}

/// The samples of a track that CTDB's checksum covers, as a range of samples in the track.
fn checked_samples(track_samples: u64, first: bool, last: bool, disc_samples: u64) -> (u64, u64) {
    let a = if first { EDGE.min(track_samples) } else { 0 };
    let cut = if last { EDGE + disc_samples % EDGE } else { 0 };
    (a, track_samples.saturating_sub(cut).max(a))
}

/// CTDB's CRC32 of one track (offset applied, silence included): the first track without the
/// first `EDGE` samples, the last without `EDGE` plus the rest of the last incomplete stride.
pub fn track_crc(raw: &[u8], first: bool, last: bool, disc_samples: u64) -> u32 {
    let (a, b) = checked_samples(raw.len() as u64 / 4, first, last, disc_samples);
    crc32(&raw[a as usize * 4..b as usize * 4])
}

// ---------------- CRC32 with a table, and rolled one sample at a time ----------------

const fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}
static TABLE: [u32; 256] = crc_table();

/// The CRC register after `data`, from `state`, without the start and end inversion (linear).
fn raw_update(mut state: u32, data: &[u8]) -> u32 {
    for &b in data {
        state = TABLE[((state ^ b as u32) & 0xFF) as usize] ^ (state >> 8);
    }
    state
}

/// CRC32 as zlib (and `crate::audio::crc32`), with a table.
fn crc32(data: &[u8]) -> u32 {
    !raw_update(!0, data)
}

/// A linear map on the 32-bit CRC register: the image of each bit.
type Mat = [u32; 32];

fn mat_apply(m: &Mat, mut v: u32) -> u32 {
    let mut out = 0;
    let mut j = 0;
    while v != 0 {
        if v & 1 != 0 {
            out ^= m[j];
        }
        v >>= 1;
        j += 1;
    }
    out
}

/// `a` after `b`.
fn mat_mul(a: &Mat, b: &Mat) -> Mat {
    std::array::from_fn(|j| mat_apply(a, b[j]))
}

/// The register after `n` zero bytes (squaring, as zlib's crc32_combine).
fn zero_bytes(mut n: u64) -> Mat {
    let mut result: Mat = std::array::from_fn(|j| 1u32 << j);
    let mut base: Mat = std::array::from_fn(|j| raw_update(1u32 << j, &[0]));
    while n > 0 {
        if n & 1 != 0 {
            result = mat_mul(&base, &result);
        }
        base = mat_mul(&base, &base);
        n >>= 1;
    }
    result
}

/// The CRC32 of a fixed-length window that moves along the audio one sample (4 bytes) at a time,
/// without reading the window again: for a fixed length, crc(M) = raw(M) ^ crc(zeros).
pub struct RollingCrc {
    raw: u32,
    constant: u32,
    /// A sample at the window's front, carried through the rest of the window.
    drop: Mat,
    /// Four zero bytes.
    add: Mat,
}

impl RollingCrc {
    /// `window` is at least one sample long and whole samples.
    pub fn new(window: &[u8]) -> RollingCrc {
        let len = window.len() as u64;
        let constant = !mat_apply(&zero_bytes(len), !0);
        RollingCrc { raw: crc32(window) ^ constant, constant, drop: zero_bytes(len.saturating_sub(4)), add: zero_bytes(4) }
    }

    /// Move one sample on: `out` leaves the front, `inp` joins at the end.
    pub fn roll(&mut self, out: [u8; 4], inp: [u8; 4]) {
        let rest = self.raw ^ mat_apply(&self.drop, raw_update(0, &out));
        self.raw = mat_apply(&self.add, rest) ^ raw_update(0, &inp);
    }

    pub fn value(&self) -> u32 {
        self.raw ^ self.constant
    }
}

// ---------------- the drive's read offset ----------------

/// Largest drive offset tried, in samples, either way (the known drives span about −1164 … +1776).
pub const OFFSET_SEARCH: i64 = 2000;

/// Track `idx` (among the audio tracks) read with offset 0, with room for the search on both
/// sides; and where the checked samples start and end in it, for offset 0.
fn read_for_search(drive: &dyn SectorRead, toc: &Toc, idx: usize, cancel: &AtomicBool, progress: &dyn Fn(f32)) -> Result<(Vec<u8>, u64, u64), String> {
    let audio = toc.audio_tracks();
    let (start, end) = toc.span(audio[idx].number).ok_or(crate::i18n::t("Titel nicht gefunden"))?;
    let margin = (OFFSET_SEARCH as u64).div_ceil(SAMPLES_PER_SECTOR as u64) as i64 + 1;
    let (raw, _) = read_range(drive, start as i64 - margin, end as i64 + margin, toc.readable_end() as i64, cancel, progress)?;
    let (a, b) = checked_samples((end - start) as u64 * SAMPLES_PER_SECTOR as u64, idx == 0, idx + 1 == audio.len(), disc_samples(toc));
    let base = margin as u64 * SAMPLES_PER_SECTOR as u64;
    Ok((raw, base + a, base + b))
}

/// Every offset (−OFFSET_SEARCH ..= +OFFSET_SEARCH) at which track `idx` matches the database,
/// with how many rips agree there. Other pressings of a disc are the same audio shifted, so one
/// track can match at several offsets.
fn search_track(
    drive: &dyn SectorRead,
    toc: &Toc,
    entries: &[Entry],
    idx: usize,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
) -> Result<Vec<(i32, u32)>, String> {
    let n = toc.audio_tracks().len();
    let (raw, a, b) = read_for_search(drive, toc, idx, cancel, progress)?;
    let at = |o: i64| ((a as i64 + o) * 4) as usize;
    let len = ((b - a) * 4) as usize;
    let mut r = RollingCrc::new(&raw[at(-OFFSET_SEARCH)..at(-OFFSET_SEARCH) + len]);
    let mut found = Vec::new();
    for o in -OFFSET_SEARCH..=OFFSET_SEARCH {
        let c = track_confidence(entries, idx, r.value(), n);
        if c > 0 {
            found.push((o as i32, c));
        }
        if o < OFFSET_SEARCH {
            let s = at(o);
            r.roll(raw[s..s + 4].try_into().unwrap_or_default(), raw[s + len..s + len + 4].try_into().unwrap_or_default());
        }
    }
    Ok(found)
}

/// How many rips agree with track `idx` at each of `offsets` (track read once).
fn confirm_track(
    drive: &dyn SectorRead,
    toc: &Toc,
    entries: &[Entry],
    idx: usize,
    offsets: &[i32],
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
) -> Result<Vec<u32>, String> {
    let n = toc.audio_tracks().len();
    let (raw, a, b) = read_for_search(drive, toc, idx, cancel, progress)?;
    let len = ((b - a) * 4) as usize;
    Ok(offsets
        .iter()
        .map(|o| {
            let s = ((a as i64 + *o as i64) * 4) as usize;
            track_confidence(entries, idx, crc32(&raw[s..s + len]), n)
        })
        .collect())
}

/// Find the drive's read offset with a disc the database knows: slide one track's checksum over
/// ±OFFSET_SEARCH samples (the shortest middle track, away from the disc's edges), then check the
/// candidates with a second track. Where other pressings also match, the offset most rips agree
/// on wins. Returns the offset and how many tracks agree (1 only on a one-track disc).
pub fn find_offset(drive: &dyn SectorRead, toc: &Toc, entries: &[Entry], cancel: &AtomicBool, progress: &dyn Fn(f32)) -> Result<(i32, usize), String> {
    use crate::i18n::t;
    let audio = toc.audio_tracks();
    let n = audio.len();
    if n == 0 || disc_confidence(entries, n) == 0 {
        return Err(t("Diese CD kennt CTDB nicht – bitte eine andere CD versuchen").into());
    }
    // the shortest middle tracks first (quickest to read); the first and last track (with their
    // edges) serve short discs
    let length = |i: usize| toc.span(audio[i].number).map(|(a, b)| b - a).unwrap_or(0);
    let mut order: Vec<usize> = (1..n.saturating_sub(1)).collect();
    order.sort_by_key(|i| length(*i));
    for i in [0, n - 1] {
        if !order.contains(&i) {
            order.push(i);
        }
    }
    let found = search_track(drive, toc, entries, order[0], cancel, &|f| progress(f * 0.5))?;
    if found.is_empty() {
        return Err(t("Kein Versatz gefunden – bitte eine andere CD versuchen").into());
    }
    let Some(&second) = order.get(1) else {
        let best = found.iter().max_by_key(|(_, c)| *c).map(|(o, _)| *o).unwrap_or_default();
        return Ok((best, 1));
    };
    let offsets: Vec<i32> = found.iter().map(|(o, _)| *o).collect();
    let second_conf = confirm_track(drive, toc, entries, second, &offsets, cancel, &|f| progress(0.5 + f * 0.5))?;
    found
        .iter()
        .zip(second_conf)
        .filter(|(_, c2)| *c2 > 0)
        .max_by_key(|((_, c1), c2)| *c1 + *c2)
        .map(|((o, _), _)| (*o, 2))
        .ok_or_else(|| t("Die Titel ergeben verschiedene Versätze (Kratzer?) – bitte eine andere CD versuchen").into())
}

/// How many rips of this disc the database knows (entries with as many tracks as the disc).
pub fn disc_confidence(entries: &[Entry], audio_tracks: usize) -> u32 {
    entries.iter().filter(|e| e.track_crcs.len() == audio_tracks).map(|e| e.confidence).sum()
}

/// How many of them have the same checksum for track `idx` (0-based among the audio tracks).
pub fn track_confidence(entries: &[Entry], idx: usize, crc: u32, audio_tracks: usize) -> u32 {
    if crc == 0 {
        return 0;
    }
    entries.iter().filter(|e| e.track_crcs.len() == audio_tracks && e.track_crcs.get(idx) == Some(&crc)).map(|e| e.confidence).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdrip::{Toc, TocTrack};

    /// A table of contents from audio track starts, an optional data track after them, and the lead-out.
    fn toc(audio: &[u32], data: Option<u32>, leadout: u32) -> Toc {
        let mut tracks: Vec<TocTrack> = audio.iter().enumerate().map(|(i, &lba)| TocTrack { number: i as u8 + 1, lba, audio: true }).collect();
        if let Some(lba) = data {
            tracks.push(TocTrack { number: tracks.len() as u8 + 1, lba, audio: false });
        }
        Toc { tracks, leadout }
    }

    // test vectors from the `cdtoc` crate (src/ctdb.rs, t_ctdb), LBA = hex value - 150
    #[test]
    fn toc_string_and_id_of_a_plain_disc() {
        let t = toc(&[0, 11413, 25024, 45713], None, 55220);
        assert_eq!(toc_string(&t), "0:11413:25024:45713:55220");
        assert_eq!(tocid(&t), "VukMWWItblELRM.CEFpXxw0FlME-");
        assert_eq!(disc_url(&t), "https://db.cue.tools/ui/?tocid=VukMWWItblELRM.CEFpXxw0FlME-");
    }

    #[test]
    fn toc_string_and_id_with_a_data_track_at_the_end() {
        let audio = [0, 15047, 30797, 45995, 60397, 78942, 94607, 108395, 125382, 146117, 162905, 177397, 194782];
        let t = toc(&audio, Some(220352), 332850);
        assert_eq!(toc_string(&t), "0:15047:30797:45995:60397:78942:94607:108395:125382:146117:162905:177397:194782:-220352:332850");
        assert_eq!(tocid(&t), "gmEsiU5wvQFA1Nq9YE_posiwgK8-");
    }

    #[test]
    fn toc_string_and_id_with_a_lead_in_before_track_one() {
        let audio = [32, 21180, 25110, 39232, 57730, 76330, 79185, 82515, 96282, 105612, 110650, 124772, 141292, 149317, 159710, 172327];
        let t = toc(&audio, None, 180325);
        assert!(toc_string(&t).starts_with("32:21180:"));
        assert!(toc_string(&t).ends_with(":172327:180325"));
        assert_eq!(tocid(&t), "iL4EZ56YD5WmG..M4v5qzPG0cFY-");
    }

    #[test]
    fn parse_reads_entries_and_skips_garbage() {
        let xml = r#"<ctdb xmlns="http://db.cuetools.net/ns/mmd-1.0#">
 <entry confidence="29" crc32="92bd37a8" id="1" trackcrcs="e04d1731 90b1451a" />
 <entry confidence="1039" id="2" trackcrcs="e04d1731 90b1451a" toc="0:13937:32822" />
 <entry confidence="viele" trackcrcs="e04d1731 90b1451a" />
</ctdb>"#;
        let e = parse(xml);
        assert_eq!(
            e,
            vec![Entry { confidence: 29, track_crcs: vec![0xe04d1731, 0x90b1451a] }, Entry { confidence: 1039, track_crcs: vec![0xe04d1731, 0x90b1451a] },]
        );
        assert!(parse("kein xml").is_empty());
    }

    #[test]
    fn edges_follow_the_5880_stride() {
        // three tracks of 30, 25 and 40 sectors: 55860 samples, 2940 of them past the last full stride
        let t = toc(&[0, 30, 55], None, 95);
        let total = disc_samples(&t);
        assert_eq!(total, 55860);
        let raw: Vec<u8> = (0..total as usize * 4).map(|i| (i * 7 + i / 13) as u8).collect();
        let (a, b, c) = (&raw[..30 * 2352], &raw[30 * 2352..55 * 2352], &raw[55 * 2352..]);
        assert_eq!(track_crc(a, true, false, total), crate::audio::crc32(&a[5880 * 4..], 0));
        assert_eq!(track_crc(b, false, false, total), crate::audio::crc32(b, 0));
        assert_eq!(track_crc(c, false, true, total), crate::audio::crc32(&c[..c.len() - (5880 + 2940) * 4], 0));
    }

    #[test]
    fn disc_samples_end_before_a_data_session() {
        // the audio ends 11400 sectors before the data track
        let t = toc(&[0, 1000], Some(20000), 25000);
        assert_eq!(disc_samples(&t), (20000 - 11400) * 588);
    }

    #[test]
    fn confidences_sum_matching_entries() {
        let (a, b, x) = (0xa, 0xb, 0x5);
        let entries = vec![
            Entry { confidence: 29, track_crcs: vec![a, b] },
            Entry { confidence: 1039, track_crcs: vec![a, b] },
            Entry { confidence: 1, track_crcs: vec![a, x] },
            Entry { confidence: 5, track_crcs: vec![a] },
        ];
        assert_eq!(disc_confidence(&entries, 2), 1069);
        assert_eq!(track_confidence(&entries, 1, b, 2), 1068);
        assert_eq!(track_confidence(&entries, 0, a, 2), 1069);
        assert_eq!(track_confidence(&entries, 1, 0, 2), 0, "a checksum 0 never counts");
    }

    /// Bytes from a fixed seed (no rand crate).
    fn noise(n: usize, seed: u32) -> Vec<u8> {
        let mut x = seed;
        (0..n)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn rolling_equals_direct() {
        let data = noise(40_000, 7);
        let len = 4_000;
        let mut r = RollingCrc::new(&data[..len]);
        for s in 0..1_000 {
            let at = s * 4;
            assert_eq!(r.value(), crate::audio::crc32(&data[at..at + len], 0), "step {s}");
            let out: [u8; 4] = data[at..at + 4].try_into().unwrap();
            let inp: [u8; 4] = data[at + len..at + len + 4].try_into().unwrap();
            r.roll(out, inp);
        }
    }

    /// A disc in memory whose drive delivers every sample `offset` samples late (as a drive with
    /// that read offset does); `truth` is the audio as it really is.
    struct LateDrive {
        truth: Vec<u8>,
        offset: i64,
    }
    impl crate::cdrip::SectorRead for LateDrive {
        fn read(&self, lba: u32, count: u32) -> Result<Vec<u8>, String> {
            let first = lba as i64 * 588 - self.offset;
            Ok((0..count as i64 * 588)
                .flat_map(|k| {
                    let i = first + k;
                    if i < 0 || (i as usize + 1) * 4 > self.truth.len() {
                        [0u8; 4]
                    } else {
                        self.truth[i as usize * 4..i as usize * 4 + 4].try_into().unwrap()
                    }
                })
                .collect())
        }
    }

    /// Four tracks of noise and the entries CTDB would have for them.
    fn noisy_disc() -> (Toc, Vec<u8>, Vec<Entry>) {
        let t = toc(&[0, 40, 75, 105], None, 150);
        let truth = noise(150 * 2352, 3);
        let total = disc_samples(&t);
        let audio = t.audio_tracks();
        let crcs = audio
            .iter()
            .enumerate()
            .map(|(i, tr)| {
                let (a, b) = t.span(tr.number).unwrap();
                track_crc(&truth[a as usize * 2352..b as usize * 2352], i == 0, i == audio.len() - 1, total)
            })
            .collect();
        (t, truth, vec![Entry { confidence: 12, track_crcs: crcs }])
    }

    #[test]
    fn finds_the_shift_on_a_disc_in_memory() {
        let (t, truth, entries) = noisy_disc();
        let none = std::sync::atomic::AtomicBool::new(false);
        for offset in [667, -1164, 0, 1776] {
            let d = LateDrive { truth: truth.clone(), offset };
            assert_eq!(find_offset(&d, &t, &entries, &none, &|_| {}), Ok((offset as i32, 2)), "drive offset {offset}");
        }
    }

    #[test]
    fn a_shifted_pressing_does_not_win() {
        // the database also knows a pressing whose audio sits 300 samples later; few people ripped it
        let (t, truth, entries) = noisy_disc();
        let mut later = vec![0u8; 300 * 4];
        later.extend_from_slice(&truth[..truth.len() - 300 * 4]);
        let total = disc_samples(&t);
        let audio = t.audio_tracks();
        let other: Vec<u32> = audio
            .iter()
            .enumerate()
            .map(|(i, tr)| {
                let (a, b) = t.span(tr.number).unwrap();
                track_crc(&later[a as usize * 2352..b as usize * 2352], i == 0, i == audio.len() - 1, total)
            })
            .collect();
        let both = vec![Entry { confidence: 1, track_crcs: other }, entries[0].clone()];
        let d = LateDrive { truth, offset: 667 };
        let none = std::sync::atomic::AtomicBool::new(false);
        assert_eq!(find_offset(&d, &t, &both, &none, &|_| {}), Ok((667, 2)), "the pressing most rips agree on wins");
    }

    #[test]
    fn unknown_disc_is_a_clear_error() {
        let (t, truth, entries) = noisy_disc();
        let none = std::sync::atomic::AtomicBool::new(false);
        let d = LateDrive { truth, offset: 667 };
        let unknown = find_offset(&d, &t, &[], &none, &|_| {}).unwrap_err();
        assert!(unknown.contains("CTDB"), "{unknown}");
        let wrong = vec![Entry { confidence: 3, track_crcs: entries[0].track_crcs.iter().map(|c| c ^ 1).collect() }];
        let no_match = find_offset(&d, &t, &wrong, &none, &|_| {}).unwrap_err();
        assert_ne!(no_match, unknown);
    }
}
