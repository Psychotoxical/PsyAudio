// Always a window program: no console window opens next to PsyAudio (also in debug builds).
#![windows_subsystem = "windows"]

mod app;
mod pages;
mod theme;
mod widgets;

// the core modules keep their `crate::…` paths in this program
pub(crate) use psyaudio_core::{audio, cdrip, cdrip_read, convert, dsp, encode, files, i18n, integration, naming, rip, settings, tags, timefmt};

use std::path::PathBuf;

/// The app icon (also built into PsyAudio-egui.exe from assets/PsyAudio.ico by build.rs).
pub(crate) const APP_ICON_PNG: &[u8] = include_bytes!("../../assets/PsyAudio.png");
/// The PsyAudio lettering, white on transparent (tinted with the accent colour when drawn).
pub(crate) const WORDMARK_PNG: &[u8] = include_bytes!("../../assets/PsyAudio-wordmark.png");

/// This window is the one other instances hand their files to.
pub(crate) static PRIMARY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether no other PsyAudio window is running (a named mutex held for the process's life).
#[cfg(windows)]
fn first_instance() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name: Vec<u16> = "Local\\PsyAudio-window\0".encode_utf16().collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    !(handle.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS)
}

/// Whether no other PsyAudio window is running (an exclusive lock on a file next to the
/// settings, held for the process's life; the system drops it when the process ends).
#[cfg(target_os = "linux")]
fn first_instance() -> bool {
    use std::os::fd::AsRawFd;
    let path = settings::handoff_dir().with_file_name("instance.lock");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(file) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path) else { return true };
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return false;
    }
    std::mem::forget(file);
    true
}

#[cfg(not(any(windows, target_os = "linux")))]
fn first_instance() -> bool {
    true
}

fn main() -> eframe::Result<()> {
    let mut screenshot: Option<PathBuf> = None;
    let mut files: Vec<PathBuf> = Vec::new();
    // args_os: a file name that is not valid Unicode must not stop the program
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next().map(PathBuf::from);
        } else {
            files.push(PathBuf::from(a));
        }
    }
    // the Explorer menu starts one PsyAudio per file, Dolphin one per selection: hand them to the open window instead
    let first = screenshot.is_none() && first_instance();
    PRIMARY.store(first, std::sync::atomic::Ordering::Relaxed);
    if screenshot.is_none() && !first && !files.is_empty() && settings::write_handoff(&settings::handoff_dir(), &files).is_ok() {
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("PsyAudio")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "PsyAudio",
        options,
        Box::new(move |cc| Ok(Box::new(app::PsyApp::new(cc, screenshot, files)))),
    )
}

fn window_icon() -> eframe::egui::IconData {
    match image::load_from_memory(APP_ICON_PNG) {
        Ok(img) => {
            let img = img.resize(256, 256, image::imageops::FilterType::Lanczos3).to_rgba8();
            let (width, height) = img.dimensions();
            eframe::egui::IconData { rgba: img.into_raw(), width, height }
        }
        Err(_) => eframe::egui::IconData::default(),
    }
}
