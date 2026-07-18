use crate::core::lexeme_transfer::LexemeTransfer;
use crate::core::screen_transfer::ScreenTransfer;

/// The surgical table manages the raw input from Gboard and the processed
/// output sent to the Windows screen. It encapsulates text substitutions,
/// erasures, and stabilization logic.
#[derive(Default)]
pub(super) struct SurgeTable {
    cutting_board: String,
    franken_board: String,
    comb: Vec<Prong>,

    /// Anchor point for Gboard stabilizations (index in cutting_board).
    /// Gboard erase commands cannot delete text prior to this point.
    gboard_anchor: usize,

    /// Internal queue for generated screen commands awaiting dispatch.
    pending_transfers: Vec<ScreenTransfer>,
}

impl SurgeTable {
    pub(super) fn new() -> Self {
        SurgeTable::default()
    }

    /// Processes a single incoming lexeme, updates internal boards,
    /// and generates output screen transfers if necessary.
    ///
    /// Currently, implements a minimal pass-through logic to keep
    /// the pipeline operational.
    pub(super) fn process_lexeme(&mut self, lexeme: &LexemeTransfer) {
        match lexeme {
            LexemeTransfer::WordPart(text) => {
                self.pending_transfers.push(ScreenTransfer::Text(text.clone()));
            }

            LexemeTransfer::Whitespace(c) | LexemeTransfer::Punctuation(c) => {
                self.pending_transfers.push(ScreenTransfer::Text(c.to_string()));
            }

            LexemeTransfer::BackspaceCount(n) => {
                self.pending_transfers.push(ScreenTransfer::Backspace(*n as usize));
            }

            // Non-significant lexemes are currently ignored.
            LexemeTransfer::WordStart
            | LexemeTransfer::WordEnd
            | LexemeTransfer::EraseStart
            | LexemeTransfer::EraseEnd
            | LexemeTransfer::Stabilization
            | LexemeTransfer::UserActivityDetected => {}
        }
    }

    /// Extracts all accumulated screen transfers and clears the internal queue.
    pub(super) fn pop_screen_transfers(&mut self) -> Vec<ScreenTransfer> {
        // std::mem::take leaves an empty Vec in place of the old one,
        // which is highly efficient.
        std::mem::take(&mut self.pending_transfers)
    }
}

impl Drop for SurgeTable {
    fn drop(&mut self) {
        // Cleanup logic if needed in the future.
    }
}

/// Represents an index mapping between the raw cutting board
/// and the processed franken board.
#[derive(Default)]
struct Prong {
    cboard_ind: usize,
    fboard_ind: usize,
}