//! Audio buffers, decoding (built-in decoders, FFmpeg as fallback) and PCM writers.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// No console window for helper programs.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn hidden(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Interleaved floating point audio in the range -1..1.
#[derive(Clone, Debug, Default)]
pub struct Audio {
    pub rate: u32,
    pub channels: usize,
    /// Bit depth of the source (16 for lossy sources).
    pub bits: u16,
    pub float: bool,
    pub lossless: bool,
    pub data: Vec<f32>,
}

impl Audio {
    /// Same format, other samples (without copying this audio's samples first).
    pub fn with_data(&self, data: Vec<f32>) -> Audio {
        Audio { rate: self.rate, channels: self.channels, bits: self.bits, float: self.float, lossless: self.lossless, data }
    }
    pub fn frames(&self) -> usize {
        self.data.len().checked_div(self.channels).unwrap_or(0)
    }
    pub fn seconds(&self) -> f64 {
        if self.rate == 0 {
            0.0
        } else {
            self.frames() as f64 / self.rate as f64
        }
    }
    pub fn peak(&self) -> f32 {
        self.data.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }
}

/// File extensions the program reads.
pub const INPUT_EXTENSIONS: &[&str] = &[
    "mp3", "mp2", "flac", "wav", "wave", "aif", "aiff", "aifc", "m4a", "m4b", "mp4", "aac", "alac", "ogg", "oga", "opus", "wma", "ape", "wv", "mpc", "spx",
    "dsf", "dff", "caf", "ac3", "mka", "webm", "amr", "au", "snd", "tta", "w64",
];

pub fn is_audio_file(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).map(|e| INPUT_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str())).unwrap_or(false)
}

/// Where FFmpeg can be found: settings first, then PATH.
pub fn find_tool(configured: &str, name: &str) -> Option<PathBuf> {
    if !configured.trim().is_empty() {
        let p = PathBuf::from(configured.trim());
        if p.exists() {
            return Some(p);
        }
    }
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let c = dir.join(&exe);
            if c.exists() {
                return Some(c);
            }
        }
    }
    // Next to our own program
    if let Ok(me) = std::env::current_exe() {
        if let Some(d) = me.parent() {
            let c = d.join(&exe);
            if c.exists() {
                return Some(c);
            }
        }
    }
    None
}

pub fn ffprobe_beside(ffmpeg: &Path) -> Option<PathBuf> {
    let dir = ffmpeg.parent()?;
    let name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    let p = dir.join(name);
    if p.exists() {
        Some(p)
    } else {
        find_tool("", "ffprobe")
    }
}

/// List the encoders the given FFmpeg was built with.
pub fn ffmpeg_encoders(ffmpeg: &Path) -> Vec<String> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-hide_banner", "-encoders"]).stdout(Stdio::piped()).stderr(Stdio::null());
    hidden(&mut cmd);
    let Ok(out) = cmd.output() else { return Vec::new() };
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            if l.starts_with('A') && l.len() > 8 {
                l.split_whitespace().nth(1).map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect()
}

pub type Progress<'a> = &'a dyn Fn(f32);

/// Decode a whole file into memory.
pub fn decode(path: &Path, ffmpeg: Option<&Path>, cancel: &AtomicBool, progress: Progress) -> Result<Audio, String> {
    match decode_builtin(path, cancel, progress) {
        Ok(a) => Ok(a),
        // a damaged file stays an error: FFmpeg would hide the damage and give shorter audio
        Err(Fail::Stop(e)) => Err(e),
        Err(Fail::Unsupported(e)) => match ffmpeg {
            Some(ff) => decode_ffmpeg(path, ff, cancel),
            None => Err(crate::i18n::tf(crate::i18n::t("{e} (für dieses Format wird FFmpeg benötigt – siehe Einstellungen)"), &[("e", e.to_string())])),
        },
    }
}

/// Why the built-in decoder gave up.
enum Fail {
    /// Not a format it reads: FFmpeg may.
    Unsupported(String),
    /// Cancelled or damaged: no second try.
    Stop(String),
}

fn decode_builtin(path: &Path, cancel: &AtomicBool, progress: Progress) -> Result<Audio, Fail> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path)
        .map_err(|e| Fail::Stop(crate::i18n::tf(crate::i18n::t("Datei kann nicht geöffnet werden: {e}"), &[("e", format!("{e}"))])))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| Fail::Unsupported(crate::i18n::tf(crate::i18n::t("Format nicht erkannt: {e}"), &[("e", format!("{e}"))])))?;
    let mut format = probed.format;
    let track =
        format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).ok_or(Fail::Unsupported(crate::i18n::t("keine Audiospur gefunden").into()))?;
    let track_id = track.id;
    let params = track.codec_params.clone();
    let total_frames = params.n_frames.unwrap_or(0);
    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(|e| Fail::Unsupported(crate::i18n::tf(crate::i18n::t("kein eingebauter Decoder: {e}"), &[("e", format!("{e}"))])))?;
    let codec = params.codec;
    use symphonia::core::codecs::*;
    let lossless = matches!(
        codec,
        CODEC_TYPE_FLAC
            | CODEC_TYPE_ALAC
            | CODEC_TYPE_PCM_S16LE
            | CODEC_TYPE_PCM_S16BE
            | CODEC_TYPE_PCM_S24LE
            | CODEC_TYPE_PCM_S24BE
            | CODEC_TYPE_PCM_S32LE
            | CODEC_TYPE_PCM_S32BE
            | CODEC_TYPE_PCM_F32LE
            | CODEC_TYPE_PCM_F32BE
            | CODEC_TYPE_PCM_F64LE
            | CODEC_TYPE_PCM_F64BE
            | CODEC_TYPE_PCM_U8
            | CODEC_TYPE_PCM_S8
            | CODEC_TYPE_PCM_S24LE_PLANAR
            | CODEC_TYPE_PCM_S16LE_PLANAR
    );
    let float = matches!(codec, CODEC_TYPE_PCM_F32LE | CODEC_TYPE_PCM_F32BE | CODEC_TYPE_PCM_F64LE | CODEC_TYPE_PCM_F64BE);
    let mut audio = Audio {
        rate: params.sample_rate.unwrap_or(0),
        channels: params.channels.map(|c| c.count()).unwrap_or(0),
        // an unknown depth of a lossless source is kept at 24 bit rather than cut to 16
        bits: if lossless { params.bits_per_sample.unwrap_or(24) as u16 } else { 16 },
        float,
        lossless,
        data: Vec::new(),
    };
    let mut buf: Option<SampleBuffer<f32>> = None;
    let mut decoded_frames: u64 = 0;
    let mut bad_packets = 0u32;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Fail::Stop(crate::i18n::t("abgebrochen").into()));
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(Fail::Stop(crate::i18n::tf(crate::i18n::t("Lesefehler: {e}"), &[("e", format!("{e}"))]))),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let cap = decoded.capacity() as u64;
                if buf.as_ref().map(|b| b.capacity() < cap as usize).unwrap_or(true) {
                    buf = Some(SampleBuffer::<f32>::new(cap, spec));
                }
                let b = buf.as_mut().unwrap();
                b.copy_interleaved_ref(decoded);
                audio.data.extend_from_slice(b.samples());
                audio.rate = spec.rate;
                audio.channels = spec.channels.count();
                decoded_frames += (b.samples().len() / audio.channels.max(1)) as u64;
                if total_frames > 0 {
                    progress((decoded_frames as f32 / total_frames as f32).min(1.0));
                }
            }
            Err(Error::DecodeError(_)) => {
                bad_packets += 1;
                continue;
            }
            Err(e) => return Err(Fail::Stop(crate::i18n::tf(crate::i18n::t("Dekodierfehler: {e}"), &[("e", format!("{e}"))]))),
        }
    }
    if audio.channels == 0 || audio.rate == 0 {
        return Err(Fail::Unsupported(crate::i18n::t("leere oder unbekannte Audiodaten").into()));
    }
    if bad_packets > 0 {
        return Err(Fail::Stop(crate::i18n::tf(crate::i18n::t("Datei ist beschädigt: {0} Stellen nicht lesbar"), &[("0", bad_packets.to_string())])));
    }
    // more than a tenth of a second missing against the length in the header: cut off
    if total_frames > 0 && decoded_frames < total_frames && total_frames - decoded_frames > (audio.rate as u64 / 10).max(1) {
        return Err(Fail::Stop(crate::i18n::tf(
            crate::i18n::t("Datei ist beschädigt: endet nach {0} von {1}"),
            &[("0", fmt_len(decoded_frames as f64 / audio.rate as f64)), ("1", fmt_len(total_frames as f64 / audio.rate as f64))],
        )));
    }
    Ok(audio)
}

#[cfg(test)]
mod decode_tests {
    use super::*;
    #[test]
    fn a_cut_off_file_is_reported_not_shortened() {
        let p = std::env::temp_dir().join(format!("psyaudio-cut-{}.wav", std::process::id()));
        let a = Audio { rate: 8000, channels: 1, bits: 16, float: false, lossless: true, data: vec![0.1; 8000] };
        write_wav(&p, &a, PcmFormat::S16).unwrap();
        let len = std::fs::metadata(&p).unwrap().len();
        std::fs::OpenOptions::new().write(true).open(&p).unwrap().set_len(len / 2).unwrap();
        let ff = find_tool("", "ffmpeg");
        let r = decode(&p, ff.as_deref(), &AtomicBool::new(false), &|_| {});
        let _ = std::fs::remove_file(&p);
        assert!(r.is_err(), "a file cut in half decoded as {:?} frames", r.map(|a| a.frames()));
    }
    #[test]
    fn unknown_stream_layout_is_an_error_not_a_guess() {
        assert!(stream_layout(None).is_err());
        assert_eq!(stream_layout(Some((48000, 6, 24, "flac".into()))).unwrap(), (48000, 6, 24, "flac".into()));
    }
}

/// Rate, channels, bits and codec FFmpeg must decode with; without FFprobe they are unknown,
/// and guessing would silently resample or downmix.
fn stream_layout(info: Option<(u32, usize, u16, String)>) -> Result<(u32, usize, u16, String), String> {
    info.ok_or_else(|| crate::i18n::t("FFprobe fehlt neben FFmpeg – ohne FFprobe lässt sich dieses Format nicht unverändert lesen").into())
}

/// Rate, channels and bits of a file as FFprobe sees it.
pub fn ffprobe_info(ffmpeg: &Path, path: &Path) -> Option<(u32, usize, u16, String)> {
    let probe = ffprobe_beside(ffmpeg)?;
    let mut cmd = Command::new(probe);
    cmd.args([
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_entries",
        "stream=sample_rate,channels,bits_per_raw_sample,codec_name",
        "-of",
        "default=noprint_wrappers=1",
    ])
    .arg(path)
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    hidden(&mut cmd);
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (mut rate, mut ch, mut bits, mut codec) = (0u32, 0usize, 0u16, String::new());
    for l in text.lines() {
        let mut it = l.splitn(2, '=');
        let (k, v) = (it.next().unwrap_or(""), it.next().unwrap_or("").trim());
        match k {
            "sample_rate" => rate = v.parse().unwrap_or(0),
            "channels" => ch = v.parse().unwrap_or(0),
            "bits_per_raw_sample" => bits = v.parse().unwrap_or(0),
            "codec_name" => codec = v.to_string(),
            _ => {}
        }
    }
    if rate == 0 || ch == 0 {
        None
    } else {
        Some((rate, ch, bits, codec))
    }
}

fn decode_ffmpeg(path: &Path, ffmpeg: &Path, cancel: &AtomicBool) -> Result<Audio, String> {
    let (rate, channels, bits, codec) = stream_layout(ffprobe_info(ffmpeg, path))?;
    let lossless =
        ["flac", "alac", "wavpack", "ape", "tta", "mlp", "truehd"].iter().any(|c| codec.starts_with(c)) || codec.starts_with("pcm") || codec.starts_with("dsd");
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-v", "error", "-nostdin", "-i"])
        .arg(path)
        .args(["-vn", "-f", "f32le", "-acodec", "pcm_f32le", "-ar"])
        .arg(rate.to_string())
        .arg("-ac")
        .arg(channels.to_string())
        .arg("-")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hidden(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| crate::i18n::tf(crate::i18n::t("FFmpeg kann nicht gestartet werden: {e}"), &[("e", format!("{e}"))]))?;
    let mut stderr = child.stderr.take().unwrap();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let mut out = child.stdout.take().unwrap();
    let mut bytes = Vec::new();
    let mut chunk = vec![0u8; 1 << 16];
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait(); // the process must be gone before its temp file is removed
            return Err(crate::i18n::t("abgebrochen").into());
        }
        match out.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(crate::i18n::tf(crate::i18n::t("FFmpeg-Lesefehler: {e}"), &[("e", format!("{e}"))])),
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let err = err_thread.join().unwrap_or_default();
    // with "-v error" FFmpeg only writes real errors: partial audio is not accepted
    if !status.success() || !err.trim().is_empty() {
        return Err(crate::i18n::tf(crate::i18n::t("FFmpeg konnte die Datei nicht lesen: {0}"), &[("0", err.trim().lines().next().unwrap_or("").to_string())]));
    }
    let data: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    Ok(Audio {
        rate,
        channels,
        bits: if lossless && bits > 0 {
            bits.min(32)
        } else if lossless {
            24
        } else {
            16
        },
        float: codec.contains("f32") || codec.contains("f64"),
        lossless,
        data,
    })
}

// ---------- quantisation helpers ----------

fn clampi(v: f64, bits: u16) -> i64 {
    let max = ((1i64 << (bits - 1)) - 1) as f64;
    let min = -((1i64 << (bits - 1)) as f64);
    v.round().clamp(min, max) as i64
}

/// Float sample to signed integer of `bits` bits.
pub fn to_int(s: f32, bits: u16) -> i64 {
    let scale = (1i64 << (bits - 1)) as f64;
    clampi(s as f64 * scale, bits)
}

// ---------- WAV / AIFF writers ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum PcmFormat {
    U8,
    S16,
    S24,
    S32,
    F32,
    F64,
}

impl PcmFormat {
    pub fn label(&self) -> &'static str {
        match self {
            PcmFormat::U8 => "8 Bit",
            PcmFormat::S16 => "16 Bit",
            PcmFormat::S24 => "24 Bit",
            PcmFormat::S32 => "32 Bit",
            PcmFormat::F32 => crate::i18n::t("32 Bit Gleitkomma (IEEE float)"),
            PcmFormat::F64 => crate::i18n::t("64 Bit Gleitkomma (IEEE float)"),
        }
    }
    pub fn bytes(&self) -> usize {
        match self {
            PcmFormat::U8 => 1,
            PcmFormat::S16 => 2,
            PcmFormat::S24 => 3,
            PcmFormat::S32 | PcmFormat::F32 => 4,
            PcmFormat::F64 => 8,
        }
    }
    pub fn bits(&self) -> u16 {
        (self.bytes() * 8) as u16
    }
    pub fn from_bits(bits: u16, float: bool) -> PcmFormat {
        if float {
            return PcmFormat::F32;
        }
        match bits {
            0..=8 => PcmFormat::U8,
            9..=16 => PcmFormat::S16,
            17..=24 => PcmFormat::S24,
            _ => PcmFormat::S32,
        }
    }
}

/// Raw little-endian samples in the given format.
pub fn pcm_bytes_le(data: &[f32], fmt: PcmFormat, out: &mut Vec<u8>) {
    out.reserve(data.len() * fmt.bytes());
    for &s in data {
        match fmt {
            PcmFormat::U8 => out.push((to_int(s, 8) + 128) as u8),
            PcmFormat::S16 => out.extend_from_slice(&(to_int(s, 16) as i16).to_le_bytes()),
            PcmFormat::S24 => {
                let v = to_int(s, 24) as i32;
                out.extend_from_slice(&v.to_le_bytes()[..3]);
            }
            PcmFormat::S32 => out.extend_from_slice(&(to_int(s, 32) as i32).to_le_bytes()),
            PcmFormat::F32 => out.extend_from_slice(&s.to_le_bytes()),
            PcmFormat::F64 => out.extend_from_slice(&(s as f64).to_le_bytes()),
        }
    }
}

/// WAV and AIFF store sizes in 32 bits: `overhead + body` must fit, else the header would lie.
fn container_len(body: u64, overhead: u64) -> Result<u32, String> {
    u32::try_from(body + overhead + (body & 1))
        .map(|_| (body + overhead) as u32)
        .map_err(|_| crate::i18n::t("Zu groß für WAV/AIFF (über 4 GB) – FLAC oder WavPack wählen").into())
}

#[cfg(test)]
mod size_tests {
    #[test]
    fn files_over_4_gb_are_refused_not_corrupted() {
        assert_eq!(super::container_len(1000, 36), Ok(1036));
        assert!(super::container_len(u32::MAX as u64 - 10, 36).is_err());
        assert!(super::container_len(6_000_000_000, 36).is_err());
    }
}

pub fn write_wav(path: &Path, a: &Audio, fmt: PcmFormat) -> Result<(), String> {
    let ch = a.channels as u16;
    let block = ch as u32 * fmt.bytes() as u32;
    let float = matches!(fmt, PcmFormat::F32 | PcmFormat::F64);
    let extensible = a.channels > 2 || matches!(fmt, PcmFormat::S24 | PcmFormat::S32);
    let fmt_len: u32 = if extensible { 40 } else { 16 };
    // checked before the samples are converted, so a too long file costs no memory
    let body_len = a.data.len() as u64 * fmt.bytes() as u64;
    let riff_len = container_len(body_len, 4 + 8 + fmt_len as u64 + 8)? + (body_len & 1) as u32;
    let mut body = Vec::new();
    pcm_bytes_le(&a.data, fmt, &mut body);
    let mut h = Vec::new();
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&riff_len.to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&fmt_len.to_le_bytes());
    let tag: u16 = if extensible {
        0xFFFE
    } else if float {
        3
    } else {
        1
    };
    h.extend_from_slice(&tag.to_le_bytes());
    h.extend_from_slice(&ch.to_le_bytes());
    h.extend_from_slice(&a.rate.to_le_bytes());
    h.extend_from_slice(&(a.rate * block).to_le_bytes());
    h.extend_from_slice(&(block as u16).to_le_bytes());
    h.extend_from_slice(&fmt.bits().to_le_bytes());
    if extensible {
        h.extend_from_slice(&22u16.to_le_bytes());
        h.extend_from_slice(&fmt.bits().to_le_bytes());
        let mask: u32 = match a.channels {
            1 => 0x4,
            2 => 0x3,
            3 => 0x7,
            4 => 0x33,
            5 => 0x37,
            6 => 0x3F,
            7 => 0x13F,
            8 => 0x63F,
            _ => 0,
        };
        h.extend_from_slice(&mask.to_le_bytes());
        let sub: u16 = if float { 3 } else { 1 };
        h.extend_from_slice(&sub.to_le_bytes());
        h.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
    }
    h.extend_from_slice(b"data");
    h.extend_from_slice(&(body.len() as u32).to_le_bytes());
    let mut f =
        std::fs::File::create(path).map_err(|e| crate::i18n::tf(crate::i18n::t("Zieldatei kann nicht angelegt werden: {e}"), &[("e", format!("{e}"))]))?;
    f.write_all(&h).and_then(|_| f.write_all(&body)).map_err(|e| e.to_string())?;
    if body.len() & 1 == 1 {
        f.write_all(&[0]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ext80(rate: f64) -> [u8; 10] {
    // IEEE 754 80-bit extended, big endian
    let mut out = [0u8; 10];
    if rate <= 0.0 {
        return out;
    }
    let mut exp: i32 = 16383 + 63;
    let mut mant = rate;
    while mant < 9.223_372_036_854_776e18 {
        mant *= 2.0;
        exp -= 1;
    }
    while mant >= 1.8446744073709552e19 {
        mant /= 2.0;
        exp += 1;
    }
    let m = mant as u64;
    out[0] = ((exp >> 8) & 0x7F) as u8;
    out[1] = (exp & 0xFF) as u8;
    out[2..10].copy_from_slice(&m.to_be_bytes());
    out
}

pub fn write_aiff(path: &Path, a: &Audio, bits: u16) -> Result<(), String> {
    let bits = match bits {
        0..=8 => 8,
        9..=16 => 16,
        17..=24 => 24,
        _ => 32,
    };
    let bps = (bits / 8) as usize;
    let form_len = container_len(a.data.len() as u64 * bps as u64, 4 + (8 + 18) + 8 + 8)?;
    let mut body = Vec::with_capacity(a.data.len() * bps);
    for &s in &a.data {
        let v = to_int(s, bits);
        let be = (v as i32).to_be_bytes();
        body.extend_from_slice(&be[4 - bps..]);
    }
    let frames = a.frames() as u32;
    let mut h = Vec::new();
    let ssnd_len = 8 + body.len() as u32;
    let form_len = form_len + (body.len() as u32 & 1);
    h.extend_from_slice(b"FORM");
    h.extend_from_slice(&form_len.to_be_bytes());
    h.extend_from_slice(b"AIFFCOMM");
    h.extend_from_slice(&18u32.to_be_bytes());
    h.extend_from_slice(&(a.channels as u16).to_be_bytes());
    h.extend_from_slice(&frames.to_be_bytes());
    h.extend_from_slice(&bits.to_be_bytes());
    h.extend_from_slice(&ext80(a.rate as f64));
    h.extend_from_slice(b"SSND");
    h.extend_from_slice(&ssnd_len.to_be_bytes());
    h.extend_from_slice(&[0u8; 8]);
    let mut f =
        std::fs::File::create(path).map_err(|e| crate::i18n::tf(crate::i18n::t("Zieldatei kann nicht angelegt werden: {e}"), &[("e", format!("{e}"))]))?;
    f.write_all(&h).and_then(|_| f.write_all(&body)).map_err(|e| e.to_string())?;
    if body.len() & 1 == 1 {
        f.write_all(&[0]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// CRC-32 (IEEE) used for the audio checksum tool and rip logs.
pub fn crc32(data: &[u8], mut crc: u32) -> u32 {
    crc = !crc;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Human readable length.
pub fn fmt_len(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn fmt_size(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.2} GB", bytes as f64 / (1u64 << 30) as f64)
    } else if bytes >= 1 << 20 {
        format!("{:.1} MB", bytes as f64 / (1u64 << 20) as f64)
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}
