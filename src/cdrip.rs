//! CD ripper: reading audio CDs (Windows raw read), disc IDs, AccurateRip checking and MusicBrainz metadata.

use crate::audio::Audio;
use crate::tags::Tags;
use std::sync::atomic::{AtomicBool, Ordering};

pub const SECTOR: usize = 2352;
pub const SAMPLES_PER_SECTOR: usize = 588;

#[derive(Clone, Debug)]
pub struct TocTrack {
    pub number: u8,
    pub lba: u32,
    pub audio: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Toc {
    pub tracks: Vec<TocTrack>,
    pub leadout: u32,
}

impl Toc {
    pub fn audio_tracks(&self) -> Vec<&TocTrack> {
        self.tracks.iter().filter(|t| t.audio).collect()
    }
    /// Lead-out used for audio: before a data session (enhanced CD) 11400 sectors earlier.
    pub fn audio_leadout(&self) -> u32 {
        let audio = self.audio_tracks();
        if let Some(last) = audio.last() {
            if let Some(next) = self.tracks.iter().find(|t| t.number > last.number && !t.audio) {
                return next.lba.saturating_sub(11400);
            }
        }
        self.leadout
    }
    /// First sector ripping must not read: the end of the audio session. On an enhanced CD the
    /// sectors between the sessions cannot be read as audio; the drive offset reaches there as zeros.
    pub fn readable_end(&self) -> u32 {
        self.audio_leadout()
    }
    /// Start and end sector of an audio track (end exclusive, never before the start).
    pub fn span(&self, number: u8) -> Option<(u32, u32)> {
        let i = self.tracks.iter().position(|t| t.number == number)?;
        let start = self.tracks[i].lba;
        let end = match self.tracks.get(i + 1) {
            Some(n) if n.audio => n.lba,
            Some(_) => self.audio_leadout(),
            None => self.leadout,
        };
        Some((start, end.max(start)))
    }
}

#[cfg(test)]
mod toc_tests {
    use super::*;
    fn enhanced() -> Toc {
        Toc {
            tracks: vec![
                TocTrack { number: 1, lba: 0, audio: true },
                TocTrack { number: 2, lba: 20000, audio: true },
                TocTrack { number: 3, lba: 60000, audio: false },
            ],
            leadout: 90000,
        }
    }
    #[test]
    fn ripping_stops_at_the_end_of_the_audio_session() {
        let t = enhanced();
        assert_eq!(t.readable_end(), 60000 - 11400);
        assert_eq!(t.span(2), Some((20000, 60000 - 11400)));
    }
    #[test]
    fn a_track_shorter_than_the_session_gap_has_no_negative_length() {
        let mut t = enhanced();
        t.tracks[2].lba = 25000;
        let (start, end) = t.span(2).unwrap();
        assert!(end >= start);
    }
}

pub fn freedb_id(toc: &Toc) -> u32 {
    let digit_sum = |mut n: u32| {
        let mut s = 0;
        while n > 0 {
            s += n % 10;
            n /= 10;
        }
        s
    };
    let n: u32 = toc.tracks.iter().map(|t| digit_sum((t.lba + 150) / 75)).sum();
    let first = toc.tracks.first().map(|t| (t.lba + 150) / 75).unwrap_or(0);
    let t = (toc.leadout + 150) / 75 - first;
    ((n % 255) << 24) | (t << 8) | toc.tracks.len() as u32
}

pub fn musicbrainz_id(toc: &Toc) -> String {
    use base64::Engine;
    use sha1::{Digest, Sha1};
    let audio = toc.audio_tracks();
    let first = audio.first().map(|t| t.number).unwrap_or(1);
    let last = audio.last().map(|t| t.number).unwrap_or(1);
    let mut s = format!("{first:02X}{last:02X}{:08X}", toc.audio_leadout() + 150);
    for i in 1..100u8 {
        let off = audio.iter().find(|t| t.number == i).map(|t| t.lba + 150).unwrap_or(0);
        s += &format!("{off:08X}");
    }
    let hash = Sha1::digest(s.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(hash).replace('+', ".").replace('/', "_").replace('=', "-")
}

/// AccurateRip disc IDs: the audio tracks' start
/// sectors plus the disc's lead-out from the TOC (not shortened before a data session);
/// id2 weights each start by its position (a start of 0 counts as 1).
pub fn accuraterip_ids(toc: &Toc) -> (u32, u32, u32) {
    let audio = toc.audio_tracks();
    let lo = toc.leadout;
    let mut id1: u32 = 0;
    let mut id2: u32 = 0;
    for (i, t) in audio.iter().enumerate() {
        id1 = id1.wrapping_add(t.lba);
        id2 = id2.wrapping_add(t.lba.max(1).wrapping_mul(i as u32 + 1));
    }
    id1 = id1.wrapping_add(lo);
    id2 = id2.wrapping_add(lo.wrapping_mul(audio.len() as u32 + 1));
    (id1, id2, freedb_id(toc))
}

pub fn accuraterip_url(toc: &Toc) -> String {
    let (id1, id2, cddb) = accuraterip_ids(toc);
    format!(
        "http://www.accuraterip.com/accuraterip/{:x}/{:x}/{:x}/dBAR-{:03}-{:08x}-{:08x}-{:08x}.bin",
        id1 & 0xF,
        (id1 >> 4) & 0xF,
        (id1 >> 8) & 0xF,
        toc.audio_tracks().len(),
        id1,
        id2,
        cddb
    )
}

/// AccurateRip checksums (v1, v2) of one track's 16-bit stereo samples.
pub fn accuraterip_crc(raw: &[u8], first: bool, last: bool) -> (u32, u32) {
    let n = raw.len() / 4;
    let skip = 5 * SAMPLES_PER_SECTOR;
    let (mut v1, mut v2) = (0u32, 0u32);
    for i in 0..n {
        if first && i < skip - 1 {
            continue;
        }
        if last && i >= n.saturating_sub(skip) {
            break;
        }
        let s = u32::from_le_bytes([raw[i * 4], raw[i * 4 + 1], raw[i * 4 + 2], raw[i * 4 + 3]]);
        let mult = (i + 1) as u32;
        v1 = v1.wrapping_add(s.wrapping_mul(mult));
        let p = s as u64 * mult as u64;
        v2 = v2.wrapping_add((p & 0xFFFF_FFFF) as u32).wrapping_add((p >> 32) as u32);
    }
    (v1, v2)
}

/// One pressing in the AccurateRip database: (confidence, crc) per track.
pub type Pressing = Vec<(u8, u32)>;

pub fn accuraterip_lookup(toc: &Toc) -> Result<Vec<Pressing>, String> {
    Ok(accuraterip_lookup_full(toc)?.into_iter().map(|p| p.into_iter().map(|(c, crc, _)| (c, crc)).collect()).collect())
}

/// Every pressing with (confidence, track CRC, offset-finding CRC of the 588 samples at sector 450).
pub fn accuraterip_lookup_full(toc: &Toc) -> Result<Vec<Vec<(u8, u32, u32)>>, String> {
    let url = accuraterip_url(toc);
    let mut resp = ureq::get(&url).header("User-Agent", "PsyAudio/1.0").call().map_err(|e| match e {
        ureq::Error::StatusCode(404) => crate::i18n::t("CD ist nicht in der AccurateRip-Datenbank").to_string(),
        e => crate::i18n::tf(crate::i18n::t("AccurateRip nicht erreichbar: {e}"), &[("e", format!("{e}"))]),
    })?;
    let data = resp.body_mut().read_to_vec().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut p = 0;
    while p + 13 <= data.len() {
        let count = data[p] as usize;
        p += 13;
        let mut pressing = Vec::new();
        for _ in 0..count {
            if p + 9 > data.len() {
                break;
            }
            let conf = data[p];
            let crc = u32::from_le_bytes([data[p + 1], data[p + 2], data[p + 3], data[p + 4]]);
            let crc450 = u32::from_le_bytes([data[p + 5], data[p + 6], data[p + 7], data[p + 8]]);
            pressing.push((conf, crc, crc450));
            p += 9;
        }
        out.push(pressing);
    }
    Ok(out)
}

#[derive(Clone, Debug, Default)]
pub struct DiscMeta {
    pub release_id: String,
    pub artist: String,
    pub album: String,
    pub year: String,
    pub genre: String,
    pub disc: u32,
    pub disc_total: u32,
    pub label: String,
    pub tracks: Vec<(String, String, String)>, // title, artist, isrc
}

fn credit(v: &serde_json::Value) -> String {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|c| format!("{}{}", c["name"].as_str().unwrap_or(""), c["joinphrase"].as_str().unwrap_or("")))
                .collect::<String>()
        })
        .unwrap_or_default()
}

pub fn musicbrainz_lookup(toc: &Toc) -> Result<Vec<DiscMeta>, String> {
    let id = musicbrainz_id(toc);
    let get = |url: &str| -> Result<serde_json::Value, String> {
        let mut resp = ureq::get(url)
            .header("User-Agent", "PsyAudio/1.0 (CD-Ripper)")
            .header("Accept", "application/json")
            .call()
            .map_err(|e| format!("MusicBrainz: {e}"))?;
        let text = resp.body_mut().read_to_string().map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    };
    let base = "https://musicbrainz.org/ws/2/discid";
    let json = match get(&format!("{base}/{id}?inc=recordings+artist-credits+isrcs+labels&fmt=json")) {
        Ok(j) => j,
        Err(_) => {
            let audio = toc.audio_tracks();
            let mut tocs = format!("1+{}+{}", audio.len(), toc.audio_leadout() + 150);
            for t in &audio {
                tocs += &format!("+{}", t.lba + 150);
            }
            get(&format!("{base}/-?toc={tocs}&inc=recordings+artist-credits+isrcs+labels&fmt=json"))?
        }
    };
    let n_audio = toc.audio_tracks().len();
    let mut out = Vec::new();
    for rel in json["releases"].as_array().cloned().unwrap_or_default() {
        let media = rel["media"].as_array().cloned().unwrap_or_default();
        let disc_total = media.len() as u32;
        let medium = media
            .iter()
            .find(|m| m["discs"].as_array().map(|d| d.iter().any(|x| x["id"] == id.as_str())).unwrap_or(false))
            .or_else(|| media.iter().find(|m| m["track-count"].as_u64() == Some(n_audio as u64)))
            .cloned();
        let Some(m) = medium else { continue };
        let artist = credit(&rel["artist-credit"]);
        let tracks = m["tracks"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|t| {
                let a = credit(&t["artist-credit"]);
                let isrc = t["recording"]["isrcs"].as_array().and_then(|v| v.first()).and_then(|v| v.as_str()).unwrap_or("").to_string();
                (t["title"].as_str().unwrap_or("").to_string(), if a.is_empty() { artist.clone() } else { a }, isrc)
            })
            .collect();
        out.push(DiscMeta {
            release_id: rel["id"].as_str().unwrap_or("").to_string(),
            artist,
            album: rel["title"].as_str().unwrap_or("").to_string(),
            year: rel["date"].as_str().unwrap_or("").chars().take(4).collect(),
            genre: String::new(),
            disc: m["position"].as_u64().unwrap_or(1) as u32,
            disc_total,
            label: rel["label-info"][0]["label"]["name"].as_str().unwrap_or("").to_string(),
            tracks,
        });
    }
    if out.is_empty() {
        Err(crate::i18n::t("Keine Treffer bei MusicBrainz").into())
    } else {
        Ok(out)
    }
}

pub fn cover_art(release_id: &str) -> Option<Vec<u8>> {
    if release_id.is_empty() {
        return None;
    }
    let mut resp = ureq::get(&format!("https://coverartarchive.org/release/{release_id}/front-500"))
        .header("User-Agent", "PsyAudio/1.0")
        .call()
        .ok()?;
    resp.body_mut().read_to_vec().ok()
}

/// Turn raw CD audio (16-bit stereo little endian) into an audio buffer.
pub fn raw_to_audio(raw: &[u8]) -> Audio {
    let data = raw.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
    Audio { rate: 44100, channels: 2, bits: 16, float: false, lossless: true, data }
}

pub fn tags_for_track(meta: &DiscMeta, number: u8, total: usize, title: &str, artist: &str) -> Tags {
    let mut t = Tags::default();
    t.set("title", title);
    t.set("artist", artist);
    t.set("album", meta.album.clone());
    if !meta.artist.is_empty() && meta.artist != artist {
        t.set("album artist", meta.artist.clone());
    }
    if meta.artist.eq_ignore_ascii_case("various artists") {
        t.set("compilation", "1");
    }
    t.set("track", number.to_string());
    t.set("track_total", total.to_string());
    if meta.disc_total > 1 {
        t.set("disc", meta.disc.to_string());
        t.set("disc_total", meta.disc_total.to_string());
    }
    t.set("year", meta.year.clone());
    t.set("genre", meta.genre.clone());
    t.set("label", meta.label.clone());
    t
}

// ---------------- drive access ----------------

#[cfg(windows)]
mod win {
    use super::*;
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, GetDriveTypeW, GetLogicalDrives, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const IOCTL_CDROM_READ_TOC: u32 = 0x0002_4000;
    const IOCTL_CDROM_RAW_READ: u32 = 0x0002_403E;
    const IOCTL_STORAGE_EJECT_MEDIA: u32 = 0x002D_4808;
    const DRIVE_CDROM: u32 = 5;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn drives() -> Vec<char> {
        let mask = unsafe { GetLogicalDrives() };
        (0..26)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| (b'A' + i as u8) as char)
            .filter(|c| unsafe { GetDriveTypeW(wide(&format!("{c}:\\")).as_ptr()) } == DRIVE_CDROM)
            .collect()
    }

    pub struct Drive {
        h: HANDLE,
    }

    unsafe impl Send for Drive {}

    impl Drop for Drive {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.h);
            }
        }
    }

    impl Drive {
        pub fn open(letter: char) -> Result<Drive, String> {
            let name = wide(&format!("\\\\.\\{letter}:"));
            let h = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };
            if h == INVALID_HANDLE_VALUE || h.is_null() {
                return Err(crate::i18n::tf(crate::i18n::t("Laufwerk {letter}: kann nicht geöffnet werden"), &[("letter", format!("{letter}"))]));
            }
            Ok(Drive { h })
        }

        fn ioctl(&self, code: u32, input: &[u8], output: &mut [u8]) -> Result<u32, String> {
            let mut ret = 0u32;
            let ok = unsafe {
                DeviceIoControl(
                    self.h,
                    code,
                    if input.is_empty() { std::ptr::null() } else { input.as_ptr() as *const _ },
                    input.len() as u32,
                    if output.is_empty() { std::ptr::null_mut() } else { output.as_mut_ptr() as *mut _ },
                    output.len() as u32,
                    &mut ret,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                Err(crate::i18n::tf(crate::i18n::t("Laufwerksbefehl fehlgeschlagen (Fehler {0})"), &[("0", format!("{}", std::io::Error::last_os_error()))]))
            } else {
                Ok(ret)
            }
        }

        pub fn toc(&self) -> Result<Toc, String> {
            let mut buf = vec![0u8; 804];
            self.ioctl(IOCTL_CDROM_READ_TOC, &[], &mut buf).map_err(|_| crate::i18n::t("Keine Audio-CD im Laufwerk").to_string())?;
            let first = buf[2];
            let last = buf[3];
            let mut toc = Toc::default();
            for i in 0..=(last.saturating_sub(first) as usize + 1) {
                let d = &buf[4 + i * 8..4 + i * 8 + 8];
                // TRACK_DATA: Control is the low nibble of byte 1; bit 2 marks a data track
                let is_data = (d[1] & 0x04) != 0;
                let lba = (d[5] as u32 * 60 + d[6] as u32) * 75 + d[7] as u32;
                let lba = lba.saturating_sub(150);
                if d[2] == 0xAA {
                    toc.leadout = lba;
                } else {
                    toc.tracks.push(TocTrack { number: d[2], lba, audio: !is_data });
                }
            }
            if toc.tracks.is_empty() {
                return Err(crate::i18n::t("Keine Titel auf der CD").into());
            }
            Ok(toc)
        }

        /// Read `count` raw audio sectors starting at `lba`.
        pub fn read(&self, lba: u32, count: u32) -> Result<Vec<u8>, String> {
            let mut input = Vec::with_capacity(16);
            input.extend_from_slice(&(lba as i64 * 2048).to_le_bytes());
            input.extend_from_slice(&count.to_le_bytes());
            input.extend_from_slice(&2u32.to_le_bytes()); // CDDA
            let mut out = vec![0u8; count as usize * SECTOR];
            let n = self.ioctl(IOCTL_CDROM_RAW_READ, &input, &mut out)?;
            out.truncate(n as usize);
            Ok(out)
        }

        pub fn eject(&self) -> Result<(), String> {
            self.ioctl(IOCTL_STORAGE_EJECT_MEDIA, &[], &mut []).map(|_| ())
        }
    }
}

#[cfg(windows)]
pub use win::{drives, Drive};

#[cfg(not(windows))]
pub fn drives() -> Vec<char> {
    Vec::new()
}

#[cfg(not(windows))]
pub struct Drive;
#[cfg(not(windows))]
impl Drive {
    pub fn open(_l: char) -> Result<Drive, String> {
        Err(crate::i18n::t("CD-Lesen gibt es nur unter Windows").into())
    }
    pub fn toc(&self) -> Result<Toc, String> {
        Err(crate::i18n::t("nicht unterstützt").into())
    }
    pub fn read(&self, _lba: u32, _c: u32) -> Result<Vec<u8>, String> {
        Err(crate::i18n::t("nicht unterstützt").into())
    }
    pub fn eject(&self) -> Result<(), String> {
        Ok(())
    }
    pub fn load(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct TrackRip {
    pub raw: Vec<u8>,
    pub rereads: u32,
    pub insecure_sectors: u32,
    pub crc: u32,
    /// Passes over the whole track (1 = AccurateRip already confirmed the first pass).
    pub passes: u32,
    pub verified_first_pass: bool,
}

/// Sectors per read command (64 KB transfer limit / 2352 bytes).
pub(crate) const READ_SECTORS: i64 = 26;

/// Read sectors `lo..hi` (relative to the disc) one large block after another, in order,
/// so the drive streams without seeking. Sectors outside the disc are silence.
/// Raw audio sectors (2352 bytes each) from a CD drive; tests use a disc in memory.
pub trait SectorRead {
    fn read(&self, lba: u32, count: u32) -> Result<Vec<u8>, String>;
}

impl SectorRead for Drive {
    fn read(&self, lba: u32, count: u32) -> Result<Vec<u8>, String> {
        Drive::read(self, lba, count)
    }
}

pub(crate) fn read_range(
    drive: &dyn SectorRead,
    s_lo: i64,
    s_hi: i64,
    max_lba: i64,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
) -> Result<(Vec<u8>, Vec<i64>), String> {
    let mut raw: Vec<u8> = Vec::with_capacity(((s_hi - s_lo).max(0) as usize) * SECTOR);
    let mut unreadable = Vec::new();
    let mut lba = s_lo;
    while lba < s_hi {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::i18n::t("abgebrochen").into());
        }
        let n = READ_SECTORS.min(s_hi - lba);
        let (data, bad) = read_block_lenient(drive, lba, n, max_lba, cancel)?;
        raw.extend_from_slice(&data);
        unreadable.extend(bad);
        lba += n;
        progress(((lba - s_lo) as f32 / (s_hi - s_lo).max(1) as f32).min(1.0));
    }
    Ok((raw, unreadable))
}

/// Like `read_block`, but a block the drive refuses is read sector by sector: sectors that
/// still fail come back as silence and are listed, so one scratch costs no whole track.
/// Only Cancel is an error.
pub(crate) fn read_block_lenient(drive: &dyn SectorRead, lba: i64, n: i64, max_lba: i64, cancel: &AtomicBool) -> Result<(Vec<u8>, Vec<i64>), String> {
    if let Ok(d) = read_block(drive, lba, n, max_lba) {
        return Ok((d, Vec::new()));
    }
    let mut out = Vec::with_capacity(n as usize * SECTOR);
    let mut bad = Vec::new();
    for l in lba..lba + n {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::i18n::t("abgebrochen").into());
        }
        match read_block(drive, l, 1, max_lba) {
            Ok(d) => out.extend_from_slice(&d),
            Err(_) => {
                out.extend(std::iter::repeat_n(0u8, SECTOR));
                bad.push(l);
            }
        }
    }
    Ok((out, bad))
}

pub(crate) fn read_block(drive: &dyn SectorRead, lba: i64, n: i64, max_lba: i64) -> Result<Vec<u8>, String> {
    let lo = lba.max(0);
    let hi = (lba + n).min(max_lba);
    let pre = (lo - lba).max(0) as usize;
    let mut chunk = vec![0u8; pre * SECTOR];
    if hi > lo {
        let want = (hi - lo) as usize * SECTOR;
        let mut tries = 0;
        let data = loop {
            match drive.read(lo as u32, (hi - lo) as u32) {
                Ok(d) if d.len() == want => break d,
                Ok(_) | Err(_) if tries < 3 => tries += 1,
                Ok(mut d) => {
                    d.resize(want, 0);
                    break d;
                }
                Err(e) => return Err(e),
            }
        };
        chunk.extend_from_slice(&data);
    }
    chunk.resize(n as usize * SECTOR, 0);
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids() {
        // A 3-track test layout
        let toc = Toc {
            tracks: vec![
                TocTrack { number: 1, lba: 0, audio: true },
                TocTrack { number: 2, lba: 15000, audio: true },
                TocTrack { number: 3, lba: 30000, audio: true },
            ],
            leadout: 45000,
        };
        assert_eq!(freedb_id(&toc) & 0xFF, 3);
        assert_eq!(musicbrainz_id(&toc).len(), 28);
        assert!(accuraterip_url(&toc).contains("dBAR-003-"));
    }
}
