//! PsyAudio core: everything that is not drawing a window.

pub mod audio;
pub mod cdrip;
pub mod cdrip_read;
pub mod convert;
pub mod ctdb;
pub mod dsp;
pub mod encode;
pub mod ffmpeg;
pub mod files;
pub mod i18n;
pub mod integration;
pub mod loudness;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod naming;
pub mod rip;
pub mod scsi;
pub mod settings;
pub mod tags;
pub mod timefmt;

/// How PsyAudio introduces itself to MusicBrainz, the Cover Art Archive and CTDB: name, the
/// real version and a contact address, as the MusicBrainz API rules ask of every application.
pub const USER_AGENT: &str = concat!("PsyAudio/", env!("CARGO_PKG_VERSION"), " ( https://github.com/Psysonic/PsyAudio )");

/// The app icon as PNG, for the file manager menu on Linux (Dolphin service menu).
pub const APP_ICON_PNG: &[u8] = include_bytes!("../../../assets/PsyAudio.png");

#[cfg(test)]
mod icon_tests {
    /// The Dolphin menu (Linux) installs this icon; it must be a PNG of the core, not of a window crate.
    #[test]
    fn app_icon_is_a_png() {
        assert!(crate::APP_ICON_PNG.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    /// MusicBrainz wants "Name/version ( contact )"; the version is the one being built.
    #[test]
    fn user_agent_names_version_and_contact() {
        let expected = format!("PsyAudio/{} ( https://github.com/Psysonic/PsyAudio )", env!("CARGO_PKG_VERSION"));
        assert_eq!(crate::USER_AGENT, expected);
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
