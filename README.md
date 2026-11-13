# PsyAudio

Audio toolbox for Windows and Linux: converter, batch converter and a secure CD ripper (AccurateRip, MusicBrainz, Cover Art Archive). The window is built with Tauri 2 and React; all audio work happens in a Rust core.

## Build and run

| Task | Command |
|---|---|
| Install | `npm ci` |
| Run with live reload | `npm run tauri dev` (starts Vite on port 1420 and the window) |
| Debug build without Vite | `npx tauri build --debug --no-bundle` (frontend embedded, `target/debug/PsyAudio.exe`) |
| Frontend checks | `npm run check` (Biome and TypeScript), `npm test` (Vitest) |
| Rust checks | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` |

`cargo test` includes a real conversion into several formats and needs FFmpeg on the `PATH`. It also writes the TypeScript types of the settings into `src/ipc/bindings` (ts-rs); CI fails when they differ from the committed files.

Settings are stored in `%APPDATA%\PsyAudio\settings.json` (Linux: `~/.config/PsyAudio/settings.json`).

## Sample mode and screenshots

`PsyAudio --demo=<konverter|stapel|cd|einstellungen> [--light]` opens a page with made-up sample content, in German, dark unless `--light`. Nothing is saved and no other PsyAudio window is asked. Any other page name opens the converter without sample files, which is handy for test runs with files passed on the command line.

`node scripts/capture.mjs <page> <file.png> [options]` starts the debug build in sample mode and saves a screenshot through the WebView2 DevTools protocol (Windows). Options, run in order: `--light`, `--file=<path>`, `--click=<css>`, `--eval=<js>`, `--wait=<css>`, `--close` (the window's close button), `--expect-exit`, `--title`.

## External programs

- WAV and AIFF are written by PsyAudio itself; decoding uses `symphonia`.
- MP3, FLAC, AAC, ALAC, Opus, Vorbis, WavPack, WMA and Speex go through the **FFmpeg command line**: PCM is piped in on stdin (`encode::run_ffmpeg`). Formats `symphonia` cannot read are decoded by FFmpeg as well.
- Monkey's Audio, Musepack and DSD need `mac`, `mpcenc` and `sox`; their paths are set on the settings page.
- HDCD decoding uses FFmpeg's `hdcd` filter.

## Layout

| Folder | Contents |
|---|---|
| `crates/core` | `psyaudio-core`: everything except the window (conversion, effects, tags, naming, CD ripping, settings, translation) |
| `src-tauri` | The Tauri app: commands (`*_cmd.rs`), background work (`jobs.rs`, `run_cmd.rs`), close guard (`close.rs`), single instance, drag and drop |
| `src` | The React frontend: `shell/` (sidebar, step tiles, side column), `pages/` (convert, batch, cd, settings), `ui/` (controls), `store/` (zustand), `ipc/` (commands and generated types) |
| `tools` | `i18n_dict.tsv` and its generator |
| `scripts` | `capture.mjs` |

### Core (`crates/core/src/`)

| File | Contents |
|---|---|
| `settings.rs` | `Settings`, `RipSettings`, `UiState` (serde, `#[serde(default)]`), profiles, load, salvage and save, reset |
| `convert.rs` | Conversion pipeline: `Plan`, `Job`, thread pool, `run_job` (decode → effects → encode → tags), album ReplayGain, playlists |
| `audio.rs` | `Audio` (interleaved f32), decoding with symphonia or FFmpeg, ffprobe, size and time formatting |
| `encode.rs` | `Codec`, `EncSettings`, FFmpeg arguments per format, external encoders, lossless verification, size estimate |
| `dsp.rs` | About 35 effects (`Effect`, `catalogue()`, `catalogue_entries()` with a description each, `apply()`) |
| `loudness.rs` | EBU R128 and ReplayGain (−18 LUFS) |
| `naming.rs` | Folder and file names built from chips, naming presets |
| `tags.rs` | Reading and writing tags with `lofty` |
| `files.rs` | Adding files and folders, existing targets |
| `rip.rs` | The rip worker: statuses (`TrackStatus`, `ArResult`, `RipPhase`, `RipSummary`) as codes, log, CUE, folder.jpg |
| `cdrip.rs` | CD: TOC (Windows IOCTL, Linux CD-ROM ioctls), freedb and MusicBrainz disc IDs, AccurateRip IDs and CRCs (v1/v2), MusicBrainz and Cover Art Archive lookups, eject |
| `cdrip_read.rs` | Secure reading: passes, re-reads, cache flushing, interpolation, automatic drive offset search |
| `integration.rs` | Explorer (per extension) and Dolphin context menu, keeping the computer awake |
| `i18n.rs` / `i18n_dict.rs` | Translation; `i18n_dict.rs` is generated |
| `timefmt.rs` | Durations and encoding speed in words |

## Commands

Every command the frontend may call is listed in `src-tauri/build.rs` (`COMMANDS`) and granted in `src-tauri/capabilities/main.json` (`allow-…`). A new command needs both entries. Long work (converting, ripping, the offset search, MusicBrainz) runs in threads and reports through Tauri channels; statuses travel as codes and are worded in the frontend.

## Translation

- Visible texts are written in **German** and the German text is the key, in Rust (`i18n::t("…")`, `tf(…)`) and in the frontend (`t("…")`, placeholders `{name}`).
- English lives in `tools/i18n_dict.tsv` (German, tab, English). Regenerate both tables with `python -I tools/i18n_gen.py tools/i18n_dict.tsv crates/core/src/i18n_dict.rs src/locales/en.json`.
- Tests check that every `t("…")` of the frontend has an English entry and that translations keep the `{…}` placeholders.

## Conventions

- Code comments are in English.
- Colours come from the Catppuccin tokens in `src/styles/tokens.css` (Mocha and Latte, accent Mauve); a test checks text contrast.
- Every option in a side column gets a one-line hint; units sit inside the value field ("192 kbit/s").
