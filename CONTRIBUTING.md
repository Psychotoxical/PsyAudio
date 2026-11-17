# Contributing to PsyAudio

Thanks for helping. Bug reports, ideas, translations and code are all welcome.

## Issues

Open an issue on GitHub. For a bug, name the PsyAudio version, the system and the package (installer, AppImage, deb, rpm, DMG), the drive if it is about ripping, and the steps that lead to it.

## Translations

Every language is one JSON file in `locales/`; [Translating PsyAudio](README.md#translating-psyaudio) in the README explains the format, the plural forms and how to start a new language. A pull request that only touches a language file is fine.

## Code

1. Set up the project as in [Development](README.md#development) (`npm ci`, then `npm run tauri dev`).
2. Make the change on a branch and open a pull request against `main`. Keep one topic per pull request.
3. Before you push, run what CI runs:

   ```
   npm run check
   npm test
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

   `cargo test` needs FFmpeg; the README says where it looks. It also writes the TypeScript types into `src/ipc/bindings`; commit them when they change.

### House rules

- **Changelog:** every pull request adds at least one line under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md), in the section that fits (`Added`, `Changed`, `Fixed`). Write it for users: what they notice, not how it is built. CI fails without it.
- **Texts in the interface:** visible texts are written in English and the English text is the key. A new text goes into `locales/en.json`; missing translations show in English, so you do not have to fill in every language.
- **Commands:** a new command the frontend calls needs its entry in `src-tauri/build.rs` (`COMMANDS`) and in `src-tauri/capabilities/main.json`.
- **Tests:** a fix comes with a test that fails without it. Test data uses neutral placeholders ("Band", "Album", files in the temp folder), never real music or personal paths.
- **Real files:** PsyAudio converts, tags, renames and can move sources to the recycle bin. Try changes on copies, never on a collection you care about.

## How a pull request goes in

CI runs the changelog check, Biome, TypeScript, rustfmt, actionlint and, when code changes, the tests on Linux. The check `ci-ok` sums it up and must be green. Only maintainers merge: a maintainer reviews the pull request and squashes it into `main`. Please do not merge your own pull request, even if you have write access.

Releases are built by the maintainers: the release workflow runs the tests on Windows and macOS too, builds all packages with the shipped FFmpeg, signs and notarizes the macOS disk image and creates a draft. Contributors do not need to change the version number.
