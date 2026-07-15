//! text_processor.rs — Post-lexical text processor.
//!
//! Consumes lexemes from the lexer and produces screen transfer commands
//! for the screen writer.
//!
//! # RESPONSIBILITY
//! - Maintain the distinction between stable and unstable text.
//! - Apply backspace commands to the unstable tail.
//! - Decide when text is ready to be flushed to the screen writer.
//! - Restore proper capitalization (future).
//! - Perform text substitutions and action commands (future).
//!
//! # CURRENT STATE
//! Transparent pass-through. All significant lexemes are forwarded
//! to the screen writer without any processing.

use std::sync::mpsc::{Receiver, SendError, Sender};
use std::thread;
use hobolib::eprntln;
use crate::lexeme_transfer::LexemeTransfer;
use crate::screen_transfer::ScreenTransfer;

pub struct TextProcessor {
    _handle: Option<thread::JoinHandle<()>>,
}   // TextProcessor

impl TextProcessor {

    /// Constructor.
    ///
    /// Spawns a worker thread that reads lexemes from `lexeme_rx`,
    /// processes them, and sends screen transfer commands into `write_cmd_tx`.
    ///
    /// The worker thread terminates gracefully when `lexeme_rx` is closed
    /// (i.e., when the lexer drops its sender).
    ///
    /// # Parameters
    /// - `lexeme_rx`: receiver end of the channel from the lexer.
    /// - `write_cmd_tx`: sender end of the channel to the screen writer.
    pub fn new(lexeme_rx: Receiver<LexemeTransfer>, write_cmd_tx: Sender<ScreenTransfer>) -> Self {

        let handle = thread::spawn(move || {
            // The loop returns Err when the output channel is closed,
            // which is the normal cascading shutdown path.
            let _ = _processor_loop(lexeme_rx, write_cmd_tx);
        });

        TextProcessor {
            _handle: Some(handle),
        }
    }   // new()

}   // impl TextProcessor

impl Drop for TextProcessor {

    /// Destructor.
    /// Waits for the worker thread to finish.
    fn drop(&mut self) {

        if let Some(handle) = self._handle.take() {
            if let Err(panic_payload) = handle.join() {
                eprntln!("TextProcessor thread panicked: {:?}", panic_payload);
            }   // if
        }   // if

        hobolib::prntln!("TextProcessor thread dropped");
    }
}   // impl Drop for TextProcessor

/// Transparent pass-through processor loop.
///
/// Forwards all significant lexemes (text, punctuation, whitespace, backspaces)
/// to the screen writer without modification. Non-significant lexemes
/// (`WordStart`, `WordEnd`, `EraseStart`, `EraseEnd`, `Stabilization`)
/// are silently consumed.
///
/// This will be replaced with full post-lexical processing later.
///
/// # Parameters
/// - `lexeme_rx`: receiver end of the channel carrying parsed lexemes from the lexer.
/// - `write_cmd_tx`: sender end of the channel carrying screen transfer commands to the screen writer.
///
/// # Returns
/// - `Ok(())` if the input channel was closed normally (lexer stopped).
/// - `Err(SendError)` if the output channel was closed (screen writer stopped).
fn _processor_loop(
    lexeme_rx: Receiver<LexemeTransfer>,
    write_cmd_tx: Sender<ScreenTransfer>,
) -> Result<(), SendError<ScreenTransfer>> {

    for lexeme in lexeme_rx {

        match lexeme {

            LexemeTransfer::WordPart(text) => {
                write_cmd_tx.send(ScreenTransfer::Text(text))?;
            }

            LexemeTransfer::Whitespace(c) => {
                write_cmd_tx.send(ScreenTransfer::Text(c.to_string()))?;
            }

            LexemeTransfer::Punctuation(c) => {
                write_cmd_tx.send(ScreenTransfer::Text(c.to_string()))?;
            }

            LexemeTransfer::BackspaceCount(n) => {
                write_cmd_tx.send(ScreenTransfer::Backspace(n as usize))?;
            }

            // Non-significant lexemes: silently consumed at this stage.
            LexemeTransfer::WordStart
            | LexemeTransfer::WordEnd
            | LexemeTransfer::EraseStart
            | LexemeTransfer::EraseEnd
            | LexemeTransfer::Stabilization => {}

        }   // match

    }   // for lexeme

    Ok(())
}   // _processor_loop()