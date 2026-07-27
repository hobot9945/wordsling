//! screen_typer — Screen output via keyboard emulation.
//!
//! Receives text output and erase events from the text processor.
//! Outputs text to the focused Windows application by emulating
//! Unicode key presses via SendInput (KEYEVENTF_UNICODE).
//!
//! Unlike `screen_paster`, this module does not use the clipboard.
//! Each `ScreenTransfer::Text` is sent immediately without buffering,
//! maximizing reactivity.
//!
//! # RESPONSIBILITY
//! - Receive screen transfer events from the text processor.
//! - Type text into the focused window via `send_unicode_string()`.
//! - Execute backspaces via `send_backspace()`.
//! - Contain no protocol parsing or text interpretation logic.

use std::sync::mpsc::Receiver;
use std::thread;
use hobolib::keyboard::{send_backspace, send_shift_enter, send_unicode_string};
use crate::{log_err, log_inf};
use crate::core::screen_transfer::ScreenTransfer;

pub struct ScreenTyper {
    _handle: Option<thread::JoinHandle<()>>,
}   // ScreenTyper

impl ScreenTyper {

    /// Constructor.
    ///
    /// Spawns a worker thread that reads prepared screen transfer commands
    /// from `screen_cmd_rx` and forwards them to the currently focused window
    /// via Unicode keyboard emulation.
    ///
    /// # Parameters
    /// - `screen_cmd_rx`: receiver end of the channel from the text processor.
    pub fn new(screen_cmd_rx: Receiver<ScreenTransfer>) -> Self {

        let handle = thread::spawn(move || {
            Self::_screen_typer_loop(screen_cmd_rx);
        });

        ScreenTyper {
            _handle: Some(handle),
        }
    }   // new()

    /// Event loop.
    ///
    /// Blocks on the input channel. For each event:
    /// - `Text`: sends the string via `send_unicode_string()`.
    /// - `Backspace`: sends the requested number of backspace keystrokes.
    ///
    /// The loop exits when the input channel is closed (cascading shutdown).
    ///
    /// # Parameters
    /// - `screen_cmd_rx`: receiver end of the channel carrying screen transfer commands.
    fn _screen_typer_loop(screen_cmd_rx: Receiver<ScreenTransfer>) {

        for transfer in screen_cmd_rx {
            match transfer {

                ScreenTransfer::Text(text) => {
                    if !text.is_empty() {
                        hobolib::user_activity::suppress_input_tracking(150);
                        Self::_type_text(&text);
                    }   // if
                }

                ScreenTransfer::Backspace(count) => {
                    if count > 0 {
                        // Заглушка для трекера активности, чтобы не поймать собственные Backspace
                        hobolib::user_activity::suppress_input_tracking(150);

                        for _ in 0..count {
                            if let Err(e) = send_backspace() {
                                log_err!("ScreenTyper: send_backspace failed: {}", e);
                                break;
                            }   // if
                        }   // for
                    }   // if
                }

            }   // match
        }   // for
    }   // _screen_typer_loop()

    /// Выводит текст в активное окно, обрабатывая спецсимволы.
    ///
    /// Обычный текст отправляется через `send_unicode_string()`.
    /// Спецсимволы эмулируются как нажатия соответствующих клавиш.
    ///
    /// # Поддерживаемые спецсимволы
    /// - `\n` — Enter (VK_RETURN)
    ///
    /// # Параметры
    /// - `text`: строка для ввода.
    fn _type_text(text: &str) {
        let mut buf = String::new();

        for c in text.chars() {
            match c {
                '\n' => {
                    // Сначала сбросить накопленный обычный текст.
                    Self::_flush_buf(&mut buf);
                    // Эмулировать нажатие Enter.
                    if let Err(e) = send_shift_enter() {
                        log_err!("ScreenTyper: send_enter failed: {}", e);
                    }   // if
                }

                _ => {
                    buf.push(c);
                }
            }   // match
        }   // for

        // Сбросить остаток.
        Self::_flush_buf(&mut buf);
    }   // _type_text()

    /// Отправляет накопленный буфер обычного текста и очищает его.
    ///
    /// # Параметры
    /// - `buf`: буфер накопленного текста.
    fn _flush_buf(buf: &mut String) {
        if !buf.is_empty() {
            if let Err(e) = send_unicode_string(buf) {
                log_err!("ScreenTyper: send_unicode_string failed: {}", e);
            }   // if
            buf.clear();
        }   // if
    }   // _flush_buf()

}   // impl ScreenTyper

impl Drop for ScreenTyper {

    /// Destructor.
    /// Waits for the worker thread to finish and checks for panics.
    fn drop(&mut self) {

        if let Some(handle) = self._handle.take() {
            if let Err(panic_payload) = handle.join() {
                log_err!("ScreenTyper thread panicked: {:?}", panic_payload);
            }   // if
        }   // if

        log_inf!("ScreenTyper thread dropped");
    }   // drop()
}   // impl Drop for ScreenTyper

