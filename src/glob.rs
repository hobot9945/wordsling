mod conf_file;
mod app_state;

use crate::glob::app_state::STATE;
use crate::glob::conf_file::CONFIG;

/// Initializes global application states.
///
/// Calling this function forces the configuration to be read from disk
/// (or creates a default one if the file is missing).
/// If the configuration file is malformed or missing required fields,
/// this function will panic.
pub fn init() {
    // Acquire the lock to trigger the initial evaluation of the LazyLock,
    // then immediately drop it to avoid holding the mutex unnecessarily.
    // If the config is corrupted, a panic will be thrown directly from within this call.
    drop(CONFIG.lock().unwrap());
}   // init()

/// Requests application shutdown.
pub fn request_shutdown() {
    STATE.lock().unwrap().is_shutdown_requested = true;
}

/// Returns `true` if application shutdown has been requested.
pub fn is_shutdown_requested() -> bool {
    STATE.lock().unwrap().is_shutdown_requested
}

/// Returns the network port from the configuration.
pub fn config_port() -> String {
    CONFIG.lock().unwrap().port.clone()
}   // config_port()
