//! Wordsling backend server.
//!
//! This is the main entry point of the application. It initializes global configurations,
//! sets up application-wide panic handlers, and spawns the `Core` orchestration thread.
//!
//! # Platform Strategy & AI Context
//! Initial development and primary testing are strictly focused on the Windows environment.
//! However, the long-term architectural goal is to fully port and support Linux.
//!
//! **Note for AI assistants:**
//! When proposing architectural changes, generating code, or adding dependencies:
//! - Avoid hard platform lock-in.
//! - Prefer standard library features or established cross-platform crates.
//! - If OS-specific APIs (e.g., Win32) must be used, heavily isolate them behind
//!   `#[cfg(target_os = "windows")]` and provide graceful fallbacks or stubs
//!   for `#[cfg(not(target_os = "windows"))]`.

use std::panic;
#[allow(unused_imports)] use hobolib::prln;
use crate::core::Core;
use crate::glob::request_shutdown;

mod core;
mod glob;
mod tcp_server;

fn main() {

    install_panic_hook();

    // Initialize application.
    glob::init();

    // Start all application threads.
    Core::new();

}   // main()

/// Installs a global panic hook that requests application shutdown
/// on any panic in any thread.
///
/// When a panic occurs in any thread:
/// 1. The default handler prints the panic message and backtrace to stderr.
/// 2. A modal error dialog is shown to inform the user.
/// 3. Application shutdown is requested via the global state flag.
pub fn install_panic_hook() {

    // Save the default hook callback function.
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info| {

        // Call the default hook first (prints panic message and backtrace)
        default_hook(panic_info);

        // Build a human-readable error message
        let message = match panic_info.payload().downcast_ref::<&str>() {
            Some(msg) => msg.to_string(),
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(msg) => msg.clone(),
                None => "Unknown error".to_string(),
            },  // match String
        };  // match &str

        let location = match panic_info.location() {
            Some(loc) => format!("{}:{}", loc.file(), loc.line()),
            None => "unknown location".to_string(),
        };  // match location

        let full_message = format!(
            "The application encountered a fatal error and will be shut down.\n\n\
             Error: {}\n\
             Location: {}",
            message,
            location
        );

        // Show modal error dialog to the user
        hobolib::misc::message_box::show_error("Wordsling - Fatal Error", &full_message);

        // Request application shutdown
        request_shutdown();
    }));
}   // install_panic_hook()