use std::sync::{LazyLock, Mutex};

/// Global application state.
///
/// Initialized lazily on first access. Thread-safe access is provided via `Mutex`.
/// Shutdown flag is now managed by `hobolib::glob`, so only app-specific runtime
/// state belongs here.
pub(super) static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));

/// Runtime application state.
pub(super) struct State {
    // App-specific runtime fields will go here.
}   // struct State

impl Default for State {
    fn default() -> Self {
        State {}
    }   // default()
}   // impl Default for State