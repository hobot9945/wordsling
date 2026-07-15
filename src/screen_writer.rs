//! screen_writer.rs
//!
//! Responsible for the final output of processed data to the currently focused window
//! on the host OS.
//!
//! # RESPONSIBILITY
//! - Receive prepared screen transfer commands.
//! - Accumulate text in a local buffer and debounce clipboard paste operations (`Ctrl+V`).
//! - Handle backspaces by shrinking the buffer or emitting real keystrokes if the buffer is empty.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};
use hobolib::clipboard::set_clipboard_text;
use hobolib::eprntln;
use hobolib::keyboard::{send_backspace, send_ctrl_v};
use crate::screen_transfer::ScreenTransfer;

pub struct ScreenWriter {
    _handle: Option<thread::JoinHandle<()>>,
}

impl ScreenWriter {
    /// Constructor.
    pub fn new(write_cmd_rx: Receiver<ScreenTransfer>) -> Self {
        let handle = thread::spawn(move || {
            _screen_writer_loop(write_cmd_rx);
        });

        ScreenWriter {
            _handle: Some(handle),
        }
    }
}

impl Drop for ScreenWriter {
    fn drop(&mut self) {
        if let Some(handle) = self._handle.take() {
            if let Err(panic_payload) = handle.join() {
                eprntln!("ScreenWriter thread panicked: {:?}", panic_payload);
            }
        }
        hobolib::prntln!("ScreenWriter thread dropped");
    }
}

/// Worker loop of the screen writer.
///
/// Drives the `PasteBuffer` state machine using events from the channel.
fn _screen_writer_loop(write_cmd_rx: Receiver<ScreenTransfer>) {

    // Cooldown interval for clipboard pasting.
    let mut buffer = _PasteBuffer::new(Duration::from_millis(80));
    let mut timeout = None;

    loop {
        // Wait for an event or timeout.
        let recv_result = match timeout {
            Some(duration) => write_cmd_rx.recv_timeout(duration),
            None => write_cmd_rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };

        // Dispatch the event to the buffer.
        timeout = match recv_result {
            Ok(ScreenTransfer::Text(text)) => {
                buffer.push_text(&text)
            }
            Ok(ScreenTransfer::Backspace(count)) => {
                buffer.apply_backspaces(count)
            }
            Err(RecvTimeoutError::Timeout) => {
                buffer.flush()
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = buffer.flush();
                break;
            }
        };
    }
}

// -----------------------------------------------------------------------------
// PasteBuffer
// -----------------------------------------------------------------------------

/// Manages accumulation of text and backspace commands,
/// executing OS operations with debounce intervals.
struct _PasteBuffer {
    text_buf: String,
    last_paste: Instant,
    cooldown: Duration,
}

impl _PasteBuffer {

    fn new(cooldown: Duration) -> Self {
        _PasteBuffer {
            text_buf: String::new(),
            // Initialize in the past so the first paste can happen immediately.
            last_paste: Instant::now() - cooldown,
            cooldown,
        }
    }

    /// Calculates how much time is left before a paste is allowed.
    fn _time_to_next_paste(&self) -> Duration {
        let elapsed = self.last_paste.elapsed();
        if elapsed >= self.cooldown {
            Duration::ZERO
        } else {
            self.cooldown - elapsed
        }
    }

    /// Calculates the required timeout for the main loop.
    /// If the buffer is empty, returns `None` (wait indefinitely).
    fn _current_timeout(&self) -> Option<Duration> {
        if self.text_buf.is_empty() {
            None
        } else {
            Some(self._time_to_next_paste())
        }
    }

    /// Adds text to the buffer. Updates clipboard immediately.
    /// Returns the timeout until the buffer should be flushed.
    fn push_text(&mut self, new_text: &str) -> Option<Duration> {
        self.text_buf.push_str(new_text);

        // Updating the clipboard is cheap, do it immediately so it's ready.
        if let Err(e) = set_clipboard_text(&self.text_buf) {
            eprntln!("ScreenWriter: clipboard update failed: {}", e);
        }

        // If cooldown has already passed, we could theoretically flush right now.
        // However, waiting for the timeout allows rapid consecutive chunks to be coalesced.
        // The loop will immediately get a 0ms timeout and call flush() on the next iteration
        // if no more text is pending in the channel.
        self._current_timeout()
    }

    /// Applies backspaces.
    /// Shrinks the buffer first. If more backspaces are needed, flushes the remaining
    /// buffer and sends real keystrokes.
    fn apply_backspaces(&mut self, mut count: usize) -> Option<Duration> {

        // 1. Eat from the buffer first (no OS delay needed).
        let buf_len = self.text_buf.chars().count();
        if count <= buf_len {
            // Simply truncate the buffer.
            let keep_chars = buf_len - count;
            self.text_buf = self.text_buf.chars().take(keep_chars).collect();

            // Update clipboard to reflect truncation.
            if !self.text_buf.is_empty() {
                let _ = set_clipboard_text(&self.text_buf);
            }
            return self._current_timeout();
        }

        // Buffer completely eaten, but we still need more backspaces.
        count -= buf_len;
        self.text_buf.clear();

        // We don't need to flush the text buffer because it's empty now.
        // But we DO need to wait out the cooldown before sending real backspace keys!
        // Why? Because the target app might still be processing a previous `Ctrl+V`.
        // If we send Backspace too early, it might delete characters *before* the paste happens.
        let delay = self._time_to_next_paste();
        if delay > Duration::ZERO {
            thread::sleep(delay);
        }

        // 2. Send real backspaces to OS.
        for _ in 0..count {
            if let Err(e) = send_backspace() {
                eprntln!("ScreenWriter: send_backspace failed: {}", e);
                break;
            }
            // Small delay between physical keys for target app to process.
            thread::sleep(Duration::from_millis(10));
        }

        // We just interacted with the OS. Reset the cooldown timer.
        self.last_paste = Instant::now();
        None
    }

    /// Flushes accumulated text to the OS via clipboard paste.
    fn flush(&mut self) -> Option<Duration> {
        if self.text_buf.is_empty() {
            return None;
        }

        // Safety check: wait out remaining cooldown if called prematurely.
        let delay = self._time_to_next_paste();
        if delay > Duration::ZERO {
            thread::sleep(delay);
        }

        if let Err(e) = send_ctrl_v() {
            eprntln!("ScreenWriter: Ctrl+V failed: {}", e);
        }

        self.text_buf.clear();
        self.last_paste = Instant::now();
        None
    }
}