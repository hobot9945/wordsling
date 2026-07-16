//! glob.rs

mod conf_file;
mod app_state;
mod log;

use std::sync::MutexGuard;
use hobolib::log::Log;
use crate::glob::conf_file::CONFIG;
use crate::glob::log::_LOG;

/// Initializes global application states.
///
/// Calling this function forces the configuration to be read from disk
/// (or creates a default one if the file is missing).
/// If the configuration file is malformed or missing required fields,
/// this function will panic.
pub fn init() {
    // Acquire the lock to trigger the initial evaluation of the LazyLock,
    // then immediately drop it to avoid holding the mutex unnecessarily.
    drop(_LOG.lock().unwrap());


    // Acquire the lock to trigger the initial evaluation of the LazyLock,
    // then immediately drop it to avoid holding the mutex unnecessarily.
    // If the config is corrupted, a panic will be thrown directly from within this call.
    drop(CONFIG.lock().unwrap());
}   // init()

/// Acquires the global logger lock.
///
/// Intended for internal use by logging macros.
pub fn log() -> MutexGuard<'static, Log> {
    _LOG.lock().unwrap()
}

/// Returns the network port from the configuration.
pub fn appconf_port() -> String {
    CONFIG.lock().unwrap().port.clone()
}   // config_port()
