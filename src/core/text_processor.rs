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

#![allow(unused)]
mod surgical_table;

use std::sync::mpsc::{Receiver, SendError, Sender};
use std::thread;
use crate::core::lexeme_transfer::LexemeTransfer;
use crate::{log_err, log_inf};
use crate::core::screen_transfer::ScreenTransfer;
use crate::core::text_processor::surgical_table::SurgeTable;

/// A wrapper around a thread, executing the text_processor state machine.
pub struct FrankenLab {
    _handle: Option<thread::JoinHandle<()>>,
}   // FrankenLab

impl FrankenLab {

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
            let _ = Self::_franken_loop(lexeme_rx, write_cmd_tx);
        });

        FrankenLab {
            _handle: Some(handle),
        }
    }   // new()

}   // impl FrankenLab

impl Drop for FrankenLab {

    /// Destructor.
    /// Waits for the worker thread to finish.
    fn drop(&mut self) {

        if let Some(handle) = self._handle.take() {
            if let Err(panic_payload) = handle.join() {
                log_err!("FrankenLab thread panicked: {:?}", panic_payload);
            }   // if
        }   // if

        log_inf!("FrankenLab thread dropped");
    }   // fn drop()
}   // impl Drop for FrankenLab

impl FrankenLab {
    /// Text processor loop.
    ///
    /// Delegates all protocol parsing and text manipulation to `SurgeTable`.
    /// Acts as a dispatcher: feeds lexemes to the table and forwards the
    /// generated screen transfers to the screen writer.
    ///
    /// # Parameters
    /// - `lexeme_rx`: receiver end of the channel carrying parsed lexemes from the lexer.
    /// - `screen_cmd_tx`: sender end of the channel carrying screen transfer commands to the screen writer.
    ///
    /// # Returns
    /// - `Ok(())` if the input channel was closed normally (lexer stopped).
    /// - `Err(SendError)` if the output channel was closed (screen writer stopped).
    fn _franken_loop(
        lexeme_rx: Receiver<LexemeTransfer>,
        screen_cmd_tx: Sender<ScreenTransfer>,
    ) -> Result<(), SendError<ScreenTransfer>> {

        // The surgical table must outlive individual lexemes to preserve state.
        let mut surge_table = SurgeTable::new();

        for lexeme in lexeme_rx {

            // 1. Feed the lexeme to the surgical table.
            surge_table.process_lexeme(&lexeme);

            // 2. Extract generated screen commands and send them to the writer.
            let transfers = surge_table.pop_screen_transfers();
            for transfer in transfers {
                screen_cmd_tx.send(transfer)?;
            }

        }   // for lexeme

        Ok(())
    }   // _franken_loop()
}   // impl FrankenLab