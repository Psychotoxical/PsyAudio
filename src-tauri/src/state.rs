//! What the window keeps between commands.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    /// Files from the command line and from second starts or drops before the frontend listens.
    pub startup_files: Mutex<Vec<PathBuf>>,
    /// The frontend has called `frontend_ready`; from then on files go out as events.
    pub frontend_ready: AtomicBool,
}
