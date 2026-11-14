//! Measures a drive's read offset with the disc in it and the CUETools database.
//!   cargo run -p psyaudio-core --example measure_offset -- G      (Windows drive letter)
//!   cargo run -p psyaudio-core --example measure_offset -- 0      (Linux /dev/sr0, macOS first drive)

use psyaudio_core::{cdrip, ctdb};
use std::sync::atomic::AtomicBool;

fn main() {
    let d = std::env::args().nth(1).and_then(|a| a.chars().next()).unwrap_or('G');
    let run = || -> Result<(), String> {
        println!("drive {} ({})", cdrip::drive_name(d), cdrip::drive_key(d));
        let drive = cdrip::Drive::open(d)?;
        let toc = drive.toc()?;
        let entries = ctdb::lookup(&toc)?;
        let tracks = toc.audio_tracks().len();
        println!("{tracks} audio tracks, CTDB knows {} rips ({})", ctdb::disc_confidence(&entries, tracks), ctdb::disc_url(&toc));
        let started = std::time::Instant::now();
        let (offset, agree) = ctdb::find_offset(&drive, &toc, &entries, &AtomicBool::new(false), &|_| {})?;
        println!("offset {offset:+} samples, {agree} tracks agree, {:.1} s", started.elapsed().as_secs_f32());
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
