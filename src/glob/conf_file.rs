//! Application configuration file.
//!
//! Contains settings that can be changed by the user without recompilation.
//! Stored in TOML format in the working directory.

use std::sync::{LazyLock, Mutex};
use serde::{Deserialize, Serialize};
use hobolib::misc::toml_interface::{read_toml_file, write_toml_file};
use crate::fatal;

const CONFIG_FILE_NAME: &str = "conf.toml";

/// Global application config.
/// Initialized lazily. The first access triggers reading from disk or creating a default file.
pub(super) static CONFIG: LazyLock<Mutex<Config>> = LazyLock::new(|| Mutex::new(Config::new()));

/// Application configuration.
///
/// Used for storing settings loaded from `conf.toml`.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(super) struct Config {
    pub port: String,              // Network port
    pub microphone_hotkey: String, // Hotkey for microphone activation
    pub language_hotkey: String,   // Hotkey for language switching
}   // struct Config

impl Config {
    /// Constructor.
    ///
    /// Creates a default configuration in memory and immediately synchronizes it with the disk.
    /// If the file is missing, it is created. If it exists, values are strictly loaded.
    pub fn new() -> Self {
        let mut config = Self::default();
        config.load();
        config
    }   // new()

    /// Synchronizes the current instance with the file on disk.
    fn load(&mut self) {
        let path = std::env::current_dir()
            .expect("Failed to get current directory")
            .join(CONFIG_FILE_NAME);

        if path.exists() {
            // File exists: try to read it. Panic on error (e.g., missing fields or bad format).
            match read_toml_file::<Config>(CONFIG_FILE_NAME) {
                Ok(loaded_config) => {
                    *self = loaded_config;
                }
                Err(e) => {
                    fatal!("Failed to load configuration file '{}': {}", CONFIG_FILE_NAME, e);
                }
            }
        } else {
            // File missing: save current (default) state to disk.
            self.save().expect("Failed to write default configuration file");
        }   // if
    }   // load()

    /// Saves the current configuration to the working directory.
    pub fn save(&self) -> Result<(), String> {
        write_toml_file(self, CONFIG_FILE_NAME)
    }   // save()

}   // impl Config

impl Default for Config {
    /// Provides pure default values.
    /// Used as a fallback to generate a new file if one doesn't exist.
    fn default() -> Self {
        Config {
            port: "51234".to_string(),
            microphone_hotkey: "<left_alt>".to_string(),
            language_hotkey: "<left_alt>+<left_win>".to_string(),
        }
    }   // default()
}   // impl Default for Config