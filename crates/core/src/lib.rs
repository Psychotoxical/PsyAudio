//! PsyAudio core: everything that is not drawing a window.

pub mod audio;
pub mod cdrip;
pub mod cdrip_read;
pub mod convert;
pub mod ctdb;
pub mod dsp;
pub mod encode;
pub mod files;
pub mod i18n;
pub mod integration;
pub mod loudness;
pub mod naming;
pub mod rip;
pub mod scsi;
pub mod settings;
pub mod tags;
pub mod timefmt;

/// The app icon as PNG, for the file manager menu on Linux (Dolphin service menu).
pub const APP_ICON_PNG: &[u8] = include_bytes!("../../../assets/PsyAudio.png");

#[cfg(test)]
mod icon_tests {
    /// The Dolphin menu (Linux) installs this icon; it must be a PNG of the core, not of a window crate.
    #[test]
    fn app_icon_is_a_png() {
        assert!(crate::APP_ICON_PNG.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
