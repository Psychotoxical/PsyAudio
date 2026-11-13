//! Formats and the programs they need: lists for the format area, what is missing, FFmpeg.

use crate::state::AppState;
use psyaudio_core::audio::{self, PcmFormat};
use psyaudio_core::encode::{self, Codec, EncSettings, Env};
use psyaudio_core::settings::Settings;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::State;

/// FFmpeg as found for the configured path, with its encoders (asking FFmpeg takes a moment).
#[derive(Clone, Default)]
pub struct ToolCache {
    configured: String,
    pub(crate) ffmpeg: Option<PathBuf>,
    pub(crate) encoders: Vec<String>,
}

#[derive(Serialize)]
pub struct CodecInfo {
    codec: Codec,
    name: &'static str,
    short: &'static str,
    lossless: bool,
    tool: bool,
    description: &'static str,
}

#[derive(Serialize)]
pub struct MpcProfile {
    name: &'static str,
    kbps: u32,
}

#[derive(Serialize)]
pub struct PcmChoice {
    format: PcmFormat,
    label: &'static str,
}

/// Everything the format area offers, in display order (labels in the current language).
#[derive(Serialize)]
pub struct EncodeOptions {
    codecs: Vec<CodecInfo>,
    lossless: Vec<Codec>,
    compressed: Vec<Codec>,
    tools: Vec<Codec>,
    audio_codecs: Vec<Codec>,
    mp3_rates: &'static [u32],
    aac_rates: &'static [u32],
    aac_vbr_kbps: Vec<u32>,
    vorbis_kbps: Vec<u32>,
    stereo_modes: Vec<&'static str>,
    wavpack_modes: Vec<&'static str>,
    ape_levels: Vec<&'static str>,
    dsd_modes: Vec<&'static str>,
    mpc_profiles: Vec<MpcProfile>,
    pcm_formats: Vec<PcmChoice>,
}

const LOSSLESS: [Codec; 6] = [Codec::Flac, Codec::Alac, Codec::Wave, Codec::Aiff, Codec::WavPack, Codec::MonkeysAudio];
const COMPRESSED: [Codec; 9] = [
    Codec::Mp3,
    Codec::AacM4a,
    Codec::Opus,
    Codec::Vorbis,
    Codec::Wma,
    Codec::AudioBook,
    Codec::Musepack,
    Codec::Speex,
    Codec::Dsd,
];
const TOOLS: [Codec; 9] = [
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

#[tauri::command]
pub fn encode_options() -> EncodeOptions {
    let all = LOSSLESS.iter().chain(COMPRESSED.iter()).chain(TOOLS.iter());
    EncodeOptions {
        codecs: all
            .map(|c| CodecInfo {
                codec: *c,
                name: c.name(),
                short: c.short(),
                lossless: c.lossless(),
                tool: c.is_tool(),
                description: c.description(),
            })
            .collect(),
        lossless: LOSSLESS.to_vec(),
        compressed: COMPRESSED.to_vec(),
        tools: TOOLS.to_vec(),
        audio_codecs: encode::AUDIO_CODECS.to_vec(),
        mp3_rates: encode::MP3_RATES,
        aac_rates: encode::AAC_RATES,
        aac_vbr_kbps: encode::AAC_VBR_KBPS.to_vec(),
        vorbis_kbps: encode::VORBIS_KBPS.to_vec(),
        stereo_modes: encode::STEREO_MODES.to_vec(),
        wavpack_modes: encode::WAVPACK_MODES.to_vec(),
        ape_levels: encode::APE_LEVELS.iter().map(|(n, _)| *n).collect(),
        dsd_modes: encode::DSD_MODES.iter().map(|(n, _)| *n).collect(),
        mpc_profiles: encode::MPC_PROFILES.iter().map(|(n, _, k)| MpcProfile { name: n, kbps: *k }).collect(),
        pcm_formats: [PcmFormat::U8, PcmFormat::S16, PcmFormat::S24, PcmFormat::S32, PcmFormat::F32, PcmFormat::F64]
            .into_iter()
            .map(|format| PcmChoice { format, label: format.label() })
            .collect(),
    }
}

/// FFmpeg for the configured path; looked up again when the path changed or `refresh` is set.
pub(crate) fn ffmpeg(state: &AppState, configured: &str, refresh: bool) -> ToolCache {
    let mut cache = state.tools.lock().unwrap_or_else(|e| e.into_inner());
    let stale = cache.as_ref().is_none_or(|c| c.configured != configured);
    if refresh || stale {
        let ffmpeg = audio::find_tool(configured, "ffmpeg");
        let encoders = ffmpeg.as_deref().map(audio::ffmpeg_encoders).unwrap_or_default();
        *cache = Some(ToolCache { configured: configured.to_string(), ffmpeg, encoders });
    }
    cache.clone().unwrap_or_default()
}

#[derive(Serialize)]
pub struct ToolStatus {
    ffmpeg: Option<String>,
    encoders: usize,
}

/// Whether FFmpeg was found (for the sidebar and the settings page).
#[tauri::command]
pub async fn tool_status(state: State<'_, AppState>, settings: Settings, refresh: bool) -> Result<ToolStatus, String> {
    let cache = ffmpeg(&state, &settings.tools.ffmpeg, refresh);
    Ok(ToolStatus { ffmpeg: cache.ffmpeg.map(|p| p.to_string_lossy().into_owned()), encoders: cache.encoders.len() })
}

/// Formats that cannot run with the programs found, with the reason.
#[tauri::command]
pub async fn codec_problems(state: State<'_, AppState>, settings: Settings) -> Result<BTreeMap<String, String>, String> {
    let cache = ffmpeg(&state, &settings.tools.ffmpeg, false);
    let env = Env { ffmpeg: cache.ffmpeg.as_deref(), ffmpeg_encoders: &cache.encoders, tools: &settings.tools, temp: Path::new("") };
    let mut out = BTreeMap::new();
    for c in LOSSLESS.iter().chain(COMPRESSED.iter()) {
        if let Some(why) = encode::missing_requirement(*c, &env) {
            out.insert(format!("{c:?}"), format!("{}: {why}", c.name()));
        }
    }
    Ok(out)
}

/// What the run plan needs that is missing, or nothing (the text the egui version showed).
#[tauri::command]
pub async fn plan_requirement(state: State<'_, AppState>, settings: Settings, codec: Codec) -> Result<Option<String>, String> {
    let cache = ffmpeg(&state, &settings.tools.ffmpeg, false);
    let env = Env { ffmpeg: cache.ffmpeg.as_deref(), ffmpeg_encoders: &cache.encoders, tools: &settings.tools, temp: Path::new("") };
    Ok(encode::plan_requirement(codec, &settings.enc, &env))
}

/// The bit rate a setting gives for CD audio, for "ca. 190 kbit/s".
#[tauri::command]
pub fn estimate_kbps(codec: Codec, enc: EncSettings) -> f64 {
    encode::estimate_bps(codec, &enc, 44100, 2, 16) / 125.0
}

/// Sets the language of messages from Rust (errors, reports, codec names).
#[tauri::command]
pub fn set_language(lang: String) {
    psyaudio_core::i18n::set_lang(&lang);
}

/// One external program: where it was found, if at all.
#[derive(Serialize)]
pub struct Program {
    key: &'static str,
    found: Option<String>,
}

fn program_list(tools: &encode::Tools, ffmpeg: Option<PathBuf>) -> Vec<Program> {
    let show = |p: Option<PathBuf>| p.map(|p| p.to_string_lossy().into_owned());
    vec![
        Program { key: "ffmpeg", found: show(ffmpeg) },
        Program { key: "mac", found: show(audio::find_tool(&tools.mac, "mac")) },
        Program { key: "mpcenc", found: show(audio::find_tool(&tools.mpcenc, "mpcenc")) },
        Program { key: "sox", found: show(audio::find_tool(&tools.sox, "sox")) },
    ]
}

/// The four programs of the settings page, each with where it was found; `refresh` asks again.
#[tauri::command]
pub async fn programs(state: State<'_, AppState>, settings: Settings, refresh: bool) -> Result<Vec<Program>, String> {
    let cache = ffmpeg(&state, &settings.tools.ffmpeg, refresh);
    Ok(program_list(&settings.tools, cache.ffmpeg))
}

#[cfg(test)]
mod program_tests {
    use super::*;
    use psyaudio_core::encode::Tools;

    #[test]
    fn programs_report_each_tool() {
        let exe = std::env::temp_dir().join(format!("psyaudio-mac-{}.exe", std::process::id()));
        std::fs::write(&exe, b"").unwrap();
        let tools = Tools { mac: exe.to_string_lossy().into_owned(), ..Tools::default() };
        let list = program_list(&tools, None);
        let _ = std::fs::remove_file(&exe);
        let keys: Vec<&str> = list.iter().map(|p| p.key).collect();
        assert_eq!(keys, ["ffmpeg", "mac", "mpcenc", "sox"]);
        assert_eq!(list[1].found.as_deref(), Some(exe.to_string_lossy().as_ref()));
    }
}
