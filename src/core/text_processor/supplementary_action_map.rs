//! supplementary_action_map.rs — Registry of supplementary actions for text substitutions.
//!
//! When a substitution is applied, an optional supplementary action can be triggered
//! to adjust the surrounding context (e.g., suppress whitespace before or after
//! the replacement text).
//!
//! # RESPONSIBILITY
//! - Define the `SupplementaryAction` function pointer type.
//! - Maintain a string-keyed registry of all available action implementations.
//! - Provide individual action functions that operate on the `SurgeTable`.
//!
//! # ADDING A NEW ACTION
//! 1. Implement a function with the `SupplementaryAction` signature.
//! 2. Register it in `SupplementaryActionMap::new()` under a descriptive string key.
//! 3. Use that key in `substitutions.toml` as the `modifier` value.

use std::collections::HashMap;
use crate::core::text_processor::surgical_table::SurgeTable;

pub(super) enum WhenApplied {
    Before,
    After,
}

/// Function pointer type for supplementary rollbacks.
///
/// # Parameters
/// - `surge_table`: mutable reference to the surgical table.
pub(super) type SupplementaryRollback = fn(surge_table: &mut SurgeTable, when_applied: WhenApplied);

/// Function pointer type for supplementary actions.
///
/// Actions receive mutable access to the surgical table and a flag
/// indicating whether the call happens before or after the replacement
/// text is inserted. This allows a single function to handle both phases
/// when needed.
///
/// # Parameters
/// - `surge_table`: mutable reference to the surgical table.
/// - `when_applied`: indicates whether the action is called `Before` or `After`
///   the replacement text is applied.
pub(super) type SupplementaryAction = fn(surge_table: &mut SurgeTable, when_applied: WhenApplied);


/// String-keyed registry of available supplementary actions.
///
/// Used during startup by `SubstitutionMap` to resolve modifier names
/// from the TOML configuration into callable function pointers.
pub(super) struct SupplementaryActionMap {
    _map: HashMap<String, (SupplementaryAction, Option<SupplementaryRollback>)>,
}   // SupplementaryActionMap

impl SupplementaryActionMap {

    /// Creates a new registry and populates it with all known actions.
    ///
    /// Every string that can appear as a `modifier` value in `substitutions.toml`
    /// must be registered here. An unregistered modifier will cause a panic
    /// at startup during `SubstitutionMap` construction.
    pub(super) fn new() -> Self {
        let mut map = HashMap::new();

        map.insert("do_nothing".to_string(), (do_nothing as SupplementaryAction, None));
        map.insert("suppress_space_before".to_string(), (suppress_space_before as SupplementaryAction, None));
        map.insert("suppress_space_after".to_string(), (suppress_space_after as SupplementaryAction, None));

        SupplementaryActionMap {
            _map: map,
        }
    }   // new()

    pub(super) fn get_pair(&self, key: &str) -> Option<(SupplementaryAction, Option<SupplementaryRollback>)> {
        self._map
            .get(key)
            .copied()
    }   // get_pair()

}   // impl SupplementaryActionMap

// =============================================================================
// Supplementary action implementations
// =============================================================================

/// Does nothing. Default action for substitutions that need no context adjustment.
pub(super) fn do_nothing(_surge_table: &mut SurgeTable, _when_applied: WhenApplied) {
}   // do_nothing()

/// Suppresses whitespace before the replacement text.
///
/// Intended for tokens that should attach to the preceding word
/// (e.g., closing parenthesis: "слово)" instead of "слово )").
///
/// Stub: will manipulate the `SurgeTable` franken_board once
/// the substitution pipeline is fully wired.
pub(super) fn suppress_space_before(_surge_table: &mut SurgeTable, _when_applied: WhenApplied) {
}   // suppress_space_before()

/// Suppresses whitespace after the replacement text.
///
/// Intended for tokens that should attach to the following word
/// (e.g., opening parenthesis: "(слово" instead of "( слово").
///
/// Stub: will manipulate the `SurgeTable` franken_board once
/// the substitution pipeline is fully wired.
pub(super) fn suppress_space_after(_surge_table: &mut SurgeTable, _when_applied: WhenApplied) {
}   // suppress_space_after()