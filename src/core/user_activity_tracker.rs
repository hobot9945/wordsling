//! user_activity_tracker.rs — User activity monitoring.
//!
//! Monitors user input activity (mouse clicks, keyboard presses)
//! to detect when the user manually interacts with the system.
//! Delegates platform-specific hooking to `hobolib`.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use hobolib::user_activity::{spawn_activity_tracker, stop_activity_tracker};
use crate::core::lexeme_transfer::LexemeTransfer;
use crate::{log_err, log_inf};

pub struct UserActivityTracker {
    _handle: Option<std::thread::JoinHandle<()>>,
    thread_id: Arc<Mutex<u32>>,
}

impl UserActivityTracker {

    pub fn new(lexeme_tx: Sender<LexemeTransfer>) -> Self {

        let (handle, thread_id) = spawn_activity_tracker(move || {
            let _ = lexeme_tx.send(LexemeTransfer::UserActivityDetected);
        });

        UserActivityTracker {
            _handle: Some(handle),
            thread_id,
        }
    }
}

impl Drop for UserActivityTracker {

    fn drop(&mut self) {
        if let Ok(tid_guard) = self.thread_id.lock() {
            stop_activity_tracker(*tid_guard);
        }

        if let Some(handle) = self._handle.take() {
            if let Err(panic_payload) = handle.join() {
                log_err!("UserActivityTracker thread panicked: {:?}", panic_payload);
            }
        }

        log_inf!("UserActivityTracker thread dropped");
    }
}