//! screen_writer.rs — Screen output manager.
//!
//! Receives text output and erase events (commands) from the text processor. Outputs text to the focused
//! Windows application using clipboard paste. To avoid spamming paste keystrokes, a debouncing
//! mechanism is applied. Erasings are executed by emulating Backspace key presses with fixed
//! delay intervals to avoid races.
//!
//! Debouncing is organized as follows: text fragments are pasted either upon receiving an event
//! from the text processor (provided the cooldown period has elapsed) or when the channel's idle
//! timer expires. With each incoming event, the channel timeout is automatically adjusted to the
//! remaining cooldown interval.
//!
//! # RESPONSIBILITY
//! - Receive screen transfer events from the text processor.
//! - Accumulate text in a debounced paste buffer to avoid clipboard race conditions.
//! - Execute backspaces by first consuming the local buffer, then emitting real keystrokes.
//! - Contain no protocol parsing or text interpretation logic.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};
use hobolib::clipboard::set_clipboard_text;
use hobolib::eprntln;
use hobolib::keyboard::{send_backspace, send_ctrl_v};
use crate::{log_err, log_inf};
use crate::screen_transfer::ScreenTransfer;

/// Cooldown interval between consecutive paste operations (Ctrl+V).
const _COOLDOWN_MS: Duration = Duration::from_millis(100);

/// Minimum delay between consecutive backspace keystrokes sent to the OS.
const _BACKSPACE_DELAY_MS: Duration = Duration::from_millis(10);

/// Minimum delay required between updating the clipboard and pasting.
const _CLIPBOARD_ASSIMILATION_MS: Duration = Duration::from_millis(20);

pub struct ScreenWriter {
    // Thread handle. Used during shutdown to perform join() from the main thread.
    _handle: Option<thread::JoinHandle<()>>,
}   // ScreenWriter

impl ScreenWriter {

    /// Constructor.
    ///
    /// Spawns a worker thread that reads prepared screen transfer commands
    /// from `write_cmd_rx` and forwards them to the currently focused window.
    ///
    /// # Parameters
    /// - `write_cmd_rx`: receiver end of the channel from the text processor.
    pub fn new(write_cmd_rx: Receiver<ScreenTransfer>) -> Self {

        let handle = thread::spawn(move || {
            Self::_screen_writer_loop(write_cmd_rx);
        });

        ScreenWriter {
            _handle: Some(handle),
        }
    }   // new()


    /// Event loop of the screen writer.
    ///
    /// Drives the `_PasteBuffer` state machine using events from the input channel.
    /// The loop itself contains no business logic — it only reads events and dispatches
    /// them to the buffer, using the returned timeout for the next iteration.
    ///
    /// # Parameters
    /// - `screen_cmd_rx`: receiver end of the channel carrying screen transfer commands.
    fn _screen_writer_loop(screen_cmd_rx: Receiver<ScreenTransfer>) {

        let mut paste_buffer = _PasteBuffer::_new();
        let mut recv_timeout: Option<Duration> = None;

        loop {
            // Wait for an event from the channel, or for the recv timeout to expire.
            let recv_result = match recv_timeout {

                // Data is pending in the buffer. Waiting is limited by the cooldown.
                Some(duration) => screen_cmd_rx.recv_timeout(duration),

                // Wait indefinitely when there is nothing to flush.
                None => screen_cmd_rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
            };

            // Dispatch the event to the paste buffer.
            recv_timeout = match recv_result {
                Ok(ScreenTransfer::Text(text)) => {
                    paste_buffer._push_text(&text)
                }
                Ok(ScreenTransfer::Backspace(count)) => {
                    paste_buffer._apply_backspaces(count)
                }
                Err(RecvTimeoutError::Timeout) => {
                    paste_buffer._flush()
                }
                Err(RecvTimeoutError::Disconnected) => {
                    // Input channel closed. Flush remaining text and exit.
                    let _ = paste_buffer._flush();
                    break;
                }
            };  // match
        }   // loop
    }   // _screen_writer_loop()
}   // impl ScreenWriter

impl Drop for ScreenWriter {

    /// Destructor.
    /// Waits for the worker thread to finish and checks for panics.
    fn drop(&mut self) {

        if let Some(handle) = self._handle.take() {
            if let Err(panic_payload) = handle.join() {
                log_err!("ScreenWriter thread panicked: {:?}", panic_payload);
            }   // if
        }   // if

        log_inf!("ScreenWriter thread dropped");
    }   // drop()
}   // impl Drop for ScreenWriter


// =============================================================================
// _PasteBuffer — debounced clipboard paste manager
// =============================================================================

/// Debounced clipboard paste buffer.
///
/// Accumulates incoming text fragments and pastes them into the focused window
/// as a single `Ctrl+V` operation after a cooldown interval elapses.
/// This prevents clipboard race conditions where the target application
/// reads stale clipboard content because a new `set_clipboard_text` call
/// overwrites it before the previous `Ctrl+V` is processed.
///
/// Backspace commands are first applied to the local buffer (free, no OS delay).
/// Only if the buffer is exhausted, real `send_backspace()` keystrokes are emitted.
struct _PasteBuffer {
    _text_buf: String,       // accumulated text not yet pasted
    _last_paste: Instant,    // timestamp of the last Ctrl+V or Backspace sent to OS
}   // _PasteBuffer

impl _PasteBuffer {

    /// Constructor.
    ///
    /// # Parameters
    fn _new() -> Self {

        _PasteBuffer {
            _text_buf: String::new(),
            // Initialize in the past so the first paste can happen immediately.
            _last_paste: Instant::now() - _COOLDOWN_MS,
        }
    }   // _new()

    /// Calculates remaining time before the next OS interaction is allowed.
    ///
    /// # Returns
    /// `Duration::ZERO` if the cooldown has already elapsed,
    /// otherwise the remaining time.
    fn _calculate_remaining_cooldown(&self) -> Duration {

        let elapsed = self._last_paste.elapsed();
        if elapsed >= _COOLDOWN_MS {
            Duration::ZERO
        } else {
            _COOLDOWN_MS - elapsed
        }   // if
    }   // _time_to_next_paste()

    /// Calculates the timeout for the main event loop.
    ///
    /// # Returns
    /// - `None` if the buffer is empty (the loop should wait indefinitely).
    /// - `Some(duration)` if the buffer is non-empty (the loop should wait
    ///   at most `duration` before triggering a flush).
    fn _recv_timeout(&self) -> Option<Duration> {

        if self._text_buf.is_empty() {
            None
        } else {
            Some(self._calculate_remaining_cooldown())
        }   // if
    }   // _current_timeout()

    /// Appends text to the internal buffer and updates the clipboard.
    ///
    /// Updating the clipboard is cheap and is done immediately so that the content
    /// is ready when `Ctrl+V` is eventually pressed. The actual paste is deferred
    /// until the cooldown interval elapses, allowing consecutive text fragments
    /// to be coalesced into a single paste operation.
    ///
    /// The returned timeout ensures that the event loop will wait at least
    /// `_CLIPBOARD_ASSIMILATION_MS` before triggering a flush, giving the OS
    /// enough time to process the clipboard update without blocking the thread.
    ///
    /// # Parameters
    /// - `new_text`: text fragment to append.
    ///
    /// # Returns
    /// Timeout for the main event loop (`None` if nothing to flush).
    fn _push_text(&mut self, new_text: &str) -> Option<Duration> {

        // Append incoming fragment to the accumulation buffer.
        self._text_buf.push_str(new_text);

        // Edge case guard: empty fragment into an already empty buffer.
        // Nothing to flush; signal the loop to wait indefinitely.
        if self._text_buf.is_empty() {
            return None;
        }   // if

        // Update clipboard immediately — cheap OS call.
        // Content will be ready when the cooldown expires.
        if let Err(e) = set_clipboard_text(&self._text_buf) {
            log_err!("PasteBuffer: clipboard update failed: {}", e);
        }   // if

        // Calculate remaining time until the cooldown allows a new paste.
        let time_to_flush = self._calculate_remaining_cooldown();

        // Enforce a minimum delay to guarantee the clipboard has stabilized.
        Some(if time_to_flush < _CLIPBOARD_ASSIMILATION_MS {
            _CLIPBOARD_ASSIMILATION_MS
        } else {
            time_to_flush
        })
    }   // _push_text()

    /// Applies backspace commands.
    ///
    /// First consumes characters from the local buffer (no OS delay needed).
    /// If the buffer is exhausted and more backspaces are still required,
    /// waits out the remaining cooldown interval (to avoid racing with a pending
    /// `Ctrl+V`) and then emits real `send_backspace()` keystrokes to the OS.
    ///
    /// # Parameters
    /// - `count`: number of characters to delete.
    ///
    /// # Returns
    /// Timeout for the main event loop.
    fn _apply_backspaces(&mut self, mut count: usize) -> Option<Duration> {

        // Step 1: consume characters from the buffer (no OS delay needed).
        let buf_char_len = self._text_buf.chars().count();

        if count <= buf_char_len {
            // Remove 'count' characters safely handling UTF-8 boundaries.
            for _ in 0..count {
                self._text_buf.pop();
            }   // for

            // If the buffer is now empty, there's nothing to flush.
            if self._text_buf.is_empty() {
                return None;
            }   // if

            // Update clipboard to reflect the truncated buffer.
            if let Err(e) = set_clipboard_text(&self._text_buf) {
                log_err!("PasteBuffer: clipboard update failed: {}", e);
            }   // if

            // Ensure the OS has time to assimilate the new clipboard state.
            let time_to_flush = self._calculate_remaining_cooldown();
            return Some(if time_to_flush < _CLIPBOARD_ASSIMILATION_MS {
                _CLIPBOARD_ASSIMILATION_MS
            } else {
                time_to_flush
            });
        }   // if

        // Buffer is completely consumed, but more backspaces are still needed.
        count -= buf_char_len;
        self._text_buf.clear();

        // Step 2: wait out the cooldown before sending real keystrokes.
        // A previous Ctrl+V might still be in the target app's input queue.
        // Sending Backspace too early could delete characters before the paste arrives.
        let delay = self._calculate_remaining_cooldown();
        if delay > Duration::ZERO {
            thread::sleep(delay);
        }   // if

        // Step 3: emit real backspace keystrokes to the OS.
        for _ in 0..count {
            if let Err(e) = send_backspace() {
                log_err!("PasteBuffer: send_backspace failed: {}", e);
                break;
            }   // if
            thread::sleep(_BACKSPACE_DELAY_MS);
        }   // for

        // Reset the cooldown timer after OS interaction.
        self._last_paste = Instant::now();
        None
    }   // _apply_backspaces()

    /// Flushes the accumulated text to the focused window via `Ctrl+V`.
    ///
    /// If the buffer is empty, does nothing. Otherwise, waits out the remaining
    /// cooldown interval (if any), sends the paste keystroke, clears the buffer,
    /// and resets the cooldown timer.
    ///
    /// # Returns
    /// Always `None` (the buffer is empty after flush).
    fn _flush(&mut self) -> Option<Duration> {

        if self._text_buf.is_empty() {
            return None;
        }   // if

        // Wait out any remaining cooldown before pressing Ctrl+V.
        let delay = self._calculate_remaining_cooldown();
        if delay > Duration::ZERO {
            thread::sleep(delay);
        }   // if

        if let Err(e) = send_ctrl_v() {
            log_err!("PasteBuffer: Ctrl+V failed: {}", e);
        }   // if

        self._text_buf.clear();
        self._last_paste = Instant::now();
        None
    }   // _flush()

}   // impl _PasteBuffer