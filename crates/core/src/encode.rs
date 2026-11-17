//! Output formats ("encoders") with their settings, and writing a decoded file in that format.

use crate::audio::{self, hidden, Audio, PcmFormat};
use crate::ffmpeg::{Ffmpeg, FfmpegSet};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum Codec {
    Mp3,
    Flac,
    AacM4a,
    AudioBook,
    Alac,
    Opus,
    Vorbis,
    Wave,
    Aiff,
    WavPack,
    MonkeysAudio,
    Wma,
    Musepack,
    Speex,
    Dsd,
    // utilities (no new audio format)
    Multi,
    ReplayGainOnly,
    TagUpdate,
    AudioInfo,
    AudioCrc,
    ChannelSplit,
    LengthSplit,
    TagFromFilename,
    TestConversion,
}

pub const AUDIO_CODECS: &[Codec] = &[
    Codec::Mp3,
    Codec::Flac,
    Codec::AacM4a,
    Codec::Alac,
    Codec::Opus,
    Codec::Vorbis,
    Codec::Wave,
    Codec::Aiff,
    Codec::WavPack,
    Codec::MonkeysAudio,
    Codec::Wma,
    Codec::AudioBook,
    Codec::Musepack,
    Codec::Speex,
    Codec::Dsd,
];

pub const TOOL_CODECS: &[Codec] = &[
    Codec::Multi,
    Codec::ReplayGainOnly,
    Codec::TagUpdate,
    Codec::AudioInfo,
    Codec::AudioCrc,
    Codec::ChannelSplit,
    Codec::LengthSplit,
    Codec::TagFromFilename,
    Codec::TestConversion,
];

impl Codec {
    pub fn name(&self) -> &'static str {
        match self {
            Codec::Mp3 => "MP3",
            Codec::Flac => "FLAC",
            Codec::AacM4a => "AAC (m4a)",
            Codec::AudioBook => crate::i18n::t("Hörbuch (m4b)"),
            Codec::Alac => "Apple Lossless",
            Codec::Opus => "Opus",
            Codec::Vorbis => "Ogg Vorbis",
            Codec::Wave => "Wave",
            Codec::Aiff => "AIFF",
            Codec::WavPack => "WavPack",
            Codec::MonkeysAudio => "Monkey's Audio",
            Codec::Wma => "Windows Media Audio",
            Codec::Musepack => "Musepack",
            Codec::Speex => "Speex",
            Codec::Dsd => "DSD (dsf)",
            Codec::Multi => crate::i18n::t("Mehrfach-Encoder"),
            Codec::ReplayGainOnly => crate::i18n::t("ReplayGain berechnen"),
            Codec::TagUpdate => crate::i18n::t("Tags aktualisieren"),
            Codec::AudioInfo => "Audio-Info",
            Codec::AudioCrc => crate::i18n::t("Audio-Prüfsumme (CRC)"),
            Codec::ChannelSplit => crate::i18n::t("Kanäle aufteilen"),
            Codec::LengthSplit => crate::i18n::t("Nach Länge aufteilen"),
            Codec::TagFromFilename => crate::i18n::t("Tags aus Dateiname"),
            Codec::TestConversion => crate::i18n::t("Testumwandlung"),
        }
    }
    pub fn short(&self) -> &'static str {
        match self {
            Codec::AacM4a => "AAC",
            Codec::AudioBook => "M4B",
            Codec::Alac => "ALAC",
            Codec::Vorbis => "Vorbis",
            Codec::MonkeysAudio => "APE",
            Codec::Wma => "WMA",
            Codec::Musepack => "MPC",
            Codec::Dsd => "DSD",
            c => c.name(),
        }
    }
    pub fn ext(&self) -> &'static str {
        match self {
            Codec::Mp3 => "mp3",
            Codec::Flac => "flac",
            Codec::AacM4a | Codec::Alac => "m4a",
            Codec::AudioBook => "m4b",
            Codec::Opus => "opus",
            Codec::Vorbis => "ogg",
            Codec::Wave => "wav",
            Codec::Aiff => "aiff",
            Codec::WavPack => "wv",
            Codec::MonkeysAudio => "ape",
            Codec::Wma => "wma",
            Codec::Musepack => "mpc",
            Codec::Speex => "spx",
            Codec::Dsd => "dsf",
            _ => "",
        }
    }
    pub fn lossless(&self) -> bool {
        matches!(self, Codec::Flac | Codec::Alac | Codec::Wave | Codec::Aiff | Codec::WavPack | Codec::MonkeysAudio)
    }
    pub fn description(&self) -> &'static str {
        match self {
            Codec::Mp3 => crate::i18n::t("Verlustbehaftet, läuft überall"),
            Codec::Flac => crate::i18n::t("Verlustfrei, offen, mit Tags"),
            Codec::AacM4a => crate::i18n::t("Verlustbehaftet, Apple/Handy"),
            Codec::AudioBook => crate::i18n::t("AAC für Hörbücher"),
            Codec::Alac => crate::i18n::t("Verlustfrei für Apple-Geräte"),
            Codec::Opus => crate::i18n::t("Modern, sehr effizient"),
            Codec::Vorbis => crate::i18n::t("Offen, verlustbehaftet"),
            Codec::Wave => crate::i18n::t("Unkomprimiert (PCM)"),
            Codec::Aiff => crate::i18n::t("Unkomprimiert (Apple)"),
            Codec::WavPack => crate::i18n::t("Verlustfrei, auch Hybrid"),
            Codec::MonkeysAudio => crate::i18n::t("Verlustfrei, stark gepackt"),
            Codec::Wma => "Windows Media Audio",
            Codec::Musepack => crate::i18n::t("Verlustbehaftet, Profile"),
            Codec::Speex => crate::i18n::t("Sprache, sehr klein"),
            Codec::Dsd => "Direct Stream Digital",
            Codec::Multi => crate::i18n::t("Mehrere Formate auf einmal"),
            Codec::ReplayGainOnly => crate::i18n::t("Lautheit messen, Tags schreiben"),
            Codec::TagUpdate => crate::i18n::t("Tag-Bearbeitung auf Quellen"),
            Codec::AudioInfo => crate::i18n::t("Format, Länge, Pegel"),
            Codec::AudioCrc => crate::i18n::t("Prüfsumme der Audiodaten"),
            Codec::ChannelSplit => crate::i18n::t("Jeder Kanal eine Datei"),
            Codec::LengthSplit => crate::i18n::t("In gleich lange Teile"),
            Codec::TagFromFilename => crate::i18n::t("Tags aus dem Namen lesen"),
            Codec::TestConversion => crate::i18n::t("Nur dekodieren und prüfen"),
        }
    }
    pub fn is_tool(&self) -> bool {
        TOOL_CODECS.contains(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum RateMode {
    Vbr,
    Cbr,
    Abr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct EncSettings {
    pub mp3_mode: RateMode,
    pub mp3_vbr: u8,
    pub mp3_kbps: u32,
    pub mp3_stereo: u8, // 0 auto, 1 joint, 2 forced joint, 3 stereo, 4 mono
    pub mp3_q: u8,
    /// " -q " is passed only when the user set it; otherwise LAME decides.
    pub mp3_q_custom: bool,
    pub flac_level: u8,
    pub flac_verify: bool,
    pub aac_vbr: bool,
    pub aac_quality: u8,
    pub aac_kbps: u32,
    pub aac_profile: u8, // 0 LC, 1 HE, 2 HEv2
    pub opus_kbps: u32,
    /// 0 VBR, 1 constrained VBR ("cvbr"), 2 hard CBR ("hard-cbr").
    pub opus_mode: u8,
    /// 0 quality (VBR), 1 average bit rate (-b), 2 managed (--managed -b).
    pub vorbis_mode: u8,
    pub vorbis_q: f32,
    pub vorbis_kbps: u32,
    pub wav_format: PcmFormat,
    pub wav_match_source: bool,
    pub aiff_bits: u16,
    pub aiff_match_source: bool,
    pub wavpack_mode: u8, // 0 fast, 1 normal, 2 high, 3 very high
    pub wavpack_verify: bool,
    pub ape_level: u8, // 0 fast .. 4 insane
    pub wma_kbps: u32,
    pub mpc_profile: u8,
    pub speex_q: u8,
    pub dsd_mode: u8,
    pub alac_verify: bool,
    pub multi: Vec<Codec>,
    pub split_seconds: u32,
    pub split_codec: Codec,
}

impl Default for EncSettings {
    fn default() -> Self {
        EncSettings {
            mp3_mode: RateMode::Vbr,
            mp3_vbr: 4,
            mp3_kbps: 192,
            mp3_stereo: 0,
            mp3_q: 3,
            mp3_q_custom: false,
            flac_level: 5,
            flac_verify: false,
            aac_vbr: true,
            aac_quality: 4,
            aac_kbps: 192,
            aac_profile: 0,
            opus_kbps: 128,
            opus_mode: 0,
            vorbis_mode: 0,
            vorbis_q: 4.0,
            vorbis_kbps: 128,
            wav_format: PcmFormat::S16,
            wav_match_source: true,
            aiff_bits: 16,
            aiff_match_source: true,
            wavpack_mode: 1,
            wavpack_verify: false,
            ape_level: 2,
            wma_kbps: 192,
            mpc_profile: 3,
            speex_q: 8,
            dsd_mode: 0,
            alac_verify: false,
            multi: vec![Codec::Flac, Codec::Mp3],
            split_seconds: 600,
            split_codec: Codec::Flac,
        }
    }
}

/// Vorbis: estimated kbps for -q -1..10, also the bit rates offered in the average/managed modes.
pub const VORBIS_KBPS: [u32; 12] = [48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 350, 500];

/// MP3 constant/average bit rates offered (kbps).
pub const MP3_RATES: &[u32] = &[8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
/// AAC CBR rates offered (kbps).
pub const AAC_RATES: &[u32] = &[8, 12, 16, 24, 32, 48, 56, 64, 80, 96, 128, 144, 160, 192, 224, 256, 320, 400, 480, 512];
/// Estimated bit rate per AAC VBR quality 1..5.
pub const AAC_VBR_KBPS: [u32; 5] = [64, 80, 96, 144, 224];
pub const MPC_PROFILES: &[(&str, &str, u32)] = &[
    ("Telefon", "--telephone", 60),
    ("Daumen", "--thumb", 90),
    ("Radio", "--radio", 130),
    ("Standard", "--standard", 180),
    ("Extrem", "--xtreme", 210),
    ("Wahnsinn", "--insane", 240),
    ("Hirntot", "--braindead", 270),
    ("Qualität 9", "--quality 9", 300),
    ("Qualität 10", "--quality 10", 350),
];
pub const DSD_MODES: &[(&str, u32)] =
    &[("DSD64", 2822400), ("DSD128", 5644800), ("DSD256", 11289600), ("DSD512", 22579200), ("DSD1024", 45158400), ("DSD2048", 90316800)];
pub const APE_LEVELS: &[(&str, &str)] = &[("Schnell", "-c1000"), ("Normal", "-c2000"), ("Hoch", "-c3000"), ("Extra hoch", "-c4000"), ("Wahnsinn", "-c5000")];
pub const WAVPACK_MODES: &[&str] = &["Schnell", "Normal", "Hohe Qualität", "Sehr hohe Qualität"];
pub const STEREO_MODES: &[&str] = &["Stereo (automatisch)", "Joint Stereo", "Erzwungenes Joint Stereo", "Stereo", "Mono"];

/// External programs used by the formats FFmpeg can't write.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Tools {
    pub ffmpeg: String,
    pub mac: String,
    pub mpcenc: String,
    pub sox: String,
}

pub struct Env<'a> {
    /// The FFmpeg for each job (see `ffmpeg::FfmpegSet`).
    pub ffmpeg: &'a FfmpegSet,
    pub tools: &'a Tools,
    pub temp: &'a Path,
}

/// The FFmpeg encoder a format needs; None for formats FFmpeg does not write.
pub fn ffmpeg_encoder(c: Codec) -> Option<&'static str> {
    Some(match c {
        Codec::Mp3 => "libmp3lame",
        Codec::Flac => "flac",
        Codec::AacM4a | Codec::AudioBook => "aac",
        Codec::Alac => "alac",
        Codec::Opus => "libopus",
        Codec::Vorbis => "libvorbis",
        Codec::WavPack => "wavpack",
        Codec::Wma => "wmav2",
        Codec::Speex => "libspeex",
        _ => return None,
    })
}

/// The FFmpeg that writes format `c`.
pub fn ffmpeg_for<'a>(c: Codec, env: &Env<'a>) -> Option<&'a Ffmpeg> {
    ffmpeg_encoder(c).and_then(|enc| env.ffmpeg.for_encoder(enc))
}

/// What a format needs to work on this computer (None = ready).
pub fn missing_requirement(c: Codec, env: &Env) -> Option<String> {
    if let Some(enc) = ffmpeg_encoder(c) {
        return match env.ffmpeg.for_encoder(enc) {
            None => Some(crate::i18n::t("FFmpeg wird benötigt (Einstellungen » Encoder-Programme)").into()),
            Some(f) if !f.has_encoder(enc) => {
                Some(crate::i18n::tf(crate::i18n::t("Das gefundene FFmpeg hat keinen Encoder „{enc}“"), &[("enc", enc.to_string())]))
            }
            Some(_) => None,
        };
    }
    match c {
        Codec::Wave | Codec::Aiff => None,
        Codec::MonkeysAudio => {
            if audio::find_tool(&env.tools.mac, "mac").is_none() {
                Some(crate::i18n::t("Programm mac.exe angeben (Einstellungen)").into())
            } else {
                None
            }
        }
        Codec::Musepack => {
            if audio::find_tool(&env.tools.mpcenc, "mpcenc").is_none() {
                Some(crate::i18n::t("Programm mpcenc.exe angeben (Einstellungen)").into())
            } else {
                None
            }
        }
        Codec::Dsd => {
            if audio::find_tool(&env.tools.sox, "sox").is_none() {
                Some(crate::i18n::t("Programm sox.exe angeben (Einstellungen)").into())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Rough output size estimate in bytes per second (for display).
pub fn estimate_bps(c: Codec, s: &EncSettings, rate: u32, ch: usize, bits: u16) -> f64 {
    let pcm = rate as f64 * ch as f64 * bits as f64 / 8.0;
    match c {
        Codec::Mp3 => match s.mp3_mode {
            RateMode::Vbr => [245.0, 225.0, 190.0, 175.0, 165.0, 130.0, 115.0, 100.0, 85.0, 65.0][s.mp3_vbr.min(9) as usize] * 125.0,
            _ => s.mp3_kbps as f64 * 125.0,
        },
        Codec::AacM4a | Codec::AudioBook => {
            if s.aac_vbr {
                AAC_VBR_KBPS[(s.aac_quality.clamp(1, 5) - 1) as usize] as f64 * 125.0
            } else {
                s.aac_kbps as f64 * 125.0
            }
        }
        Codec::Opus => s.opus_kbps as f64 * 125.0,
        Codec::Vorbis if s.vorbis_mode > 0 => s.vorbis_kbps as f64 * 125.0,
        Codec::Vorbis => {
            let i = (s.vorbis_q.round().clamp(-1.0, 10.0) + 1.0) as usize;
            VORBIS_KBPS[i] as f64 * 125.0
        }
        Codec::Wma => s.wma_kbps as f64 * 125.0,
        Codec::Musepack => MPC_PROFILES[s.mpc_profile as usize % MPC_PROFILES.len()].2 as f64 * 125.0,
        Codec::Speex => 3000.0,
        Codec::Flac | Codec::Alac | Codec::WavPack | Codec::MonkeysAudio => pcm * 0.58,
        Codec::Dsd => DSD_MODES[s.dsd_mode as usize % DSD_MODES.len()].1 as f64 * ch as f64 / 8.0,
        _ => pcm,
    }
}

fn feed_format(audio: &Audio, lossless_target: bool) -> (PcmFormat, &'static str) {
    if !lossless_target || audio.float {
        (PcmFormat::F32, "f32le")
    } else if audio.bits <= 16 {
        (PcmFormat::S16, "s16le")
    } else {
        (PcmFormat::S32, "s32le")
    }
}

/// FFmpeg arguments for format `c`, for the FFmpeg `ff` that will write it.
fn ffmpeg_args(c: Codec, s: &EncSettings, audio: &Audio, ff: &Ffmpeg) -> Vec<String> {
    let v = |x: &str| x.to_string();
    let mut a: Vec<String> = Vec::new();
    match c {
        Codec::Mp3 => {
            a.extend([v("-c:a"), v("libmp3lame")]);
            match s.mp3_mode {
                RateMode::Vbr => a.extend([v("-q:a"), s.mp3_vbr.min(9).to_string()]),
                RateMode::Cbr => a.extend([v("-b:a"), format!("{}k", s.mp3_kbps)]),
                RateMode::Abr => a.extend([v("-b:a"), format!("{}k", s.mp3_kbps), v("-abr"), v("1")]),
            }
            match s.mp3_stereo {
                1 | 2 => a.extend([v("-joint_stereo"), v("1")]),
                3 => a.extend([v("-joint_stereo"), v("0")]),
                4 => a.extend([v("-ac"), v("1")]),
                _ => {}
            }
            if s.mp3_q_custom {
                a.extend([v("-compression_level"), s.mp3_q.min(9).to_string()]);
            }
            a.extend([v("-id3v2_version"), v("3")]);
        }
        Codec::Flac => {
            a.extend([v("-c:a"), v("flac"), v("-compression_level"), s.flac_level.min(8).to_string()]);
            if audio.bits > 16 && !audio.float {
                a.extend([v("-sample_fmt"), v("s32"), v("-bits_per_raw_sample"), audio.bits.min(24).to_string()]);
            }
            if audio.float {
                a.extend([v("-sample_fmt"), v("s32"), v("-bits_per_raw_sample"), v("24")]);
            }
        }
        Codec::AacM4a | Codec::AudioBook => {
            // only a listed fdk: an unknown encoder list must not pick it
            if ff.encoders.iter().any(|e| e == "libfdk_aac") {
                a.extend([v("-c:a"), v("libfdk_aac")]);
                if s.aac_vbr {
                    a.extend([v("-vbr"), s.aac_quality.clamp(1, 5).to_string()]);
                } else {
                    a.extend([v("-b:a"), format!("{}k", s.aac_kbps)]);
                }
                match s.aac_profile {
                    1 => a.extend([v("-profile:a"), v("aac_he")]),
                    2 => a.extend([v("-profile:a"), v("aac_he_v2")]),
                    _ => {}
                }
            } else {
                let kbps = if s.aac_vbr { AAC_VBR_KBPS[(s.aac_quality.clamp(1, 5) - 1) as usize] } else { s.aac_kbps };
                a.extend([v("-c:a"), v("aac"), v("-b:a"), format!("{kbps}k")]);
            }
            a.extend([v("-f"), v("ipod")]);
        }
        Codec::Alac => {
            a.extend([v("-c:a"), v("alac"), v("-f"), v("ipod")]);
            if audio.bits > 16 || audio.float {
                a.extend([v("-sample_fmt"), v("s32p")]);
            }
        }
        Codec::Opus => {
            a.extend([v("-c:a"), v("libopus"), v("-b:a"), format!("{}k", s.opus_kbps)]);
            a.extend([v("-vbr"), v(["on", "constrained", "off"][s.opus_mode as usize % 3])]);
        }
        Codec::Vorbis => {
            a.extend([v("-c:a"), v("libvorbis")]);
            match s.vorbis_mode {
                1 => a.extend([v("-b:a"), format!("{}k", s.vorbis_kbps)]),
                2 => {
                    let k = format!("{}k", s.vorbis_kbps);
                    a.extend([v("-b:a"), k.clone(), v("-minrate"), k.clone(), v("-maxrate"), k]);
                }
                _ => a.extend([v("-q:a"), format!("{:.1}", s.vorbis_q)]),
            }
        }
        Codec::WavPack => {
            let lvl = [1, 2, 3, 5][s.wavpack_mode as usize % 4];
            a.extend([v("-c:a"), v("wavpack"), v("-compression_level"), lvl.to_string()]);
        }
        Codec::Wma => a.extend([v("-c:a"), v("wmav2"), v("-b:a"), format!("{}k", s.wma_kbps)]),
        Codec::Speex => {
            a.extend([v("-c:a"), v("libspeex"), v("-cbr_quality"), s.speex_q.min(10).to_string(), v("-ar"), v("32000")]);
            if audio.channels > 2 {
                a.extend([v("-ac"), v("2")]);
            }
            a.extend([v("-f"), v("ogg")]);
        }
        _ => {}
    }
    a
}

// one call per file with everything FFmpeg needs; a struct would only rename the parameters
#[allow(clippy::too_many_arguments)]
fn run_ffmpeg(
    ff: &Path,
    audio: &Audio,
    out: &Path,
    args: &[String],
    meta: &[String],
    lossless_target: bool,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
) -> Result<(), String> {
    let (fmt, name) = feed_format(audio, lossless_target);
    let mut cmd = Command::new(ff);
    cmd.args(["-v", "error", "-nostdin", "-y", "-f", name, "-ar"])
        .arg(audio.rate.to_string())
        .arg("-ac")
        .arg(audio.channels.to_string())
        .args(["-i", "-"])
        .args(meta)
        .args(args)
        .arg(out)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    hidden(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| crate::i18n::tf(crate::i18n::t("FFmpeg kann nicht gestartet werden: {e}"), &[("e", format!("{e}"))]))?;
    let mut stderr = child.stderr.take().unwrap();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut stderr, &mut s);
        s
    });
    {
        let mut stdin = child.stdin.take().unwrap();
        let block = 65536usize;
        let total = audio.data.len().max(1);
        let mut buf = Vec::with_capacity(block * 4);
        for (i, chunk) in audio.data.chunks(block).enumerate() {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait(); // the process must be gone before its temp file is removed
                return Err(crate::i18n::t("abgebrochen").into());
            }
            buf.clear();
            audio::pcm_bytes_le(chunk, fmt, &mut buf);
            if stdin.write_all(&buf).is_err() {
                break;
            }
            progress(((i + 1) * block).min(total) as f32 / total as f32);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let err = err_thread.join().unwrap_or_default();
    if !status.success() {
        let _ = std::fs::remove_file(out);
        return Err(crate::i18n::tf(crate::i18n::t("Encoder-Fehler: {0}"), &[("0", err.lines().last().unwrap_or("unbekannt").to_string())]));
    }
    Ok(())
}

/// Run an external encoder; Cancel stops it within a tenth of a second.
fn run_cli(exe: &Path, args: &[String], cancel: &AtomicBool) -> Result<(), String> {
    let mut cmd = Command::new(exe);
    cmd.args(args).stdout(Stdio::null()).stderr(Stdio::piped()).stdin(Stdio::null());
    hidden(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| crate::i18n::tf(crate::i18n::t("{0} kann nicht gestartet werden: {e}"), &[("0", format!("{}", exe.display())), ("e", format!("{e}"))]))?;
    let mut stderr = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr.as_mut() {
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(crate::i18n::t("abgebrochen").into());
        }
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let err = err_thread.join().unwrap_or_default();
    if !status.success() {
        return Err(crate::i18n::tf(
            crate::i18n::t("{0} meldet: {1}"),
            &[("0", format!("{}", exe.display())), ("1", err.lines().last().unwrap_or(crate::i18n::t("Fehler")).to_string())],
        ));
    }
    Ok(())
}

#[cfg(all(test, windows))]
mod cli_tests {
    #[test]
    fn an_external_encoder_stops_on_cancel() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            c.store(true, Ordering::Relaxed);
        });
        let t0 = std::time::Instant::now();
        let args: Vec<String> = ["/C", "ping", "-n", "30", "127.0.0.1"].iter().map(|s| s.to_string()).collect();
        let r = super::run_cli(std::path::Path::new("cmd"), &args, &cancel);
        assert!(r.is_err());
        assert!(t0.elapsed().as_secs() < 5, "took {:?}", t0.elapsed());
    }
}

fn temp_wav(env: &Env, audio: &Audio, bits: u16) -> Result<PathBuf, String> {
    static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed);
    let p = env.temp.join(format!("psyaudio-{}-{n}.wav", std::process::id()));
    audio::write_wav(&p, audio, PcmFormat::from_bits(bits, false))?;
    Ok(p)
}

/// Write `audio` to `out` in codec `c`.
#[allow(clippy::too_many_arguments)]
pub fn encode(
    c: Codec,
    s: &EncSettings,
    audio: &Audio,
    out: &Path,
    meta: &[String],
    env: &Env,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
) -> Result<(), String> {
    if let Some(m) = missing_requirement(c, env) {
        return Err(m);
    }
    match c {
        Codec::Wave => {
            let fmt = if s.wav_match_source { PcmFormat::from_bits(audio.bits, audio.float) } else { s.wav_format };
            audio::write_wav(out, audio, fmt)?;
            progress(1.0);
            Ok(())
        }
        Codec::Aiff => {
            let bits = if s.aiff_match_source { audio.bits } else { s.aiff_bits };
            audio::write_aiff(out, audio, bits)?;
            progress(1.0);
            Ok(())
        }
        Codec::MonkeysAudio => {
            let exe = audio::find_tool(&env.tools.mac, "mac").unwrap();
            let wav = temp_wav(env, audio, audio.bits.min(24))?;
            let r = run_cli(&exe, &[wav.display().to_string(), out.display().to_string(), APE_LEVELS[s.ape_level as usize % 5].1.to_string()], cancel);
            let _ = std::fs::remove_file(&wav);
            progress(1.0);
            r
        }
        Codec::Musepack => {
            let exe = audio::find_tool(&env.tools.mpcenc, "mpcenc").unwrap();
            let wav = temp_wav(env, audio, 16)?;
            let mut args: Vec<String> = MPC_PROFILES[s.mpc_profile as usize % MPC_PROFILES.len()].1.split_whitespace().map(|x| x.to_string()).collect();
            args.push("--overwrite".into());
            args.push(wav.display().to_string());
            args.push(out.display().to_string());
            let r = run_cli(&exe, &args, cancel);
            let _ = std::fs::remove_file(&wav);
            progress(1.0);
            r
        }
        Codec::Dsd => {
            let exe = audio::find_tool(&env.tools.sox, "sox").unwrap();
            let mut a2 = audio.clone();
            if a2.channels > 6 {
                a2 = crate::dsp::resample(&a2, a2.rate);
            }
            let wav = temp_wav(env, &a2, 24)?;
            let rate = DSD_MODES[s.dsd_mode as usize % DSD_MODES.len()].1;
            let args = vec![
                wav.display().to_string(),
                "--no-glob".into(),
                "-t".into(),
                "dsf".into(),
                out.display().to_string(),
                "rate".into(),
                "-v".into(),
                rate.to_string(),
                "sdm".into(),
            ];
            let r = run_cli(&exe, &args, cancel);
            let _ = std::fs::remove_file(&wav);
            progress(1.0);
            r
        }
        _ => {
            let ff = ffmpeg_for(c, env).ok_or(crate::i18n::t("FFmpeg wird benötigt"))?;
            let args = ffmpeg_args(c, s, audio, ff);
            run_ffmpeg(&ff.path, audio, out, &args, meta, c.lossless(), cancel, progress)?;
            let verify = match c {
                Codec::Flac => s.flac_verify,
                Codec::WavPack => s.wavpack_verify,
                Codec::Alac => s.alac_verify,
                _ => false,
            };
            if verify {
                verify_lossless(out, audio, &ff.path, cancel)?;
            }
            Ok(())
        }
    }
}

/// Decode the written file again, with the FFmpeg that wrote it, and compare it with what was encoded.
pub fn verify_lossless(out: &Path, original: &Audio, ffmpeg: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let back = audio::decode(out, Some(ffmpeg), cancel, &|_| {})?;
    if back.frames() != original.frames() || back.channels != original.channels {
        return Err(crate::i18n::tf(
            crate::i18n::t("Prüfung fehlgeschlagen: Länge {0} statt {1} Frames"),
            &[("0", format!("{}", back.frames())), ("1", format!("{}", original.frames()))],
        ));
    }
    let bits = if original.float { 24 } else { original.bits.clamp(8, 24) };
    let tol = 1.5 / (1u64 << (bits - 1)) as f32;
    if let Some(i) = back.data.iter().zip(&original.data).position(|(a, b)| (a - b).abs() > tol) {
        return Err(crate::i18n::tf(crate::i18n::t("Prüfung fehlgeschlagen: Abweichung bei Sample {i}"), &[("i", format!("{i}"))]));
    }
    Ok(())
}

/// Why a run with `codec` cannot start (a program or encoder is missing), or nothing.
pub fn plan_requirement(codec: Codec, enc: &EncSettings, env: &Env) -> Option<String> {
    let codecs = match codec {
        Codec::Multi => enc.multi.clone(),
        Codec::ChannelSplit | Codec::LengthSplit => vec![enc.split_codec],
        c if c.is_tool() => vec![],
        c => vec![c],
    };
    if codec == Codec::Multi && codecs.is_empty() {
        return Some(crate::i18n::t("Im Mehrfach-Encoder ist kein Format gewählt").into());
    }
    codecs.into_iter().find_map(|c| missing_requirement(c, env).map(|m| format!("{}: {m}", c.name())))
}

#[cfg(test)]
mod ffmpeg_choice_tests {
    use super::*;
    use crate::ffmpeg::{Ffmpeg, FfmpegSet, Source};

    fn ff(source: Source, encoders: &[&str]) -> Ffmpeg {
        Ffmpeg { path: PathBuf::from(format!("{source:?}")), source, encoders: encoders.iter().map(|e| e.to_string()).collect(), probe: true, hdcd: true }
    }

    fn env<'a>(set: &'a FfmpegSet, tools: &'a Tools) -> Env<'a> {
        Env { ffmpeg: set, tools, temp: Path::new("") }
    }

    #[test]
    fn each_codec_gets_its_own_ffmpeg() {
        let set = FfmpegSet { main: Some(ff(Source::System, &["libmp3lame", "aac"])), fallback: Some(ff(Source::Bundled, &["libmp3lame", "aac", "libspeex"])) };
        let tools = Tools::default();
        let env = env(&set, &tools);
        assert_eq!(missing_requirement(Codec::Speex, &env), None);
        assert_eq!(ffmpeg_for(Codec::Speex, &env).map(|f| f.source), Some(Source::Bundled));
        assert_eq!(ffmpeg_for(Codec::Mp3, &env).map(|f| f.source), Some(Source::System));
        assert!(ffmpeg_for(Codec::Wave, &env).is_none(), "WAV needs no FFmpeg");
    }

    #[test]
    fn a_configured_ffmpeg_without_the_encoder_reports_it() {
        let set = FfmpegSet { main: Some(ff(Source::Configured, &["libmp3lame", "aac"])), fallback: Some(ff(Source::Bundled, &["libspeex"])) };
        let tools = Tools::default();
        let why = missing_requirement(Codec::Speex, &env(&set, &tools)).expect("Speex is missing");
        assert!(why.contains("libspeex"), "{why}");
        assert!(missing_requirement(Codec::Mp3, &env(&FfmpegSet::default(), &tools)).is_some(), "no FFmpeg at all");
    }

    #[test]
    fn aac_uses_fdk_only_when_the_chosen_ffmpeg_has_it() {
        let audio = Audio { rate: 44100, channels: 2, bits: 16, float: false, lossless: true, data: Vec::new() };
        let s = EncSettings::default();
        let fdk = ff(Source::System, &["aac", "libfdk_aac"]);
        assert!(ffmpeg_args(Codec::AacM4a, &s, &audio, &fdk).iter().any(|a| a == "libfdk_aac"));
        let plain = ff(Source::Bundled, &["aac"]);
        let args = ffmpeg_args(Codec::AacM4a, &s, &audio, &plain);
        assert!(!args.iter().any(|a| a == "libfdk_aac") && args.iter().any(|a| a == "aac"), "{args:?}");
    }
}
