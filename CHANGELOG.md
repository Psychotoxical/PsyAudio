# Changelog

Every version has its own section. The section of a version becomes the text of its release draft; the download table is added automatically. Changes that are not released yet collect under `## [Unreleased]`; every pull request adds an entry there, the CI checks it. The main → next workflow turns `[Unreleased]` into the next version's section and raises the version number once the current one is released; after the release, main gets that section through a pull request.

## [Unreleased]

The first release of PsyAudio: a bit-perfect CD ripper, a converter and a batch converter in one window.

### Changed

- The effects are cleared out: Reverse, Loop, Karaoke, Speed, Band-pass, Minimum and Maximum length, Insert silence, Write metadata file, CPU throttle and Delete output on error are gone. Saved profiles that used them keep everything else; "CPU throttle" becomes the "Low priority" setting.
- Every effect explains itself: what it does, when to use it and an example, with plain labels and sensible defaults (the compressor now starts at 3:1 instead of doing nothing at 1:1).
- Tag processing numbers tracks by itself (per folder, in list order), can take other tags into a value (`<artist>`, `<album>` …) and offers only combinations that make sense.
- Converted files always get an album artist: the artist when the whole album has one, "Various Artists" for a compilation.
- "Delete source file" moves the source to the recycle bin.
- A release can be built straight from main, without a release candidate: the new “main → release” workflow raises the version the same way and creates the draft.
- The README opens with the PsyAudio wordmark, white on GitHub's dark theme and black on the light one.
- The README shows what PsyAudio does on Windows, Linux and macOS in one table, lists the downloads, and keeps the developer notes in fold-out sections.

### Added

- **Bit-perfect CD rips**, every track checked against the CUETools database (CTDB). A confirmed track is identical, sample for sample, to other people's rips of the same disc.
- **Drive offset measured per drive**, once with any bought audio CD that is in the database, or on its own the first time PsyAudio reads one.
- **Secure reading that stays fast**: one pass when the database confirms a track, otherwise a second pass and re-reads past the drive's buffer until two reads agree.
- **Log, CUE sheet, cover art and tags** from MusicBrainz and the Cover Art Archive in every album folder.
- **Converter and batch converter** for MP3, FLAC, AAC, ALAC, Opus, Vorbis, WavPack, WMA, Speex, WAV, AIFF, Monkey's Audio, Musepack and DSD, with about 35 effects, ReplayGain and EBU R128 loudness, and file and folder names built from tags.
- **"Convert with PsyAudio"** in the right-click menu of the Explorer (Windows) and Dolphin (Linux).
- **Setup on first start** in five steps: look, programs, file manager menu, rips and CD drive.
- Catppuccin Mocha (dark) and Latte (light), English and German.
- Downloads for Windows (installer), Linux (AppImage, deb, rpm) and macOS (disk image for Apple Silicon and Intel).
- **CD ripping on macOS**, with the same database check, drive offset and buffer measurement as on Windows and Linux.
- **Finder integration on macOS**: a "Convert with PsyAudio" quick action (switched on in the settings), Open With and dropping files on the Dock icon. Audio formats macOS does not know, like Opus, APE and WavPack, count as audio files.
- **The macOS disk image is signed and notarized**, so it opens without a Gatekeeper warning.
- **On macOS, ripping and converting keep the Mac awake**, ⌘Q asks like the close button while work runs, and FFmpeg and the other programs are also found when PsyAudio is started from the Finder.
- **FFmpeg ships with every package**: MP3, FLAC, AAC, ALAC, Opus, Vorbis, WavPack, WMA, Speex and HDCD work right after installing. An FFmpeg already on the computer is still preferred; the shipped one steps in for anything it lacks. The source code of the shipped FFmpeg and its libraries comes with every release.

### Fixed

- On Linux the tray opens after a rip and with the eject button also when the drive locked it; USB drives answered the eject command but stayed shut. A failed eject now shows in the rip log.
- "Run external program" works on Linux and macOS (it called the Windows shell), with paths quoted for the system's shell.
- "Play a sound when done" sounds on Linux and macOS too.
- The files of a release candidate carry its number, for example `PsyAudio-1.0.0-rc.1-x86_64.AppImage`, so they can't be mixed up with the release.
- The AppImage shows its own icon in the Wayland taskbar; it no longer took the entry it carries inside for an installed package.
- The Dolphin menu follows PsyAudio when its AppImage is moved, for example by Gear Lever; it is set to the program's current file on every start.
- While converting, the window title shows the same percentage as the progress bar; it could run a few percent ahead.
- In the AppImage, “Open output folder” and the links to the CUETools database and the FFmpeg download open again. Programs started from the AppImage, the system's FFmpeg included, no longer pick up its bundled libraries.
