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

use std::thread;
use std::time::Duration;
use hobolib::glob::is_shutdown_requested;
use hobolib::misc::panic_hook::install_panic_hook;
#[allow(unused_imports)] use hobolib::prln;
use crate::core::Core;

mod core;
mod glob;
mod lexeme_transfer;
mod lexer;
mod screen_writer;
mod tcp_server;
mod text_processor;
mod user_activity_tracker;
mod screen_transfer;

fn main() {

    install_panic_hook();

    // Initialize application.
    glob::init();

    // Start all application threads.
    let _core = Core::new();

    // Keep the process alive while the worker threads are running.
    while !is_shutdown_requested() {
        thread::sleep(Duration::from_millis(500));
    }   // while

}   // main()
