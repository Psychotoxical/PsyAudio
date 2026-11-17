//! Closing the window while work runs: ask first, or stop the work and close once it ended.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_closes() {
        assert_eq!(close_decision(false, false, false), CloseDecision::Allow);
    }

    #[test]
    fn running_asks() {
        assert_eq!(close_decision(true, false, false), CloseDecision::Ask);
        assert_eq!(close_decision(false, true, false), CloseDecision::Ask);
    }

    #[test]
    fn closing_waits_until_done() {
        assert_eq!(close_decision(true, false, true), CloseDecision::Wait);
        assert_eq!(close_decision(false, true, true), CloseDecision::Wait);
        assert_eq!(close_decision(false, false, true), CloseDecision::Allow);
    }
}

use crate::jobs::Kind;
use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, CloseRequestApi, Emitter, ExitRequestApi, Manager, State, Window};

/// What a click on the window's close button does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CloseDecision {
    /// Nothing runs: close.
    Allow,
    /// Something runs: ask "Quit PsyAudio?" first.
    Ask,
    /// The answer was "Cancel and quit": the window closes once the work has stopped.
    Wait,
}

pub fn close_decision(conversion_running: bool, rip_running: bool, closing: bool) -> CloseDecision {
    match (conversion_running || rip_running, closing) {
        (false, _) => CloseDecision::Allow,
        (true, true) => CloseDecision::Wait,
        (true, false) => CloseDecision::Ask,
    }
}

/// The window's close button: closes, or keeps the window and asks the frontend.
pub fn on_close_requested(window: &Window, api: &CloseRequestApi) {
    api.prevent_close();
    close_or_ask(window.app_handle());
}

/// An exit request without a code (the last window gone): the same as the close button. Exits
/// PsyAudio itself asks for (`app.exit`) carry a code and pass. ⌘Q on macOS: `mac_menu`.
pub fn on_exit_requested(app: &AppHandle, code: Option<i32>, api: &ExitRequestApi) {
    if code.is_none() {
        api.prevent_exit();
        close_or_ask(app);
    }
}

/// Closes once the window has saved its settings, or asks first while work runs.
pub fn close_or_ask(app: &AppHandle) {
    let state = app.state::<AppState>();
    let decision = close_decision(state.jobs.busy_with(Kind::Conversion), state.jobs.busy_with(Kind::Rip), state.closing.load(Ordering::SeqCst));
    match decision {
        CloseDecision::Allow => {
            // the window saves a settings change still waiting, then confirms; if it does not
            // answer, PsyAudio closes anyway
            let _ = app.emit_to("main", "close-now", ());
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(UNANSWERED_CLOSE);
                app.exit(0);
            });
        }
        CloseDecision::Ask => {
            let _ = app.emit_to("main", "close-requested", ());
        }
        CloseDecision::Wait => {}
    }
}

/// How long a close with nothing running waits for the window to save its settings.
const UNANSWERED_CLOSE: Duration = Duration::from_secs(2);

/// How long closing waits for the work to stop before it ends PsyAudio anyway (a hanging
/// encoder must not keep a window open that can no longer be closed).
const STOP_WAIT: Duration = Duration::from_secs(15);

/// The answer to "Quit PsyAudio?": `cancel_work` stops conversion and rip and closes PsyAudio
/// as soon as both have ended; otherwise the work goes on.
#[tauri::command]
pub fn confirm_close(app: AppHandle, state: State<'_, AppState>, cancel_work: bool) {
    if !cancel_work {
        return;
    }
    state.closing.store(true, Ordering::SeqCst);
    state.jobs.cancel(Kind::Conversion);
    state.jobs.cancel(Kind::Rip);
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let started = Instant::now();
        while state.jobs.busy() && started.elapsed() < STOP_WAIT {
            std::thread::sleep(Duration::from_millis(100));
        }
        app.exit(0);
    });
}
