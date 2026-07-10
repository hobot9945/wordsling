//! Application configuration file.
//!
//! Contains settings that can be changed by the user without recompilation.
//! Stored in TOML format in the working directory.

use serde::{Deserialize, Serialize};
use hobolib::misc::toml_interface::{read_toml_file, write_toml_file};

const CONFIG_FILE_NAME: &str = "conf.toml";

/// Application configuration.
///
/// Used for storing settings loaded from `conf.toml`.
/// If the file is missing on startup, a default configuration is created and saved.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(default)]
pub struct ConfFile {
    pub port: u16,                // Network port
    pub microphone_hotkey: String, // Hotkey for microphone activation
    pub language_hotkey: String,   // Hotkey for language switching
}   // struct ConfFile

impl ConfFile {

    /// Loads configuration from the working directory.
    ///
    /// If the file does not exist, creates a default configuration, saves it to disk,
    /// and returns it.
    ///
    /// # Returns
    /// - `Ok(ConfFile)` with the loaded or newly created configuration.
    /// - `Err(String)` if reading or writing fails.
    pub fn load() -> Result<Self, String> {
        let path = std::env::current_dir()
            .map_err(|err| format!("Failed to get current directory: {}", err))?
            .join(CONFIG_FILE_NAME);

        if path.exists() {
            read_toml_file(CONFIG_FILE_NAME)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }   // if
    }   // load()

    /// Saves the current configuration to the working directory.
    ///
    /// # Returns
    /// - `Ok(())` if saved successfully.
    /// - `Err(String)` if serialization or writing fails.
    pub fn save(&self) -> Result<(), String> {
        write_toml_file(self, CONFIG_FILE_NAME)
    }   // save()
}   // impl ConfFile

impl Default for ConfFile {
    fn default() -> Self {
        ConfFile {
            port: 51234,
            microphone_hotkey: "<left_alt>".to_string(),
            language_hotkey: "<left_alt>+<left_win>".to_string(),
        }
    }   // default()
}   // impl Default for ConfFile
