//! Core application module.
//!
//! Responsible for orchestrating the application's workflow: receiving, processing,
//! and sending recognized text to user interface input fields. Spawns its own thread
//! for operation, as the main thread is reserved for the UI.
//!
//! During initialization, it creates all necessary working structures. These structures,
//! in turn, spawn their own threads and coroutines if needed.

use std::thread;
use hobolib::{eprntln, prntln};

pub struct Core {
    // Thread handle. Used during shutdown to perform join() from the main thread.
    handle: Option<thread::JoinHandle<()>>
}

impl Core {

    /// Constructor.
    pub fn new() -> Self {

        // Initialize and return the instance.
        Core {
            // handle: Some(handle)
            handle: None
        }
    }   // new()
}   // impl Core

impl Drop for Core {

    /// Destructor.
    /// Waits for the core thread to finish and checks for panics.
    fn drop(&mut self) {

        if let Err(panic_payload) = self.handle.take().unwrap().join() {
            // Thread panicked, log the error.
            eprntln!("Core thread panicked: {:?}", panic_payload);
        }   // if

        prntln!("Core thread dropped");
    }
}   // impl Drop for Core