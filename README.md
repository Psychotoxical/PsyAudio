<div align="center">

<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/PsyAudio-wordmark.png">
    <img src="assets/PsyAudio-wordmark-black.png" alt="PsyAudio" width="360">
  </picture>
</h1>

**A bit-perfect CD ripper, a converter and a batch converter, in one window.**<br>
For Windows, Linux and macOS.

[![CI](https://github.com/Psychotoxical/PsyAudio/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Psychotoxical/PsyAudio/actions/workflows/ci.yml)
[![License: GPL-3.0](https://img.shields.io/github/license/Psychotoxical/PsyAudio?color=cba6f7)](LICENSE)
![Windows, Linux, macOS](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20Linux%20%C2%B7%20macOS-cba6f7)

[Download](#download) · [CD rips](#bit-perfect-cd-rips) · [Converter](#converter-and-batch-converter) · [Systems](#on-every-system) · [Development](#development)

</div>

![PsyAudio: CD ripper, first-start setup, converter and batch converter, in Mocha and Latte](assets/screenshots.webp)

## Download

Installers and packages are published on the [releases page](https://github.com/Psychotoxical/PsyAudio/releases). What changed in each version is in the [changelog](CHANGELOG.md).

| System | Package |
|---|---|
| Windows 10 and 11 | Installer (`PsyAudio-Setup-<version>.exe`) |
| Linux, any distribution | AppImage |
| Debian, Ubuntu, Mint | `.deb` |
| Fedora, openSUSE | `.rpm` |
| macOS, Apple Silicon and Intel | Disk image (`.dmg`), signed and notarized by Apple |

Every package brings its own [FFmpeg](https://ffmpeg.org/), which PsyAudio uses for most formats, so there is nothing else to install. An FFmpeg already on the computer is preferred, and a path set on the settings page always wins.

## Bit-perfect CD rips

PsyAudio rips a CD so that every file holds exactly the samples on the disc, and it proves it track by track.

- **Verified against the CUETools database (CTDB).** Every track is compared with the checksums of other people's rips of the same disc. A confirmed track is identical, sample for sample, to what they got. The CD header shows how many rips the database knows and links to the disc's entry, so anyone can check.
- **Drive offset measured for you.** Every CD drive reads a tiny bit shifted. PsyAudio measures the shift once with any bought audio CD that is in the database (almost every one is), keeps it per drive and corrects it in every rip. The first-start setup offers the measurement. Without a CD at hand, PsyAudio measures on its own the first time it reads one.
- **Secure reading that stays fast.** Each track is read in one go. When the database confirms it, the track is done. Otherwise a second pass follows and only the spots that differ are read again, until two reads agree. Before a re-read, PsyAudio reads past the drive's buffer (it asks the drive how large it is), so the spot really comes from the disc and not from the drive's memory.
- **Nothing hidden.** A log with the checksums and the database result, a CUE sheet, cover art and tags from MusicBrainz and the Cover Art Archive go into the album folder. A track that could not be confirmed is marked as such, never passed off as good.
- **Any format at once.** Rip to FLAC, ALAC, WAV, AIFF, WavPack, MP3, AAC, Opus and more, or to several formats in one go.

The CUETools database is open: its data is public domain, and lookups need no account or key.

## Converter and batch converter

- **Formats:** MP3, FLAC, AAC, ALAC, M4B, Opus, Vorbis, WavPack, WMA, Speex, WAV, AIFF, Monkey's Audio, Musepack and DSD.
- **36 effects**, ReplayGain and EBU R128 loudness. Tags and covers are carried over, and file and folder names can be built from tags.
- **Batch converter:** converts a whole folder tree and keeps its structure.
- **From the file manager:** "Convert with PsyAudio" in the right-click menu, on every system (see below).
- **Looks:** Catppuccin Mocha (dark) and Latte (light), in English and German.

## On every system

| | Windows | Linux | macOS |
|---|---|---|---|
| CD ripping (CTDB, offset, buffer) | ✓ | ✓ | ✓ |
| Right-click menu | Explorer | Dolphin | Finder (quick action) |
| Open files from outside | Explorer, second start | Dolphin, second start | Open With, Dock |
| Drag and drop | ✓ | ✓ (also Wayland) | ✓ |
| Stays awake while working | ✓ | – | ✓ |

The right-click menu ("Convert with PsyAudio") is switched on in the setup or on the settings page and only touches your own user account.

Settings are kept in `settings.json` in your user folder:

- Windows: `%APPDATA%\PsyAudio\`
- Linux: `~/.config/PsyAudio/`
- macOS: `~/Library/Application Support/PsyAudio/`

## Development

<details>
<summary><b>Build and run</b></summary>

| Task | Command |
|---|---|
| Install | `npm ci` |
| Run with live reload | `npm run tauri dev` (starts Vite on port 1420 and the window) |
| Debug build without Vite | `npx tauri build --debug --no-bundle` (frontend embedded, `target/debug/PsyAudio`, `.exe` on Windows) |
| Debug app bundle on macOS | `npx tauri build --debug --bundles app` (`target/debug/bundle/macos/PsyAudio.app`, needed for the Finder entry and Open With) |
| Frontend checks | `npm run check` (Biome and TypeScript), `npm test` (Vitest) |
| Rust checks | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` |

`cargo test` includes a real conversion into several formats and needs FFmpeg (on the `PATH`; on macOS also in Homebrew, MacPorts or `~/.local/bin`). With `PSYAUDIO_TEST_FFMPEG=<path>` it uses exactly that FFmpeg and expects all nine FFmpeg formats, Speex included; the package workflow runs it this way with the shipped FFmpeg. It also writes the TypeScript types of the settings into `src/ipc/bindings` (ts-rs); CI fails when they differ from the committed files.

</details>

<details>
<summary><b>Sample mode and screenshots</b></summary>

`PsyAudio --demo=<konverter|stapel|cd|einstellungen|einrichtung> [--light] [--en]` opens a page with made-up sample content, in German unless `--en`, dark unless `--light`. Nothing is saved and no other PsyAudio window is asked. Any other page name opens the converter without sample files, which is handy for test runs with files passed on the command line.

`node scripts/capture.mjs <page> <file.png> [options]` starts the debug build in sample mode and saves a screenshot through the WebView2 DevTools protocol (Windows only). Options, run in order: `--light`, `--en`, `--file=<path>`, `--click=<css>`, `--eval=<js>`, `--wait=<css>`, `--close` (the window's close button), `--expect-exit`, `--title`.

`assets/screenshots.webp` is made from five such screenshots (1280 × 800, English) with FFmpeg: `xfade` between them, encoded with `libwebp_anim` from `bgra` frames (a `yuv420p` input pales the colours). The setup page and the drive box show this computer's drives; check every picture for paths and device names before it goes into the repository.

</details>

<details>
<summary><b>External programs</b></summary>

- WAV and AIFF are written by PsyAudio itself; decoding uses `symphonia`.
- MP3, FLAC, AAC, ALAC, Opus, Vorbis, WavPack, WMA and Speex go through the **FFmpeg command line**: PCM is piped in on stdin (`encode::run_ffmpeg`). Formats `symphonia` cannot read are decoded by FFmpeg as well.
- Monkey's Audio, Musepack and DSD need `mac`, `mpcenc` and `sox`; their paths are set on the settings page.
- HDCD decoding uses FFmpeg's `hdcd` filter.
- Programs are found in this order: the path on the settings page, the `PATH`, on macOS Homebrew, MacPorts and `~/.local/bin` (an app started from the Finder gets a bare `PATH`), then the folder of PsyAudio itself.
- For FFmpeg the shipped copy comes last (`ffmpeg.rs`). An FFmpeg found on the system lends the shipped one each job it cannot do itself: a missing encoder, FFprobe for reading, the `hdcd` filter. A path on the settings page stands alone. `PSYAUDIO_FFMPEG=bundled` takes only the shipped FFmpeg, for testing it on a computer that has its own.

</details>

<details>
<summary><b>The shipped FFmpeg</b></summary>

`packaging/build_ffmpeg.sh linux|windows|macos <folder>` builds FFmpeg 9.0.2 and FFprobe from source, statically and under the LGPL. It contains only what PsyAudio uses: the nine encoders, the readers for every format PsyAudio opens, and the `hdcd` filter. LAME, libogg, libvorbis, Opus and Speex are built from their sources first. Every download is checked against a pinned SHA256. Windows is built on Linux with mingw-w64, macOS on a Mac for each architecture.

The package workflow builds it once per system and keeps it in the cache until the script changes. It then puts it into every package: as the resource folder `ffmpeg` on Windows and Linux (`/usr/lib/PsyAudio/ffmpeg` in the deb and rpm, never in `/usr/bin`), and as one universal file in `Contents/MacOS` on macOS, signed with the app. `packaging/check_ffmpeg.sh` checks each package's copy. The license texts and the source of every part are in `ffmpeg/licenses` inside the package.

</details>

<details>
<summary><b>Layout</b></summary>

| Folder | Contents |
|---|---|
| `crates/core` | `psyaudio-core`: everything except the window (conversion, effects, tags, naming, CD ripping, settings, translation) |
| `src-tauri` | The Tauri app: commands (`*_cmd.rs`), background work (`jobs.rs`, `run_cmd.rs`), close guard (`close.rs`), macOS menu (`mac_menu.rs`), single instance, drag and drop; `Info.plist` adds the macOS file types |
| `src` | The React frontend: `shell/` (sidebar, step tiles, side column), `pages/` (convert, batch, cd, settings, setup), `ui/` (controls), `store/` (zustand), `ipc/` (commands and generated types) |
| `tools` | `i18n_dict.tsv` and its generator |
| `scripts` | `capture.mjs` |

Core (`crates/core/src/`):

| File | Contents |
|---|---|
| `settings.rs` | `Settings`, `RipSettings`, `UiState` (serde, `#[serde(default)]`), profiles, load, salvage and save, reset |
| `convert.rs` | Conversion pipeline: `Plan`, `Job`, thread pool, `run_job` (decode → effects → encode → tags), album ReplayGain, playlists |
| `audio.rs` | `Audio` (interleaved f32), decoding with symphonia or FFmpeg, ffprobe, finding external programs, size and time formatting |
| `encode.rs` | `Codec`, `EncSettings`, FFmpeg arguments per format, external encoders, lossless verification, size estimate |
| `ffmpeg.rs` | Which FFmpeg does which job: the settings path, the system's, or the shipped one (`FfmpegSet`) |
| `dsp.rs` | The effects (`Effect`, `catalogue()`, `catalogue_entries()` with a description each, `apply()`) |
| `loudness.rs` | EBU R128 and ReplayGain (−18 LUFS) |
| `naming.rs` | Folder and file names built from chips, naming presets |
| `tags.rs` | Reading and writing tags with `lofty` |
| `files.rs` | Adding files and folders, existing targets |
| `rip.rs` | The rip worker: statuses (`TrackStatus`, `CtdbResult`, `RipPhase`, `RipSummary`) as codes, log, CUE, folder.jpg |
| `cdrip.rs` | CD: TOC (Windows IOCTL, Linux CD-ROM ioctls, macOS CD-DA ioctls), drive model, freedb and MusicBrainz disc IDs, MusicBrainz and Cover Art Archive lookups, eject |
| `ctdb.rs` | CUETools database (CTDB): lookup, track checksums, disc page, rolling CRC32 and the drive offset search |
| `cdrip_read.rs` | Secure reading: passes, re-reads, cache flushing, interpolation |
| `scsi.rs` | Raw SCSI commands to the drive (Windows pass-through, Linux SG_IO, macOS SCSITaskLib): buffer size, cache flush size, stopping the disc (not on macOS) |
| `macos.rs`, `macos_scsi.c` | macOS: optical drives from the I/O Registry, DiskArbitration (unmount, eject), staying awake; the C file wraps SCSITaskLib and is built by `build.rs` on macOS only |
| `integration.rs` | Context menu (Explorer per extension, Dolphin, Finder quick action), keeping the computer awake |
| `i18n.rs` / `i18n_dict.rs` | Translation; `i18n_dict.rs` is generated |
| `timefmt.rs` | Durations and encoding speed in words |

</details>

<details>
<summary><b>Commands, translation and conventions</b></summary>

**Commands.** Every command the frontend may call is listed in `src-tauri/build.rs` (`COMMANDS`) and granted in `src-tauri/capabilities/main.json` (`allow-…`). A new command needs both entries. Long work (converting, ripping, the offset search, MusicBrainz) runs in threads and reports through Tauri channels; statuses travel as codes and are worded in the frontend.

**Translation.**
- Visible texts are written in **German** and the German text is the key, in Rust (`i18n::t("…")`, `tf(…)`) and in the frontend (`t("…")`, placeholders `{name}`).
- English lives in `tools/i18n_dict.tsv` (German, tab, English). Regenerate both tables with `python -I tools/i18n_gen.py tools/i18n_dict.tsv crates/core/src/i18n_dict.rs src/locales/en.json`.
- Tests check that every `t("…")` of the frontend has an English entry and that translations keep the `{…}` placeholders.

**Conventions.**
- Code comments are in English.
- Colours come from the Catppuccin tokens in `src/styles/tokens.css` (Mocha and Latte, accent Mauve); a test checks text contrast.
- Every option in a side column gets a one-line hint; units sit inside the value field ("192 kbit/s").
- Every pull request adds an entry under `[Unreleased]` in the [changelog](CHANGELOG.md); the CI checks it.

</details>

## License

[GPL-3.0](LICENSE)
