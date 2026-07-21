//! macros.rs — Application-level logging macros.
//!
//! These macros provide a convenient interface to the global logger,
//! automatically capturing the source file and line number.

/// Writes an error-level message to the global log.
///
/// Usage: `log_err!("Failed to connect: {}", err);`
#[macro_export]
macro_rules! log_err {
    ($($arg:tt)*) => {{
        let mut log_guard = $crate::glob::log();
        let _ = log_guard.write(
            hobolib::log::LogLevel::Error,
            file!(), line!(),
            format_args!($($arg)*)
        );
    }};
}

/// Writes a warning-level message to the global log.
///
/// Usage: `log_wrn!("Retrying in {} ms", delay);`
#[macro_export]
macro_rules! log_wrn {
    ($($arg:tt)*) => {{
        let mut log_guard = $crate::glob::log();
        let _ = log_guard.write(
            hobolib::log::LogLevel::Warning,
            file!(), line!(),
            format_args!($($arg)*)
        );
    }};
}

/// Writes an info-level message to the global log.
///
/// Usage: `log_inf!("Pipeline started");`
#[macro_export]
macro_rules! log_inf {
    ($($arg:tt)*) => {{
        let mut log_guard = $crate::glob::log();
        let _ = log_guard.write(
            hobolib::log::LogLevel::Info,
            file!(), line!(),
            format_args!($($arg)*)
        );
    }};
}