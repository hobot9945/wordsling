//! substitution_toml.rs — Substitution rules configuration.
//!
//! Loads text substitution rules from a TOML file (`substitutions.toml`).
//! If the file does not exist, a default configuration is created and written to disk.
//! If the file exists but is malformed or missing required fields, the application
//! panics at startup.
//!
//! # RESPONSIBILITY
//! - Define the TOML-serializable data structures for substitution rules.
//! - Provide a single loading entry point (`SubstitutionToml::new()`).
//! - Generate a sensible default file when none exists.

use serde::{Deserialize, Serialize};
use hobolib::misc::toml_interface::{read_toml_file, write_toml_file};

/// Name of the substitution configuration file in the working directory.
pub const SUBSTITUTIONS_FILE_NAME: &str = "substitutions.toml";

/// A single text substitution rule.
///
/// Each rule maps one or more trigger phrases to a replacement string.
/// When any of the `key_phrases` is recognized in the dictated text,
/// the phrase is replaced with `replacement`.
///
/// The `modifier` field names the supplementary action that is invoked
/// alongside the substitution (e.g., `"suppress_space_before"`).
/// It must match a registered key in `SupplementaryActionMap`.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SubstitutionRule {

    /// Trigger phrases that activate this substitution.
    /// Multiple phrases allow synonyms (e.g., "левая скобка" and "open parenthesis").
    pub key_phrases: Vec<String>,

    /// The text that replaces the matched trigger phrase.
    pub replacement: String,

    /// Name of the supplementary action associated with this substitution.
    /// Must correspond to a registered key in `SupplementaryActionMap`.
    /// Use `"do_nothing"` when no supplementary behavior is needed.
    pub modifier: String,
}   // SubstitutionRule

impl SubstitutionRule {

    /// Creates a rule manually (used for building the default configuration).
    ///
    /// # Parameters
    /// - `key_phrases`: list of trigger phrases.
    /// - `replacement`: replacement text.
    /// - `modifier`: name of the supplementary action.
    pub fn new(key_phrases: Vec<String>, replacement: &str, modifier: &str) -> Self {
        SubstitutionRule {
            key_phrases,
            replacement: replacement.to_string(),
            modifier: modifier.to_string()
        }
    }   // new()
}   // impl SubstitutionRule

/// Root container for the substitution configuration file.
///
/// Wraps a vector of `SubstitutionRule` entries. The TOML representation
/// uses the key `subs_vec` as the top-level array of tables.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SubstitutionToml {

    /// List of all substitution rules loaded from the file.
    #[serde(default)]
    pub subs_vec: Vec<SubstitutionRule>,
}   // SubstitutionToml

impl SubstitutionToml {

    /// Loads the substitution configuration from disk.
    ///
    /// If the file exists, it is parsed strictly: any missing required field
    /// or malformed syntax causes a panic. If the file does not exist,
    /// a default configuration is written to disk and returned.
    ///
    /// # Panics
    /// - If the current directory cannot be determined.
    /// - If the file exists but cannot be parsed.
    /// - If the default file cannot be written to disk.
    pub fn new() -> Self {

        let path = std::env::current_dir()
            .unwrap_or_else(|e| panic!("Failed to get current directory: {}", e))
            .join(SUBSTITUTIONS_FILE_NAME);

        if path.exists() {
            // Strict loading: malformed file or missing fields cause a panic.
            read_toml_file::<SubstitutionToml>(SUBSTITUTIONS_FILE_NAME)
                .unwrap_or_else(|err| panic!("{}", err))
        } else {
            // No file on disk — generate a default one for the user to customize.
            let substitutions = Self::default();
            write_toml_file(&substitutions, SUBSTITUTIONS_FILE_NAME)
                .unwrap_or_else(|err| panic!("{}", err));
            substitutions
        }
    }   // new()
}   // impl SubstitutionToml

impl Default for SubstitutionToml {

    /// Provides a minimal set of example rules.
    ///
    /// These rules serve as both a starting template for the user
    /// and a smoke test for the loading pipeline.
    fn default() -> Self {
        let mut subs_vec: Vec<SubstitutionRule> = Vec::new();

        // Left parenthesis: suppress trailing space so that
        // "скобка открывается слово" produces "(слово" instead of "( слово".
        subs_vec.push(SubstitutionRule::new(
            vec![
                "левая скобка".to_string(), "скобка открывается".to_string(),
                "left parenthesis".to_string(), "open parenthesis".to_string()
            ],
            "(",
            "suppress_space_after",
        ));

        // Right parenthesis: suppress leading space so that
        // "слово скобка закрывается" produces "слово)" instead of "слово )".
        subs_vec.push(SubstitutionRule::new(
            vec![
                "правая скобка".to_string(), "скобка закрывается".to_string(),
                "right parenthesis".to_string(), "close parenthesis".to_string()
            ],
            ")",
            "suppress_space_before",
        ));

        SubstitutionToml {
            subs_vec
        }
    }   // default()
}   // impl Default for SubstitutionToml