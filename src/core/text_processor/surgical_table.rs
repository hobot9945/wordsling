//! surgical_table.rs — The surgical table for text processing.
//!
//! Manages the raw input from Gboard (`cutting_board`) and the processed
//! output mirroring the Windows screen (`franken_board`). Handles erasures,
//! stabilization anchors, and (in the future) text substitutions.
//!
//! # RESPONSIBILITY
//! - Store the raw and processed text as `Vec<char>` for safe character-level operations.
//! - Enforce Gboard stabilization boundaries on erase commands.
//! - Generate `ScreenTransfer` commands in response to incoming lexemes.
//! - Maintain a mapping (`comb`) between raw and processed text for future substitutions.

use crate::core::lexeme_transfer::LexemeTransfer;
use crate::core::screen_transfer::ScreenTransfer;
use crate::core::text_processor::substitution_map::SubstitutionMap;

/// Maximum number of characters retained in the boards.
/// Prevents unbounded growth during long dictation sessions
/// without user activity resets.
const _MAX_BOARD_CAPACITY: usize = 2000;

#[derive(Default)]
pub(super) struct Flags {
    pub(super) dummy_flag: bool,
}

impl Flags {
    fn new() -> Self {
        Self::default()
    }
}

/// The surgical table manages the relationship between the raw Gboard input
/// and the processed text that has been sent to the screen.
///
/// `cutting_board` holds the raw stream from Gboard (after lexer decapitalization).
/// `franken_board` mirrors what the focused Windows input field should contain.
/// Currently, these two boards are identical (no substitutions implemented yet).
pub(super) struct SurgeTable {
    /// Raw text received from Gboard (via lexer).
    _cutting_board: Vec<char>,

    /// Processed text mirroring the screen content.
    _franken_board: Vec<char>,

    /// Index mapping for substitutions. A prong marks the boundaries
    /// of a replaced segment in both boards. Only exists in the unstable
    /// region; cleared on stabilization. Currently unused.
    _comb: Vec<_Prong>,


    pub(super) flags: Flags,

    /// Gboard stabilization anchor (character index in `_cutting_board`).
    /// Gboard erase commands cannot delete text before this point.
    _stabilization_anchor: usize,

    _substitution: SubstitutionMap,
    
    /// Internal queue of screen commands awaiting dispatch.
    _pending_transfers: Vec<ScreenTransfer>,
}   // SurgeTable

impl SurgeTable {

    /// Constructor.
    pub(super) fn new() -> Self {
        SurgeTable {
            _cutting_board: Vec::new(),
            _franken_board: Vec::new(),
            _comb: Vec::new(),
            flags: Flags::new(),
            _stabilization_anchor: 0,
            _substitution: SubstitutionMap::new(),
            _pending_transfers: Vec::new(),
        }
    }   // new()

    /// Processes a single incoming lexeme.
    ///
    /// Updates internal boards and generates screen transfer commands
    /// as needed. The caller must retrieve generated commands via
    /// `pop_screen_transfers()` after each call.
    pub(super) fn process_lexeme(&mut self, lexeme: &LexemeTransfer) {
        
        match lexeme {

            LexemeTransfer::WordPart(text) => {
                self._push_text(text);
            }

            LexemeTransfer::Whitespace(c) | LexemeTransfer::Punctuation(c) => {
                self._push_char(*c);
            }

            LexemeTransfer::BackspaceCount(n) => {
                self._apply_gboard_erase(*n as usize);
            }

            LexemeTransfer::Stabilization => {
                self._mark_gboard_stabilization();
            }

            LexemeTransfer::UserActivityDetected => {
                self._clear_all();
            }

            // Non-significant lexemes: silently consumed.
            LexemeTransfer::WordStart
            | LexemeTransfer::WordEnd
            | LexemeTransfer::EraseStart
            | LexemeTransfer::EraseEnd => {}

        }   // match
    }   // process_lexeme()

    /// Extracts all accumulated screen transfers and clears the internal queue.
    pub(super) fn pop_screen_transfers(&mut self) -> Vec<ScreenTransfer> {
        std::mem::take(&mut self._pending_transfers)
    }   // pop_screen_transfers()

    /// Appends a text fragment to both boards and enqueues a screen command.
    fn _push_text(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        self._cutting_board.extend_from_slice(&chars);
        self._franken_board.extend_from_slice(&chars);
        self._pending_transfers.push(ScreenTransfer::Text(text.to_string()));
        self._prune_if_needed();
    }   // _push_text()

    /// Appends a single character to both boards and enqueues a screen command.
    fn _push_char(&mut self, c: char) {
        self._cutting_board.push(c);
        self._franken_board.push(c);
        self._pending_transfers.push(ScreenTransfer::Text(c.to_string()));
        self._prune_if_needed();
    }   // _push_char()

    /// Applies an erase command from Gboard.
    ///
    /// Respects the Gboard stabilization anchor: erase cannot
    /// delete characters before `_stabilization_anchor`.
    /// Uses the `_franken_board` to determine how many screen
    /// backspaces to emit (currently 1:1 with cutting_board).
    fn _apply_gboard_erase(&mut self, n: usize) {
        let current_len = self._cutting_board.len();

        // How far back Gboard wants to go.
        let target_len = current_len.saturating_sub(n);

        // Clamp to the stabilization anchor.
        let target_len = target_len.max(self._stabilization_anchor);

        // Actual number of characters to erase.
        let actual_erase = current_len - target_len;

        if actual_erase == 0 {
            return;
        }   // if

        self._cutting_board.truncate(target_len);
        self._franken_board.truncate(target_len);

        // Remove any comb prongs that fall beyond the new length.
        self._trim_comb(target_len);

        self._pending_transfers.push(ScreenTransfer::Backspace(actual_erase));
    }   // _apply_gboard_erase()

    /// Records a Gboard stabilization event.
    ///
    /// Moves the anchor to the current end of the cutting board.
    /// Clears all comb prongs (they only exist in the unstable region).
    fn _mark_gboard_stabilization(&mut self) {
        self._stabilization_anchor = self._cutting_board.len();
        self._comb.clear();
    }   // _mark_gboard_stabilization()

    /// Clears all state. Called when user activity is detected
    /// (mouse click, keyboard input on the host), meaning the cursor
    /// position is no longer known and the franken_board no longer
    /// reflects the screen content.
    fn _clear_all(&mut self) {
        self._cutting_board.clear();
        self._franken_board.clear();
        self._comb.clear();
        self._stabilization_anchor = 0;
    }   // _clear_all()

    /// Removes comb prongs that reference positions beyond `new_len`.
    ///
    /// Stub: currently the comb is always empty (no substitutions).
    fn _trim_comb(&mut self, _new_len: usize) {
        // Will be implemented when substitutions are added.
        // For now, the comb is always empty.
    }   // _trim_comb()

    /// Trims the oldest portion of the boards when they exceed
    /// `_MAX_BOARD_CAPACITY`. Adjusts the anchor and comb accordingly.
    ///
    /// Stub: will be implemented when needed.
    fn _prune_if_needed(&mut self) {
        // TODO: implement sliding window pruning.
    }   // _prune_if_needed()

}   // impl SurgeTable

impl Drop for SurgeTable {
    fn drop(&mut self) {
        // Cleanup logic if needed in the future.
    }   // drop()
}   // impl Drop for SurgeTable

/// Represents an index mapping between a segment in the cutting board
/// and the corresponding segment in the franken board.
/// Used exclusively for text substitutions (future feature).
#[derive(Default)]
struct _Prong {
    _cboard_ind: usize,
    _fboard_ind: usize,
}   // _Prong