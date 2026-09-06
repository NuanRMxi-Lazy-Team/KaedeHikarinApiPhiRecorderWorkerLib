use std::sync::{Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlError {
    Canceled,
}

#[derive(Debug, Default)]
pub struct JobControl {
    state: Mutex<ControlState>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct ControlState {
    cancel_requested: bool,
    pause_requested: bool,
}

impl JobControl {
    pub fn cancel(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.cancel_requested = true;
        state.pause_requested = false;
        self.changed.notify_all();
    }

    pub fn pause(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.cancel_requested {
            state.pause_requested = true;
        }
    }

    pub fn resume(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.pause_requested = false;
        self.changed.notify_all();
    }

    pub fn is_cancel_requested(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .cancel_requested
    }

    pub fn is_pause_requested(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pause_requested
    }

    pub fn wait_if_paused(&self) -> Result<(), ControlError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while state.pause_requested && !state.cancel_requested {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        if state.cancel_requested {
            Err(ControlError::Canceled)
        } else {
            Ok(())
        }
    }
}
