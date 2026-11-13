//! What the window keeps between commands.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn os(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn demo_args_are_read() {
        assert_eq!(
            Demo::from_args(&os(&["PsyAudio", "--demo=cd", "--light"])),
            Some(Demo { page: "cd".into(), light: true })
        );
        assert_eq!(Demo::from_args(&os(&["PsyAudio", "--demo=konverter"])).map(|d| d.light), Some(false));
        assert_eq!(Demo::from_args(&os(&["PsyAudio", "a.flac"])), None);
    }
}

/// `--demo=<page>` (with `--light` for Latte): sample content for screenshots; nothing is saved
/// and no other PsyAudio window is asked.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Demo {
    pub page: String,
    pub light: bool,
}

impl Demo {
    pub fn from_args(args: &[std::ffi::OsString]) -> Option<Demo> {
        let mut page = None;
        let mut light = false;
        for a in args.iter().skip(1) {
            let a = a.to_string_lossy();
            if let Some(p) = a.strip_prefix("--demo=") {
                page = Some(p.to_string());
            } else if a == "--light" {
                light = true;
            }
        }
        page.map(|page| Demo { page, light })
    }
}

#[derive(Default)]
pub struct AppState {
    pub demo: Option<Demo>,
    /// FFmpeg as last found (see tools_cmd).
    pub tools: Mutex<Option<crate::tools_cmd::ToolCache>>,
    /// Files from the command line and from second starts or drops before the frontend listens.
    pub startup_files: Mutex<Vec<PathBuf>>,
    /// The frontend has called `frontend_ready`; from then on files go out as events.
    pub frontend_ready: AtomicBool,
    /// The conversion and the rip running in the background.
    pub jobs: crate::jobs::Jobs,
    /// The disc read last and what the lookups found.
    pub cd: Mutex<crate::cd_cmd::CdState>,
}
