//! CD ripper: reading audio CDs (Windows and Linux raw read), disc IDs and MusicBrainz metadata.

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
        .map(|a| a.iter().map(|c| format!("{}{}", c["name"].as_str().unwrap_or(""), c["joinphrase"].as_str().unwrap_or(""))).collect::<String>())
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
    let mut resp = ureq::get(&format!("https://coverartarchive.org/release/{release_id}/front-500")).header("User-Agent", "PsyAudio/1.0").call().ok()?;
    resp.body_mut().read_to_vec().ok()
}

/// Turn raw CD audio (16-bit stereo little endian) into an audio buffer.
pub fn raw_to_audio(raw: &[u8]) -> Audio {
    let data = raw.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
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
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, GetDriveTypeW, GetLogicalDrives, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const IOCTL_CDROM_READ_TOC: u32 = 0x0002_4000;
    const IOCTL_CDROM_RAW_READ: u32 = 0x0002_403E;
    const IOCTL_STORAGE_EJECT_MEDIA: u32 = 0x002D_4808;
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
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
                CreateFileW(name.as_ptr(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut())
            };
            if h == INVALID_HANDLE_VALUE || h.is_null() {
                return Err(crate::i18n::tf(crate::i18n::t("Laufwerk {0} kann nicht geöffnet werden"), &[("0", drive_name(letter))]));
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

        /// Vendor and product from the storage device descriptor.
        pub fn model(&self) -> Option<String> {
            // STORAGE_PROPERTY_QUERY: StorageDeviceProperty (0), PropertyStandardQuery (0)
            let query = [0u8; 12];
            let mut out = vec![0u8; 1024];
            let n = self.ioctl(IOCTL_STORAGE_QUERY_PROPERTY, &query, &mut out).ok()? as usize;
            model_from_descriptor(&out[..n.min(out.len())])
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

/// Linux: drives are `/dev/sr0` … `/dev/sr9`, named by their digit; access through the kernel's CD-ROM ioctls.
#[cfg(target_os = "linux")]
mod lin {
    use super::*;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::fs::OpenOptionsExt;

    const CDROMREADTOCHDR: libc::c_ulong = 0x5305;
    const CDROMREADTOCENTRY: libc::c_ulong = 0x5306;
    const CDROMEJECT: libc::c_ulong = 0x5309;
    const CDROMREADAUDIO: libc::c_ulong = 0x530e;
    const CDROM_LBA: u8 = 0x01;
    const CDROM_LEADOUT: u8 = 0xAA;
    const CDROM_DATA_TRACK: u8 = 0x04;

    #[repr(C)]
    #[derive(Default)]
    struct TocHeader {
        first: u8,
        last: u8,
    }

    /// `struct cdrom_tocentry`: `adr` and `ctrl` share one byte (adr in the low nibble).
    #[repr(C)]
    #[derive(Default)]
    struct TocEntry {
        track: u8,
        adr_ctrl: u8,
        format: u8,
        lba: i32,
        datamode: u8,
    }

    /// `struct cdrom_read_audio`.
    #[repr(C)]
    struct ReadAudio {
        lba: i32,
        addr_format: u8,
        nframes: libc::c_int,
        buf: *mut u8,
    }

    pub fn path(d: char) -> String {
        format!("/dev/sr{d}")
    }

    pub fn drives() -> Vec<char> {
        ('0'..='9').filter(|d| std::path::Path::new(&path(*d)).exists()).collect()
    }

    pub struct Drive {
        fd: OwnedFd,
    }

    impl Drive {
        pub fn open(d: char) -> Result<Drive, String> {
            std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path(d))
                .map(|f| Drive { fd: f.into() })
                .map_err(|_| crate::i18n::tf(crate::i18n::t("Laufwerk {0} kann nicht geöffnet werden"), &[("0", path(d))]))
        }

        fn ioctl<T>(&self, code: libc::c_ulong, arg: *mut T) -> Result<(), String> {
            if unsafe { libc::ioctl(self.fd.as_raw_fd(), code as _, arg) } < 0 {
                Err(crate::i18n::tf(crate::i18n::t("Laufwerksbefehl fehlgeschlagen (Fehler {0})"), &[("0", format!("{}", std::io::Error::last_os_error()))]))
            } else {
                Ok(())
            }
        }

        fn entry(&self, track: u8) -> Result<TocEntry, String> {
            let mut e = TocEntry { track, format: CDROM_LBA, ..Default::default() };
            self.ioctl(CDROMREADTOCENTRY, &mut e)?;
            Ok(e)
        }

        pub fn toc(&self) -> Result<Toc, String> {
            let mut h = TocHeader::default();
            self.ioctl(CDROMREADTOCHDR, &mut h).map_err(|_| crate::i18n::t("Keine Audio-CD im Laufwerk").to_string())?;
            let mut toc = Toc::default();
            for n in h.first..=h.last {
                let e = self.entry(n)?;
                let ctrl = e.adr_ctrl >> 4;
                toc.tracks.push(TocTrack { number: n, lba: e.lba.max(0) as u32, audio: ctrl & CDROM_DATA_TRACK == 0 });
            }
            toc.leadout = self.entry(CDROM_LEADOUT)?.lba.max(0) as u32;
            if toc.tracks.is_empty() {
                return Err(crate::i18n::t("Keine Titel auf der CD").into());
            }
            Ok(toc)
        }

        /// Read `count` raw audio sectors starting at `lba`.
        pub fn read(&self, lba: u32, count: u32) -> Result<Vec<u8>, String> {
            let mut out = vec![0u8; count as usize * SECTOR];
            let mut ra = ReadAudio { lba: lba as i32, addr_format: CDROM_LBA, nframes: count as libc::c_int, buf: out.as_mut_ptr() };
            self.ioctl(CDROMREADAUDIO, &mut ra)?;
            Ok(out)
        }

        pub fn eject(&self) -> Result<(), String> {
            self.ioctl::<u8>(CDROMEJECT, std::ptr::null_mut())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::mem::{offset_of, size_of};

        /// The structs must match `<linux/cdrom.h>` byte for byte.
        #[test]
        fn layouts_match_the_kernel() {
            assert_eq!(size_of::<TocHeader>(), 2);
            assert_eq!(size_of::<TocEntry>(), 12);
            assert_eq!(offset_of!(TocEntry, lba), 4);
            assert_eq!(offset_of!(TocEntry, datamode), 8);
            assert_eq!(offset_of!(ReadAudio, addr_format), 4);
            assert_eq!(offset_of!(ReadAudio, nframes), 8);
            assert_eq!(offset_of!(ReadAudio, buf), 16);
            assert_eq!(size_of::<ReadAudio>(), 24);
        }

        #[test]
        fn drives_are_named_by_device() {
            assert_eq!(path('0'), "/dev/sr0");
        }
    }
}

#[cfg(target_os = "linux")]
pub use lin::{drives, Drive};

/// How a drive is shown: `D:` on Windows, `/dev/sr0` on Linux.
pub fn drive_name(d: char) -> String {
    #[cfg(target_os = "linux")]
    {
        lin::path(d)
    }
    #[cfg(not(target_os = "linux"))]
    {
        format!("{d}:")
    }
}

/// Vendor and product as one model name ("VENDOR CD-RW X1"), blanks collapsed; `None` when empty.
pub fn normalize_model(vendor: &str, product: &str) -> Option<String> {
    let s = format!("{vendor} {product}").split_whitespace().collect::<Vec<_>>().join(" ");
    (!s.is_empty()).then_some(s)
}

/// What a drive's measurements are stored under: its model, else its name.
pub(crate) fn key_from(model: Option<String>, name: &str) -> String {
    model.unwrap_or_else(|| name.to_string())
}

/// The model in a Windows STORAGE_DEVICE_DESCRIPTOR: vendor and product, the firmware left out.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn model_from_descriptor(buf: &[u8]) -> Option<String> {
    let offset = |at: usize| buf.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let text = |off: usize| -> String {
        if off == 0 || off >= buf.len() {
            return String::new();
        }
        let end = buf[off..].iter().position(|b| *b == 0).map(|p| off + p).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[off..end]).to_string()
    };
    normalize_model(&text(offset(12)?), &text(offset(16)?))
}

/// The drive's model ("VENDOR CD-RW X1"), if the system tells it.
pub fn drive_model(d: char) -> Option<String> {
    #[cfg(windows)]
    {
        Drive::open(d).ok()?.model()
    }
    #[cfg(target_os = "linux")]
    {
        let read = |f: &str| std::fs::read_to_string(format!("/sys/block/sr{d}/device/{f}")).unwrap_or_default();
        normalize_model(&read("vendor"), &read("model"))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = d;
        None
    }
}

/// The key a drive's offset and buffer size are stored under.
pub fn drive_key(d: char) -> String {
    key_from(drive_model(d), &drive_name(d))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn drives() -> Vec<char> {
    Vec::new()
}

#[cfg(not(any(windows, target_os = "linux")))]
pub struct Drive;
#[cfg(not(any(windows, target_os = "linux")))]
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
    /// Passes over the whole track (1 = the CUETools database already confirmed the first pass).
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

/// Read `n` sectors from `lba`; sectors before the disc or past `max_lba` are silence. An answer
/// that stays too short after retries is a read error, never filled up with silence.
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
                Ok(_) => return Err(crate::i18n::t("Laufwerk lieferte zu wenig Daten").into()),
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
    }

    /// A drive that answers with one sector too few, `short` times (then in full).
    struct Short {
        short: std::cell::Cell<u32>,
    }
    impl SectorRead for Short {
        fn read(&self, _lba: u32, count: u32) -> Result<Vec<u8>, String> {
            let n = if self.short.get() > 0 {
                self.short.set(self.short.get() - 1);
                count - 1
            } else {
                count
            };
            Ok(vec![1u8; n as usize * SECTOR])
        }
    }

    #[test]
    fn short_reads_become_an_error() {
        let d = Short { short: std::cell::Cell::new(u32::MAX) };
        assert!(read_block(&d, 0, 4, 100).is_err(), "no silent zeros for missing data");
        let (data, bad) = read_block_lenient(&d, 0, 4, 100, &AtomicBool::new(false)).unwrap();
        assert_eq!(bad, vec![0, 1, 2, 3], "every sector counts as unread");
        assert_eq!(data.len(), 4 * SECTOR);
    }

    #[test]
    fn one_short_read_is_retried() {
        let d = Short { short: std::cell::Cell::new(1) };
        assert_eq!(read_block(&d, 0, 4, 100), Ok(vec![1u8; 4 * SECTOR]));
    }

    #[test]
    fn normalize_model_joins_and_trims() {
        assert_eq!(normalize_model("VENDOR", "CD-RW  X1 "), Some("VENDOR CD-RW X1".to_string()));
        assert_eq!(normalize_model("  ", ""), None);
    }

    #[test]
    fn drive_key_falls_back_to_the_name() {
        assert_eq!(key_from(Some("Vendor Model X".into()), "G:"), "Vendor Model X");
        assert_eq!(key_from(None, "G:"), "G:");
    }

    #[test]
    fn storage_descriptor_gives_vendor_and_product() {
        // STORAGE_DEVICE_DESCRIPTOR: vendor offset at 12, product at 16, revision at 20
        let mut buf = vec![0u8; 64];
        buf[12..16].copy_from_slice(&40u32.to_le_bytes());
        buf[16..20].copy_from_slice(&48u32.to_le_bytes());
        buf[20..24].copy_from_slice(&58u32.to_le_bytes());
        buf[40..47].copy_from_slice(b"VENDOR ");
        buf[48..56].copy_from_slice(b"DRIVE X ");
        buf[58..62].copy_from_slice(b"1.01");
        assert_eq!(model_from_descriptor(&buf), Some("VENDOR DRIVE X".to_string()));
        assert_eq!(model_from_descriptor(&[0u8; 8]), None);
    }
}
