//! screen_writer.rs
//!
//! Responsible for the final output of processed data to the currently focused window
//! on the host OS.
//!
//! # RESPONSIBILITY
//! - Receive prepared write commands from the text processor.
//! - Execute them mechanically via clipboard paste and keyboard emulation.
//! - Contain no protocol parsing or text interpretation logic.

use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
use hobolib::clipboard::set_clipboard_text;
use hobolib::eprntln;
use hobolib::keyboard::{send_backspace, send_ctrl_v};
use crate::screen_transfer::ScreenTransfer;

pub struct ScreenWriter {
    _handle: Option<thread::JoinHandle<()>>,
}   // ScreenWriter

impl ScreenWriter {

    /// Constructor.
    ///
    /// Spawns a worker thread that reads prepared write commands from `write_cmd_rx`
    /// and forwards them to the currently focused window.
    ///
    /// # Parameters
    /// - `write_cmd_rx`: receiver end of the channel from the text processor.
    pub fn new(write_cmd_rx: Receiver<ScreenTransfer>) -> Self {

        let handle = thread::spawn(move || {
            _screen_writer_loop(write_cmd_rx);
        });

        ScreenWriter {
            _handle: Some(handle),
        }
    }   // new()

}   // impl ScreenWriter

impl Drop for ScreenWriter {

    /// Destructor.
    /// Waits for the worker thread to finish.
    fn drop(&mut self) {

        if let Some(handle) = self._handle.take() {
            if let Err(panic_payload) = handle.join() {
                eprntln!("ScreenWriter thread panicked: {:?}", panic_payload);
            }   // if
        }   // if

        hobolib::prntln!("ScreenWriter thread dropped");
    }   // drop()

}   // impl Drop for ScreenWriter

fn _screen_writer_loop(write_cmd_rx: Receiver<ScreenTransfer>) {

    for transfer in write_cmd_rx {

        let result = match &transfer {
            ScreenTransfer::Text(text) => send_to_screen(None, text),
            ScreenTransfer::Backspace(count) => send_to_screen(Some(*count), ""),
        };  // match

        if let Err(error_text) = result {
            eprntln!("ScreenWriter failed to send output to screen: {}", error_text);
        }   // if
    }   // for

}   // _screen_writer_loop()

/// Description: Outputs backspaces and/or text to the active window.
///
/// # Execution order
/// 1. Emits the specified number of backspaces (if any) to remove old text.
/// 2. Pastes the provided text chunk (if it is not empty).
///
/// # Parameters
/// - `backspaces`: Optional number of backspaces to send.
/// - `chunk`: The text to be pasted.
///
/// # Errors
/// Returns `Err(String)` if clipboard access or keyboard emulation fails.
pub fn send_to_screen(backspaces: Option<usize>, chunk: &str) -> Result<(), String> {

    /// Delay after a paste operation to let the target application process the input.
    const BACKSPACE_DELAY_MS: u64 = 20;
    const PASTE_DELAY_MS: u64 = 20;


    // First, process backspaces to remove any trailing text that was corrected.
    if let Some(count) = backspaces {
        for _ in 0..count {
            send_backspace()?;
            thread::sleep(Duration::from_millis(BACKSPACE_DELAY_MS));
        }   // for
    }   // if

    // Second, output the new text chunk.
    if !chunk.is_empty() {
        set_clipboard_text(chunk)?;
        send_ctrl_v()?;
        // Allow the target application to process the paste before the next operation.
        thread::sleep(Duration::from_millis(PASTE_DELAY_MS));
    }   // if

    Ok(())
}   // send_to_screen()
