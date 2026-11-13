// Always a window program: no console window opens next to PsyAudio (also in debug builds).
#![windows_subsystem = "windows"]

fn main() {
    psyaudio_lib::run();
}
