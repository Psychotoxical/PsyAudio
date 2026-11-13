# PsyAudio

Audio toolbox for Windows, written in Rust with eframe/egui: converter, batch converter, secure CD ripper (AccurateRip, MusicBrainz, Cover Art Archive) and tag editor.

## Build and run

| Task | Command |
|---|---|
| Build | `cargo build --workspace` (dev profile with `opt-level = 2` and `incremental = false`) |
| Tests | `cargo test --workspace` (includes a real conversion into several formats; needs FFmpeg on the `PATH`) |
| Screenshot | `PsyAudio-egui.exe --screenshot out.png` draws about 120 frames, saves the PNG and exits |
| Test pages | Only in screenshot mode: `PSYAUDIO_PAGE=cd\|settings\|tags\|batch` (`cd` shows a sample disc with rip rows), `PSYAUDIO_LIGHT=1`, `PSYAUDIO_LANG=de\|en` |

The workspace has two crates: `crates/core` (`psyaudio-core`, everything except drawing) and `egui` (`psyaudio-egui`, the current window, built as `PsyAudio-egui.exe`). The egui window is being replaced by a Tauri interface.

If `PsyAudio-egui.exe` is still running, linking fails with `os error 5`. Rename `target/debug/PsyAudio-egui.exe` and build again.

Settings are stored in `%APPDATA%\PsyAudio\settings.json` (not in screenshot mode).

## External programs

- WAV and AIFF are written by PsyAudio itself; decoding uses `symphonia`.
- MP3, FLAC, AAC, ALAC, Opus, Vorbis, WavPack, WMA and Speex go through the **FFmpeg command line**: PCM is piped in on stdin (`encode::run_ffmpeg`). Formats `symphonia` cannot read are decoded by FFmpeg as well.
- Monkey's Audio, Musepack and DSD need `mac.exe`, `mpcenc.exe` and `sox.exe`; their paths are set on the settings page.
- HDCD decoding uses FFmpeg's `hdcd` filter.

## Source (`crates/core/src/` and `egui/src/`)

| File | Contents |
|---|---|
| `main.rs` | Entry point: arguments (`--screenshot`, files), window 1360×860, app icon |
| `app.rs` | `PsyApp` (all state), `update()` loop, sidebar, converter page, output panel, overwrite dialog, progress and summary |
| `pages.rs` | Batch, CD ripper (`cd_page`, `rip_options`, `rip_thread` with log, CUE and folder.jpg), tags, settings |
| `widgets.rs` | Shared UI parts: codec settings, effect editor, folder field, slider rows, Explorer context menu |
| `theme.rs` | Colour palettes and button styles |
| `i18n.rs` / `i18n_dict.rs` | Translation: `t("German text")`, `tf(template, &[(key, value)])`; `i18n_dict.rs` is generated |
| `settings.rs` | `Settings` and `RipSettings` (serde, `#[serde(default)]`), profiles, load and save |
| `convert.rs` | Conversion pipeline: `Plan`, `Job`, thread pool, `run_job` (decode → effects → encode → tags), album ReplayGain, playlists |
| `audio.rs` | `Audio` (interleaved f32), decoding with symphonia or FFmpeg, ffprobe, size and time formatting |
| `encode.rs` | `Codec`, `EncSettings`, FFmpeg arguments per format, external encoders, lossless verification, size estimate |
| `dsp.rs` | About 35 effects (`Effect`, `catalogue()`, `apply()`): normalize, ReplayGain, resample, EQ, compressor, HDCD and more |
| `loudness.rs` | EBU R128 and ReplayGain (−18 LUFS) |
| `naming.rs` | Folder and file names built from chips (album artist, title, separators, new folder) |
| `tags.rs` | Reading and writing tags with `lofty` |
| `cdrip.rs` | CD: TOC via Windows IOCTL, freedb and MusicBrainz disc IDs, AccurateRip IDs and CRCs (v1/v2), MusicBrainz and Cover Art Archive lookups, eject |
| `cdrip_read.rs` | Secure reading: passes, re-reads, cache flushing, interpolation, automatic drive offset search |

`build.rs` embeds `assets/PsyAudio.ico` and the version info into the exe.

## Translation

- Visible texts are written in **German** in the code and wrapped in `crate::i18n::t("…")` or `tf(...)`. The German text is the key.
- English translations live in `tools/i18n_dict.tsv` (one line: German, tab, English). Regenerate the table with `python -I tools/i18n_gen.py tools/i18n_dict.tsv crates/core/src/i18n_dict.rs`.
- A test checks that every translation keeps the same `{…}` placeholders as the German text.
- Texts in constant arrays (e.g. `tags::FIELDS`, `naming::label`, `dsp::NORMALIZE_MODES`) are translated with `t()` where they are shown.

## Conventions

- Code comments are in English.
- UI: side panels use `side_panel_width()`, drop-downs `dropdown_height()`, folder fields `folder_field()`, buttons `theme::primary` or `theme::ghost`.
- Long-running work (converting, ripping, offset search, MusicBrainz) runs in threads; messages come back over `mpsc` channels that `PsyApp::poll` drains every frame.
