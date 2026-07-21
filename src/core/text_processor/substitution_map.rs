//! substitution_map.rs — Substitution dictionary and prefix-aware phrase search.
//!
//! Loads substitution rules from `substitutions.toml` and provides search primitives
//! for the streaming text processor.
//!
//! # SEARCH MODEL
//! The text processor works on a growing unstable tail. Because of that, a plain
//! exact lookup is not enough. The dictionary must distinguish four situations:
//! - no match at all;
//! - only a partial prefix match;
//! - an exact match that is still ambiguous because a longer key starts with it;
//! - an exact unambiguous match.
//!
//! Example with keys:
//! - `точка`
//! - `точка с запятой`
//!
//! Search results:
//! - `морковка` -> `NoMatch`
//! - `точ` -> `PartialMatch`
//! - `точка` -> `ExactMatchWithContinuation`
//! - final search for `точка` -> `ExactMatch`
//! - `точка с запятой` -> `ExactMatch`

use std::collections::BTreeMap;
use std::ops::Bound;
use crate::core::text_processor::supplementary_action_map::{
    SupplementaryAction,
    SupplementaryActionMap,
};
use crate::glob::substitution_toml::{SubstitutionToml, SUBSTITUTIONS_FILE_NAME};

/// A single substitution payload stored in the dictionary.
///
/// It contains:
/// - replacement text to be emitted instead of the matched key phrase;
/// - a supplementary action to adjust surrounding spacing or other behavior.
pub(super) struct SubstitutionElement {
    _replacement_text: String,
    _action: SupplementaryAction,
}   // SubstitutionElement

impl SubstitutionElement {

    /// Returns the replacement text associated with this entry.
    ///
    /// # Returns
    /// A string slice with the text that should replace the matched key phrase.
    pub(super) fn replacement_text(&self) -> &str {
        &self._replacement_text
    }   // replacement_text()

    /// Returns the supplementary action associated with this entry.
    ///
    /// # Returns
    /// A function pointer to the action that should be invoked
    /// alongside the substitution.
    pub(super) fn action(&self) -> SupplementaryAction {
        self._action
    }   // action()

}   // impl SubstitutionElement

/// Result of phrase lookup in `SubstitutionMap`.
///
/// `ExactMatchWithContinuation` means:
/// - the current query exactly matches some key;
/// - but the dictionary also contains a longer key starting with the same text;
/// - therefore a non-final caller must keep waiting for more input.
pub(super) enum SubstitutionSearchResult<'a> {

    /// No key starts with the query.
    NoMatch,

    /// Some keys start with the query, but none equals it yet.
    PartialMatch,

    /// The query exactly matches a key, and this match is safe to apply now.
    ExactMatch(&'a SubstitutionElement),

    /// The query exactly matches a key, but a longer continuation also exists.
    ExactMatchWithContinuation(&'a SubstitutionElement),
}   // SubstitutionSearchResult

/// In-memory substitution dictionary.
///
/// Keys are stored in normalized form:
/// - trimmed;
/// - internal whitespace collapsed to single spaces;
/// - lowercased.
///
/// `BTreeMap` is used because all keys sharing the same prefix form one contiguous
/// lexical range. That makes prefix search simple and deterministic.
pub(super) struct SubstitutionMap {
    _map: BTreeMap<String, SubstitutionElement>,
}   // SubstitutionMap

impl SubstitutionMap {

    /// Loads `substitutions.toml` and builds the in-memory dictionary.
    ///
    /// This constructor is intentionally strict:
    /// - malformed TOML causes panic;
    /// - unknown modifiers cause panic;
    /// - empty normalized key phrases cause panic;
    /// - duplicate normalized key phrases cause panic.
    ///
    /// The signature stays simple because the rest of the project already treats
    /// configuration loading as mandatory application startup work.
    pub(super) fn new() -> Self {

        let substitution_toml = SubstitutionToml::new();

        let action_map = SupplementaryActionMap::new();
        let mut map: BTreeMap<String, SubstitutionElement> = BTreeMap::new();

        for rule in substitution_toml.subs_vec {

            // Resolve the modifier once per rule.
            // All key phrases of the rule share the same replacement and action.
            let action = *action_map
                .get(&rule.modifier)
                .unwrap_or_else(|| panic!("in file `{}` unknown modifier: `{}`",
                                          SUBSTITUTIONS_FILE_NAME,
                                          rule.modifier));

            for key_phrase in rule.key_phrases {

                let normalized_key_phrase = Self::_normalize_key_phrase(&key_phrase);

                // Empty phrases after normalization are forbidden.
                // Example: "   " would collapse to an empty string.
                if normalized_key_phrase.is_empty() {
                    panic!(
                        "in file `{}` rule for replacement `{}` contains an empty key phrase.",
                        SUBSTITUTIONS_FILE_NAME,
                        rule.replacement
                    );
                }   // if

                // Duplicate normalized keys are forbidden because they make lookup ambiguous.
                let previous = map.insert(
                    normalized_key_phrase.clone(),
                    SubstitutionElement {
                        _replacement_text: rule.replacement.clone(),
                        _action: action,
                    },
                );

                if previous.is_some() {
                    panic!(
                        "in file `{}` duplicate key phrase: `{}`.",
                        SUBSTITUTIONS_FILE_NAME,
                        normalized_key_phrase
                    );
                }   // if

            }   // for key_phrase
        }   // for rule

        SubstitutionMap {
            _map: map,
        }
    }   // new()

    /// Performs a regular non-final search.
    ///
    /// Used while the unstable tail may still grow.
    /// If an exact match exists but a longer continuation also exists,
    /// the function returns `ExactMatchWithContinuation` to signal
    /// that the caller should keep accumulating input.
    ///
    /// # Parameters
    /// - `query`: raw search phrase (will be normalized internally).
    ///
    /// # Returns
    /// A `SubstitutionSearchResult` indicating the match outcome.
    pub(super) fn search(&self, query: &str) -> SubstitutionSearchResult<'_> {
        let normalized_query = Self::_normalize_key_phrase(query);
        self._search(&normalized_query, false)
    }   // search()

    /// Performs a final search.
    ///
    /// Used when the caller knows that no more input will extend the current
    /// query (e.g., after a Gboard stabilization marker).
    /// In final mode, `ExactMatchWithContinuation` is promoted to `ExactMatch`.
    ///
    /// # Parameters
    /// - `query`: raw search phrase (will be normalized internally).
    ///
    /// # Returns
    /// A `SubstitutionSearchResult` indicating the match outcome.
    pub(super) fn final_search(&self, query: &str) -> SubstitutionSearchResult<'_> {
        let normalized_query = Self::_normalize_key_phrase(query);
        self._search(&normalized_query, true)
    }   // final_search()

}   // impl SubstitutionMap

impl SubstitutionMap {

    /// Converts a phrase to canonical lookup form.
    ///
    /// Normalization rules:
    /// - trims leading and trailing whitespace;
    /// - collapses internal whitespace to single spaces;
    /// - lowercases the result.
    ///
    /// # Parameters
    /// - `src`: raw input phrase.
    ///
    /// # Returns
    /// Normalized string ready for dictionary lookup.
    fn _normalize_key_phrase(src: &str) -> String {
        src.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    }   // _normalize_key_phrase()

    /// Internal search implementation shared by `search()` and `final_search()`.
    ///
    /// # Parameters
    /// - `normalized_query`: already normalized query string.
    /// - `is_final`: if `true`, an exact match is accepted even when a longer
    ///   continuation exists in the dictionary.
    ///
    /// # Returns
    /// A `SubstitutionSearchResult` indicating the match outcome.
    ///
    /// # Search order
    /// 1. Empty query -> `NoMatch`
    /// 2. Exact key exists
    ///    - if a longer continuation also exists:
    ///      - non-final mode -> `ExactMatchWithContinuation`
    ///      - final mode -> `ExactMatch`
    ///    - otherwise -> `ExactMatch`
    /// 3. No exact key, but some key starts with the query -> `PartialMatch`
    /// 4. Otherwise -> `NoMatch`
    fn _search(&self, normalized_query: &str, is_final: bool) -> SubstitutionSearchResult<'_> {

        if normalized_query.is_empty() {
            return SubstitutionSearchResult::NoMatch;
        }   // if

        // Exact match has priority over partial match.
        // First determine whether the query is already a full key.
        if let Some(element) = self._map.get(normalized_query) {

            // If a longer key begins with the same query, this exact match is still
            // ambiguous in streaming mode. The caller must wait unless the search
            // is explicitly marked as final.
            if !is_final && self._has_continuation(normalized_query) {
                return SubstitutionSearchResult::ExactMatchWithContinuation(element);
            }   // if

            return SubstitutionSearchResult::ExactMatch(element);
        }   // if

        // No exact match. Check whether the query is still a valid prefix
        // of at least one dictionary key.
        if self._has_prefix_candidate(normalized_query) {
            SubstitutionSearchResult::PartialMatch
        } else {
            SubstitutionSearchResult::NoMatch
        }
    }   // _search()

    /// Checks whether any dictionary key starts with the given query.
    ///
    /// `BTreeMap` keeps keys in lexical order, so all keys sharing the same prefix
    /// form one contiguous region. It is enough to inspect the first key in the range
    /// `[normalized_query, +inf)`.
    ///
    /// # Parameters
    /// - `normalized_query`: already normalized query string.
    ///
    /// # Returns
    /// `true` if at least one key has `normalized_query` as a prefix.
    fn _has_prefix_candidate(&self, normalized_query: &str) -> bool {

        let mut range = self._map.range(normalized_query.to_string()..);

        match range.next() {
            Some((key, _)) => key.starts_with(normalized_query),
            None => false,
        }
    }   // _has_prefix_candidate()

    /// Checks whether the dictionary contains a key that is strictly longer
    /// than `normalized_query` and starts with it.
    ///
    /// Called only after an exact match has already been found.
    /// The search starts strictly after the exact key itself
    /// (using `Bound::Excluded`).
    ///
    /// # Parameters
    /// - `normalized_query`: already normalized query string (known to be
    ///   an existing key in the map).
    ///
    /// # Returns
    /// `true` if a longer continuation key exists.
    fn _has_continuation(&self, normalized_query: &str) -> bool {

        let mut range = self._map.range((
            Bound::Excluded(normalized_query.to_string()),
            Bound::Unbounded,
        ));

        match range.next() {
            Some((next_key, _)) => next_key.starts_with(normalized_query),
            None => false,
        }
    }   // _has_continuation()

}   // impl SubstitutionMap

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text_processor::supplementary_action_map::do_nothing;

    /// Builds a test dictionary with two entries:
    /// - `"точка"` -> `"."`
    /// - `"точка с запятой"` -> `";"`
    ///
    /// # Returns
    /// A `SubstitutionMap` populated with the test entries.
    fn _make_test_map() -> SubstitutionMap {
        let mut map = BTreeMap::new();

        map.insert(
            "точка".to_string(),
            SubstitutionElement {
                _replacement_text: ".".to_string(),
                _action: do_nothing,
            },
        );

        map.insert(
            "точка с запятой".to_string(),
            SubstitutionElement {
                _replacement_text: ";".to_string(),
                _action: do_nothing,
            },
        );

        SubstitutionMap {
            _map: map,
        }
    }   // _make_test_map()

    #[test]
    fn test_search_no_match() {
        let map = _make_test_map();

        match map.search("морковка") {
            SubstitutionSearchResult::NoMatch => {}
            _ => panic!("Expected NoMatch"),
        }   // match
    }   // test_search_no_match()

    #[test]
    fn test_search_partial_match() {
        let map = _make_test_map();

        match map.search("точ") {
            SubstitutionSearchResult::PartialMatch => {}
            _ => panic!("Expected PartialMatch"),
        }   // match
    }   // test_search_partial_match()

    #[test]
    fn test_search_exact_match_with_continuation() {
        let map = _make_test_map();

        match map.search("точка") {
            SubstitutionSearchResult::ExactMatchWithContinuation(element) => {
                assert_eq!(element.replacement_text(), ".");
            }

            _ => panic!("Expected ExactMatchWithContinuation"),
        }   // match
    }   // test_search_exact_match_with_continuation()

    #[test]
    fn test_final_search_promotes_exact_match() {
        let map = _make_test_map();

        match map.final_search("точка") {
            SubstitutionSearchResult::ExactMatch(element) => {
                assert_eq!(element.replacement_text(), ".");
            }

            _ => panic!("Expected ExactMatch"),
        }   // match
    }   // test_final_search_promotes_exact_match()

    #[test]
    fn test_search_exact_unambiguous_match() {
        let map = _make_test_map();

        match map.search("точка с запятой") {
            SubstitutionSearchResult::ExactMatch(element) => {
                assert_eq!(element.replacement_text(), ";");
            }

            _ => panic!("Expected ExactMatch"),
        }   // match
    }   // test_search_exact_unambiguous_match()

    #[test]
    fn test_query_normalization_is_applied() {
        let map = _make_test_map();

        match map.search("   ТОчка   С   Запятой   ") {
            SubstitutionSearchResult::ExactMatch(element) => {
                assert_eq!(element.replacement_text(), ";");
            }

            _ => panic!("Expected normalized ExactMatch"),
        }   // match
    }   // test_query_normalization_is_applied()

}   // mod tests