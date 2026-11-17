//! The work running in the background: one conversion and one rip at a time, side by side.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_conversion_is_refused_while_one_runs() {
        let jobs = Jobs::default();
        let first = jobs.begin(Kind::Conversion).expect("first run starts");
        assert!(jobs.begin(Kind::Conversion).is_err());
        first.finish();
        assert!(jobs.begin(Kind::Conversion).is_ok(), "a finished run makes room");
    }

    #[test]
    fn rip_may_run_during_conversion() {
        let jobs = Jobs::default();
        let _conversion = jobs.begin(Kind::Conversion).unwrap();
        assert!(jobs.begin(Kind::Rip).is_ok());
        assert!(jobs.busy());
    }
}

/// What kind of work runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Conversion,
    Rip,
}

/// One running piece of work: its stop switch and whether it is over.
#[derive(Clone, Default)]
pub struct RunHandle {
    pub cancel: Arc<AtomicBool>,
    pub done: Arc<AtomicBool>,
}

impl RunHandle {
    pub fn finish(&self) {
        self.done.store(true, Ordering::SeqCst);
    }
    pub fn running(&self) -> bool {
        !self.done.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
pub struct Jobs {
    conversion: Mutex<Option<RunHandle>>,
    rip: Mutex<Option<RunHandle>>,
}

impl Jobs {
    fn slot(&self, kind: Kind) -> &Mutex<Option<RunHandle>> {
        match kind {
            Kind::Conversion => &self.conversion,
            Kind::Rip => &self.rip,
        }
    }

    /// Starts a run of this kind, or says why not (one of each kind at a time).
    pub fn begin(&self, kind: Kind) -> Result<RunHandle, String> {
        let mut slot = self.slot(kind).lock().unwrap_or_else(|e| e.into_inner());
        if slot.as_ref().is_some_and(RunHandle::running) {
            return Err(match kind {
                Kind::Conversion => psyaudio_core::i18n::t("A conversion is already running – wait for it or cancel it"),
                Kind::Rip => psyaudio_core::i18n::t("A rip is already running – wait or cancel it first"),
            }
            .to_string());
        }
        let handle = RunHandle::default();
        *slot = Some(handle.clone());
        Ok(handle)
    }

    /// Work of this kind is running.
    pub fn busy_with(&self, kind: Kind) -> bool {
        self.slot(kind).lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(RunHandle::running)
    }

    /// Asks the running work of this kind to stop.
    pub fn cancel(&self, kind: Kind) {
        if let Some(h) = self.slot(kind).lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            h.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Some work is running (closing the window would stop it).
    pub fn busy(&self) -> bool {
        [Kind::Conversion, Kind::Rip].iter().any(|k| self.slot(*k).lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(RunHandle::running))
    }
}
