//! lexeme_transfer — LexemeTransfer type definition.
//!
//! Defines the set of lexemes produced by the lexer or user activity tracker.
//! This is the data contract between the post-lexical text processor and it's counterparts.
//!
//! # RESPONSIBILITY
//! - Provide a single, shared definition of the `LexemeTransfer` enum used across pipeline stages.

/// A single semantic element extracted from the incoming text stream.
///
/// The lexer converts a continuous character stream into a sequence of these lexemes.
/// Word boundaries are represented as explicit events (`WordStart`, `WordEnd`),
/// while word content is delivered separately via `WordPart`.
///
/// Service protocol constructs (`*`, `[n]`) are also represented as distinct lexemes,
/// keeping the protocol visible to downstream processors.
#[derive(Debug, PartialEq)]
pub enum LexemeTransfer {

    // Phone events.

    /// Beginning of a word.
    WordStart,
    /// A fragment of a word (already lowercased by the lexer).
    WordPart(String),
    /// End of a word.
    WordEnd,
    /// A single whitespace character (space, newline, tab, etc.).
    Whitespace(char),
    /// A single punctuation character from the predefined set.
    Punctuation(char),
    /// Start of an erase command: `[`.
    EraseStart,
    /// The parsed backspace count from an erase command `[n]`.
    BackspaceCount(u32),
    /// End of an erase command: `]`.
    EraseEnd,
    /// Stabilization marker: `*`.
    Stabilization,

    // User events

    // User has moved the mouse or touched a key.
    UserActivityDetected

}   // Lexeme
