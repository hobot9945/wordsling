//! screen_transfer.rs — Screen transfer type definition.
//!
//! Defines the data contract between the text processor and the screen writer.
//!
//! # RESPONSIBILITY
//! - Provide a single, shared definition of `ScreenTransfer` used across pipeline stages.

/// A single output operation for the screen writer.
///
/// By the time this value reaches the screen writer, all protocol parsing
/// and text processing has already been performed. The screen writer
/// simply executes the operation mechanically.
pub enum ScreenTransfer {
    /// Text to paste into the focused window.
    Text(String),
    /// Number of backspaces to send.
    Backspace(usize),
}   // ScreenTransfer