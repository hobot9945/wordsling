use std::sync::{LazyLock, Mutex};

/// Global application state.
///
/// Initialized lazily on first access. Thread-safe access is provided via `Mutex`.
pub(super) static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));

/// Runtime application state.
pub(super) struct State {
    pub(super) is_shutdown_requested: bool, // Application shutdown requested
}   // struct State

impl Default for State {
    fn default() -> Self {
        State {
            is_shutdown_requested: false,
        }
    }   // default()
}   // impl Default for State
