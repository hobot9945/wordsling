//! lexer.rs — Lexeme parser.
//!
//! Converts a continuous stream of decoded text chunks into a stream of lexemes.
//! Operates as a state machine in its own worker thread.
//!
//! # RESPONSIBILITY
//! - Read text chunks from the input channel (connected to the TCP server).
//! - Parse the stream character by character, maintaining state across chunk boundaries.
//! - Emit lexemes into the output channel for consumption by the text processor.
//!
//! # INVARIANTS
//! - No artificial delays (`sleep` or similar).
//! - Significant text characters (words, punctuation, whitespace) are never lost or reordered.
//! - TCP chunk boundaries do not affect the semantic result.
//! - All word content is lowercased before emission.

use std::sync::mpsc::{Receiver, SendError, Sender};
use std::thread;
use hobolib::eprntln;
use crate::lexeme_transfer::LexemeTransfer;

/// Predefined set of punctuation characters.
const PUNCTUATION: &[char] = &['.', ',', ';', ':', '?', '!', '#', '/', '\'', '@', '-'];

/// Internal state of the lexer's state machine.
///
/// Tracks the current parsing context across character and chunk boundaries.
enum _LexerState {
    /// Outside any word or service construct.
    _Normal,
    /// Inside a word (between WordStart and WordEnd).
    _InWord,
    /// After `[`, reading digits of the backspace count.
    _InEraseCommand(String),
}   // _LexerState

pub struct Lexer {
    _handle: Option<thread::JoinHandle<()>>,
}   // Lexer

impl Lexer {

    /// Constructor.
    ///
    /// Spawns a worker thread that reads text chunks from `text_rx`,
    /// parses them into lexemes, and sends the lexemes into `lexeme_tx`.
    ///
    /// The worker thread terminates gracefully when `text_rx` is closed
    /// (i.e., when the TCP server drops its sender).
    ///
    /// # Parameters
    /// - `text_rx`: receiver end of the channel from the TCP server.
    /// - `lexeme_tx`: sender end of the channel to the text processor.
    pub fn new(text_rx: Receiver<String>, lexeme_tx: Sender<LexemeTransfer>) -> Self {

        let handle = thread::spawn(move || {
            // The loop returns Err when the output channel is closed,
            // which is the normal cascading shutdown path.
            let _ = _lexer_loop(text_rx, lexeme_tx);
        });

        Lexer {
            _handle: Some(handle),
        }
    }   // new()

}   // impl Lexer

impl Drop for Lexer {

    /// Destructor.
    /// Waits for the worker thread to finish.
    fn drop(&mut self) {

        if let Some(handle) = self._handle.take() {
            if let Err(panic_payload) = handle.join() {
                eprntln!("Lexer thread panicked: {:?}", panic_payload);
            }   // if
        }   // if

        hobolib::prntln!("Lexer thread dropped");
    }
}   // impl Drop for Lexer

/// Checks whether a character belongs to the predefined punctuation set.
///
/// # Parameters
/// - `c`: character to check.
///
/// # Returns
/// `true` if `c` is in the `PUNCTUATION` list.
fn _is_punctuation(c: char) -> bool {
    PUNCTUATION.contains(&c)
}   // _is_punctuation()

/// Main lexer loop.
///
/// Reads chunks from `text_rx`, processes them character by character,
/// and emits lexemes into `lexeme_tx`. Returns `Err` when the output
/// channel is closed, which terminates the loop as part of cascading shutdown.
///
/// # Parameters
/// - `text_rx`: receiver end of the channel carrying raw text chunks from the TCP server.
/// - `lexeme_tx`: sender end of the channel carrying parsed lexemes to the text processor.
///
/// # Returns
/// - `Ok(())` if the input channel was closed normally (TCP server stopped).
/// - `Err(SendError)` if the output channel was closed (downstream consumer stopped).
///
/// # Algorithm
/// The loop maintains a `_LexerState` across chunks.
/// For each character:
/// - If it is `*`: flush any pending word, emit `Stabilization`.
/// - If it is `[`: flush any pending word, emit `EraseStart`, switch to `_InEraseCommand`.
/// - If in `_InEraseCommand` and digit: accumulate digit.
/// - If in `_InEraseCommand` and `]`: parse count, emit `BackspaceCount` + `EraseEnd`.
/// - If whitespace: flush any pending word, emit `Whitespace(c)`.
/// - If punctuation: flush any pending word, emit `Punctuation(c)`.
/// - Otherwise: if not in word, emit `WordStart`; accumulate into word buffer.
///
/// At the end of each chunk, any accumulated word buffer is flushed as `WordPart`
/// (but `WordEnd` is NOT emitted, because the word may continue in the next chunk).
fn _lexer_loop(
    text_rx: Receiver<String>,
    lexeme_tx: Sender<LexemeTransfer>,
) -> Result<(), SendError<LexemeTransfer>> {

    let mut state = _LexerState::_Normal;
    let mut word_buf = String::new();

    // Main loop: read chunks until the input channel is closed.
    for chunk in text_rx {

        for c in chunk.chars() {

            match &mut state {

                // --- Inside an erase command: reading digits after '[' ---
                _LexerState::_InEraseCommand(digits) => {
                    if c.is_ascii_digit() {
                        // Accumulate digit.
                        digits.push(c);
                    } else if c == ']' {
                        // End of erase command: parse the count and emit.
                        let count = digits.parse::<u32>().unwrap_or(0);
                        lexeme_tx.send(LexemeTransfer::BackspaceCount(count))?;
                        lexeme_tx.send(LexemeTransfer::EraseEnd)?;
                        state = _LexerState::_Normal;
                    } else {
                        // Malformed erase command: non-digit character inside [n].
                        // This is a programming error on the Android side — log and discard.
                        eprntln!("Lexer: malformed erase command: '[{}{}', discarding", digits, c);
                        state = _LexerState::_Normal;
                    }   // if
                }   // _InEraseCommand

                // --- Normal or InWord state ---
                _ => {
                    if c == '*' {
                        // Stabilization marker.
                        _flush_word(&mut word_buf, &lexeme_tx)?;
                        if matches!(state, _LexerState::_InWord) {
                            lexeme_tx.send(LexemeTransfer::WordEnd)?;
                        }   // if
                        lexeme_tx.send(LexemeTransfer::Stabilization)?;
                        state = _LexerState::_Normal;

                    } else if c == '[' {
                        // Start of erase command.
                        _flush_word(&mut word_buf, &lexeme_tx)?;
                        if matches!(state, _LexerState::_InWord) {
                            lexeme_tx.send(LexemeTransfer::WordEnd)?;
                        }   // if
                        lexeme_tx.send(LexemeTransfer::EraseStart)?;
                        state = _LexerState::_InEraseCommand(String::new());

                    } else if c.is_whitespace() {
                        // Whitespace: flush word if needed.
                        _flush_word(&mut word_buf, &lexeme_tx)?;
                        if matches!(state, _LexerState::_InWord) {
                            lexeme_tx.send(LexemeTransfer::WordEnd)?;
                        }   // if
                        lexeme_tx.send(LexemeTransfer::Whitespace(c))?;
                        state = _LexerState::_Normal;

                    } else if _is_punctuation(c) {
                        // Punctuation: flush word if needed.
                        _flush_word(&mut word_buf, &lexeme_tx)?;
                        if matches!(state, _LexerState::_InWord) {
                            lexeme_tx.send(LexemeTransfer::WordEnd)?;
                        }   // if
                        lexeme_tx.send(LexemeTransfer::Punctuation(c))?;
                        state = _LexerState::_Normal;

                    } else {
                        // Word character.
                        if matches!(state, _LexerState::_Normal) {
                            lexeme_tx.send(LexemeTransfer::WordStart)?;
                            state = _LexerState::_InWord;
                        }   // if
                        word_buf.push(c);
                    }   // if
                }   // Normal / InWord
            }   // match state
        }   // for c

        // End of chunk: flush any accumulated word text.
        // Do NOT emit WordEnd here — the word may continue in the next chunk.
        _flush_word(&mut word_buf, &lexeme_tx)?;

    }   // for chunk

    // Input channel closed. Finalize any open word.
    if matches!(state, _LexerState::_InWord) {
        _flush_word(&mut word_buf, &lexeme_tx)?;
        lexeme_tx.send(LexemeTransfer::WordEnd)?;
    }   // if

    Ok(())
}   // _lexer_loop()

/// Flushes the word buffer as a `WordPart` lexeme.
///
/// If the buffer is non-empty, its contents are lowercased and sent
/// as `LexemeTransfer::WordPart`. The buffer is cleared after sending.
/// Does NOT emit `WordEnd` — the caller decides when the word actually ends.
///
/// # Parameters
/// - `buf`: mutable reference to the word accumulation buffer.
/// - `tx`: sender end of the lexeme channel.
///
/// # Returns
/// - `Ok(())` if the buffer was empty or the lexeme was sent successfully.
/// - `Err(SendError)` if the output channel is closed.
fn _flush_word(
    buf: &mut String,
    tx: &Sender<LexemeTransfer>,
) -> Result<(), SendError<LexemeTransfer>> {

    if !buf.is_empty() {
        let text = buf.drain(..).collect::<String>().to_lowercase();
        tx.send(LexemeTransfer::WordPart(text))?;
    }   // if

    Ok(())
}   // _flush_word()

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// Feeds chunks into the lexer and collects all emitted lexemes.
    ///
    /// # Parameters
    /// - `chunks`: sequence of text chunks simulating TCP input.
    ///
    /// # Returns
    /// A vector of all lexemes emitted by the lexer.
    fn _run_lexer(chunks: &[&str]) -> Vec<LexemeTransfer> {

        let (text_tx, text_rx) = mpsc::channel::<String>();
        let (lexeme_tx, lexeme_rx) = mpsc::channel::<LexemeTransfer>();

        // Spawn the lexer in a separate thread.
        let handle = thread::spawn(move || {
            let _ = _lexer_loop(text_rx, lexeme_tx);
        });

        // Feed all chunks.
        for chunk in chunks {
            text_tx.send(chunk.to_string()).unwrap();
        }   // for

        // Close the input channel to signal end of stream.
        drop(text_tx);

        // Wait for the lexer to finish.
        handle.join().unwrap();

        // Collect all emitted lexemes.
        lexeme_rx.iter().collect()
    }   // _run_lexer()

    /// Test based on the example from technical_specification.md (section 5.10).
    ///
    /// Dictated phrase: "Имею я златые горы."
    /// Protocol stream:  "Имею ли я [4]я златые горы.*"
    /// Arrival pattern:
    /// - "Имею ли я [4]"
    /// - "я зла"
    /// - "тые горы.*"
    #[test]
    fn test_spec_example() {

        let chunks = &[
            "Имею ли я [4]",
            "я зла",
            "тые горы.*",
        ];

        let lexemes = _run_lexer(chunks);

        let expected = vec![
            // "Имею"
            LexemeTransfer::WordStart,
            LexemeTransfer::WordPart("имею".to_string()),
            LexemeTransfer::WordEnd,
            // " "
            LexemeTransfer::Whitespace(' '),
            // "ли"
            LexemeTransfer::WordStart,
            LexemeTransfer::WordPart("ли".to_string()),
            LexemeTransfer::WordEnd,
            // " "
            LexemeTransfer::Whitespace(' '),
            // "я"
            LexemeTransfer::WordStart,
            LexemeTransfer::WordPart("я".to_string()),
            LexemeTransfer::WordEnd,
            // " "
            LexemeTransfer::Whitespace(' '),
            // [4]
            LexemeTransfer::EraseStart,
            LexemeTransfer::BackspaceCount(4),
            LexemeTransfer::EraseEnd,
            // "я" (second chunk starts)
            LexemeTransfer::WordStart,
            LexemeTransfer::WordPart("я".to_string()),
            LexemeTransfer::WordEnd,
            // " "
            LexemeTransfer::Whitespace(' '),
            // "зла" + "тые" (word split across chunks)
            LexemeTransfer::WordStart,
            LexemeTransfer::WordPart("зла".to_string()),
            LexemeTransfer::WordPart("тые".to_string()),
            LexemeTransfer::WordEnd,
            // " "
            LexemeTransfer::Whitespace(' '),
            // "горы"
            LexemeTransfer::WordStart,
            LexemeTransfer::WordPart("горы".to_string()),
            LexemeTransfer::WordEnd,
            // "."
            LexemeTransfer::Punctuation('.'),
            // "*"
            LexemeTransfer::Stabilization,
        ];

        assert_eq!(lexemes.len(), expected.len(),
                   "Lexeme count mismatch: got {}, expected {}", lexemes.len(), expected.len());

        for (i, (got, exp)) in lexemes.iter().zip(expected.iter()).enumerate() {
            assert_eq!(got, exp, "Mismatch at lexeme #{}", i);
        }   // for
    }   // test_spec_example()
}   // mod tests