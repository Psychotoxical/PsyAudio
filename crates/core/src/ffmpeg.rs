//! Which FFmpeg does which job: one from the settings, one on the system, or the one PsyAudio ships.
//!
//! A path from the settings always wins and stands alone. Otherwise an FFmpeg on the system
//! comes first, and the shipped one steps in for whatever the system one cannot do.

use crate::audio;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where an FFmpeg came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Configured,
    System,
    Bundled,
}

/// One FFmpeg and what it can do.
#[derive(Clone, Debug)]
pub struct Ffmpeg {
    pub path: PathBuf,
    pub source: Source,
    /// Encoder names from `-encoders`; empty when FFmpeg did not list them.
    pub encoders: Vec<String>,
    /// FFprobe lies beside it (or on the `PATH`): needed to read formats symphonia cannot.
    pub probe: bool,
    /// It has the `hdcd` filter.
    pub hdcd: bool,
}

impl Ffmpeg {
    /// Asks the program what it can do; None when it does not start.
    pub fn probe(path: &Path, source: Source) -> Option<Ffmpeg> {
        if !runs(path, &["-hide_banner", "-version"]) {
            return None;
        }
        let filters = output(path, &["-hide_banner", "-filters"]);
        Some(Ffmpeg {
            path: path.to_path_buf(),
            source,
            encoders: audio::ffmpeg_encoders(path),
            probe: audio::ffprobe_beside(path).is_some(),
            hdcd: filters.lines().any(|l| l.split_whitespace().nth(1) == Some("hdcd")),
        })
    }

    /// True when the encoder list is unknown (empty) or contains `enc`.
    pub fn has_encoder(&self, enc: &str) -> bool {
        self.encoders.is_empty() || self.encoders.iter().any(|e| e == enc)
    }
}

fn runs(path: &Path, args: &[&str]) -> bool {
    let mut cmd = Command::new(path);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    audio::hidden(&mut cmd);
    cmd.status().is_ok_and(|s| s.success())
}

fn output(path: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new(path);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    audio::hidden(&mut cmd);
    cmd.output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// The FFmpeg that does the work and the one that steps in for it.
#[derive(Clone, Debug, Default)]
pub struct FfmpegSet {
    pub main: Option<Ffmpeg>,
    pub fallback: Option<Ffmpeg>,
}

impl FfmpegSet {
    /// Looks for FFmpeg: the settings path, then the system, then `bundled_dir`.
    /// `force_bundled` takes only the shipped one (for tests).
    pub fn find(configured: &str, bundled_dir: Option<&Path>, force_bundled: bool) -> FfmpegSet {
        let configured = Some(configured.trim()).filter(|c| !c.is_empty()).map(PathBuf::from);
        let system = if force_bundled { None } else { audio::find_tool("", "ffmpeg") };
        let bundled = bundled_dir.and_then(bundled_in);
        let (main, fallback) = choose(configured, system, bundled, force_bundled);
        let main = main.and_then(|(path, source)| Ffmpeg::probe(&path, source));
        let fallback = fallback.and_then(|path| Ffmpeg::probe(&path, Source::Bundled));
        match main {
            Some(main) => FfmpegSet { main: Some(main), fallback },
            // a system FFmpeg that does not start leaves the shipped one alone
            None => FfmpegSet { main: fallback, fallback: None },
        }
    }

    /// Only this FFmpeg, as if it were set in the settings (empty when it does not start).
    pub fn only(path: &Path) -> FfmpegSet {
        FfmpegSet { main: Ffmpeg::probe(path, Source::Configured), fallback: None }
    }

    /// The FFmpeg that writes with `enc`.
    pub fn for_encoder(&self, enc: &str) -> Option<&Ffmpeg> {
        self.pick(|f| f.has_encoder(enc))
    }

    /// The FFmpeg that reads files symphonia cannot (it needs FFprobe).
    pub fn for_decode(&self) -> Option<&Ffmpeg> {
        self.pick(|f| f.probe)
    }

    /// The FFmpeg that decodes HDCD.
    pub fn for_hdcd(&self) -> Option<&Ffmpeg> {
        self.pick(|f| f.hdcd)
    }

    /// The main FFmpeg when it can do the job; the fallback only for a system FFmpeg that cannot.
    fn pick(&self, can: impl Fn(&Ffmpeg) -> bool) -> Option<&Ffmpeg> {
        let main = self.main.as_ref()?;
        if can(main) || main.source != Source::System {
            return Some(main);
        }
        match &self.fallback {
            Some(f) if can(f) => Some(f),
            _ => Some(main),
        }
    }
}

/// Where the main FFmpeg and its fallback come from; checks only which files exist.
pub(crate) fn choose(
    configured: Option<PathBuf>,
    system: Option<PathBuf>,
    bundled: Option<PathBuf>,
    force_bundled: bool,
) -> (Option<(PathBuf, Source)>, Option<PathBuf>) {
    let as_bundled = |b: Option<PathBuf>| (b.map(|b| (b, Source::Bundled)), None);
    if force_bundled {
        return as_bundled(bundled);
    }
    if let Some(c) = configured.filter(|c| c.exists()) {
        return (Some((c, Source::Configured)), None);
    }
    let system = system.filter(|s| !bundled.as_deref().is_some_and(|b| same_file(s, b)));
    match system {
        Some(s) => (Some((s, Source::System)), bundled),
        None => as_bundled(bundled),
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// The FFmpeg inside `dir`, if there is one.
pub fn bundled_in(dir: &Path) -> Option<PathBuf> {
    let p = dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
    p.is_file().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder in the temp directory for one test.
    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("psyaudio-ffmpeg-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn file(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, b"").unwrap();
        p
    }

    fn ff(source: Source, encoders: &[&str], probe: bool, hdcd: bool) -> Ffmpeg {
        Ffmpeg { path: PathBuf::from(format!("{source:?}")), source, encoders: encoders.iter().map(|e| e.to_string()).collect(), probe, hdcd }
    }

    const ALL: &[&str] = &["libmp3lame", "flac", "aac", "libspeex"];

    #[test]
    fn force_bundled_ignores_everything_else() {
        let dir = folder("force");
        let conf = file(&dir, "configured");
        let bundled = file(&dir, "bundled");
        let got = choose(Some(conf), Some(PathBuf::from("system")), Some(bundled.clone()), true);
        assert_eq!(got, (Some((bundled, Source::Bundled)), None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_configured_path_gets_no_fallback() {
        let dir = folder("configured");
        let conf = file(&dir, "configured");
        let bundled = file(&dir, "bundled");
        let got = choose(Some(conf.clone()), Some(PathBuf::from("system")), Some(bundled), false);
        assert_eq!(got, (Some((conf, Source::Configured)), None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_configured_path_is_ignored() {
        let dir = folder("missing");
        let gone = dir.join("deleted-ffmpeg");
        let bundled = file(&dir, "bundled");
        let system = PathBuf::from("system");
        let got = choose(Some(gone), Some(system.clone()), Some(bundled.clone()), false);
        assert_eq!(got, (Some((system, Source::System)), Some(bundled)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_system_ffmpeg_comes_first_with_the_bundled_as_fallback() {
        let dir = folder("system");
        let system = file(&dir, "system");
        let bundled = file(&dir, "bundled");
        let got = choose(None, Some(system.clone()), Some(bundled.clone()), false);
        assert_eq!(got, (Some((system, Source::System)), Some(bundled)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_bundled_file_found_on_the_system_path_counts_as_bundled() {
        let dir = folder("same");
        let bundled = file(&dir, "ffmpeg");
        // the same file, reached by a different spelling of the path
        let via_dot = dir.join(".").join("ffmpeg");
        let got = choose(None, Some(via_dot), Some(bundled.clone()), false);
        assert_eq!(got, (Some((bundled, Source::Bundled)), None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_system_the_bundled_is_main() {
        let dir = folder("nosystem");
        let bundled = file(&dir, "bundled");
        assert_eq!(choose(None, None, Some(bundled.clone()), false), (Some((bundled, Source::Bundled)), None));
        assert_eq!(choose(None, None, None, false), (None, None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_encoder_falls_back_only_for_a_system_ffmpeg() {
        let system = FfmpegSet { main: Some(ff(Source::System, &["libmp3lame", "aac"], true, true)), fallback: Some(ff(Source::Bundled, ALL, true, true)) };
        assert_eq!(system.for_encoder("libspeex").map(|f| f.source), Some(Source::Bundled));
        assert_eq!(system.for_encoder("libmp3lame").map(|f| f.source), Some(Source::System));
        let configured =
            FfmpegSet { main: Some(ff(Source::Configured, &["libmp3lame", "aac"], true, true)), fallback: Some(ff(Source::Bundled, ALL, true, true)) };
        assert_eq!(configured.for_encoder("libspeex").map(|f| f.source), Some(Source::Configured));
        // the fallback lacks it too: the main one, so the caller names what is missing
        let neither = FfmpegSet { main: Some(ff(Source::System, &["aac"], true, true)), fallback: Some(ff(Source::Bundled, &["aac"], true, true)) };
        assert_eq!(neither.for_encoder("libspeex").map(|f| f.source), Some(Source::System));
        assert!(FfmpegSet::default().for_encoder("aac").is_none());
    }

    #[test]
    fn decoding_falls_back_only_for_a_system_ffmpeg() {
        let system = FfmpegSet { main: Some(ff(Source::System, ALL, false, true)), fallback: Some(ff(Source::Bundled, ALL, true, true)) };
        assert_eq!(system.for_decode().map(|f| f.source), Some(Source::Bundled));
        let configured = FfmpegSet { main: Some(ff(Source::Configured, ALL, false, true)), fallback: Some(ff(Source::Bundled, ALL, true, true)) };
        assert_eq!(configured.for_decode().map(|f| f.source), Some(Source::Configured));
        let with_probe = FfmpegSet { main: Some(ff(Source::System, ALL, true, true)), fallback: Some(ff(Source::Bundled, ALL, true, true)) };
        assert_eq!(with_probe.for_decode().map(|f| f.source), Some(Source::System));
    }

    #[test]
    fn hdcd_falls_back_like_encoders() {
        let system = FfmpegSet { main: Some(ff(Source::System, ALL, true, false)), fallback: Some(ff(Source::Bundled, ALL, true, true)) };
        assert_eq!(system.for_hdcd().map(|f| f.source), Some(Source::Bundled));
        let configured = FfmpegSet { main: Some(ff(Source::Configured, ALL, true, false)), fallback: Some(ff(Source::Bundled, ALL, true, true)) };
        assert_eq!(configured.for_hdcd().map(|f| f.source), Some(Source::Configured));
    }

    #[test]
    fn an_unknown_encoder_list_counts_as_having_it() {
        assert!(ff(Source::System, &[], true, true).has_encoder("libspeex"));
        assert!(!ff(Source::System, &["aac"], true, true).has_encoder("libspeex"));
        let set = FfmpegSet { main: Some(ff(Source::System, &[], true, true)), fallback: Some(ff(Source::Bundled, ALL, true, true)) };
        assert_eq!(set.for_encoder("libspeex").map(|f| f.source), Some(Source::System));
    }

    #[test]
    fn probe_rejects_a_file_that_does_not_run() {
        let dir = folder("probe");
        let fake = file(&dir, if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
        assert!(Ffmpeg::probe(&fake, Source::Bundled).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Reads a real FFmpeg's lists (skipped without one): the filter list's layout is FFmpeg's, not ours.
    #[test]
    fn probe_reads_a_real_ffmpeg() {
        let Some(path) = audio::find_tool("", "ffmpeg") else { return };
        let f = Ffmpeg::probe(&path, Source::System).expect("FFmpeg starts");
        assert!(f.has_encoder("flac") && !f.encoders.is_empty(), "{:?}", f.encoders);
        assert!(f.hdcd, "hdcd filter");
    }

    #[test]
    fn bundled_in_finds_only_an_existing_ffmpeg() {
        let dir = folder("bundled-in");
        assert_eq!(bundled_in(&dir), None);
        let exe = file(&dir, if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
        assert_eq!(bundled_in(&dir), Some(exe));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
