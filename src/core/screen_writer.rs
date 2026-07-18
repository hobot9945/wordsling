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
//! - Preserve the user's clipboard content across paste sessions.
//! - Contain no protocol parsing or text interpretation logic.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};
use hobolib::clipboard::{get_clipboard_text, set_clipboard_text};
use hobolib::keyboard::{send_backspace, send_ctrl_v};
use crate::{log_err, log_inf};
use crate::core::screen_transfer::ScreenTransfer;

/// Cooldown interval between consecutive paste operations (Ctrl+V).
const _COOLDOWN_MS: Duration = Duration::from_millis(100);

/// Minimum delay between consecutive backspace keystrokes sent to the OS.
///
/// Originally introduced to prevent race conditions during rapid text deletion.
/// Observation showed that artificial delays are unnecessary because the OS
/// places keystrokes into the system input queue, guaranteeing safe sequential
/// execution. The value is currently set to zero, but the parameter is kept
/// for potential future tuning.
const _BACKSPACE_DELAY_MS: Duration = Duration::from_millis(0);

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

        let mut paste_buffer = _Debouncer::_new();
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
                    // Channel is closed. We must not only print the remaining tail,
                    // but also wait long enough to restore the clipboard if needed.
                    let mut tail_timeout = paste_buffer._flush();

                    while let Some(duration) = tail_timeout {
                        thread::sleep(duration);

                        // paste_buffer was clear by the previous _flash(). New call of _flush(),
                        // when paste_buffer is empty will restore the clipboard original content.
                        tail_timeout = paste_buffer._flush();
                    }   // while

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
// _Debouncer — debounced clipboard paste manager
// =============================================================================

/// Debounced clipboard paste buffer.
///
/// Accumulates incoming text fragments and pastes them into the focused window
/// as a single `Ctrl+V` operation after a cooldown interval elapses.
/// This prevents clipboard race conditions where the target application
/// reads stale clipboard content because a new `set_clipboard_text` call
/// overwrites it before the previous `Ctrl+V` is processed.
///
/// The user's original clipboard content is saved before the first paste
/// in a session and restored after the last paste completes. This ensures
/// that dictation does not destroy clipboard data the user copied earlier.
///
/// Backspace commands are first applied to the local buffer (free, no OS delay).
/// Only if the buffer is exhausted, real `send_backspace()` keystrokes are emitted.
struct _Debouncer {
    _text_buf: String,                    // accumulated text not yet pasted into the target window
    _last_paste: Instant,                 // timestamp of the last OS interaction (paste or backspace)
    _saved_clipboard_text: Option<String>,// user's clipboard text; Some(_) also indicates an active session
}   // _Debouncer
impl _Debouncer {

    /// Constructor.
    ///
    /// # Parameters
    fn _new() -> Self {
        _Debouncer {
            _text_buf: String::new(),
            // Initialize in the past so the first paste can happen immediately.
            _last_paste: Instant::now() - _COOLDOWN_MS,
            _saved_clipboard_text: None,
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

        if !self._text_buf.is_empty() || self._saved_clipboard_text.is_some() {
            Some(self._calculate_remaining_cooldown())
        } else {
            None
        }   // if
    }   // _recv_timeout()

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

        // Save the user's original text before our first clipboard override.
        self._capture_clipboard_if_needed();

        // Update clipboard immediately — cheap OS call.
        // Content will be ready when the cooldown expires.
        if let Err(e) = set_clipboard_text(&self._text_buf) {
            log_err!("Debouncer: clipboard update failed: {}", e);
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

            // If the local buffer becomes empty, the unprinted tail was consumed locally.
            // WARNING: We must NOT instantly restore the clipboard here, because
            // a PREVIOUS Ctrl+V operation might have been emitted just milliseconds ago.
            // Instead, we return the remaining cooldown. The event loop will wait it out,
            // fire a Timeout, and call _flush() which will safely restore the clipboard.
            if self._text_buf.is_empty() {
                if self._saved_clipboard_text.is_some() {
                    return Some(self._calculate_remaining_cooldown());
                } else {
                    return None;
                }   // if
            }   // if

            // Update clipboard to reflect the truncated buffer.
            if let Err(e) = set_clipboard_text(&self._text_buf) {
                log_err!("Debouncer: clipboard update failed: {}", e);
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
                log_err!("Debouncer: send_backspace failed: {}", e);
                break;
            }   // if
            thread::sleep(_BACKSPACE_DELAY_MS);
        }   // for

        // Reset the cooldown timer after OS interaction.
        self._last_paste = Instant::now();

        // After emitting real backspaces, our clipboard session is effectively over:
        // any preceding paste operation is guaranteed to have finished processing
        // thanks to the cooldown wait above.
        self._finish_clipboard_session();
        None
    }   // _apply_backspaces()

    /// Flushes the accumulated text to the focused window via `Ctrl+V`.
    ///
    /// If the buffer is empty and a clipboard session is active, waits out
    /// the remaining cooldown and restores the user's original clipboard content.
    ///
    /// If the buffer is non-empty, waits out the remaining cooldown,
    /// sends the paste keystroke, clears the buffer, and resets the cooldown timer.
    ///
    /// # Returns
    /// - `None` if nothing remains to do (buffer empty, clipboard restored or inactive).
    /// - `Some(duration)` if the clipboard session is still active after a paste
    ///   and the caller must wait before restoring.
    fn _flush(&mut self) -> Option<Duration> {

        // If there is nothing to print, but the clipboard session is still active,
        // it is time to restore the user's original clipboard content.
        if self._text_buf.is_empty() {
            if self._saved_clipboard_text.is_some() {
                // CRITICAL SAFETY GUARD:
                // If _flush() was invoked prematurely (e.g. channel Disconnected
                // right after a paste), we MUST wait out the remaining cooldown
                // synchronously before restoring the clipboard. Otherwise, we break
                // the async Ctrl+V operation in the target window.
                let delay = self._calculate_remaining_cooldown();
                if delay > Duration::ZERO {
                    thread::sleep(delay);
                }   // if

                self._finish_clipboard_session();
            }   // if
            return None;
        }   // if

        // Wait out any remaining cooldown before pressing Ctrl+V.
        let delay = self._calculate_remaining_cooldown();
        if delay > Duration::ZERO {
            thread::sleep(delay);
        }   // if

        if let Err(e) = send_ctrl_v() {
            log_err!("Debouncer: Ctrl+V failed: {}", e)

        }   // if

        self._text_buf.clear();
        self._last_paste = Instant::now();

        // If the clipboard was overridden, do not restore it immediately:
        // the target application needs time to process the Ctrl+V and read our text.
        // We return the cooldown duration to let the main loop wait asynchronously.
        if self._saved_clipboard_text.is_some() {
            Some(_COOLDOWN_MS)
        } else {
            None
        }   // if
    }   // _flush()

    /// Saves the text content of the system clipboard before our first override.
    ///
    /// The capture is performed only once per active clipboard paste session
    /// (when `_saved_clipboard_text` is `None`). If the clipboard contains non-text
    /// data (like an image) or is empty, we store an empty string. This allows us
    /// to mark the session as "active" without crashing.
    fn _capture_clipboard_if_needed(&mut self) {

        if self._saved_clipboard_text.is_none() {
            // Read text. If it fails (e.g. image or empty), default to an empty string.
            let text = get_clipboard_text().unwrap_or_default();
            self._saved_clipboard_text = Some(text);
        }   // if
    }   // _capture_clipboard_if_needed()

    /// Ends the current clipboard paste session.
    ///
    /// If there is a saved clipboard string and it is not empty, it restores it
    /// back to the system clipboard. The session is deactivated by `take()`.
    fn _finish_clipboard_session(&mut self) {

        if let Some(text) = self._saved_clipboard_text.take() {
            if !text.is_empty() {
                if let Err(e) = set_clipboard_text(&text) {
                    log_err!("Debouncer: failed to restore saved clipboard text: {}", e);
                }   // if
            }   // if
        }   // if
    }   // _finish_clipboard_session()
}   // impl _Debouncer