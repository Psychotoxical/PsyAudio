//! DSP effects: processing applied between decoding and encoding, plus file actions after conversion.

use crate::audio::Audio;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum Effect {
    Resample {
        rate: u32,
    },
    BitDepth {
        bits: u16,
        dither: bool,
    },
    ChannelCount {
        channels: usize,
    },
    ChannelMapper {
        mode: u8,
    }, // 0 swap, 1 left only, 2 right only
    /// Mode 0 peak to peak, 1 ReplayGain (track), 2 EBU R128 (track),
    /// 3 adaptive EBU R128, 4 adaptive peak, 5 fixed amplification, 6 reduce if above.
    VolumeNormalize {
        #[serde(default)]
        mode: u8,
        #[serde(default)]
        target_db: f32,
        #[serde(default = "default_max_amp")]
        max_amp: f32,
        #[serde(default = "default_window")]
        window_ms: u32,
        #[serde(default)]
        fixed_db: f32,
    },
    ReplayGain {
        target_lufs: f32,
        max_gain: f32,
        album: bool,
        clip_prevention: bool,
        true_peak_ratio: u32,
    },
    ReplayGainApply {
        album: bool,
        limit_peaks: bool,
        missing_db: f32,
    },
    Fade {
        in_ms: u32,
        out_ms: u32,
    },
    Trim {
        start_ms: u32,
        end_ms: u32,
    },
    TrimSilence {
        begin: bool,
        end: bool,
        digital_only: bool,
        threshold_db: f32,
    },
    Reverse,
    Lowpass {
        hz: f32,
    },
    Highpass {
        hz: f32,
    },
    Bandpass {
        low: f32,
        high: f32,
    },
    GraphicEq {
        gains: [f32; 9],
    },
    Compressor {
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
    },
    Karaoke {
        keep_voice: bool,
    },
    Speed {
        percent: f32,
    },
    WriteSilence {
        start_ms: u32,
        end_ms: u32,
    },
    Loop {
        times: u32,
    },
    MinimumLength {
        seconds: f32,
    },
    MaximumLength {
        percent: f32,
        seconds: f32,
    },
    DeEmphasis,
    /// Decode HDCD to 24 bit (only 16 bit / 44.1 kHz / stereo PCM), optionally raised by 6 dB.
    Hdcd {
        #[serde(default)]
        amp_6db: bool,
    },
    // ----- actions around the file -----
    DeleteSource,
    PreserveAttributes,
    FolderJpgPreserve,
    PlaylistWriter {
        kind: u8,
        same_folder: bool,
    }, // 0 m3u8, 1 m3u, 2 pls
    PlaySound,
    WriteMetadataFile,
    RunExternal {
        command: String,
    },
    ConditionalEncoding {
        skip_ext: String,
        copy_instead: bool,
    },
    DeleteDestOnError,
    MoveDestOnError {
        folder: String,
    },
    TagProcessing {
        field: String,
        value: String,
        mode: u8,
    }, // 0 set, 1 remove, 2 set if empty
    CpuThrottle,
}

fn default_max_amp() -> f32 {
    NORMALIZE_MAX_AMP
}
fn default_window() -> u32 {
    NORMALIZE_WINDOW_MS
}

/// Normalize defaults: maximum amplification x8, adaptive window 6000 ms, EBU reference = target - 18.
pub const NORMALIZE_MAX_AMP: f32 = 8.0;
pub const NORMALIZE_WINDOW_MS: u32 = 6000;
pub const NORMALIZE_EBU_OFFSET: f32 = 18.0;
pub const NORMALIZE_MODES: [&str; 7] = [
    "Spitze auf Zielpegel",
    "ReplayGain (Titel)",
    "EBU R128 (Titel)",
    "Adaptiv (EBU R128)",
    "Adaptiv (Spitze)",
    "Feste Verstärkung",
    "Nur absenken, wenn darüber",
];

pub const EQ_BANDS: [f32; 9] = [60.0, 130.0, 260.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

/// One effect as the effect picker shows it (texts in the current language).
#[derive(Clone, Debug, Serialize)]
pub struct EffectEntry {
    pub category: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub effect: Effect,
}

/// The catalogue with one line of explanation per effect, for the searchable picker.
pub fn catalogue_entries() -> Vec<EffectEntry> {
    catalogue()
        .into_iter()
        .map(|(category, label, effect)| EffectEntry { category, label, description: crate::i18n::t(description(&effect)), effect })
        .collect()
}

/// What an effect does, in one German sentence (translated by the caller).
fn description(e: &Effect) -> &'static str {
    match e {
        Effect::Resample { .. } => "Rechnet auf eine andere Abtastrate um, etwa von 96 kHz auf 44,1 kHz.",
        Effect::BitDepth { .. } => "Ändert die Bittiefe; beim Verringern glättet Dither die Rundungsfehler.",
        Effect::ChannelCount { .. } => "Mischt auf Mono, Stereo oder 5.1 um.",
        Effect::ChannelMapper { .. } => "Tauscht links und rechts oder behält nur einen Kanal.",
        Effect::VolumeNormalize { .. } => "Bringt jede Datei auf einen einheitlichen Pegel (Spitze, ReplayGain oder EBU R128).",
        Effect::ReplayGain { .. } => "Misst die Lautheit und schreibt ReplayGain-Werte in die Tags; der Ton bleibt unverändert.",
        Effect::ReplayGainApply { .. } => "Ändert die Lautstärke fest um die ReplayGain-Werte aus den Tags.",
        Effect::Compressor { .. } => "Verringert den Abstand zwischen leisen und lauten Stellen.",
        Effect::Fade { .. } => "Blendet am Anfang ein und am Ende aus.",
        Effect::Trim { .. } => "Schneidet am Anfang und am Ende eine feste Zeit ab.",
        Effect::TrimSilence { .. } => "Entfernt Stille am Anfang und am Ende.",
        Effect::WriteSilence { .. } => "Fügt vorne und hinten Stille an.",
        Effect::Reverse => "Dreht die Aufnahme um, sodass sie rückwärts läuft.",
        Effect::Loop { .. } => "Hängt die Aufnahme mehrmals hintereinander.",
        Effect::Speed { .. } => "Spielt schneller oder langsamer ab; die Tonhöhe ändert sich mit.",
        Effect::MinimumLength { .. } => "Überspringt Dateien, die kürzer sind als die angegebene Zeit.",
        Effect::MaximumLength { .. } => "Kürzt auf eine Höchstlänge in Sekunden oder Prozent.",
        Effect::Lowpass { .. } => "Lässt nur Frequenzen unterhalb der Grenze durch.",
        Effect::Highpass { .. } => "Lässt nur Frequenzen oberhalb der Grenze durch, etwa gegen Rumpeln.",
        Effect::Bandpass { .. } => "Lässt nur den Bereich zwischen zwei Frequenzen durch, etwa für Telefonklang.",
        Effect::GraphicEq { .. } => "Hebt oder senkt neun Frequenzbänder.",
        Effect::Karaoke { .. } => "Entfernt die Stimme aus der Mitte oder behält nur sie; nur bei Stereo.",
        Effect::DeEmphasis => "Gleicht die Höhenanhebung alter CDs mit Pre-Emphasis aus.",
        Effect::Hdcd { .. } => "Erkennt HDCDs und macht aus ihnen 24-Bit-Audio.",
        Effect::DeleteSource => "Löscht die Quelldatei, wenn ihre Umwandlung geklappt hat.",
        Effect::PreserveAttributes => "Gibt der neuen Datei das Änderungsdatum der Quelle.",
        Effect::FolderJpgPreserve => "Kopiert folder.jpg oder cover.jpg aus dem Quellordner in den Zielordner.",
        Effect::PlaylistWriter { .. } => "Schreibt eine Wiedergabeliste (M3U8, M3U oder PLS) mit den neuen Dateien.",
        Effect::WriteMetadataFile => "Legt neben jede neue Datei eine .txt mit ihren Tags.",
        Effect::RunExternal { .. } => "Startet nach jeder Datei ein Programm, etwa zum Nachbearbeiten.",
        Effect::ConditionalEncoding { .. } => "Überspringt oder kopiert Quellen mit einer bestimmten Endung, statt sie umzuwandeln.",
        Effect::DeleteDestOnError => "Löscht eine halbfertige Zieldatei, wenn ein Fehler auftritt.",
        Effect::MoveDestOnError { .. } => "Verschiebt die Zieldatei bei einem Fehler in einen eigenen Ordner.",
        Effect::TagProcessing { .. } => "Setzt, ergänzt oder entfernt ein Tag-Feld in allen Dateien.",
        Effect::PlaySound => "Spielt einen Ton, wenn alle Dateien fertig sind.",
        Effect::CpuThrottle => "Wandelt mit niedriger Priorität um, damit der Rechner bedienbar bleibt.",
    }
}

/// Every effect the "add effect" menu offers: (group, name, default).
pub fn catalogue() -> Vec<(&'static str, &'static str, Effect)> {
    vec![
        ("Format", crate::i18n::t("Abtastrate ändern (Resample)"), Effect::Resample { rate: 44100 }),
        ("Format", crate::i18n::t("Bittiefe"), Effect::BitDepth { bits: 16, dither: true }),
        ("Format", crate::i18n::t("Kanalanzahl"), Effect::ChannelCount { channels: 2 }),
        ("Format", crate::i18n::t("Kanäle zuordnen"), Effect::ChannelMapper { mode: 0 }),
        (
            crate::i18n::t("Lautstärke"),
            crate::i18n::t("Lautstärke normalisieren"),
            Effect::VolumeNormalize { mode: 0, target_db: 0.0, max_amp: NORMALIZE_MAX_AMP, window_ms: NORMALIZE_WINDOW_MS, fixed_db: 0.0 },
        ),
        (
            crate::i18n::t("Lautstärke"),
            crate::i18n::t("ReplayGain (Werte in Tags schreiben)"),
            Effect::ReplayGain { target_lufs: -18.0, max_gain: 25.0, album: true, clip_prevention: true, true_peak_ratio: 1 },
        ),
        (crate::i18n::t("Lautstärke"), crate::i18n::t("ReplayGain anwenden"), Effect::ReplayGainApply { album: false, limit_peaks: true, missing_db: 0.0 }),
        (
            crate::i18n::t("Lautstärke"),
            crate::i18n::t("Dynamikkompression"),
            Effect::Compressor { threshold_db: -3.0, ratio: 1.0, attack_ms: 3.0, release_ms: 1000.0 },
        ),
        (crate::i18n::t("Bearbeiten"), crate::i18n::t("Ein-/Ausblenden (Fade)"), Effect::Fade { in_ms: 0, out_ms: 3000 }),
        (crate::i18n::t("Bearbeiten"), crate::i18n::t("Kürzen (Trim)"), Effect::Trim { start_ms: 0, end_ms: 0 }),
        (
            crate::i18n::t("Bearbeiten"),
            crate::i18n::t("Stille entfernen"),
            Effect::TrimSilence { begin: true, end: true, digital_only: false, threshold_db: -60.0 },
        ),
        (crate::i18n::t("Bearbeiten"), crate::i18n::t("Stille einfügen"), Effect::WriteSilence { start_ms: 0, end_ms: 2000 }),
        (crate::i18n::t("Bearbeiten"), crate::i18n::t("Rückwärts"), Effect::Reverse),
        (crate::i18n::t("Bearbeiten"), crate::i18n::t("Schleife (wiederholen)"), Effect::Loop { times: 2 }),
        (crate::i18n::t("Bearbeiten"), crate::i18n::t("Geschwindigkeit"), Effect::Speed { percent: 100.0 }),
        (crate::i18n::t("Bearbeiten"), crate::i18n::t("Mindestlänge (sonst überspringen)"), Effect::MinimumLength { seconds: 10.0 }),
        (crate::i18n::t("Bearbeiten"), crate::i18n::t("Maximale Länge"), Effect::MaximumLength { percent: 100.0, seconds: 0.0 }),
        ("Filter", crate::i18n::t("Tiefpass"), Effect::Lowpass { hz: 16000.0 }),
        ("Filter", crate::i18n::t("Hochpass"), Effect::Highpass { hz: 40.0 }),
        ("Filter", "Bandpass", Effect::Bandpass { low: 300.0, high: 3400.0 }),
        ("Filter", crate::i18n::t("Grafischer Equalizer"), Effect::GraphicEq { gains: [0.0; 9] }),
        ("Filter", crate::i18n::t("Karaoke (Stimme / Instrumente entfernen)"), Effect::Karaoke { keep_voice: false }),
        ("Filter", "Audio-CD De-Emphasis", Effect::DeEmphasis),
        ("Filter", crate::i18n::t("HDCD-Dekodierung"), Effect::Hdcd { amp_6db: false }),
        (crate::i18n::t("Dateien"), crate::i18n::t("Quelldatei löschen"), Effect::DeleteSource),
        (crate::i18n::t("Dateien"), crate::i18n::t("Quell-Attribute übernehmen (Datum)"), Effect::PreserveAttributes),
        (crate::i18n::t("Dateien"), crate::i18n::t("Folder.jpg mitkopieren"), Effect::FolderJpgPreserve),
        (crate::i18n::t("Dateien"), crate::i18n::t("Wiedergabeliste schreiben"), Effect::PlaylistWriter { kind: 0, same_folder: true }),
        (crate::i18n::t("Dateien"), crate::i18n::t("Metadaten-Datei schreiben (.txt)"), Effect::WriteMetadataFile),
        (crate::i18n::t("Dateien"), crate::i18n::t("Externes Programm ausführen"), Effect::RunExternal { command: String::new() }),
        (crate::i18n::t("Dateien"), crate::i18n::t("Bedingte Umwandlung"), Effect::ConditionalEncoding { skip_ext: "mp3".into(), copy_instead: false }),
        (crate::i18n::t("Dateien"), crate::i18n::t("Zieldatei bei Fehler löschen"), Effect::DeleteDestOnError),
        (crate::i18n::t("Dateien"), crate::i18n::t("Zieldatei bei Fehler verschieben"), Effect::MoveDestOnError { folder: String::new() }),
        (crate::i18n::t("Dateien"), crate::i18n::t("Tag-Bearbeitung"), Effect::TagProcessing { field: "comment".into(), value: String::new(), mode: 1 }),
        ("System", crate::i18n::t("Ton nach der Umwandlung"), Effect::PlaySound),
        ("System", crate::i18n::t("CPU schonen (niedrige Priorität)"), Effect::CpuThrottle),
    ]
}

impl Effect {
    pub fn name(&self) -> String {
        for (_, n, e) in catalogue() {
            if std::mem::discriminant(&e) == std::mem::discriminant(self) {
                return n.to_string();
            }
        }
        crate::i18n::t("Effekt").into()
    }

    /// Short description of the current settings.
    pub fn summary(&self) -> String {
        match self {
            Effect::Resample { rate } => format!("{rate} Hz"),
            Effect::BitDepth { bits, dither } => crate::i18n::tf(
                crate::i18n::t("{bits} Bit{0}"),
                &[("bits", format!("{bits}")), ("0", (if *dither { crate::i18n::t(", Dither") } else { "" }).to_string())],
            ),
            Effect::ChannelCount { channels } => crate::i18n::tf(crate::i18n::t("{channels} Kanal/Kanäle"), &[("channels", format!("{channels}"))]),
            Effect::ChannelMapper { mode } => {
                [crate::i18n::t("L/R tauschen"), crate::i18n::t("nur links"), crate::i18n::t("nur rechts")][*mode as usize % 3].into()
            }
            Effect::VolumeNormalize { mode, target_db, max_amp, fixed_db, .. } => match mode {
                5 => format!("{fixed_db:+.1} dB"),
                1..=3 => format!("{} · {:.0} LUFS · max. ×{max_amp:.0}", crate::i18n::t(NORMALIZE_MODES[*mode as usize]), target_db - NORMALIZE_EBU_OFFSET),
                m => format!("{} · {target_db:.1} dB · max. ×{max_amp:.0}", crate::i18n::t(NORMALIZE_MODES[*m as usize % 7])),
            },
            Effect::ReplayGain { target_lufs, max_gain, album, .. } => {
                format!("{target_lufs:.0} LUFS, max. {max_gain:.0} dB{}", if *album { crate::i18n::t(", Album") } else { "" })
            }
            Effect::ReplayGainApply { album, .. } => {
                if *album {
                    crate::i18n::t("Album-Gain").into()
                } else {
                    crate::i18n::t("Titel-Gain").into()
                }
            }
            Effect::Fade { in_ms, out_ms } => {
                crate::i18n::tf(crate::i18n::t("ein {in_ms} ms, aus {out_ms} ms"), &[("in_ms", format!("{in_ms}")), ("out_ms", format!("{out_ms}"))])
            }
            Effect::Trim { start_ms, end_ms } => crate::i18n::tf(
                crate::i18n::t("Anfang {start_ms} ms, Ende {end_ms} ms"),
                &[("start_ms", format!("{start_ms}")), ("end_ms", format!("{end_ms}"))],
            ),
            Effect::TrimSilence { threshold_db, digital_only, .. } => {
                if *digital_only {
                    crate::i18n::t("nur digitale Stille").into()
                } else {
                    crate::i18n::tf(crate::i18n::t("unter {threshold_db} dB"), &[("threshold_db", format!("{threshold_db:.0}"))])
                }
            }
            Effect::WriteSilence { start_ms, end_ms } => crate::i18n::tf(
                crate::i18n::t("vorne {start_ms} ms, hinten {end_ms} ms"),
                &[("start_ms", format!("{start_ms}")), ("end_ms", format!("{end_ms}"))],
            ),
            Effect::Loop { times } => format!("{times}×"),
            Effect::Speed { percent } => format!("{percent:.0} %"),
            Effect::MinimumLength { seconds } => format!("{seconds:.0} s"),
            Effect::MaximumLength { percent, seconds } => {
                if *seconds > 0.0 {
                    format!("{seconds:.0} s")
                } else {
                    crate::i18n::tf(crate::i18n::t("{percent} % des Originals"), &[("percent", format!("{percent:.0}"))])
                }
            }
            Effect::Lowpass { hz } | Effect::Highpass { hz } => format!("{hz:.0} Hz"),
            Effect::Bandpass { low, high } => format!("{low:.0}–{high:.0} Hz"),
            Effect::GraphicEq { gains } => {
                if gains.iter().all(|g| *g == 0.0) {
                    "flach".into()
                } else {
                    "angepasst".into()
                }
            }
            Effect::Compressor { threshold_db, ratio, .. } => format!("{threshold_db:.0} dB, 1:{ratio:.1}"),
            Effect::Karaoke { keep_voice } => {
                if *keep_voice {
                    crate::i18n::t("Stimme behalten").into()
                } else {
                    crate::i18n::t("Stimme entfernen").into()
                }
            }
            Effect::PlaylistWriter { kind, .. } => ["M3U8", "M3U", "PLS"][*kind as usize % 3].into(),
            Effect::RunExternal { command } => command.clone(),
            Effect::ConditionalEncoding { skip_ext, copy_instead } => {
                format!(".{skip_ext} {}", if *copy_instead { crate::i18n::t("1:1 kopieren") } else { crate::i18n::t("überspringen") })
            }
            Effect::MoveDestOnError { folder } => folder.clone(),
            Effect::TagProcessing { field, value, mode } => match mode {
                1 => crate::i18n::tf(crate::i18n::t("{field} entfernen"), &[("field", field.to_string())]),
                2 => crate::i18n::tf(crate::i18n::t("{field} = {value} (wenn leer)"), &[("field", field.to_string()), ("value", value.to_string())]),
                _ => format!("{field} = {value}"),
            },
            _ => String::new(),
        }
    }

    /// Effects that change the audio (the others act on files).
    /// Effects after which the source's ReplayGain values no longer describe the audio.
    pub fn changes_level(&self) -> bool {
        matches!(
            self,
            Effect::VolumeNormalize { .. }
                | Effect::ReplayGainApply { .. }
                | Effect::Compressor { .. }
                | Effect::GraphicEq { .. }
                | Effect::Karaoke { .. }
                | Effect::ChannelCount { .. }
                | Effect::DeEmphasis
                | Effect::Hdcd { .. }
                | Effect::Lowpass { .. }
                | Effect::Highpass { .. }
                | Effect::Bandpass { .. }
        )
    }

    pub fn is_audio(&self) -> bool {
        !matches!(
            self,
            Effect::DeleteSource
                | Effect::PreserveAttributes
                | Effect::FolderJpgPreserve
                | Effect::PlaylistWriter { .. }
                | Effect::PlaySound
                | Effect::WriteMetadataFile
                | Effect::RunExternal { .. }
                | Effect::ConditionalEncoding { .. }
                | Effect::DeleteDestOnError
                | Effect::MoveDestOnError { .. }
                | Effect::TagProcessing { .. }
                | Effect::CpuThrottle
                | Effect::ReplayGain { .. }
                | Effect::MinimumLength { .. }
        )
    }
}

// ---------------- processing ----------------

fn ms_frames(a: &Audio, ms: u32) -> usize {
    (a.rate as u64 * ms as u64 / 1000) as usize
}

/// Windowed-sinc resampler (Blackman window, 64 taps per side).
pub fn resample(a: &Audio, rate: u32) -> Audio {
    if rate == 0 || rate == a.rate || a.frames() == 0 {
        let mut b = a.clone();
        if rate != 0 {
            b.rate = rate;
        }
        return b;
    }
    let ch = a.channels;
    let ratio = rate as f64 / a.rate as f64;
    let cutoff = ratio.min(1.0) * 0.97;
    let taps: isize = 64;
    let in_frames = a.frames();
    let out_frames = (in_frames as f64 * ratio).round() as usize;
    let mut out = vec![0f32; out_frames * ch];
    let pi = std::f64::consts::PI;
    // polyphase table: 1024 fractional positions x (2*taps) weights, normalised
    const PHASES: usize = 1024;
    let width = (taps * 2) as usize;
    let mut table = vec![0f32; (PHASES + 1) * width];
    for p in 0..=PHASES {
        let frac = p as f64 / PHASES as f64;
        let mut sum = 0.0;
        for j in 0..width {
            let k = j as isize - taps + 1;
            let x = frac - k as f64;
            let s = if x.abs() < 1e-12 { 1.0 } else { (pi * x * cutoff).sin() / (pi * x * cutoff) };
            let n = (x / taps as f64 + 1.0) / 2.0;
            let w = if (0.0..=1.0).contains(&n) { 0.42 - 0.5 * (2.0 * pi * n).cos() + 0.08 * (4.0 * pi * n).cos() } else { 0.0 };
            table[p * width + j] = (s * w) as f32;
            sum += s * w;
        }
        for j in 0..width {
            table[p * width + j] /= sum.max(1e-9) as f32;
        }
    }
    for o in 0..out_frames {
        let pos = o as f64 / ratio;
        let center = pos.floor() as isize;
        let phase = ((pos - center as f64) * PHASES as f64).round() as usize;
        let w = &table[phase * width..(phase + 1) * width];
        for c in 0..ch {
            let mut acc = 0f32;
            for (j, wt) in w.iter().enumerate() {
                let k = center - taps + 1 + j as isize;
                if k >= 0 && (k as usize) < in_frames {
                    acc += a.data[k as usize * ch + c] * wt;
                }
            }
            out[o * ch + c] = acc;
        }
    }
    Audio { rate, ..a.with_data(out) }
}

#[derive(Clone, Copy)]
struct Bq {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Bq {
    fn peaking(rate: f64, f: f64, q: f64, gain_db: f64) -> Bq {
        let a = 10f64.powf(gain_db / 40.0);
        let w = 2.0 * std::f64::consts::PI * f / rate;
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha / a;
        Bq { b0: (1.0 + alpha * a) / a0, b1: -2.0 * w.cos() / a0, b2: (1.0 - alpha * a) / a0, a1: -2.0 * w.cos() / a0, a2: (1.0 - alpha / a) / a0 }
    }
    fn shelf_high(rate: f64, f: f64, gain_db: f64) -> Bq {
        let a = 10f64.powf(gain_db / 40.0);
        let w = 2.0 * std::f64::consts::PI * f / rate;
        let alpha = w.sin() / 2.0 * (2f64).sqrt();
        let cw = w.cos();
        let sa = 2.0 * a.sqrt() * alpha;
        let a0 = (a + 1.0) - (a - 1.0) * cw + sa;
        Bq {
            b0: a * ((a + 1.0) + (a - 1.0) * cw + sa) / a0,
            b1: -2.0 * a * ((a - 1.0) + (a + 1.0) * cw) / a0,
            b2: a * ((a + 1.0) + (a - 1.0) * cw - sa) / a0,
            a1: 2.0 * ((a - 1.0) - (a + 1.0) * cw) / a0,
            a2: ((a + 1.0) - (a - 1.0) * cw - sa) / a0,
        }
    }
    fn run(&self, a: &mut Audio) {
        let ch = a.channels;
        for c in 0..ch {
            let (mut x1, mut x2, mut y1, mut y2) = (0f64, 0f64, 0f64, 0f64);
            for i in 0..a.frames() {
                let x = a.data[i * ch + c] as f64;
                let y = self.b0 * x + self.b1 * x1 + self.b2 * x2 - self.a1 * y1 - self.a2 * y2;
                x2 = x1;
                x1 = x;
                y2 = y1;
                y1 = y;
                a.data[i * ch + c] = y as f32;
            }
        }
    }
}

/// FIR filter (windowed sinc) for low, high and band pass.
fn fir(a: &mut Audio, low: Option<f32>, high: Option<f32>) {
    let rate = a.rate as f64;
    let nyq = rate / 2.0;
    let taps: isize = 255;
    let half = taps / 2;
    let pi = std::f64::consts::PI;
    let lp = |fc: f64, n: isize| -> f64 {
        let x = n as f64;
        if n == 0 {
            2.0 * fc
        } else {
            (2.0 * pi * fc * x).sin() / (pi * x)
        }
    };
    let mut h = vec![0f64; taps as usize];
    for i in 0..taps {
        let n = i - half;
        let hi = high.map(|f| (f as f64).min(nyq * 0.999) / rate);
        let lo = low.map(|f| (f as f64).max(1.0) / rate);
        let v = match (lo, hi) {
            (None, Some(fh)) => lp(fh, n),
            (Some(fl), None) => (if n == 0 { 1.0 } else { 0.0 }) - lp(fl, n),
            (Some(fl), Some(fh)) => lp(fh, n) - lp(fl, n),
            (None, None) => {
                if n == 0 {
                    1.0
                } else {
                    0.0
                }
            }
        };
        let w = 0.42 - 0.5 * (2.0 * pi * i as f64 / (taps - 1) as f64).cos() + 0.08 * (4.0 * pi * i as f64 / (taps - 1) as f64).cos();
        h[i as usize] = v * w;
    }
    let ch = a.channels;
    let frames = a.frames();
    let src = a.data.clone();
    for c in 0..ch {
        for i in 0..frames {
            let mut acc = 0.0;
            for (k, hk) in h.iter().enumerate() {
                let idx = i as isize + half - k as isize;
                if idx >= 0 && (idx as usize) < frames {
                    acc += src[idx as usize * ch + c] as f64 * hk;
                }
            }
            a.data[i * ch + c] = acc as f32;
        }
    }
}

fn change_channels(a: &Audio, n: usize) -> Audio {
    if n == a.channels || n == 0 {
        return a.clone();
    }
    let frames = a.frames();
    let mut out = vec![0f32; frames * n];
    for i in 0..frames {
        let src = &a.data[i * a.channels..(i + 1) * a.channels];
        if n == 1 {
            out[i] = src.iter().sum::<f32>() / a.channels as f32;
        } else if a.channels == 1 {
            for c in 0..n {
                out[i * n + c] = if c < 2 { src[0] } else { 0.0 };
            }
        } else if n == 2 {
            // simple downmix: front L/R plus centre and surrounds at -3 dB
            let g = std::f32::consts::FRAC_1_SQRT_2;
            let mut l = src[0];
            let mut r = src[1];
            if a.channels >= 3 {
                l += g * src[2];
                r += g * src[2];
            }
            if a.channels >= 5 {
                l += g * src[a.channels - 2];
                r += g * src[a.channels - 1];
            }
            let norm = if a.channels >= 3 { 1.0 / (1.0 + 2.0 * g) } else { 1.0 };
            out[i * 2] = l * norm;
            out[i * 2 + 1] = r * norm;
        } else {
            for c in 0..n {
                out[i * n + c] = if c < a.channels { src[c] } else { 0.0 };
            }
        }
    }
    Audio { channels: n, ..a.with_data(out) }
}

/// Simple pseudo random generator for dither.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) - 0.5
    }
}

pub fn quantize(a: &mut Audio, bits: u16, dither: bool) {
    let step = 1.0 / (1u64 << (bits.clamp(8, 32) - 1)) as f32;
    let mut rng = Lcg(0x1234_5678);
    for s in a.data.iter_mut() {
        let d = if dither { (rng.next() + rng.next()) * step } else { 0.0 };
        *s = ((*s + d) / step).round() * step;
    }
    a.bits = bits;
    a.float = false;
}

pub enum Outcome {
    Continue,
    Skip(String),
}

/// Apply one audio effect. `tags` lets ReplayGain-apply read gains.
pub fn apply(e: &Effect, a: &mut Audio, tags: &crate::tags::Tags, original_seconds: f64) -> Outcome {
    let ch = a.channels.max(1);
    match e {
        Effect::Resample { rate } => *a = resample(a, *rate),
        Effect::BitDepth { bits, dither } => quantize(a, *bits, *dither && *bits < a.bits.max(17)),
        Effect::ChannelCount { channels } => *a = change_channels(a, *channels),
        Effect::ChannelMapper { mode } => {
            if ch >= 2 {
                for f in a.data.chunks_exact_mut(ch) {
                    match mode {
                        0 => f.swap(0, 1),
                        1 => f[1] = f[0],
                        _ => f[0] = f[1],
                    }
                }
            }
        }
        Effect::VolumeNormalize { mode, target_db, max_amp, window_ms, fixed_db } => {
            normalize(a, *mode, *target_db, if *max_amp > 0.0 { *max_amp } else { NORMALIZE_MAX_AMP }, *window_ms, *fixed_db);
        }
        Effect::ReplayGainApply { album, limit_peaks, missing_db } => {
            let key = if *album { "replaygain_album_gain" } else { "replaygain_track_gain" };
            let pk = if *album { "replaygain_album_peak" } else { "replaygain_track_peak" };
            let parse = |s: &str| s.trim().trim_end_matches("dB").trim().parse::<f32>().ok();
            let gain_db = parse(tags.get(key)).unwrap_or(*missing_db);
            let mut g = 10f32.powf(gain_db / 20.0);
            if *limit_peaks {
                let peak = parse(tags.get(pk)).unwrap_or_else(|| a.peak());
                if peak * g > 1.0 && peak > 0.0 {
                    g = 1.0 / peak;
                }
            }
            a.data.iter_mut().for_each(|s| *s *= g);
        }
        Effect::Fade { in_ms, out_ms } => {
            let frames = a.frames();
            let fi = ms_frames(a, *in_ms).min(frames);
            let fo = ms_frames(a, *out_ms).min(frames);
            for i in 0..fi {
                let g = i as f32 / fi as f32;
                for c in 0..ch {
                    a.data[i * ch + c] *= g;
                }
            }
            for i in 0..fo {
                let g = i as f32 / fo as f32;
                let f = frames - 1 - i;
                for c in 0..ch {
                    a.data[f * ch + c] *= g;
                }
            }
        }
        Effect::Trim { start_ms, end_ms } => {
            let frames = a.frames();
            let s = ms_frames(a, *start_ms).min(frames);
            let e2 = ms_frames(a, *end_ms).min(frames - s);
            a.data = a.data[s * ch..(frames - e2) * ch].to_vec();
        }
        Effect::TrimSilence { begin, end, digital_only, threshold_db } => {
            let th = if *digital_only { 0.0 } else { 10f32.powf(threshold_db / 20.0) };
            let silent = |f: &[f32]| f.iter().all(|s| s.abs() <= th);
            let frames = a.frames();
            let mut s = 0;
            let mut e2 = frames;
            if *begin {
                while s < frames && silent(&a.data[s * ch..(s + 1) * ch]) {
                    s += 1;
                }
            }
            if *end {
                while e2 > s && silent(&a.data[(e2 - 1) * ch..e2 * ch]) {
                    e2 -= 1;
                }
            }
            a.data = a.data[s * ch..e2 * ch].to_vec();
        }
        Effect::Reverse => {
            let frames = a.frames();
            let src = a.data.clone();
            for i in 0..frames {
                a.data[i * ch..(i + 1) * ch].copy_from_slice(&src[(frames - 1 - i) * ch..(frames - i) * ch]);
            }
        }
        Effect::Lowpass { hz } => fir(a, None, Some(*hz)),
        Effect::Highpass { hz } => fir(a, Some(*hz), None),
        Effect::Bandpass { low, high } => fir(a, Some(*low), Some(*high)),
        Effect::GraphicEq { gains } => {
            for (i, g) in gains.iter().enumerate() {
                if *g != 0.0 && (EQ_BANDS[i] as f64) < a.rate as f64 / 2.0 {
                    Bq::peaking(a.rate as f64, EQ_BANDS[i] as f64, 1.0, *g as f64).run(a);
                }
            }
        }
        Effect::Compressor { threshold_db, ratio, attack_ms, release_ms } => {
            let th = 10f32.powf(threshold_db / 20.0);
            let ratio = ratio.max(1.0);
            let att = (-1.0 / (a.rate as f32 * attack_ms.max(0.1) / 1000.0)).exp();
            let rel = (-1.0 / (a.rate as f32 * release_ms.max(1.0) / 1000.0)).exp();
            let mut env = 0f32;
            for f in a.data.chunks_exact_mut(ch) {
                let lvl = f.iter().fold(0f32, |m, s| m.max(s.abs()));
                env = if lvl > env { att * env + (1.0 - att) * lvl } else { rel * env + (1.0 - rel) * lvl };
                if env > th {
                    let over_db = 20.0 * (env / th).log10();
                    let gain_db = -(over_db - over_db / ratio);
                    let g = 10f32.powf(gain_db / 20.0);
                    f.iter_mut().for_each(|s| *s *= g);
                }
            }
        }
        Effect::Karaoke { keep_voice } => {
            if ch == 2 {
                for f in a.data.as_chunks_mut::<2>().0 {
                    if *keep_voice {
                        let m = (f[0] + f[1]) * 0.5;
                        f[0] = m;
                        f[1] = m;
                    } else {
                        let l = f[0] - f[1];
                        f[0] = l;
                        f[1] = -l;
                    }
                }
            }
        }
        Effect::Speed { percent } => {
            if *percent > 0.0 && (*percent - 100.0).abs() > 0.01 {
                let rate = a.rate;
                let virt = (rate as f32 * 100.0 / percent) as u32;
                let mut b = a.clone();
                b.rate = rate;
                let mut r = resample(&b, virt);
                r.rate = rate;
                *a = r;
            }
        }
        Effect::WriteSilence { start_ms, end_ms } => {
            let s = ms_frames(a, *start_ms) * ch;
            let e2 = ms_frames(a, *end_ms) * ch;
            let mut d = vec![0f32; s];
            d.extend_from_slice(&a.data);
            d.extend(std::iter::repeat_n(0f32, e2));
            a.data = d;
        }
        Effect::Loop { times } => {
            let one = a.data.clone();
            for _ in 1..(*times).max(1) {
                a.data.extend_from_slice(&one);
            }
        }
        Effect::MinimumLength { seconds } => {
            if original_seconds < *seconds as f64 {
                return Outcome::Skip(crate::i18n::tf(crate::i18n::t("kürzer als {seconds} s"), &[("seconds", format!("{seconds:.0}"))]));
            }
        }
        Effect::MaximumLength { percent, seconds } => {
            let frames = a.frames();
            let keep = if *seconds > 0.0 { (a.rate as f32 * seconds) as usize } else { (frames as f32 * percent / 100.0) as usize };
            a.data.truncate(keep.min(frames) * ch);
        }
        Effect::DeEmphasis => {
            // 50/15 µs de-emphasis as a high shelf: -10 dB above ~5.3 kHz (approximation)
            Bq::shelf_high(a.rate as f64, 5283.0, -9.477).run(a);
        }
        _ => {}
    }
    Outcome::Continue
}

/// Gain of one stretch of audio for the normalize modes (linear factor, before the limits).
fn segment_gain(a: &Audio, mode: u8, target_db: f32) -> f32 {
    let target_lin = 10f32.powf(target_db / 20.0);
    match mode {
        1..=3 => {
            let lufs = crate::loudness::integrated(&crate::loudness::block_energies(a)) as f32;
            if lufs <= -69.9 {
                1.0
            } else {
                10f32.powf(((target_db - NORMALIZE_EBU_OFFSET) - lufs) / 20.0)
            }
        }
        _ => {
            let p = a.peak();
            if p > 0.0 {
                target_lin / p
            } else {
                1.0
            }
        }
    }
}

/// Volume Normalize: whole-file modes, adaptive (windowed, gain interpolated between windows),
/// fixed amplification and reduce-if-above. The gain never exceeds `max_amp` and never clips.
pub fn normalize(a: &mut Audio, mode: u8, target_db: f32, max_amp: f32, window_ms: u32, fixed_db: f32) {
    let ch = a.channels.max(1);
    let frames = a.frames();
    if frames == 0 {
        return;
    }
    match mode {
        5 => {
            let g = 10f32.powf(fixed_db / 20.0);
            a.data.iter_mut().for_each(|s| *s = (*s * g).clamp(-1.0, 1.0));
        }
        6 => {
            let p = a.peak();
            let limit = 10f32.powf(target_db / 20.0);
            if p > limit && p > 0.0 {
                let g = limit / p;
                a.data.iter_mut().for_each(|s| *s *= g);
            }
        }
        3 | 4 => {
            let win = ((a.rate as u64 * window_ms.max(100) as u64 / 1000) as usize).max(1);
            let measure_mode = if mode == 3 { 2 } else { 0 };
            let mut centers = Vec::new();
            let mut gains = Vec::new();
            let mut start = 0;
            while start < frames {
                let end = (start + win).min(frames);
                let seg = a.with_data(a.data[start * ch..end * ch].to_vec());
                let peak = seg.peak();
                let mut g = segment_gain(&seg, measure_mode, target_db).min(max_amp);
                if peak > 0.0 && peak * g > 1.0 {
                    g = 1.0 / peak;
                }
                centers.push((start + end) as f32 / 2.0);
                gains.push(g);
                start = end;
            }
            // the first centre right of the frame only moves forward
            let mut k = 0;
            for i in 0..frames {
                let x = i as f32;
                while k < centers.len() && centers[k] <= x {
                    k += 1;
                }
                let g = if k == 0 {
                    gains[0]
                } else if k >= centers.len() {
                    *gains.last().unwrap()
                } else {
                    let t = (x - centers[k - 1]) / (centers[k] - centers[k - 1]);
                    gains[k - 1] + (gains[k] - gains[k - 1]) * t
                };
                for c in 0..ch {
                    a.data[i * ch + c] = (a.data[i * ch + c] * g).clamp(-1.0, 1.0);
                }
            }
        }
        _ => {
            let mut g = segment_gain(a, mode, target_db).min(max_amp);
            let peak = a.peak();
            if peak > 0.0 && peak * g > 1.0 {
                g = 1.0 / peak;
            }
            a.data.iter_mut().for_each(|s| *s *= g);
        }
    }
}

#[cfg(test)]
mod normalize_tests {
    use super::*;
    #[test]
    fn adaptive_mode_interpolates_window_gains() {
        // two windows of 0.1 s: quiet then loud; gains move smoothly between the window centres
        let rate = 1000;
        let mut data = vec![0.1f32; 100];
        data.extend(vec![0.5f32; 100]);
        let mut a = Audio { rate, channels: 1, bits: 16, float: false, lossless: true, data };
        normalize(&mut a, 4, 0.0, 8.0, 100, 0.0);
        // window centres at 50 and 150: gain 10→2 is capped at 8; before 50 constant, after 150 constant
        assert!((a.data[10] - 0.8).abs() < 1e-5, "{}", a.data[10]);
        assert!((a.data[190] - 1.0).abs() < 1e-5, "{}", a.data[190]);
        let mid = a.data[99] / 0.1; // gain at frame 99: 49 % of the way from 8 to 2
        assert!((mid - (8.0 - 6.0 * 0.49)).abs() < 1e-3, "{mid}");
    }

    #[test]
    fn peak_mode_reaches_target_and_respects_max() {
        let mut a = Audio { rate: 44100, channels: 1, bits: 16, float: false, lossless: true, data: vec![0.25, -0.5, 0.1] };
        normalize(&mut a, 0, 0.0, 8.0, 6000, 0.0);
        assert!((a.peak() - 1.0).abs() < 1e-6);
        let mut q = Audio { data: vec![0.01, -0.01], ..a.clone() };
        normalize(&mut q, 0, 0.0, 8.0, 6000, 0.0);
        assert!((q.peak() - 0.08).abs() < 1e-6);
    }
}

/// Only 16-bit, 44100 Hz, 2-channel PCM is passed to FFmpeg's hdcd filter. When HDCD is found
/// the audio becomes 24 bit, and with the 6 dB option every sample is doubled with clipping.
/// Other audio stays as it is.
/// Returns Ok(true) when HDCD was found.
pub fn hdcd_decode(a: &mut Audio, ffmpeg: Option<&std::path::Path>, amp_6db: bool) -> Result<bool, String> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    if a.rate != 44100 || a.channels != 2 || a.bits != 16 || a.float {
        return Ok(false);
    }
    let Some(ff) = ffmpeg else { return Err(crate::i18n::t("FFmpeg wird benötigt").into()) };
    let mut cmd = Command::new(ff);
    cmd.args([
        "-hide_banner",
        "-nostdin",
        "-f",
        "s16le",
        "-ar",
        "44100",
        "-ac",
        "2",
        "-i",
        "pipe:0",
        "-af",
        "hdcd",
        "-c:a",
        "pcm_s32le",
        "-f",
        "s32le",
        "pipe:1",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| crate::i18n::tf(crate::i18n::t("FFmpeg kann nicht gestartet werden: {e}"), &[("e", e.to_string())]))?;
    let pcm: Vec<u8> = a.data.iter().flat_map(|s| ((s * 32768.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes()).collect();
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&pcm);
    });
    let mut err = child.stderr.take().unwrap();
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        s
    });
    let mut out = Vec::new();
    child.stdout.take().unwrap().read_to_end(&mut out).map_err(|e| e.to_string())?;
    let _ = writer.join();
    let log = err_reader.join().unwrap_or_default();
    let _ = child.wait();
    let detected = log.contains("HDCD detected: yes");
    if !detected {
        return Ok(false);
    }
    let gain = if amp_6db { 2.0 } else { 1.0 };
    a.data = out.as_chunks::<4>().0.iter().map(|b| (i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2147483648.0 * gain).clamp(-1.0, 1.0)).collect();
    a.bits = 24;
    Ok(true)
}

#[cfg(test)]
mod hdcd_tests {
    use super::*;
    #[test]
    fn hdcd_only_touches_detected_cd_audio() {
        let sine: Vec<f32> = (0..44100 * 2).map(|i| ((i / 2) as f32 * 0.0627).sin() * 0.5).map(|s| (s * 32768.0).round() / 32768.0).collect();
        let mut a = Audio { rate: 48000, channels: 2, bits: 16, float: false, lossless: true, data: sine.clone() };
        assert_eq!(hdcd_decode(&mut a, None, false), Ok(false)); // not CD format: left alone, FFmpeg not needed
        let Some(ff) = crate::audio::find_tool("", "ffmpeg") else { return };
        let mut b = Audio { rate: 44100, ..a.clone() };
        assert_eq!(hdcd_decode(&mut b, Some(&ff), true), Ok(false));
        assert_eq!(b.bits, 16);
        assert_eq!(b.data, sine);
    }
}

#[cfg(test)]
mod catalogue_tests {
    use super::*;

    #[test]
    fn every_effect_has_category_and_description() {
        let entries = catalogue_entries();
        assert_eq!(entries.len(), catalogue().len());
        let mut categories: Vec<&str> = entries.iter().map(|e| e.category).collect();
        categories.dedup();
        assert_eq!(categories.len(), 6, "{categories:?}");
        let mut seen = std::collections::HashSet::new();
        for e in &entries {
            assert!(!e.label.is_empty() && !e.description.is_empty(), "{}", e.label);
            assert!(seen.insert(e.description), "same description twice: {}", e.description);
        }
    }
}
