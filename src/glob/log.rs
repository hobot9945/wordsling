//! log.rs

use std::sync::{LazyLock, Mutex};
use hobolib::log::Log;

const LOG_FILENAME: &str = "wordsling.log";

/// Global logger instance.
/// The file is opened (and truncated) when the LazyLock is first accessed.
pub(super) static _LOG: LazyLock<Mutex<Log>> = LazyLock::new(|| {
    Mutex::new(Log::new(LOG_FILENAME).expect("Failed to open log file"))
});
