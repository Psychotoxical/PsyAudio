# Changelog

Every version has its own section. The section of a version becomes the text of its release draft; the download table is added automatically. Changes that are not released yet collect under `## [Unreleased]`; every pull request adds an entry there, the CI checks it. The main → next workflow turns `[Unreleased]` into the next version's section and raises the version number once the current one is released; after the release, main gets that section through a pull request.

## [Unreleased]

## [1.0.0] - 2026-11-14

The first release of PsyAudio: a bit-perfect CD ripper, a converter and a batch converter in one window.

### Added

- **Bit-perfect CD rips**, every track checked against the CUETools database (CTDB). A confirmed track is identical, sample for sample, to other people's rips of the same disc.
- **Drive offset measured per drive**, once with any bought audio CD that is in the database, or on its own the first time PsyAudio reads one.
- **Secure reading that stays fast**: one pass when the database confirms a track, otherwise a second pass and re-reads past the drive's buffer until two reads agree.
- **Log, CUE sheet, cover art and tags** from MusicBrainz and the Cover Art Archive in every album folder.
- **Converter and batch converter** for MP3, FLAC, AAC, ALAC, Opus, Vorbis, WavPack, WMA, Speex, WAV, AIFF, Monkey's Audio, Musepack and DSD, with about 35 effects, ReplayGain and EBU R128 loudness, and file and folder names built from tags.
- **"Convert with PsyAudio"** in the right-click menu of the Explorer (Windows) and Dolphin (Linux).
- **Setup on first start** in five steps: look, programs, file manager menu, rips and CD drive.
- Catppuccin Mocha (dark) and Latte (light), English and German.
- Packages for Windows (installer), Linux (AppImage, deb, rpm) and macOS (disk image for Apple Silicon and Intel; no CD ripping on macOS yet).

